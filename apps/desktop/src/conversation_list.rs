mod markdown;
mod view;
pub(crate) const CHAT_COLUMN_WIDTH: f32 = 736.;
use crate::{app::HiveApp, history_navigation::HistoryDirection};
use gpui_kit::{component::v_flex, prelude::FluentBuilder, *};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum RowKey {
    LoadOlder,
    LoadNewer,
    Message(String),
    Group(String),
    TurnDiff(String),
    Tail,
}
#[derive(Clone, Debug)]
pub struct ReadingAnchor {
    pub key: RowKey,
    pub offset: Pixels,
}
#[derive(Clone, PartialEq, Eq)]
enum Kind {
    LoadHistory {
        direction: HistoryDirection,
        remaining: Option<usize>,
        loading: Option<HistoryDirection>,
    },
    Message {
        index: usize,
        indented: bool,
        last: bool,
        merge_prev: bool,
        merge_next: bool,
    },
    Group {
        start: usize,
        end: usize,
        expanded: bool,
    },
    TurnDiff {
        anchor_id: String,
    },
    Tail,
}
#[derive(Clone, PartialEq, Eq)]
struct Row {
    key: RowKey,
    kind: Kind,
    revision: u64,
    expanded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TurnDiffFileSummary {
    pub display_path: String,
    pub git_path: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TurnDiffSummary {
    pub anchor_id: String,
    pub files: Vec<TurnDiffFileSummary>,
    pub total_additions: usize,
    pub total_deletions: usize,
    pub tool_changes: Vec<(String, serde_json::Value, Option<String>)>,
}

#[derive(Clone)]
pub(crate) struct LatestTurnFile {
    pub display_path: String,
    pub git_path: String,
    pub additions: usize,
    pub deletions: usize,
    pub diff_files: Vec<gpui_kit::component::diff::DiffFile>,
    pub raw_patch: String,
    pub from_real_patch: bool,
}

#[derive(Clone)]
pub(crate) enum LatestTurnSnapshot {
    Empty {
        has_active_turn: bool,
        session_open: bool,
    },
    Ready {
        anchor_id: String,
        files: Vec<LatestTurnFile>,
        total_additions: usize,
        total_deletions: usize,
        in_progress: bool,
    },
}

pub(crate) fn collect_latest_turn_snapshot(
    messages: &[crate::app::Message],
    tool_inputs: &HashMap<String, serde_json::Value>,
    session_open: bool,
    session_busy: bool,
    english_labels: bool,
) -> LatestTurnSnapshot {
    if !session_open || messages.is_empty() {
        return LatestTurnSnapshot::Empty {
            has_active_turn: false,
            session_open,
        };
    }
    let last_turn_start = messages
        .iter()
        .rposition(|m| m.role == "user" || m.delivery.is_some())
        .map_or(0, |idx| idx + 1);
    let slice = &messages[last_turn_start..];
    let in_progress = session_busy
        || slice
            .iter()
            .any(|m| m.tool.as_ref().is_some_and(|t| t.status == "执行中"));
    if slice.is_empty() {
        return LatestTurnSnapshot::Empty {
            has_active_turn: in_progress,
            session_open: true,
        };
    }

    let mut file_indices: HashMap<String, usize> = HashMap::new();
    let mut files: Vec<LatestTurnFile> = Vec::new();
    let mut total_additions = 0usize;
    let mut total_deletions = 0usize;

    for msg in slice {
        let Some(tool) = &msg.tool else {
            continue;
        };
        if tool.status != "完成" {
            continue;
        }
        let args = tool_inputs
            .get(&msg.id)
            .unwrap_or(&serde_json::Value::Null);
        if !crate::tool_diff::data::available(&tool.name, args, tool.patch.as_deref()) {
            continue;
        }
        let from_real_patch = tool.patch.as_deref().is_some_and(|p| !p.is_empty());
        let changes =
            crate::tool_diff::data::changes(&tool.name, args, tool.patch.as_deref(), english_labels);
        let explicit_arg_path = args["file_path"]
            .as_str()
            .or_else(|| args["path"].as_str())
            .filter(|p| !p.trim().is_empty());
        let is_edit_tool = matches!(
            tool.name.to_ascii_lowercase().as_str(),
            "edit" | "multiedit"
        );
        for change in changes {
            let doc = crate::tool_diff::data::build(vec![change]);
            if doc.files.is_empty() {
                continue;
            }
            let single_file = doc.files.len() == 1;
            let sectioned_patches: Vec<(Vec<gpui_kit::component::diff::DiffFile>, String)> =
                if single_file {
                    Vec::new()
                } else {
                    crate::diffs_settings::split_patch_by_file(&doc.raw)
                        .into_iter()
                        .filter_map(|section| {
                            gpui_kit::component::diff::DiffFile::parse(&section)
                                .ok()
                                .map(|parsed| (parsed, section))
                        })
                        .collect()
                };
            for diff_file in doc.files {
                let raw_path = diff_file.path().to_string();
                let key_path = if is_edit_tool {
                    explicit_arg_path.unwrap_or(&raw_path).to_owned()
                } else {
                    raw_path.clone()
                };
                let additions = diff_file.additions();
                let deletions = diff_file.deletions();
                total_additions += additions;
                total_deletions += deletions;
                let file_patch = if single_file {
                    doc.raw.clone()
                } else {
                    let target_path = diff_file.path().as_ref();
                    let target_orig = diff_file.original_path().map(|p| p.as_ref().to_owned());
                    sectioned_patches
                        .iter()
                        .filter(|(parsed, _)| {
                            parsed.iter().any(|p| {
                                p.path().as_ref() == target_path
                                    && p.original_path().map(|o| o.as_ref())
                                        == target_orig.as_deref()
                            })
                        })
                        .map(|(_, section)| section.as_str())
                        .collect::<String>()
                };
                if let Some(&idx) = file_indices.get(&key_path) {
                    files[idx].additions += additions;
                    files[idx].deletions += deletions;
                    files[idx].diff_files.push(diff_file);
                    files[idx].from_real_patch |= from_real_patch;
                    if !file_patch.is_empty() {
                        if !files[idx].raw_patch.is_empty() && !files[idx].raw_patch.ends_with('\n')
                        {
                            files[idx].raw_patch.push('\n');
                        }
                        files[idx].raw_patch.push_str(&file_patch);
                    }
                } else {
                    file_indices.insert(key_path.clone(), files.len());
                    files.push(LatestTurnFile {
                        display_path: key_path.clone(),
                        git_path: key_path,
                        additions,
                        deletions,
                        diff_files: vec![diff_file],
                        raw_patch: file_patch,
                        from_real_patch,
                    });
                }
            }
        }
    }

    if files.is_empty() {
        return LatestTurnSnapshot::Empty {
            has_active_turn: in_progress,
            session_open: true,
        };
    }

    let anchor_id = slice[slice.len() - 1].id.clone();
    LatestTurnSnapshot::Ready {
        anchor_id,
        files,
        total_additions,
        total_deletions,
        in_progress,
    }
}

pub(crate) fn is_plain_user_message(message: &crate::app::Message) -> bool {
    message.role == "user"
        && message.delivery.is_none()
        && message.tool.is_none()
        && !message.display_body().trim().is_empty()
}

fn has_user_trailing_extras(message: &crate::app::Message) -> bool {
    let body = if message.explicit_user {
        crate::quote_source::body_without_quote_source(message.display_body())
    } else {
        std::borrow::Cow::Borrowed(message.display_body())
    };
    !message.images.is_empty()
        || message.history_images.is_some()
        || !message.files.is_empty()
        || crate::quote_source::body_without_quote_source(message.display_body()).len()
            != message.display_body().len()
        || view::is_collapsible_user_message(&body)
}

pub(crate) fn user_bubble_group_flags(
    messages: &[crate::app::Message],
    index: usize,
    live_start: Option<usize>,
) -> (bool, bool) {
    let Some(curr) = messages.get(index) else {
        return (false, false);
    };
    if !is_plain_user_message(curr) {
        return (false, false);
    }
    let merge_prev = index > 0
        && live_start != Some(index)
        && messages.get(index - 1).is_some_and(|prev| {
            is_plain_user_message(prev)
                && !has_user_trailing_extras(prev)
                && prev.steered == curr.steered
        });
    let merge_next = index + 1 < messages.len()
        && live_start != Some(index + 1)
        && !has_user_trailing_extras(curr)
        && messages
            .get(index + 1)
            .is_some_and(|next| is_plain_user_message(next) && next.steered == curr.steered);
    (merge_prev, merge_next)
}

pub(crate) fn collect_turn_diff_summary(
    anchor_id: &str,
    slice: &[crate::app::Message],
    tool_inputs: &HashMap<String, serde_json::Value>,
    english_labels: bool,
) -> Option<TurnDiffSummary> {
    let mut file_indices: HashMap<String, usize> = HashMap::new();
    let mut files: Vec<TurnDiffFileSummary> = Vec::new();
    let mut tool_changes: Vec<(String, serde_json::Value, Option<String>)> = Vec::new();
    let mut total_additions = 0usize;
    let mut total_deletions = 0usize;

    for msg in slice {
        let Some(tool) = &msg.tool else {
            continue;
        };
        if tool.status != "完成" {
            continue;
        }
        let args = tool_inputs
            .get(&msg.id)
            .unwrap_or(&serde_json::Value::Null);
        if !crate::tool_diff::data::available(&tool.name, args, tool.patch.as_deref()) {
            continue;
        }
        let changes =
            crate::tool_diff::data::changes(&tool.name, args, tool.patch.as_deref(), english_labels);
        let doc = crate::tool_diff::data::build(changes);
        if doc.files.is_empty() {
            continue;
        }
        tool_changes.push((tool.name.clone(), args.clone(), tool.patch.clone()));
        let explicit_arg_path = args["file_path"]
            .as_str()
            .or_else(|| args["path"].as_str())
            .filter(|p| !p.trim().is_empty());
        let is_edit_tool = matches!(
            tool.name.to_ascii_lowercase().as_str(),
            "edit" | "multiedit"
        );
        for file in &doc.files {
            let raw_path = file.path().to_string();
            let key_path = if is_edit_tool {
                explicit_arg_path.unwrap_or(&raw_path).to_owned()
            } else {
                raw_path.clone()
            };
            let additions = file.additions();
            let deletions = file.deletions();
            total_additions += additions;
            total_deletions += deletions;
            if let Some(&idx) = file_indices.get(&key_path) {
                files[idx].additions += additions;
                files[idx].deletions += deletions;
            } else {
                file_indices.insert(key_path.clone(), files.len());
                files.push(TurnDiffFileSummary {
                    display_path: key_path.clone(),
                    git_path: key_path,
                    additions,
                    deletions,
                });
            }
        }
    }

    if files.is_empty() {
        return None;
    }

    Some(TurnDiffSummary {
        anchor_id: anchor_id.to_owned(),
        files,
        total_additions,
        total_deletions,
        tool_changes,
    })
}

pub struct ConversationList {
    pub state: ListState,
    rows: Vec<Row>,
    pub pending_anchor: Option<ReadingAnchor>,
    pub tail_focus: FocusHandle,
    hint_width: Rc<Cell<Pixels>>,
    font_size: Pixels,
    pub(crate) markdown: HashMap<String, markdown::MarkdownView>,
    pub(crate) visible_history_images: HashSet<String>,
    pub(crate) detached_pill_visible: bool,
    detached_debounce_task: Option<Task<()>>,
    turn_diff_cache: HashMap<String, (u64, Option<TurnDiffSummary>)>,
    pub(crate) active_turn_diffs: HashMap<String, TurnDiffSummary>,
}
impl ConversationList {
    pub fn new(tail_focus: FocusHandle) -> Self {
        let state = ListState::new(0, ListAlignment::Bottom, px(350.));
        state.set_follow_mode(FollowMode::Tail);
        Self {
            state,
            rows: Vec::new(),
            pending_anchor: None,
            tail_focus,
            hint_width: Rc::new(Cell::new(px(0.))),
            font_size: px(0.),
            markdown: HashMap::new(),
            visible_history_images: HashSet::new(),
            detached_pill_visible: false,
            detached_debounce_task: None,
            turn_diff_cache: HashMap::new(),
            active_turn_diffs: HashMap::new(),
        }
    }
    pub(crate) fn clear_detached_pill(&mut self) {
        self.detached_pill_visible = false;
        self.detached_debounce_task = None;
    }
    pub fn anchor(&self) -> Option<ReadingAnchor> {
        let offset = self.state.logical_scroll_top();
        self.rows.get(offset.item_ix).map(|row| ReadingAnchor {
            key: row.key.clone(),
            offset: offset.offset_in_item,
        })
    }
    pub fn reset(&mut self, anchor: Option<ReadingAnchor>) {
        self.rows = Vec::new();
        self.markdown = HashMap::new();
        self.visible_history_images = HashSet::new();
        self.detached_pill_visible = false;
        self.detached_debounce_task = None;
        self.turn_diff_cache.clear();
        self.active_turn_diffs.clear();
        self.state.reset(0);
        self.state.set_follow_mode(FollowMode::Tail);
        self.pending_anchor = anchor;
    }
}

impl HiveApp {
    pub(crate) fn latest_turn_diff(&self) -> LatestTurnSnapshot {
        let session_open = self.session.is_some();
        let session_busy = self
            .session
            .as_ref()
            .is_some_and(|s| self.busy.contains(&s.id));
        collect_latest_turn_snapshot(
            &self.messages,
            &self.tool_inputs,
            session_open,
            session_busy,
            crate::language::english(),
        )
    }

    fn conversation_message_groups(&self) -> impl Iterator<Item = &[crate::app::Message]> {
        let (history, live) = self.messages.split_at(
            self.history_navigation.live_start.unwrap_or(self.messages.len()),
        );
        history.chunk_by(|a, b| a.completed_tool() && b.completed_tool())
            .chain(live.chunk_by(|a, b| a.completed_tool() && b.completed_tool()))
    }

    pub(crate) fn sync_conversation_list(&mut self, cx: &App) {
        let live_start = self.history_navigation.live_start;
        let session_busy = self
            .session
            .as_ref()
            .is_some_and(|s| self.busy.contains(&s.id));
        let eng = crate::language::english();
        let mut turn_end_diffs: HashMap<usize, (u64, String)> = HashMap::new();
        let mut next_active_diffs: HashMap<String, TurnDiffSummary> = HashMap::new();
        let mut seg_start: Option<usize> = None;
        let mut flush_turn = |start: usize, end: usize, settled: bool| {
            if !settled || start >= end || live_start == Some(end) {
                return;
            }
            let slice = &self.messages[start..end];
            if slice
                .iter()
                .any(|m| m.tool.as_ref().is_some_and(|t| t.status == "执行中"))
            {
                return;
            }
            let anchor_id = slice[end - start - 1].id.clone();
            let mut rev: u64 = u64::from(eng) + (slice.len() as u64).wrapping_mul(31);
            for m in slice {
                rev = rev.wrapping_mul(131).wrapping_add(m.layout_revision + 1);
                if let Some(t) = &m.tool
                    && t.status == "完成"
                {
                    rev = rev.wrapping_add(17);
                    if t.patch.is_some() {
                        rev = rev.wrapping_add(53);
                    }
                    if self.tool_inputs.contains_key(&m.id) {
                        rev = rev.wrapping_add(97);
                    }
                }
            }
            let cached = self
                .conversation_list
                .turn_diff_cache
                .get(&anchor_id)
                .filter(|(cached_rev, _)| *cached_rev == rev)
                .map(|(_, summary)| summary.clone());
            let summary = match cached {
                Some(hit) => hit,
                None => {
                    let computed =
                        collect_turn_diff_summary(&anchor_id, slice, &self.tool_inputs, eng);
                    self.conversation_list
                        .turn_diff_cache
                        .insert(anchor_id.clone(), (rev, computed.clone()));
                    computed
                }
            };
            if let Some(summary) = summary {
                turn_end_diffs.insert(end - 1, (rev, anchor_id.clone()));
                next_active_diffs.insert(anchor_id, summary);
            }
        };
        for (idx, msg) in self.messages.iter().enumerate() {
            let is_boundary = msg.role == "user" || msg.delivery.is_some();
            if is_boundary {
                if let Some(start) = seg_start.take() {
                    flush_turn(start, idx, true);
                }
            } else if seg_start.is_none() {
                seg_start = Some(idx);
            }
        }
        if let Some(start) = seg_start.take() {
            flush_turn(start, self.messages.len(), !session_busy);
        }
        self.conversation_list.active_turn_diffs = next_active_diffs;

        let mut rows = Vec::with_capacity(self.messages.len() + 4);
        let loading = self.history_navigation.loading_page;
        if let Some(page) = self.history_page.as_ref().filter(|page| page.has_older) {
            rows.push(Row {
                key: RowKey::LoadOlder,
                kind: Kind::LoadHistory {
                    direction: HistoryDirection::Older,
                    remaining: (page.start_index > 0).then_some(page.start_index),
                    loading,
                },
                revision: 0,
                expanded: false,
            });
        }
        let mut gap = self.history_navigation.live_start.map(|_| Row {
            key: RowKey::LoadNewer,
            kind: Kind::LoadHistory { direction: HistoryDirection::Newer, remaining: None, loading },
            revision: 0,
            expanded: false,
        });
        let mut offset = 0;
        for group in self.conversation_message_groups() {
            if self.history_navigation.live_start == Some(offset) && let Some(row) = gap.take() {
                rows.push(row);
            }
            let grouped = group.len() > 1
                && cx.global::<crate::composer::ComposerPreferences>().settings.tool_activity
                    == crate::composer::ToolActivity::Grouped;
            let expanded = grouped
                && (self.expanded_tool_groups.contains(&group[0].id)
                    || group
                        .iter()
                        .any(|m| m.tool.as_ref().is_some_and(|t| t.expanded)));
            if grouped {
                rows.push(Row {
                    key: RowKey::Group(group[0].id.clone()),
                    kind: Kind::Group {
                        start: offset,
                        end: offset + group.len(),
                        expanded,
                    },
                    revision: 0,
                    expanded,
                });
            }
            if !grouped || expanded {
                for (index, message) in group.iter().enumerate() {
                    if self.hidden_current_failure(offset + index)
                        && message.text.is_empty()
                        && message.thinking.is_empty()
                        && message.images.is_empty()
                        && message.history_images.is_none()
                        && message.files.is_empty()
                    {
                        continue;
                    }
                    let (merge_prev, merge_next) =
                        user_bubble_group_flags(&self.messages, offset + index, live_start);
                    rows.push(Row {
                        key: RowKey::Message(message.id.clone()),
                        kind: Kind::Message {
                            index: offset + index,
                            indented: grouped,
                            last: index + 1 == group.len(),
                            merge_prev,
                            merge_next,
                        },
                        revision: message.layout_revision,
                        expanded: message.tool.as_ref().is_some_and(|t| t.expanded)
                            || message.delivery.as_ref().is_some_and(|c| c.expanded)
                            || message.user_expanded
                            || self.conversation_find_matches(&message.id),
                    });
                }
            }
            let group_last_idx = offset + group.len().saturating_sub(1);
            if let Some((rev, anchor_id)) = turn_end_diffs.get(&group_last_idx) {
                rows.push(Row {
                    key: RowKey::TurnDiff(anchor_id.clone()),
                    kind: Kind::TurnDiff {
                        anchor_id: anchor_id.clone(),
                    },
                    revision: *rev,
                    expanded: false,
                });
            }
            offset += group.len();
        }
        rows.extend(gap);
        // Pending reports and native interaction forms remain at the end of the
        // same scroll area. Invalidate this small dynamic row on each view update.
        rows.push(Row {
            key: RowKey::Tail,
            kind: Kind::Tail,
            revision: 0,
            expanded: false,
        });
        let list = &mut self.conversation_list;
        let font_size = cx.global::<gpui_kit::component::Theme>().font_size;
        let typography_changed = list.font_size != font_size;
        list.font_size = font_size;
        let following = list.state.is_following_tail();
        let anchor = (!following).then(|| list.anchor()).flatten();
        let same_keys = rows.len() == list.rows.len()
            && rows.iter().zip(&list.rows).all(|(a, b)| a.key == b.key);
        if same_keys {
            for (index, (next, old)) in rows.iter().zip(&list.rows).enumerate() {
                if typography_changed || next != old || next.kind == Kind::Tail {
                    list.state.remeasure_items(index..index + 1);
                }
            }
        } else {
            let prefix = rows
                .iter()
                .zip(&list.rows)
                .take_while(|(a, b)| a == b)
                .count();
            list.state.splice_focusable(
                prefix..list.rows.len(),
                rows[prefix..].iter().map(|row| match row.kind {
                    Kind::Tail => Some(list.tail_focus.clone()),
                    Kind::Message { index, .. } => self.messages[index].markdown_focus.clone(),
                    _ => None,
                }),
            );
            // Hints give an initial scrollbar extent without measuring every
            // message; actual visible heights replace them during layout.
            let _ = list.state.clone().with_uniform_item_height(px(160.));
        }
        list.rows = rows;
        if let Some(anchor) = list.pending_anchor.take().or(anchor) {
            let index = list
                .rows
                .iter()
                .position(|row| row.key == anchor.key)
                .or_else(|| {
                    let id = match &anchor.key {
                        RowKey::Message(id) | RowKey::Group(id) => id,
                        _ => return None,
                    };
                    let message_index = *self.message_indices.get(id)?;
                    list.rows.iter().position(|row| match row.kind {
                        Kind::Group { start, end, .. } => (start..end).contains(&message_index),
                        Kind::Message { index, .. } => index == message_index,
                        _ => false,
                    })
                });
            if let Some(index) = index {
                list.state.scroll_to(ListOffset {
                    item_ix: index,
                    offset_in_item: anchor.offset,
                });
            }
        } else if following {
            list.state.scroll_to_end();
        }
        if self.conversation_find.is_some() {
            list.state.pause_following_tail();
        }
        if !self.is_detached_from_bottom() {
            self.conversation_list.clear_detached_pill();
        }
    }

    pub(crate) fn update_detached_scroll_pill(&mut self, cx: &mut Context<Self>) {
        if !self.is_detached_from_bottom() {
            if self.conversation_list.detached_pill_visible
                || self.conversation_list.detached_debounce_task.is_some()
            {
                self.conversation_list.clear_detached_pill();
                cx.notify();
            }
            return;
        }
        if self.conversation_list.detached_pill_visible
            || self.conversation_list.detached_debounce_task.is_some()
        {
            return;
        }
        self.conversation_list.detached_debounce_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let _ = this.update(cx, |app, cx| {
                app.conversation_list.detached_debounce_task = None;
                if app.is_detached_from_bottom() {
                    app.conversation_list.detached_pill_visible = true;
                    cx.notify();
                }
            });
        }));
    }

    pub(crate) fn prepare_conversation_expansion(&mut self, id: &str) {
        let list = &mut self.conversation_list;
        list.state.pause_following_tail();
        // Tail-following can retain an end sentinel rather than a visible row.
        // Anchor the clicked row when there is no concrete reading position.
        if list.anchor().is_none() {
            let key = list
                .rows
                .iter()
                .find(|row| matches!(&row.key, RowKey::Message(key) if key == id))
                .or_else(|| {
                    list.rows
                        .iter()
                        .find(|row| matches!(&row.key, RowKey::Group(key) if key == id))
                })
                .map(|row| row.key.clone());
            if let Some(key) = key {
                list.pending_anchor = Some(ReadingAnchor {
                    key,
                    offset: px(0.),
                });
            }
        }
    }

    pub(crate) fn reveal_conversation_tool_group(&mut self, id: &str) {
        let group = self.conversation_message_groups()
            .find(|group| group.len() > 1 && group.iter().any(|message| message.id == id))
            .map(|group| group[0].id.clone());
        if let Some(id) = group {
            self.expanded_tool_groups.insert(id);
        }
    }

    pub(crate) fn scroll_to_conversation_message(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        self.reveal_conversation_tool_group(id);
        self.sync_conversation_list(cx);
        if let Some(index) = self
            .conversation_list
            .rows
            .iter()
            .position(|row| matches!(&row.key, RowKey::Message(key) if key == id))
        {
            self.conversation_list.state.scroll_to(ListOffset {
                item_ix: index,
                offset_in_item: px(0.),
            });
            self.update_detached_scroll_pill(cx);
            true
        } else {
            false
        }
    }

    pub(crate) fn render_conversation_list(
        &self,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rendered_markdown = Rc::new(RefCell::new(HashSet::new()));
        let rendered_rows = rendered_markdown.clone();
        let rendered_history_images = Rc::new(RefCell::new(HashSet::new()));
        let image_rows = rendered_history_images.clone();
        let content = list(
            self.conversation_list.state.clone(),
            cx.processor(move |this, index: usize, window, cx| {
                let Some(row) = this.conversation_list.rows.get(index).cloned() else {
                    return div().into_any_element();
                };
                let next_row = this.conversation_list.rows.get(index + 1).cloned();
                let is_tail = matches!(row.kind, Kind::Tail);
                let content = match row.kind {
                    Kind::LoadHistory { direction, remaining, loading } => {
                        use crate::language::{english, tr};
                        use gpui_kit::component::{Disableable, Sizable, button::Button, h_flex};
                        let newer = direction == HistoryDirection::Newer;
                        let loading_here = loading == Some(direction);
                        let label = if this.history_navigation.snapshot_stale {
                            tr("重新读取会话历史").to_owned()
                        } else if newer {
                            tr(if loading_here { "正在加载更新历史…" } else { "加载更新历史" }).to_owned()
                        } else if loading_here {
                            tr("正在加载更早历史…").to_owned()
                        } else {
                            match remaining {
                                Some(remaining) if english() => format!("Load earlier history ({remaining} more)"),
                                Some(remaining) => format!("加载更早历史（还有 {remaining} 条）"),
                                None => tr("加载更早历史").to_owned(),
                            }
                        };
                        h_flex()
                            .w_full()
                            .justify_center()
                            .py_2()
                            .child(
                                Button::new(if newer { "load-newer-history" } else { "load-older-history" })
                                    .outline()
                                    .compact()
                                    .xsmall()
                                    .h(px(28.))
                                    .px_3()
                                    .rounded_full()
                                    .icon(if newer { gpui_kit::assets::IconName::ChevronDown } else { gpui_kit::assets::IconName::ChevronUp })
                                    .label(label)
                                    .disabled(loading.is_some() || !this.connected)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_history_page(direction, cx);
                                    })),
                            )
                            .into_any_element()
                    }
                    Kind::Message { index: msg_idx, merge_prev, merge_next, .. } => {
                        let markdown = this.show_message_markdown(msg_idx, cx);
                        if markdown.is_some() {
                            rendered_rows.borrow_mut().insert(this.messages[msg_idx].id.clone());
                        }
                        if this.messages[msg_idx].history_images.is_some() {
                            image_rows.borrow_mut().insert(this.messages[msg_idx].id.clone());
                        }
                        this.render_conversation_message(
                            msg_idx,
                            markdown.as_ref(),
                            merge_prev,
                            merge_next,
                            cx,
                        )
                    }
                    Kind::Group { start, end, .. } => this
                        .render_completed_tools_header(start, &this.messages[start..end], cx)
                        .into_any_element(),
                    Kind::TurnDiff { ref anchor_id } => {
                        this.render_turn_diff_card(anchor_id, window, cx)
                    }
                    Kind::Tail => this.render_conversation_tail(cx).into_any_element(),
                };
                let indented = matches!(row.kind, Kind::Message { indented: true, .. });
                let compact = matches!(
                    row.kind,
                    Kind::Group { expanded: true, .. }
                        | Kind::Message {
                            indented: true,
                            last: false,
                            ..
                        }
                );
                let merge_next_user = matches!(row.kind, Kind::Message { merge_next: true, .. });
                let consecutive_tool = match (&row.kind, next_row.as_ref().map(|r| &r.kind)) {
                    (Kind::Message { index: cur, .. }, Some(Kind::Message { index: nxt, .. })) => {
                        this.messages.get(*cur).is_some_and(|m| m.tool.is_some())
                            && this.messages.get(*nxt).is_some_and(|m| m.tool.is_some())
                    }
                    (Kind::Message { index: cur, .. }, Some(Kind::Group { .. })) => {
                        this.messages.get(*cur).is_some_and(|m| m.tool.is_some())
                    }
                    (Kind::Group { expanded: false, .. }, Some(Kind::Message { index: nxt, .. })) => {
                        this.messages.get(*nxt).is_some_and(|m| m.tool.is_some())
                    }
                    _ => false,
                };
                let chat_max_w = cx
                    .global::<crate::appearance::Appearance>()
                    .chat_width_max_px();
                v_flex()
                    .w_full()
                    .px(if narrow { px(16.) } else { px(32.) })
                    .when(index == 0, |row| row.pt_3())
                    .when(is_tail, |row| row.pb_3())
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .when_some(chat_max_w, |col, w| col.max_w(px(w)))
                            .mx_auto()
                            .pb(if is_tail {
                                px(0.)
                            } else if compact {
                                px(2.)
                            } else if merge_next_user || consecutive_tool {
                                px(4.)
                            } else {
                                px(16.)
                            })
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .when(indented, |body| {
                                        body.ml_2p5()
                                            .pl_2p5()
                                            .border_l_1()
                                            .border_color(crate::palette::Color::Divider.get(cx))
                                    })
                                    .child(content),
                            ),
                    )
                    .into_any_element()
            }),
        )
        .flex_1()
        .min_h_0()
        .min_w_0();
        let hint_width = self.conversation_list.hint_width.clone();
        let state = self.conversation_list.state.clone();
        let weak = cx.weak_entity();
        let open_request = self.open_request;
        div()
            .on_children_prepainted(move |bounds, window, cx| {
                let rendered = std::mem::take(&mut *rendered_markdown.borrow_mut());
                let history_images = std::mem::take(&mut *rendered_history_images.borrow_mut());
                let view = weak.clone();
                window.defer(cx, move |window, cx| {
                    let Some(view) = view.upgrade() else {
                        return;
                    };
                    let app = view.read(cx);
                    if app.open_request != open_request
                        || !app.viewing_conversation()
                        || app.showing_welcome_guide
                        || (app.sidebar_open && window.viewport_size().width < px(820.))
                    {
                        return;
                    }
                    let keep = |id: &String, markdown: &markdown::MarkdownView, cx: &App| {
                        rendered.contains(id)
                            || markdown.state.read(cx).focus_handle().contains_focused(window, cx)
                    };
                    let release_markdown = app.conversation_list.markdown.iter().any(|(id, view)| !keep(id, view, cx));
                    let images_changed = app.conversation_list.visible_history_images != history_images;
                    let detached = app.is_detached_from_bottom();
                    let pill_out_of_sync = if detached {
                        !app.conversation_list.detached_pill_visible
                            && app.conversation_list.detached_debounce_task.is_none()
                    } else {
                        app.conversation_list.detached_pill_visible
                            || app.conversation_list.detached_debounce_task.is_some()
                    };
                    if release_markdown || images_changed || pill_out_of_sync {
                        view.update(cx, |app, cx| {
                            if release_markdown {
                                app.conversation_list.markdown.retain(|id, view| keep(id, view, cx));
                            }
                            if images_changed {
                                app.sync_history_image_visibility(history_images, cx);
                            }
                            if pill_out_of_sync {
                                app.update_detached_scroll_pill(cx);
                            }
                        });
                    }
                });
                let Some(bounds) = bounds.first() else {
                    return;
                };
                let width = bounds.size.width;
                if width > px(0.) && hint_width.replace(width) != width {
                    // ListState invalidates hints when its width changes. Restore
                    // estimates after that layout, without measuring offscreen rows.
                    let state = state.clone();
                    let weak = weak.clone();
                    let hint_width = hint_width.clone();
                    window.on_next_frame(move |_, cx| {
                        if hint_width.get() != width {
                            return;
                        }
                        let _ = state.with_uniform_item_height(px(160.));
                        let _ = weak.update(cx, |_, cx| cx.notify());
                    });
                }
            })
            .id("conversation-history")
            .track_focus(self.workspace_focus.region(crate::workspace_focus::Area::History))
            .tab_stop(false)
            .role(accesskit::Role::Region)
            .aria_label(crate::language::tr("会话历史"))
            .border_b_1()
            .border_color(gpui_kit::transparent_black())
            .focus(|style| style.border_color(crate::palette::Color::Accent.get(cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.workspace_focus.region(crate::workspace_focus::Area::History).is_focused(window)
                    || event.keystroke.modifiers != Modifiers::none()
                {
                    return;
                }
                let state = &this.conversation_list.state;
                let page = (window.viewport_size().height * 0.5).max(px(40.));
                let scroll_step = |delta: Pixels| {
                    let max_offset = state.max_offset_for_scrollbar().y;
                    if max_offset > px(0.) {
                        let current =
                            (-state.scroll_px_offset_for_scrollbar().y).clamp(px(0.), max_offset);
                        let next = (current + delta).clamp(px(0.), max_offset);
                        state.set_offset_from_scrollbar(point(px(0.), -next));
                    } else {
                        state.scroll_by(delta);
                    }
                };
                match event.keystroke.key.as_str() {
                    "up" => scroll_step(-px(40.)),
                    "down" => scroll_step(px(40.)),
                    "pageup" => scroll_step(-page),
                    "pagedown" => scroll_step(page),
                    "home" => state.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) }),
                    "end" => state.scroll_to_end(),
                    _ => return,
                }
                this.schedule_reading_position_save(cx);
                this.update_detached_scroll_pill(cx);
                cx.notify();
                cx.stop_propagation();
            }))
            .relative()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(content)
            .when(
                self.conversation_list.detached_pill_visible && self.is_detached_from_bottom(),
                |root| {
                    use gpui_kit::component::button::{
                        Button, ButtonCustomVariant, ButtonVariants as _,
                    };
                    let pill_variant = ButtonCustomVariant::new(cx)
                        .color(crate::palette::Color::Overlay.get(cx).into())
                        .foreground(crate::palette::Color::Text.get(cx).into())
                        .hover(crate::palette::Color::Inset.get(cx).into())
                        .active(crate::palette::Color::Inset.get(cx).into());
                    root.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(px(12.))
                            .flex()
                            .justify_center()
                            .child(
                                Button::new("scroll-to-bottom-pill")
                                    .custom(pill_variant)
                                    .compact()
                                    .h(px(30.))
                                    .px_3()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(crate::palette::Color::BorderStrong.get(cx))
                                    .shadow(crate::palette::overlay_shadow(cx))
                                    .icon(gpui_kit::assets::IconName::ArrowDown)
                                    .label(crate::language::tr("回到底部"))
                                    .accessibility_label(crate::language::tr("回到底部最新消息"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scroll_to_bottom(cx);
                                    })),
                            ),
                    )
                },
            )
            .child(gpui_kit::component::scroll::Scrollbar::vertical(
                &self.conversation_list.state,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::{RowKey, collect_turn_diff_summary, user_bubble_group_flags};
    use crate::app::{DeliveryCard, HiveApp, Message, ToolPresentation, init_test_globals};
    use crate::protocol::{Session, Workspace};
    use gpui_kit::{
        AppContext as _, ScrollDelta, ScrollEvent, ScrollHandle, TestAppContext, point, px,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::time::Duration;

    fn user_msg(id: &str, text: &str, steered: bool) -> Message {
        Message {
            layout_revision: 0,
            id: id.into(),
            role: "user".into(),
            native_user_item_id: None,
            handoff_item_id: None,
            source_session_id: None,
            text: text.into(),
            timestamp_ms: None,
            steered,
            user_expanded: false,
            markdown_focus: None,
            tool: None,
            thinking: vec![],
            delivery: None,
            explicit_user: true,
            failure: None,
            files: vec![],
            history_images: None,
            images: vec![],
            image_sources: vec![],
            images_loading: false,
            images_error: None,
            image_task: None,
        }
    }

    fn tool_msg(id: &str, name: &str, status: &'static str, patch: Option<&str>) -> Message {
        Message {
            role: "toolResult".into(),
            explicit_user: false,
            tool: Some(ToolPresentation {
                patch: patch.map(str::to_owned),
                name: name.into(),
                status,
                expanded: false,
                details_scroll: ScrollHandle::new(),
                output_windows: vec![],
                started_at: None,
                duration_ms: None,
            }),
            ..user_msg(id, "", false)
        }
    }

    #[test]
    fn consecutive_plain_user_grouping_respects_boundaries_steer_and_extras() {
        let mut delivery_msg = user_msg("d1", "dispatch", false);
        delivery_msg.role = "notice".into();
        delivery_msg.delivery = Some(DeliveryCard {
            dispatch_id: None,
            report: false,
            worker_id: None,
            artifacts: vec![],
            heading: "派单",
            worker_name: None,
            raw: String::new(),
            expanded: false,
            missing_provenance: false,
        });
        let messages = vec![
            user_msg("u1", "hello", false),
            user_msg("u2", "follow up", false),
            user_msg("u3", "steer here", true),
            user_msg("u4", "steer two", true),
            delivery_msg,
            user_msg("u5", "after delivery", false),
        ];

        assert_eq!(user_bubble_group_flags(&messages, 0, None), (false, true));
        assert_eq!(user_bubble_group_flags(&messages, 1, None), (true, false));
        // Steer start does not merge with previous non-steer, but merges with next steer.
        assert_eq!(user_bubble_group_flags(&messages, 2, None), (false, true));
        assert_eq!(user_bubble_group_flags(&messages, 3, None), (true, false));
        // Delivery breaks grouping.
        assert_eq!(user_bubble_group_flags(&messages, 4, None), (false, false));
        assert_eq!(user_bubble_group_flags(&messages, 5, None), (false, false));
        // LoadNewer gap at index 1 prevents merging u1 <-> u2.
        assert_eq!(user_bubble_group_flags(&messages, 0, Some(1)), (false, false));
        assert_eq!(user_bubble_group_flags(&messages, 1, Some(1)), (false, false));

        // Empty-body image/attachment user message must not merge with preceding text bubble.
        let mut image_only = user_msg("u-img", "", false);
        image_only.images_loading = true;
        let whitespace_only = user_msg("u-ws", "   \n ", false);
        let with_empty = vec![
            user_msg("u-text-1", "first", false),
            image_only,
            user_msg("u-text-2", "second", false),
            whitespace_only,
        ];
        assert_eq!(user_bubble_group_flags(&with_empty, 0, None), (false, false));
        assert_eq!(user_bubble_group_flags(&with_empty, 1, None), (false, false));
        assert_eq!(user_bubble_group_flags(&with_empty, 2, None), (false, false));
        assert_eq!(user_bubble_group_flags(&with_empty, 3, None), (false, false));
    }

    #[test]
    fn turn_diff_summary_aggregates_real_edits_and_returns_none_without_changes() {
        let mut inputs = HashMap::new();
        inputs.insert("t-read".into(), json!({"file_path": "README.md"}));
        inputs.insert(
            "t-edit".into(),
            json!({"file_path": "src/lib.rs", "oldText": "old\n", "newText": "new1\nnew2\n"}),
        );
        inputs.insert(
            "t-fc".into(),
            json!({"changes": [{"path": "src/app.rs", "kind": {"type": "add"}, "diff": "line1\n"}]}),
        );

        let read_only = [tool_msg("t-read", "read", "完成", None)];
        assert!(collect_turn_diff_summary("t-read", &read_only, &inputs, false).is_none());

        let with_edits = [
            tool_msg("t-read", "read", "完成", None),
            tool_msg("t-edit", "edit", "完成", None),
            tool_msg("t-fc", "fileChange", "完成", None),
        ];
        let summary = collect_turn_diff_summary("t-fc", &with_edits, &inputs, false).unwrap();
        assert_eq!(summary.files.len(), 2);
        assert_eq!(summary.total_additions, 3);
        assert_eq!(summary.total_deletions, 1);
    }

    #[test]
    fn latest_turn_snapshot_strictly_reads_last_turn_without_falling_back() {
        use super::{LatestTurnSnapshot, collect_latest_turn_snapshot};
        let mut inputs = HashMap::new();
        inputs.insert(
            "t1-edit".into(),
            json!({"file_path": "src/a.rs", "oldText": "a1\n", "newText": "a2\n"}),
        );
        inputs.insert("t2-read".into(), json!({"file_path": "README.md"}));
        inputs.insert(
            "t3-edit-1".into(),
            json!({"file_path": "src/b.rs", "oldText": "b1\n", "newText": "b2\nb3\n"}),
        );
        inputs.insert(
            "t3-edit-2".into(),
            json!({"file_path": "src/b.rs", "oldText": "b3\n", "newText": "b4\n"}),
        );

        let no_session = collect_latest_turn_snapshot(&[], &inputs, false, false, false);
        assert!(matches!(
            no_session,
            LatestTurnSnapshot::Empty {
                session_open: false,
                ..
            }
        ));

        // Turn 1 had an edit, Turn 2 is read-only -> MUST return Empty, never Turn 1.
        let read_only_last = vec![
            user_msg("u1", "edit a", false),
            tool_msg("t1-edit", "edit", "完成", None),
            user_msg("u2", "read readme", false),
            tool_msg("t2-read", "read", "完成", None),
        ];
        let snap = collect_latest_turn_snapshot(&read_only_last, &inputs, true, false, false);
        assert!(matches!(
            snap,
            LatestTurnSnapshot::Empty {
                session_open: true,
                has_active_turn: false
            }
        ));

        // Turn 3 edits src/b.rs twice -> Ready with 1 file and 2 DiffFiles.
        let mut multi_edit_last = read_only_last;
        multi_edit_last.push(user_msg("u3", "edit b", false));
        multi_edit_last.push(tool_msg("t3-edit-1", "edit", "完成", None));
        multi_edit_last.push(tool_msg("t3-edit-2", "edit", "完成", None));
        match collect_latest_turn_snapshot(&multi_edit_last, &inputs, true, false, false) {
            LatestTurnSnapshot::Ready {
                files,
                total_additions,
                total_deletions,
                ..
            } => {
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].git_path, "src/b.rs");
                assert_eq!(files[0].diff_files.len(), 2);
                assert_eq!(total_additions, 3);
                assert_eq!(total_deletions, 2);
            }
            _ => panic!("expected Ready snapshot for latest turn"),
        }

        // Multi-file tool.patch must slice per-file raw_patch without leaking sibling files.
        let multi_file_patch = concat!(
            "diff --git a/src/ws_only.rs b/src/ws_only.rs\n",
            "--- a/src/ws_only.rs\n",
            "+++ b/src/ws_only.rs\n",
            "@@ -1,1 +1,1 @@\n",
            "-  let a = 1;\n",
            "+    let a = 1;\n",
            "diff --git a/src/real.rs b/src/real.rs\n",
            "--- a/src/real.rs\n",
            "+++ b/src/real.rs\n",
            "@@ -1,1 +1,1 @@\n",
            "-let b = 1;\n",
            "+let b = 2;\n",
        );
        let multi_file_turn = vec![
            user_msg("u4", "patch both", false),
            tool_msg("t4-patch", "apply_patch", "完成", Some(multi_file_patch)),
        ];
        match collect_latest_turn_snapshot(&multi_file_turn, &inputs, true, false, false) {
            LatestTurnSnapshot::Ready { files, .. } => {
                assert_eq!(files.len(), 2);
                assert_eq!(files[0].git_path, "src/ws_only.rs");
                assert!(files[0].raw_patch.contains("src/ws_only.rs"));
                assert!(!files[0].raw_patch.contains("src/real.rs"));
                assert_eq!(files[1].git_path, "src/real.rs");
                assert!(files[1].raw_patch.contains("src/real.rs"));
                assert!(!files[1].raw_patch.contains("src/ws_only.rs"));
            }
            _ => panic!("expected Ready snapshot for multi-file patch"),
        }
    }

    fn setup_conversation_window(
        cx: &mut TestAppContext,
        messages: Vec<Message>,
    ) -> (
        gpui_kit::Entity<HiveApp>,
        &mut gpui_kit::VisualTestContext,
        crate::language::TestLocaleGuard,
    ) {
        let locale = init_test_globals(cx);
        let (window, app) = cx.update(|cx| {
            let bounds = gpui_kit::Bounds::maximized(None, cx);
            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| HiveApp::new_for_test(window, cx)),
            )
            .unwrap()
        });
        let cx = gpui_kit::VisualTestContext::from_window(window, cx).into_mut();
        cx.run_until_parked();
        let session: Session = serde_json::from_value(json!({
            "id": "s1",
            "title": "Test Session",
            "workspaceId": "w1",
            "agentId": "pi"
        }))
        .unwrap();
        app.update(cx, |app, cx| {
            app.connected = true;
            app.loading = false;
            app.conversation_ready = true;
            app.sidebar_open = false;
            app.workspaces = vec![Workspace {
                id: "w1".into(),
                name: "Workspace".into(),
                path: "/tmp/w1".into(),
            }];
            app.workspace = Some("w1".into());
            app.sessions = vec![session.clone()];
            app.session = Some(session);
            for (ix, msg) in messages.into_iter().enumerate() {
                app.message_indices.insert(msg.id.clone(), ix);
                app.messages.push(msg);
            }
            app.sync_conversation_list(cx);
            cx.notify();
        });
        cx.run_until_parked();
        (app, cx, locale)
    }

    fn sample_messages(count: usize) -> Vec<Message> {
        (0..count)
            .map(|i| {
                let mut msg = user_msg(&format!("u-{i}"), &format!("message number {i}"), false);
                if i % 2 == 1 {
                    msg.role = "assistant".into();
                    msg.explicit_user = false;
                }
                msg
            })
            .collect()
    }

    #[gpui_kit::test]
    fn wheel_scroll_does_not_panic_and_activates_detached_pill_after_debounce(
        cx: &mut TestAppContext,
    ) {
        let (app, cx, _locale) = setup_conversation_window(cx, sample_messages(30));
        app.read_with(cx, |app, _| {
            assert!(app.conversation_list.state.is_following_tail());
            assert!(!app.is_detached_from_bottom());
            assert!(!app.conversation_list.detached_pill_visible);
        });

        // Dispatch a real ScrollEvent over the conversation list (previously panicked with
        // RefCell already mutably borrowed inside set_scroll_handler).
        cx.simulate_event(ScrollEvent {
            position: point(px(300.), px(250.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(220.))),
            ..Default::default()
        });
        cx.run_until_parked();

        app.read_with(cx, |app, _| {
            assert!(app.is_detached_from_bottom());
            assert!(
                !app.conversation_list.detached_pill_visible,
                "pill must remain hidden during the 150ms debounce"
            );
        });

        cx.background_executor
            .advance_clock(Duration::from_millis(160));
        cx.run_until_parked();

        app.read_with(cx, |app, _| {
            assert!(app.is_detached_from_bottom());
            assert!(
                app.conversation_list.detached_pill_visible,
                "pill must appear after 150ms debounce"
            );
        });

        // Scroll back to bottom via wheel event and confirm immediate pill dismissal.
        cx.simulate_event(ScrollEvent {
            position: point(px(300.), px(250.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-10_000.))),
            ..Default::default()
        });
        cx.run_until_parked();

        app.read_with(cx, |app, _| {
            assert!(!app.is_detached_from_bottom());
            assert!(!app.conversation_list.detached_pill_visible);
        });
    }

    #[gpui_kit::test]
    fn scroll_by_scrollbar_drag_and_scroll_to_end_sync_detached_pill(cx: &mut TestAppContext) {
        let (app, cx, _locale) = setup_conversation_window(cx, sample_messages(30));

        // 1. Keyboard scroll_by ("pageup") and "end" on focused Area::History.
        let focus = app.read_with(cx, |app, _| {
            app.workspace_focus
                .region(crate::workspace_focus::Area::History)
                .clone()
        });
        cx.update(|window, cx| focus.focus(window, cx));
        cx.run_until_parked();

        cx.simulate_keystrokes("pageup");
        cx.background_executor
            .advance_clock(Duration::from_millis(160));
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.is_detached_from_bottom());
            assert!(app.conversation_list.detached_pill_visible);
        });

        cx.simulate_keystrokes("end");
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(!app.is_detached_from_bottom());
            assert!(!app.conversation_list.detached_pill_visible);
        });

        // 2. Scrollbar::vertical drag (`set_offset_from_scrollbar`) + `scroll_to_end`
        // bypasses `scroll_handler` and syncs via `on_children_prepainted`.
        app.update(cx, |app, cx| {
            app.conversation_list
                .state
                .set_offset_from_scrollbar(point(px(0.), -px(100.)));
            cx.notify();
        });
        cx.run_until_parked();
        cx.background_executor
            .advance_clock(Duration::from_millis(160));
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.is_detached_from_bottom());
            assert!(
                app.conversation_list.detached_pill_visible,
                "scrollbar drag detachment must surface the pill"
            );
        });

        app.update(cx, |app, cx| {
            app.conversation_list.state.scroll_to_end();
            cx.notify();
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(!app.is_detached_from_bottom());
            assert!(
                !app.conversation_list.detached_pill_visible,
                "scroll_to_end must clear the pill"
            );
        });
    }

    #[gpui_kit::test]
    fn conversation_find_match_and_unmatch_remeasures_long_user_message(
        cx: &mut TestAppContext,
    ) {
        let mut messages = sample_messages(30);
        messages[25] = user_msg(
            "u-25",
            &format!("needle-25\n{}", "long body line for collapse test\n".repeat(30)),
            false,
        );
        let (app, cx, _locale) = setup_conversation_window(cx, messages);
        // Prime measurements for all rows (0..30) so offscreen items don't mix unmeasured hints.
        for target in ["u-0", "u-15"] {
            app.update(cx, |app, cx| {
                assert!(app.scroll_to_conversation_message(target, cx));
                cx.notify();
            });
            cx.run_until_parked();
        }
        app.update(cx, |app, cx| app.scroll_to_bottom(cx));
        cx.run_until_parked();

        // Ensure u-25 is measured in collapsed state (when at tail, items 25..30 are visible).
        let base_max_y = app.read_with(cx, |app, _| {
            let row = app
                .conversation_list
                .rows
                .iter()
                .find(|r| matches!(&r.key, RowKey::Message(id) if id == "u-25"))
                .unwrap();
            assert!(!row.expanded);
            f32::from(app.conversation_list.state.max_offset_for_scrollbar().y)
        });

        // Open find and match u-25.
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_conversation_find(window, cx);
                app.set_conversation_find_query_for_test("needle-25", window, cx);
            });
        });
        cx.run_until_parked();

        let matched_max_y = app.read_with(cx, |app, _| {
            assert!(app.conversation_find_matches("u-25"));
            let row = app
                .conversation_list
                .rows
                .iter()
                .find(|r| matches!(&r.key, RowKey::Message(id) if id == "u-25"))
                .unwrap();
            assert!(
                row.expanded,
                "Row.expanded must track conversation_find match"
            );
            f32::from(app.conversation_list.state.max_offset_for_scrollbar().y)
        });
        assert!(
            matched_max_y > base_max_y + 150.0,
            "matched long user message should expand: base={base_max_y}, matched={matched_max_y}"
        );

        // Scroll u-25 out of the viewport (to u-0), then close conversation_find.
        app.update(cx, |app, cx| {
            assert!(app.scroll_to_conversation_message("u-0", cx));
            cx.notify();
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(
                app.conversation_list.state.item_is_below_viewport(25),
                Some(true)
            );
        });

        cx.update(|window, cx| {
            app.update(cx, |app, cx| app.close_conversation_find(window, cx));
        });
        cx.run_until_parked();

        app.read_with(cx, |app, _| {
            assert!(!app.conversation_find_matches("u-25"));
            let row = app
                .conversation_list
                .rows
                .iter()
                .find(|r| matches!(&r.key, RowKey::Message(id) if id == "u-25"))
                .unwrap();
            assert!(
                !row.expanded,
                "Row.expanded must revert when conversation_find closes"
            );
        });

        // Scroll back to bottom so u-25 is re-measured at its exact collapsed height.
        app.update(cx, |app, cx| app.scroll_to_bottom(cx));
        cx.run_until_parked();
        let restored_max_y = app.read_with(cx, |app, _| {
            f32::from(app.conversation_list.state.max_offset_for_scrollbar().y)
        });
        assert!(
            (restored_max_y - base_max_y).abs() < 1.0,
            "max_offset should return to baseline after remeasure: base={base_max_y}, restored={restored_max_y}"
        );
    }
}
