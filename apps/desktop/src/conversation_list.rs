mod view;
pub(crate) const CHAT_COLUMN_WIDTH: f32 = 736.;
use crate::app::HiveApp;
use gpui_kit::{component::v_flex, prelude::FluentBuilder, *};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKey {
    Message(String),
    Group(String),
    Tail,
}
#[derive(Clone, Debug)]
pub struct ReadingAnchor {
    pub key: RowKey,
    pub offset: Pixels,
}
#[derive(Clone, PartialEq, Eq)]
enum Kind {
    Message {
        index: usize,
        indented: bool,
        last: bool,
    },
    Group {
        start: usize,
        end: usize,
        expanded: bool,
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

pub struct ConversationList {
    pub state: ListState,
    rows: Vec<Row>,
    pub pending_anchor: Option<ReadingAnchor>,
    pub tail_focus: FocusHandle,
    hint_width: Rc<Cell<Pixels>>,
    font_size: Pixels,
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
        }
    }
    pub fn anchor(&self) -> Option<ReadingAnchor> {
        let offset = self.state.logical_scroll_top();
        self.rows.get(offset.item_ix).map(|row| ReadingAnchor {
            key: row.key.clone(),
            offset: offset.offset_in_item,
        })
    }
    pub fn reset(&mut self, anchor: Option<ReadingAnchor>) {
        self.rows.clear();
        self.state.reset(0);
        self.state.set_follow_mode(FollowMode::Tail);
        self.pending_anchor = anchor;
    }
}

impl HiveApp {
    pub(crate) fn sync_conversation_list(&mut self, cx: &App) {
        let mut rows = Vec::with_capacity(self.messages.len() + 1);
        let mut offset = 0;
        for group in self
            .messages
            .chunk_by(|a, b| a.completed_tool() && b.completed_tool())
        {
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
                        && message.files.is_empty()
                    {
                        continue;
                    }
                    rows.push(Row {
                        key: RowKey::Message(message.id.clone()),
                        kind: Kind::Message {
                            index: offset + index,
                            indented: grouped,
                            last: index + 1 == group.len(),
                        },
                        revision: message.layout_revision,
                        expanded: message.tool.as_ref().is_some_and(|t| t.expanded)
                            || message.delivery.as_ref().is_some_and(|c| c.expanded),
                    });
                }
            }
            offset += group.len();
        }
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
                    Kind::Message { index, .. } => self.messages[index]
                        .markdown
                        .as_ref()
                        .map(|state| state.read(cx).focus_handle().clone()),
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
                    let message_index = self.messages.iter().position(|m| &m.id == id)?;
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

    pub(crate) fn scroll_to_conversation_message(&mut self, id: &str, cx: &App) {
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
        }
    }

    pub(crate) fn render_conversation_list(
        &self,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content = list(
            self.conversation_list.state.clone(),
            cx.processor(move |this, index: usize, _, cx| {
                let Some(row) = this.conversation_list.rows.get(index).cloned() else {
                    return div().into_any_element();
                };
                let content = match row.kind {
                    Kind::Message { index, .. } => this.render_conversation_message(index, cx),
                    Kind::Group { start, end, .. } => this
                        .render_completed_tools_header(start, &this.messages[start..end], cx)
                        .into_any_element(),
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
                v_flex()
                    .w_full()
                    .px(if narrow { px(16.) } else { px(32.) })
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(CHAT_COLUMN_WIDTH))
                            .mx_auto()
                            .pb(if matches!(row.kind, Kind::Tail) {
                                px(0.)
                            } else if compact {
                                px(2.)
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
        .min_w_0()
        .py_3();
        let hint_width = self.conversation_list.hint_width.clone();
        let state = self.conversation_list.state.clone();
        let weak = cx.weak_entity();
        div()
            .on_children_prepainted(move |bounds, window, _| {
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
            .relative()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(content)
            .child(gpui_kit::component::scroll::Scrollbar::vertical(
                &self.conversation_list.state,
            ))
    }
}
