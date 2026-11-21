use crate::app::HiveApp;
use gpui_kit::{component::WindowExt, *};

#[derive(Clone, Copy)]
pub(crate) enum Area {
    Sidebar,
    History,
    Composer,
    Terminal,
    Git,
    SplitPeek,
    Files,
}

const AREAS: [Area; 7] = [
    Area::Sidebar,
    Area::History,
    Area::Composer,
    Area::Terminal,
    Area::Git,
    Area::SplitPeek,
    Area::Files,
];

pub(crate) struct WorkspaceFocus {
    pub root: FocusHandle,
    regions: [FocusHandle; AREAS.len()],
    last_focus: [Option<WeakFocusHandle>; AREAS.len()],
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceFocus {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        let root = cx.focus_handle();
        let regions = std::array::from_fn(|_| cx.focus_handle());
        let weak = cx.weak_entity();
        let subscriptions = AREAS
            .into_iter()
            .filter(|area| !matches!(area, Area::Files))
            .map(|area| {
                let weak = weak.clone();
                window.on_focus_out(&regions[area as usize], cx, move |event, _, cx| {
                    let _ = weak.update(cx, |app, _| {
                        app.workspace_focus.last_focus[area as usize] = Some(event.blurred);
                    });
                })
            })
            .collect();
        Self {
            root,
            regions,
            last_focus: Default::default(),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn region(&self, area: Area) -> &FocusHandle {
        &self.regions[area as usize]
    }
}

impl HiveApp {
    pub(crate) fn focus_workspace_area(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.closing
            || self.showing_welcome_guide
            || !self.viewing_conversation()
            || self.choosing_model
            || window.has_active_dialog(cx)
            || window.has_active_sheet(cx)
            || window
                .context_stack()
                .iter()
                .any(|context| context.contains("PopupMenu"))
        {
            return;
        }

        let composer = self.input.read(cx).focus_handle(cx);
        if composer.is_focused(window)
            && (self.command_picker.open
                || self.mention_picker.open
                || self.input.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                }))
        {
            return;
        }

        let files = self.project_files_workspace_focus(cx);
        let areas: Vec<_> = AREAS
            .into_iter()
            .filter_map(|area| {
                let mut region = self.workspace_focus.region(area).clone();
                let target = match area {
                    Area::Sidebar => self.sidebar_search.read(cx).query.read(cx).focus_handle(cx),
                    Area::History | Area::Git | Area::SplitPeek => region.clone(),
                    Area::Composer if !self.current_worktree_released() => composer.clone(),
                    Area::Terminal => self.workspace_terminal_focus(cx)?,
                    Area::Files => {
                        let (dock, query) = files.as_ref()?;
                        region = dock.clone();
                        query.clone()
                    }
                    _ => return None,
                };
                (self.workspace_focus.root.contains(&region, window)
                    && region.contains(&target, window))
                .then_some((area, region, target))
            })
            .collect();
        if areas.is_empty() {
            return;
        }

        let current = window.focused(cx).and_then(|focused| {
            let index = areas
                .iter()
                .position(|(_, region, _)| region.contains(&focused, window))?;
            self.workspace_focus.last_focus[areas[index].0 as usize] = Some(focused.downgrade());
            Some(index)
        });
        let index = next_index(current, areas.len(), forward);
        let (area, region, default) = &areas[index];
        let target = self.workspace_focus.last_focus[*area as usize]
            .as_ref()
            .and_then(WeakFocusHandle::upgrade)
            .filter(|last| region.contains(last, window))
            .unwrap_or_else(|| default.clone());
        target.focus(window, cx);
    }
}

fn next_index(current: Option<usize>, len: usize, forward: bool) -> usize {
    match (current, forward) {
        (Some(index), true) => (index + 1) % len,
        (Some(index), false) => (index + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    }
}

#[cfg(test)]
mod tests {
    use super::next_index;

    #[test]
    fn cycles_and_wraps_from_either_end_of_visible_areas() {
        assert_eq!(next_index(Some(0), 3, true), 1);
        assert_eq!(next_index(Some(2), 3, true), 0);
        assert_eq!(next_index(Some(2), 3, false), 1);
        assert_eq!(next_index(Some(0), 3, false), 2);
        assert_eq!(next_index(None, 3, true), 0);
        assert_eq!(next_index(None, 3, false), 2);
        assert_eq!(next_index(Some(0), 1, true), 0);
        assert_eq!(next_index(Some(0), 1, false), 0);
    }
}
