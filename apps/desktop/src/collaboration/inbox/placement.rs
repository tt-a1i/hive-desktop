use super::{Entry, Priority, sort_entries};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct RowId {
    workspace: String,
    item: String,
}

impl RowId {
    pub(super) fn from_key(workspace: &str, item: String) -> Self {
        Self {
            workspace: workspace.to_owned(),
            item,
        }
    }

    pub(super) fn for_entry(workspace: &str, entry: &Entry) -> Self {
        Self {
            workspace: workspace.to_owned(),
            item: entry
                .snooze_key
                .clone()
                .unwrap_or_else(|| entry.target.key()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Attention,
    Waiting,
    Snoozed,
}

type Row<'a> = (&'a str, Entry);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SnoozedUntil {
    At(u64),
    Indefinitely,
}

#[derive(Default)]
pub(super) struct InboxSections<'a> {
    pub(super) attention: Vec<Row<'a>>,
    pub(super) waiting: Vec<Row<'a>>,
    pub(super) snoozed: Vec<Row<'a>>,
}

impl<'a> InboxSections<'a> {
    pub(super) fn from_rows(
        mut rows: Vec<Row<'a>>,
        mut snoozed_until: impl FnMut(&str, &Entry) -> Option<SnoozedUntil>,
    ) -> Self {
        sort_entries(&mut rows);
        let mut sections = Self::default();
        let mut snoozed = Vec::new();
        for (project, entry) in rows {
            if let Some(until) = snoozed_until(project, &entry) {
                snoozed.push((until, (project, entry)));
            } else if entry.priority == Priority::Active {
                sections.waiting.push((project, entry));
            } else {
                sections.attention.push((project, entry));
            }
        }
        // Entries with the same deadline retain the normal priority order.
        snoozed.sort_by_key(|(until, _)| *until);
        sections.snoozed = snoozed.into_iter().map(|(_, row)| row).collect();
        sections
    }

    fn group_mut(&mut self, section: Section) -> &mut Vec<Row<'a>> {
        match section {
            Section::Attention => &mut self.attention,
            Section::Waiting => &mut self.waiting,
            Section::Snoozed => &mut self.snoozed,
        }
    }

    fn visible_order(&self, waiting: bool, snoozed: bool) -> Vec<(Section, RowId)> {
        [
            (Section::Attention, &self.attention, true),
            (Section::Waiting, &self.waiting, waiting),
            (Section::Snoozed, &self.snoozed, snoozed),
        ]
        .into_iter()
        .filter(|(_, _, visible)| *visible)
        .flat_map(|(section, rows, _)| {
            rows.iter()
                .map(move |(workspace, entry)| (section, RowId::for_entry(workspace, entry)))
        })
        .collect()
    }

    fn section_of(&self, id: &RowId) -> Option<Section> {
        [
            (Section::Attention, &self.attention),
            (Section::Waiting, &self.waiting),
            (Section::Snoozed, &self.snoozed),
        ]
        .into_iter()
        .find_map(|(section, rows)| {
            rows.iter()
                .any(|(workspace, entry)| {
                    id.workspace == *workspace
                        && id.item
                            == entry
                                .snooze_key
                                .clone()
                                .unwrap_or_else(|| entry.target.key())
                })
                .then_some(section)
        })
    }

    fn hold(self, order: &[(Section, RowId)]) -> Self {
        let groups = [
            (Section::Attention, self.attention),
            (Section::Waiting, self.waiting),
            (Section::Snoozed, self.snoozed),
        ];
        let mut latest = HashMap::new();
        let mut sorted = Vec::new();
        for (section, rows) in groups {
            for row in rows {
                let id = RowId::for_entry(row.0, &row.1);
                sorted.push(id.clone());
                latest.insert(id, (section, row));
            }
        }
        let mut held = Self::default();
        let mut moved = Vec::new();
        for (previous, id) in order {
            let Some((current, row)) = latest.remove(id) else {
                continue;
            };
            if *previous != current
                && (current == Section::Snoozed || *previous == Section::Snoozed)
            {
                moved.push((current, row));
            } else {
                held.group_mut(*previous).push(row);
            }
        }
        for (section, row) in moved {
            held.group_mut(section).push(row);
        }
        // Snooze changes take effect immediately. Unrelated new attention and
        // running items enter in priority order when the user leaves the controls.
        for id in sorted {
            if let Some((Section::Snoozed, row)) = latest.remove(&id) {
                held.snoozed.push(row);
            }
        }
        held
    }
}

#[derive(Default)]
pub(super) struct RowPlacement {
    order: Vec<(Section, RowId)>,
    show_waiting: bool,
    show_snoozed: bool,
    hovered: Option<RowId>,
    menu_open: Option<RowId>,
}

impl RowPlacement {
    pub(super) fn arrange<'a>(
        &mut self,
        latest: InboxSections<'a>,
        focused: Option<&RowId>,
        waiting: bool,
        snoozed: bool,
    ) -> InboxSections<'a> {
        let stays_visible = |id: &RowId| {
            self.order
                .iter()
                .find(|(_, previous)| previous == id)
                .is_some_and(|(old, _)| {
                    self.section_visible(*old)
                        && latest.section_of(id).is_some_and(|current| {
                            (*old == Section::Snoozed) == (current == Section::Snoozed)
                        })
                })
        };
        let hovered = self.hovered.clone().filter(stays_visible);
        let menu_open = self.menu_open.clone().filter(stays_visible);
        let held = hovered.is_some() || menu_open.is_some() || focused.is_some_and(stays_visible);
        let displayed = if held {
            latest.hold(&self.order)
        } else {
            latest
        };
        self.hovered = hovered;
        self.menu_open = menu_open;
        self.show_waiting = waiting;
        self.show_snoozed = snoozed;
        self.order = displayed.visible_order(true, true);
        displayed
    }

    pub(super) fn shows(&self, id: &RowId) -> bool {
        self.visible_rows().any(|visible| visible == id)
    }

    pub(super) fn visible_rows(&self) -> impl Iterator<Item = &RowId> {
        self.order
            .iter()
            .filter_map(|(section, id)| self.section_visible(*section).then_some(id))
    }

    fn section_visible(&self, section: Section) -> bool {
        match section {
            Section::Attention => true,
            Section::Waiting => self.show_waiting,
            Section::Snoozed => self.show_snoozed,
        }
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn hover(&mut self, id: &RowId, active: bool) -> bool {
        Self::track(&mut self.hovered, id, active)
    }

    pub(super) fn menu(&mut self, id: &RowId, active: bool) -> bool {
        Self::track(&mut self.menu_open, id, active)
    }

    fn track(signal: &mut Option<RowId>, id: &RowId, active: bool) -> bool {
        if (signal.as_ref() == Some(id)) == active {
            return false;
        }
        *signal = active.then(|| id.clone());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{InboxSections, Row, RowId, RowPlacement, SnoozedUntil};
    use crate::collaboration::inbox::{Destination, Entry, Priority};

    fn row(
        workspace: &'static str,
        target: &str,
        priority: Priority,
        updated_at: u64,
    ) -> Row<'static> {
        (
            workspace,
            Entry {
                label: "",
                subject: target.into(),
                target: Destination::Dispatch(target.into()),
                snooze_key: Some("message:same-key-in-two-projects".into()),
                wake: None,
                priority,
                updated_at,
            },
        )
    }

    fn labels<'a>(rows: &'a [Row<'_>]) -> Vec<&'a str> {
        rows.iter()
            .map(|(_, entry)| entry.subject.as_str())
            .collect()
    }

    #[test]
    fn snoozed_rows_use_deadlines_with_normal_ties_and_indefinite_last() {
        let sections = InboxSections::from_rows(
            vec![
                row("beacon", "priority tie", Priority::NeedsUser, 10),
                row("alpha", "workspace tie", Priority::NeedsUser, 10),
                row("atlas", "unread tie", Priority::Unread, 99),
                row("atlas", "early working", Priority::Active, 1),
                row("other", "indefinite", Priority::NeedsUser, 120),
                row("other", "ready", Priority::Unread, 1),
            ],
            |_, entry| match entry.subject.as_str() {
                "early working" => Some(SnoozedUntil::At(100)),
                "workspace tie" | "priority tie" | "unread tie" => Some(SnoozedUntil::At(200)),
                "indefinite" => Some(SnoozedUntil::Indefinitely),
                _ => None,
            },
        );
        assert_eq!(labels(&sections.attention), ["ready"]);
        assert_eq!(
            labels(&sections.snoozed),
            [
                "early working",
                "workspace tie",
                "priority tie",
                "unread tie",
                "indefinite"
            ]
        );
    }

    #[test]
    fn rescheduling_reorders_only_after_row_interaction_releases() {
        let active = row("beacon", "existing", Priority::NeedsUser, 2);
        let id = RowId::for_entry(active.0, &active.1);
        let mut placement = RowPlacement::default();
        let fresh = |beacon| {
            InboxSections::from_rows(
                vec![
                    row("beacon", "existing", Priority::NeedsUser, 2),
                    row("atlas", "earlier", Priority::Unread, 5),
                ],
                |workspace, _| {
                    if workspace == "beacon" {
                        beacon
                    } else {
                        Some(SnoozedUntil::At(100))
                    }
                },
            )
        };
        let before = placement.arrange(fresh(Some(SnoozedUntil::At(200))), None, false, true);
        assert_eq!(labels(&before.snoozed), ["earlier", "existing"]);
        assert!(placement.menu(&id, true));
        let held = placement.arrange(fresh(Some(SnoozedUntil::At(50))), Some(&id), false, true);
        assert_eq!(labels(&held.snoozed), ["earlier", "existing"]);

        assert!(placement.menu(&id, false));
        let released = placement.arrange(fresh(Some(SnoozedUntil::At(50))), None, false, true);
        assert_eq!(labels(&released.snoozed), ["existing", "earlier"]);
        let undone = placement.arrange(fresh(Some(SnoozedUntil::At(200))), None, false, true);
        assert_eq!(labels(&undone.snoozed), ["earlier", "existing"]);
        let woke = placement.arrange(fresh(None), None, false, true);
        assert_eq!(labels(&woke.snoozed), ["earlier"]);
        assert_eq!(labels(&woke.attention), ["existing"]);
    }

    #[test]
    fn an_inserted_higher_priority_row_waits_for_pointer_or_keyboard_to_leave() {
        for keyboard in [false, true] {
            let mut placement = RowPlacement::default();
            let initial = row("beacon", "beacon-original", Priority::Unread, 2);
            let original = RowId::for_entry(initial.0, &initial.1);
            placement.arrange(
                InboxSections::from_rows(vec![initial], |_, _| None),
                None,
                false,
                false,
            );
            let focus = keyboard.then_some(&original);
            if !keyboard {
                assert!(placement.hover(&original, true));
            }
            let fresh = || {
                InboxSections::from_rows(
                    vec![
                        row("beacon", "beacon-original", Priority::Unread, 2),
                        row("atlas", "atlas-new-question", Priority::NeedsUser, 7),
                    ],
                    |_, _| None,
                )
            };
            let held = placement.arrange(fresh(), focus, false, false);
            assert_eq!(labels(&held.attention), ["beacon-original"]);
            assert_eq!(held.attention[0].1.target.key(), "dispatch:beacon-original");
            assert_eq!(placement.visible_rows().collect::<Vec<_>>(), [&original]);
            if !keyboard {
                assert!(placement.hover(&original, false));
            }
            let released = placement.arrange(fresh(), None, false, false);
            assert_eq!(
                labels(&released.attention),
                ["atlas-new-question", "beacon-original"]
            );
        }
    }

    #[test]
    fn a_reprioritized_row_updates_in_place_while_the_snooze_menu_is_open() {
        let mut placement = RowPlacement::default();
        let active = row("beacon", "beacon-original", Priority::Active, 2);
        let id = RowId::for_entry(active.0, &active.1);
        let original = InboxSections::from_rows(vec![active], |_, _| None);
        placement.arrange(original, None, true, false);
        assert!(placement.hover(&id, true));
        assert!(placement.menu(&id, true));
        assert!(placement.hover(&id, false));
        let fresh = |snoozed| {
            InboxSections::from_rows(
                vec![
                    row("beacon", "beacon-original", Priority::NeedsUser, 7),
                    row("atlas", "atlas-new-question", Priority::NeedsUser, 9),
                ],
                |workspace, _| {
                    (snoozed && workspace == "beacon").then_some(SnoozedUntil::Indefinitely)
                },
            )
        };
        let held = placement.arrange(fresh(false), None, true, true);
        assert!(held.attention.is_empty());
        assert_eq!(labels(&held.waiting), ["beacon-original"]);
        assert!(held.waiting[0].1.priority == Priority::NeedsUser);

        let settled = placement.arrange(fresh(true), None, true, true);
        assert_eq!(labels(&settled.attention), ["atlas-new-question"]);
        assert_eq!(labels(&settled.snoozed), ["beacon-original"]);
    }

    #[test]
    fn removing_the_active_project_releases_the_latest_items() {
        let mut placement = RowPlacement::default();
        let active = row("beacon", "beacon-original", Priority::Unread, 2);
        let id = RowId::for_entry(active.0, &active.1);
        let pending = || row("atlas", "atlas-waiting", Priority::Active, 3);
        placement.arrange(
            InboxSections::from_rows(vec![active, pending()], |_, _| None),
            None,
            false,
            false,
        );
        assert!(placement.hover(&id, true));
        let same = placement.arrange(
            InboxSections::from_rows(
                vec![
                    row("beacon", "beacon-original", Priority::Unread, 2),
                    pending(),
                ],
                |_, _| None,
            ),
            None,
            false,
            false,
        );
        assert_eq!(labels(&same.waiting), ["atlas-waiting"]);

        let filtered = InboxSections::from_rows(
            vec![row("atlas", "atlas-new-question", Priority::NeedsUser, 8)],
            |_, _| None,
        );
        let displayed = placement.arrange(filtered, Some(&id), false, false);
        assert!(!placement.shows(&id));
        assert_eq!(labels(&displayed.attention), ["atlas-new-question"]);
    }
}
