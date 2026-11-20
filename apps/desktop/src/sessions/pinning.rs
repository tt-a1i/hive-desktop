use super::*;
use crate::{
    language::{english, tr},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Icon, WindowExt as _,
        menu::{PopupMenu, PopupMenuItem},
    },
    prelude::FluentBuilder,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(crate) struct PinnedSessions {
    pub records: Vec<SessionHit>,
    pub collapsed: bool,
    pub loading: bool,
    pub dirty: bool,
    pub error: Option<String>,
    pub reordering: bool,
    manually_ordered: bool,
    generation: u64,
}

#[derive(serde::Deserialize)]
struct PinResult {
    session: Session,
    sessions: Vec<Session>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PinIdentity {
    workspace_id: String,
    session_id: String,
}

impl From<&Session> for PinIdentity {
    fn from(session: &Session) -> Self {
        Self {
            workspace_id: session.workspace_id.clone(),
            session_id: session.id.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum PinPlacement {
    Before,
    After,
}

#[derive(Clone)]
struct PinDrag {
    source: PinIdentity,
    order: Arc<[String]>,
    connection: u64,
    title: String,
}

impl Render for PinDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(224.))
            .truncate()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Surface.get(cx))
            .text_color(Color::Text.get(cx))
            .text_xs()
            .child(self.title.clone())
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinMoveResult {
    ordered_ids: Vec<String>,
}

fn move_order(
    order: &[String],
    source: &str,
    target: &str,
    placement: PinPlacement,
) -> Option<Vec<String>> {
    if source == target {
        return None;
    }
    let mut moved = order.to_vec();
    let from = moved.iter().position(|id| id == source)?;
    let item = moved.remove(from);
    let target = moved.iter().position(|id| id == target)?;
    moved.insert(target + usize::from(placement == PinPlacement::After), item);
    (moved != order).then_some(moved)
}

impl HiveApp {
    pub(crate) fn pin_order_snapshot(&self) -> Arc<[String]> {
        self.pinned_sessions
            .records
            .iter()
            .map(|entry| entry.session.id.clone())
            .collect()
    }

    fn can_move_pins(&self) -> bool {
        self.connected
            && !self.closing
            && !self.pinned_sessions.reordering
            && self.pinned_sessions.records.len() > 1
    }

    fn has_reorderable_pin(&self, pin: &PinIdentity) -> bool {
        self.workspace_removing.as_ref() != Some(&pin.workspace_id)
            && !self.removed_sessions.contains(&pin.session_id)
            && !self.session_pin_loading.contains(&pin.session_id)
            && !self.session_archive_loading.contains(&pin.session_id)
            && self.pinned_sessions.records.iter().any(|entry| {
                let session = &entry.session;
                session.id == pin.session_id
                    && session.workspace_id == pin.workspace_id
                    && session.pinned
                    && !session.archived
            })
    }

    pub(crate) fn pin_move_menu(
        &self,
        mut menu: PopupMenu,
        session: &Session,
        app: WeakEntity<Self>,
    ) -> PopupMenu {
        let source = PinIdentity::from(session);
        if !self.can_move_pins() || !self.has_reorderable_pin(&source) {
            return menu;
        }
        let records = &self.pinned_sessions.records;
        let Some(index) = records
            .iter()
            .position(|entry| entry.session.id == source.session_id)
        else {
            return menu;
        };
        let origin = PinDrag {
            source,
            order: self.pin_order_snapshot(),
            connection: self.connection_generation,
            title: session.title.clone(),
        };
        for (neighbor, placement, label) in [
            (index.checked_sub(1), PinPlacement::Before, "上移置顶会话"),
            (Some(index + 1), PinPlacement::After, "下移置顶会话"),
        ] {
            let Some(neighbor) = neighbor.and_then(|index| records.get(index)) else {
                continue;
            };
            let target = PinIdentity::from(&neighbor.session);
            let app = app.clone();
            let origin = origin.clone();
            menu = menu.item(
                PopupMenuItem::new(tr(label))
                    .disabled(!self.has_reorderable_pin(&target))
                    .on_click(move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| {
                            this.move_pinned_session(&origin, &target, placement, cx)
                        });
                    }),
            );
        }
        menu
    }

    pub(crate) fn pin_drag_handle(
        &self,
        session: &Session,
        order: Arc<[String]>,
        title: &str,
        workspace: &str,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if order.len() < 2 {
            return None;
        }
        let source = PinIdentity::from(session);
        let enabled = self.can_move_pins() && self.has_reorderable_pin(&source);
        let drag = PinDrag {
            source,
            order,
            connection: self.connection_generation,
            title: title.to_owned(),
        };
        let instruction = tr("拖动以调整置顶顺序");
        Some(
            div()
                .id(SharedString::from(format!("pin-drag-{}", session.id)))
                .flex()
                .items_center()
                .justify_center()
                .w(px(18.))
                .h(px(30.))
                .flex_shrink_0()
                .role(Role::Group)
                .aria_label(format!("{instruction} · {title} · {workspace}"))
                .opacity(if enabled { 1. } else { 0.45 })
                .child(
                    Icon::new(Glyph::GripVertical)
                        .size(px(13.))
                        .text_color(Color::Muted.get(cx)),
                )
                .when(enabled, |handle| {
                    handle
                        .cursor_grab()
                        .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                })
                .into_any_element(),
        )
    }

    pub(crate) fn pin_drop_zones(&self, session: &Session, cx: &Context<Self>) -> Vec<AnyElement> {
        let target = PinIdentity::from(session);
        if !cx.has_active_drag() || !self.can_move_pins() || !self.has_reorderable_pin(&target) {
            return Vec::new();
        }
        [PinPlacement::Before, PinPlacement::After]
            .into_iter()
            .map(|placement| {
                let target = target.clone();
                let hover = target.clone();
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .h(px(15.))
                    .when(placement == PinPlacement::Before, |zone| zone.top_0())
                    .when(placement == PinPlacement::After, |zone| zone.bottom_0())
                    .drag_over::<PinDrag>(move |style, drag, _, cx| {
                        if drag.source == hover {
                            return style;
                        }
                        let style = style.border_color(Color::Accent.get(cx));
                        match placement {
                            PinPlacement::Before => style.border_t_2(),
                            PinPlacement::After => style.border_b_2(),
                        }
                    })
                    .on_drop(cx.listener(move |this, drag: &PinDrag, _, cx| {
                        this.move_pinned_session(drag, &target, placement, cx)
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn move_pinned_session(
        &mut self,
        drag: &PinDrag,
        target: &PinIdentity,
        placement: PinPlacement,
        cx: &mut Context<Self>,
    ) {
        if !self.can_move_pins()
            || self.connection_generation != drag.connection
            || !self.has_reorderable_pin(&drag.source)
            || !self.has_reorderable_pin(target)
        {
            return;
        }
        if self.pin_order_snapshot() != drag.order {
            self.error = Some("置顶顺序已变化，请重试。".into());
            self.refresh_pinned_sessions(cx);
            return;
        }
        let Some(wanted) = move_order(
            &drag.order,
            &drag.source.session_id,
            &target.session_id,
            placement,
        ) else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = drag.connection;
        let prior = drag.order.clone();
        let params = json!({"source":drag.source,"target":target,"position":placement,"expectedOrder":prior.as_ref()});
        self.pinned_sessions.reordering = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.pins.move", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<PinMoveResult>(value)
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.pinned_sessions.reordering = false;
                match result {
                    Ok(result) if result.ordered_ids == wanted => {
                        this.pinned_sessions.manually_ordered = true;
                        if this.pin_order_snapshot() == prior {
                            let positions: HashMap<_, _> = wanted
                                .iter()
                                .enumerate()
                                .map(|(index, id)| (id.as_str(), index))
                                .collect();
                            this.pinned_sessions
                                .records
                                .sort_by_key(|entry| positions[entry.session.id.as_str()]);
                        }
                    }
                    Ok(_) => this.error = Some("返回的置顶顺序与请求不一致。".into()),
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "保存置顶顺序失败",
                            error.to_string(),
                        ))
                    }
                }
                this.refresh_pinned_sessions(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_session_pinned(
        &mut self,
        session: Session,
        pinned: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if pinned || !crate::general::current(cx).confirm_unpin_session {
            self.set_session_pinned(session, pinned, cx);
            return;
        }
        if !self.connected
            || self.closing
            || self.session_pin_loading.contains(&session.id)
            || self.session_archive_loading.contains(&session.id)
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
            || window.has_active_dialog(cx)
        {
            return;
        }
        let connection = self.connection_generation;
        let weak = cx.weak_entity();
        let title = session.title.clone();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let confirm_session = session.clone();
            dialog
                .title(tr("取消置顶？"))
                .description(if english() {
                    format!(
                        "Unpin \"{title}\"? It will return to its normal chronological position."
                    )
                } else {
                    format!("确定取消置顶“{title}”吗？该会话将回到工作区的常规时间位置。")
                })
                .show_cancel(true)
                .ok_text(tr("取消置顶"))
                .cancel_text(tr("取消"))
                .on_ok(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if this.connection_generation == connection {
                            this.set_session_pinned(confirm_session.clone(), false, cx);
                        }
                    })
                    .is_ok()
                })
        });
    }

    pub(crate) fn set_session_pinned(
        &mut self,
        session: Session,
        pinned: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.connected
            || self.closing
            || self.session_pin_loading.contains(&session.id)
            || self.session_archive_loading.contains(&session.id)
            || self.workspace_removing.as_ref() == Some(&session.workspace_id)
        {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let connection = self.connection_generation;
        let id = session.id;
        let project = session.workspace_id;
        self.session_pin_loading.insert(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.pin", json!({"sessionId":id,"pinned":pinned}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<PinResult>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.session_pin_loading.remove(&id);
                match result {
                    Ok(result) => {
                        if this
                            .workspaces
                            .iter()
                            .any(|workspace| workspace.id == project)
                        {
                            this.refresh_pinned_sessions(cx);
                            if this.workspace.as_ref() == Some(&project) {
                                this.sessions = result.sessions;
                            } else {
                                this.project_sessions.insert(project, result.sessions);
                            }
                            if let Some(selected) =
                                this.session.as_mut().filter(|selected| selected.id == id)
                            {
                                selected.pinned = result.session.pinned;
                            }
                        }
                    }
                    Err(error) => {
                        this.error = Some(crate::app::ErrorNotice::Detail(
                            "保存会话置顶失败",
                            error.to_string(),
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl HiveApp {
    pub(crate) fn refresh_pinned_sessions(&mut self, cx: &mut Context<Self>) {
        if !self.connected || self.closing {
            return;
        }
        self.pinned_sessions.generation += 1;
        self.pinned_sessions.dirty = true;
        if self.pinned_sessions.loading {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        self.pinned_sessions.loading = true;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(generation)) = this.update(cx, |this, _| {
                    if this.connection_generation != connection {
                        return None;
                    }
                    this.pinned_sessions.dirty = false;
                    Some(this.pinned_sessions.generation)
                }) else {
                    return;
                };
                let result = runtime
                    .request("chat.pinned", json!({}))
                    .await
                    .and_then(|value| {
                        serde_json::from_value::<Vec<SessionHit>>(value)
                            .map_err(|error| error.to_string())
                    });
                let Ok(again) = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return false;
                    }
                    if generation == this.pinned_sessions.generation && !this.pinned_sessions.dirty
                    {
                        match result {
                            Ok(records) => {
                                this.pinned_sessions.manually_ordered =
                                    records.iter().any(|entry| entry.session.pin_rank.is_some());
                                this.pinned_sessions.records = records;
                                this.pinned_sessions.error = None;
                            }
                            Err(error) => this.pinned_sessions.error = Some(error),
                        }
                    }
                    let again = this.pinned_sessions.dirty;
                    if !again {
                        this.pinned_sessions.loading = false;
                    }
                    cx.notify();
                    again
                }) else {
                    return;
                };
                if !again {
                    return;
                }
            }
        })
        .detach();
    }
}

impl HiveApp {
    pub(crate) fn sync_pinned_project(&mut self) {
        // Conversation events already refresh the active project's list. Merge that
        // snapshot instead of rereading every pinned project's team on each burst.
        for hit in &mut self.pinned_sessions.records {
            if let Some(session) = self
                .sessions
                .iter()
                .find(|session| session.id == hit.session.id)
            {
                hit.session.title = session.title.clone();
                hit.session.custom_title = session.custom_title;
                hit.session.agent_id = session.agent_id.clone();
                hit.session.updated_at = session.updated_at;
                hit.session.archived = session.archived;
                hit.title = if !session.custom_title {
                    session
                        .agent_id
                        .as_ref()
                        .and_then(|id| self.members.iter().find(|member| &member.id == id))
                        .map(|member| member.name.clone())
                        .unwrap_or_else(|| session.title.clone())
                } else {
                    session.title.clone()
                };
            }
        }
        self.pinned_sessions
            .records
            .retain(|hit| !hit.session.archived);
        if !self.pinned_sessions.manually_ordered {
            self.pinned_sessions.records.sort_by(|a, b| {
                b.session
                    .updated_at
                    .cmp(&a.session.updated_at)
                    .then_with(|| a.session.id.cmp(&b.session.id))
            });
        }
    }
}
