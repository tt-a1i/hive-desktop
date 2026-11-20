use super::*;
use gpui_kit::component::{
    diff::{DiffDecoration, DiffLinePosition},
    input::{Input, InputEvent, InputState},
};

const FIND_LABEL: &str = "查找当前差异";

pub(super) struct DiffFind {
    input: Entity<InputState>,
    _subscription: Subscription,
    generation: u64,
    path: SharedString,
    patch: SharedString,
    query: String,
    hits: Vec<DiffLinePosition>,
    selected: usize,
}

impl DiffFind {
    fn active(&self) -> Option<&DiffLinePosition> {
        self.hits.get(self.selected)
    }

    fn step(&mut self, previous: bool) {
        if !self.hits.is_empty() {
            let count = self.hits.len();
            self.selected = if previous {
                (self.selected + count - 1) % count
            } else {
                (self.selected + 1) % count
            };
        }
    }
}

struct HunkLines {
    old: usize,
    old_remaining: usize,
    new: usize,
    new_remaining: usize,
}

impl HunkLines {
    fn next(&mut self, marker: u8) -> Option<(DiffSide, usize)> {
        let old = self.old;
        let new = self.new;
        if matches!(marker, b' ' | b'-') && self.old_remaining > 0 && old > 0 {
            self.old_remaining -= 1;
            self.old = self.old.saturating_add(1);
        } else if marker != b'+' {
            return None;
        }
        if matches!(marker, b' ' | b'+') && self.new_remaining > 0 && new > 0 {
            self.new_remaining -= 1;
            self.new = self.new.saturating_add(1);
            // Context exists on both sides. Keep one result, at the modified line.
            Some((DiffSide::Modified, new))
        } else if marker == b'-' {
            Some((DiffSide::Original, old))
        } else {
            None
        }
    }
}

fn matching_lines(patch: &str, path: &SharedString, query: &str) -> Vec<DiffLinePosition> {
    if query.is_empty() {
        return Vec::new();
    }
    let query = query.to_lowercase();
    let mut hunk = None;
    let mut hits = Vec::new();
    for line in patch.lines() {
        if let Some((old, old_remaining, new, new_remaining, _)) =
            diffs_settings::parse_hunk_coords(line)
        {
            hunk = Some(HunkLines {
                old,
                old_remaining,
                new,
                new_remaining,
            });
            continue;
        }
        if line.starts_with('\\') {
            continue;
        }
        let Some((side, number)) = line
            .as_bytes()
            .first()
            .and_then(|marker| hunk.as_mut()?.next(*marker))
        else {
            hunk = None;
            continue;
        };
        if line[1..].to_lowercase().contains(&query) {
            hits.push(DiffLinePosition::new(path.clone(), side, number));
        }
    }
    hits
}

impl GitWorkspace {
    fn diff_find_available(&self, cx: &App) -> bool {
        self.current(cx)
            && !self.show_current_file
            && !self.status_loading
            && !self.diff_loading
            && !self.diff_raw
            && self.diff_has_structured
            && self.diff_state.read(cx).files().len() == 1
            && self.diff.as_ref().is_some_and(|diff| {
                !diff.binary
                    && diff.complete
                    && diff.comparison == self.comparison.wire()
                    && self
                        .selected_change()
                        .is_some_and(|change| diff.path == change.path)
                    && self.status.as_ref().is_some_and(|status| {
                        diff.identity.project_root == status.identity.project_root
                    })
            })
    }

    fn valid_diff_find(&self, cx: &App) -> Option<&DiffFind> {
        self.diff_find
            .as_ref()
            .filter(|find| find.generation == self.diff_generation && self.diff_find_available(cx))
    }

    pub(super) fn open_diff_find(
        &mut self,
        _: &crate::OpenConversationFind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.diff_find_available(cx) {
            return;
        }
        if self.valid_diff_find(cx).is_none() {
            let (Some(file), Some(patch)) = (
                self.diff_state.read(cx).files().first(),
                self.diff.as_ref().and_then(|diff| diff.patch.as_ref()),
            ) else {
                return;
            };
            let path = file.path().clone();
            let patch = if self.ignore_whitespace {
                SharedString::from(diffs_settings::filter_patch_whitespace(patch))
            } else {
                patch.clone()
            };
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(tr(FIND_LABEL)));
            let subscription = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.update_diff_find(&input, cx);
                }
            });
            self.diff_find = Some(DiffFind {
                input,
                _subscription: subscription,
                generation: self.diff_generation,
                path,
                patch,
                query: String::new(),
                hits: Vec::new(),
                selected: 0,
            });
        }
        cx.stop_propagation();
        let Some(find) = self.valid_diff_find(cx) else {
            return;
        };
        let input = find.input.clone();
        let previous_focus = window.focused(cx);
        let workspace = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            if window.focused(cx) == previous_focus
                && workspace.upgrade().is_some_and(|workspace| {
                    workspace
                        .read(cx)
                        .valid_diff_find(cx)
                        .is_some_and(|find| find.input == input)
                })
            {
                input.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }
        });
        cx.notify();
    }

    fn update_diff_find(&mut self, input: &Entity<InputState>, cx: &mut Context<Self>) {
        let Some(find) = self.valid_diff_find(cx).filter(|find| &find.input == input) else {
            return;
        };
        let query = input.read(cx).value().to_string();
        if query == find.query {
            return;
        }
        let hits = matching_lines(&find.patch, &find.path, &query);
        let Some(find) = &mut self.diff_find else {
            return;
        };
        find.query = query;
        find.hits = hits;
        find.selected = 0;
        self.reveal_diff_find(cx);
    }

    fn reveal_diff_find(&self, cx: &mut Context<Self>) {
        if let Some(active) = self.valid_diff_find(cx).and_then(DiffFind::active).cloned() {
            self.diff_state
                .update(cx, |state, cx| state.scroll_to_line(active, cx));
        }
        cx.notify();
    }

    fn diff_find_composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.valid_diff_find(cx).is_some_and(|find| {
            find.input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        })
    }

    fn step_diff_find(&mut self, previous: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.valid_diff_find(cx).is_none() || self.diff_find_composing(window, cx) {
            cx.propagate();
            return;
        }
        cx.stop_propagation();
        if let Some(find) = &mut self.diff_find {
            find.step(previous);
        }
        self.reveal_diff_find(cx);
    }

    pub(super) fn close_diff_find_shortcut(
        &mut self,
        _: &crate::CloseConversationFind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.valid_diff_find(cx).is_none() {
            return;
        }
        if !self.diff_find_composing(window, cx) {
            cx.stop_propagation();
            self.close_diff_find(window, cx);
        }
    }

    fn close_diff_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.diff_find = None;
        self.diff_state.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(super) fn diff_find_decoration(&self, cx: &App) -> Option<DiffDecoration> {
        let active = self.valid_diff_find(cx)?.active()?;
        Some(DiffDecoration::new(DiffLineRange::new(
            active.path().clone(),
            active.side(),
            active.line(),
            active.line(),
        )))
    }

    pub(super) fn render_diff_find(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let find = self.valid_diff_find(cx)?;
        let status = if let Some(active) = find.active() {
            let count = tr("第 {current} 项，共 {total} 项")
                .replace("{current}", &(find.selected + 1).to_string())
                .replace("{total}", &find.hits.len().to_string());
            let location = match active.side() {
                DiffSide::Original => tr("原文件第 {line} 行"),
                DiffSide::Modified => tr("新文件第 {line} 行"),
            }
            .replace("{line}", &active.line().to_string());
            format!("{count} · {location}")
        } else if !find.query.is_empty() {
            tr("未找到匹配项").to_owned()
        } else {
            String::new()
        };

        Some(
            h_flex()
                .id("git-diff-find")
                .p_2()
                .gap_1p5()
                .items_center()
                .flex_wrap()
                .on_action(
                    cx.listener(|this, _: &crate::NextConversationMatch, window, cx| {
                        this.step_diff_find(false, window, cx)
                    }),
                )
                .on_action(
                    cx.listener(|this, _: &crate::PreviousConversationMatch, window, cx| {
                        this.step_diff_find(true, window, cx)
                    }),
                )
                .child(
                    div()
                        .min_w(px(124.))
                        .flex_1()
                        .key_context("HiveGitDiffFind")
                        .child(
                            Input::new(&find.input)
                                .small()
                                .cleanable(true)
                                .aria_label(tr(FIND_LABEL)),
                        ),
                )
                .child(
                    div()
                        .id("git-diff-find-count")
                        .role(Role::Status)
                        .aria_label(status.clone())
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(status),
                )
                .children(
                    [
                        (
                            "git-diff-find-previous",
                            "上一个差异匹配",
                            Glyph::ChevronUp,
                            true,
                        ),
                        (
                            "git-diff-find-next",
                            "下一个差异匹配",
                            Glyph::ChevronDown,
                            false,
                        ),
                    ]
                    .into_iter()
                    .map(|(id, label, icon, previous)| {
                        Button::new(id)
                            .ghost()
                            .xsmall()
                            .icon(icon)
                            .tooltip(tr(label))
                            .accessibility_label(tr(label))
                            .disabled(find.hits.is_empty())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.step_diff_find(previous, window, cx)
                            }))
                    }),
                )
                .child(
                    Button::new("git-diff-find-close")
                        .ghost()
                        .xsmall()
                        .icon(Glyph::X)
                        .tooltip(tr("关闭差异查找"))
                        .accessibility_label(tr("关闭差异查找"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.close_diff_find(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::matching_lines;
    use gpui_kit::{
        SharedString,
        component::diff::{DiffFile, DiffSide},
    };

    #[test]
    fn matches_materialized_patch_lines_in_order_with_exact_sides() {
        let patch = concat!(
            "diff --git a/match.txt b/match.txt\n--- a/match.txt\n+++ b/match.txt\n",
            "@@ -20,3 +40,4 @@ MATCH header\n",
            " MATCH before context\n-MATCH previous\n+MATCH changed\n+新增 Match 🐝\n MATCH after context\n",
            "@@ -70 +91 @@\n-MATCH at end\n+Unicode: ÉLAN 🐝\n\\ No newline at end of file\n",
        );
        let files = DiffFile::parse(patch).unwrap();
        let path = files[0].path();
        let hits = matching_lines(patch, path, "match");
        let positions: Vec<_> = hits
            .iter()
            .map(|hit| (hit.path(), hit.side(), hit.line()))
            .collect();
        assert_eq!(
            positions,
            [
                (path, DiffSide::Modified, 40),
                (path, DiffSide::Original, 21),
                (path, DiffSide::Modified, 41),
                (path, DiffSide::Modified, 42),
                (path, DiffSide::Modified, 43),
                (path, DiffSide::Original, 70),
            ]
        );
        assert_eq!(matching_lines(patch, path, "élan 🐝")[0].line(), 91);
        assert!(matching_lines(patch, path, "header").is_empty());
        assert!(matching_lines(patch, path, "no newline").is_empty());
        assert!(matching_lines(patch, path, "").is_empty());
    }

    #[test]
    fn zero_count_for_created_or_deleted_files_never_fabricates_source_lines() {
        let path = SharedString::from("same.txt");
        let created = matching_lines("@@ -0,0 +1,2 @@\n+one\n+two\n", &path, "one");
        assert_eq!(
            (created[0].side(), created[0].line()),
            (DiffSide::Modified, 1)
        );
        let deleted = matching_lines("@@ -18,2 +0,0 @@\n-one\n-two\n", &path, "one");
        assert_eq!(
            (deleted[0].side(), deleted[0].line()),
            (DiffSide::Original, 18)
        );
        assert!(
            matching_lines("@@ -1 +1 @@\n-x\n+y\n+out of hunk\n", &path, "out of hunk").is_empty()
        );
    }
}
