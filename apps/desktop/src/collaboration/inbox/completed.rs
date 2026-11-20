use super::{GlobalInbox, display_project, inbox_status};
use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::{Session, SessionHit},
    sessions::search::Page,
    transport::Runtime,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    *,
};
use serde_json::json;
use std::collections::BTreeSet;

pub(super) const PAGE_SIZE: usize = 20;
pub(super) const INVALID_PAGE: &str = "收件箱返回了其他工作区的数据。";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub(super) connection: u64,
    pub(super) workspace: Option<String>,
    pub(super) member: Option<String>,
    pub(super) workspaces: BTreeSet<String>,
}

impl Scope {
    pub(super) fn contains(&self, session: &Session) -> bool {
        self.workspaces.contains(&session.workspace_id)
            && self
                .workspace
                .as_ref()
                .is_none_or(|id| id == &session.workspace_id)
            && self
                .member
                .as_deref()
                .is_none_or(|id| session.display_agent_id() == Some(id))
    }
}

pub(super) async fn read_page(
    runtime: &Runtime,
    scope: &Scope,
    offset: usize,
) -> Result<Page, String> {
    let mut params = json!({"query":"", "archived":true, "navigationOnly":true, "offset":offset, "limit":PAGE_SIZE});
    if let Some(workspace) = &scope.workspace {
        params["workspaceId"] = json!(workspace);
    }
    if let Some(member) = &scope.member {
        params["memberId"] = json!(member);
    }
    let value = runtime.search(params).await?;
    let mut page: Page = serde_json::from_value(value).map_err(|error| error.to_string())?;
    if page.matches.len() > PAGE_SIZE
        || page
            .matches
            .iter()
            .any(|hit| !hit.session.archived || !scope.contains(&hit.session))
    {
        return Err(tr(INVALID_PAGE).to_owned());
    }
    // This is a metadata-only query. Intentionally do not navigate to a
    // history-message offset even if the response carries one.
    for hit in &mut page.matches {
        hit.body_match = None;
    }
    Ok(page)
}

#[derive(Default)]
pub(super) struct Completed {
    pub(super) expanded: bool,
    pub(super) scope: Option<Scope>,
    pub(super) generation: u64,
    pub(super) rows: Vec<SessionHit>,
    pub(super) next: Option<usize>,
    pub(super) refresh: bool,
    pub(super) task: Option<Task<()>>,
    pub(super) restore_error: Option<String>,
    error: Option<(usize, String)>,
}

impl Completed {
    pub(super) fn change_scope(&mut self, scope: Option<Scope>) {
        if self.scope != scope {
            self.scope = scope;
            self.rows.clear();
            self.invalidate();
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.next = None;
        self.error = None;
        self.restore_error = None;
        self.task = None;
        self.refresh = self.scope.is_some();
    }

    pub(super) fn requested_offset(&self) -> Option<usize> {
        self.error.as_ref().map(|(offset, _)| *offset).or(self.next)
    }

    pub(super) fn has(&self, workspace: &str, id: &str) -> Option<&SessionHit> {
        self.rows
            .iter()
            .find(|hit| hit.session.workspace_id == workspace && hit.session.id == id)
    }

    pub(super) fn apply(
        &mut self,
        generation: u64,
        offset: usize,
        result: Result<Page, String>,
    ) -> bool {
        if self.generation != generation {
            return false;
        }
        self.task = None;
        self.error = None;
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                self.error = Some((offset, error));
                return true;
            }
        };
        if offset == 0 {
            self.rows.clear();
        }
        let next = offset.saturating_add(page.matches.len());
        self.next = (!page.matches.is_empty() && next < page.total).then_some(next);
        let mut existing: BTreeSet<_> = self
            .rows
            .iter()
            .map(|hit| (hit.session.workspace_id.clone(), hit.session.id.clone()))
            .collect();
        self.rows.extend(page.matches.into_iter().filter(|hit| {
            existing.insert((hit.session.workspace_id.clone(), hit.session.id.clone()))
        }));
        true
    }

    pub(super) fn elements(
        &self,
        app: &HiveApp,
        scroll: &ScrollHandle,
        cx: &Context<GlobalInbox>,
    ) -> Vec<AnyElement> {
        let action = tr(if self.expanded {
            "收起已处理会话"
        } else {
            "展开已处理会话"
        });
        let mut elements = vec![
            Reveal::new(
                "global-inbox-completed",
                scroll,
                Button::new("toggle-inbox-completed")
                    .ghost()
                    .compact()
                    .xsmall()
                    .icon(if self.expanded {
                        Glyph::ChevronDown
                    } else {
                        Glyph::ChevronRight
                    })
                    .label(tr("已处理"))
                    .accessibility_label(action)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_completed(cx))),
            )
            .into_any_element(),
        ];
        if !self.expanded {
            return elements;
        }
        if let Some(error) = &self.restore_error {
            elements.push(inbox_status(error.clone(), cx));
        }
        if let Some((_, error)) = &self.error {
            elements.push(inbox_status(
                format!(
                    "{} · {}",
                    tr("无法读取已处理会话"),
                    localize_owned_error(error)
                ),
                cx,
            ));
        } else if self.task.is_some() || self.scope.is_none() {
            elements.push(inbox_status(tr("正在读取已处理会话…"), cx));
        } else if self.rows.is_empty() {
            elements.push(inbox_status(tr("暂无已处理会话。"), cx));
        }
        for hit in &self.rows {
            let session = &hit.session;
            let Some(project) = app
                .workspaces
                .iter()
                .find(|project| project.id == session.workspace_id)
            else {
                continue;
            };
            if app.workspace_removing.as_deref() == Some(&project.id) {
                continue;
            }
            let title = if hit.title.trim().is_empty() {
                tr("未命名会话")
            } else {
                hit.title.as_str()
            };
            let subject = format!("{} · {title}", display_project(project, &app.workspaces));
            let busy = !app.connected
                || app.session_archive_loading.contains(&session.id)
                || app.session_pin_loading.contains(&session.id);
            let row_id =
                SharedString::from(format!("completed:{}:{}", session.workspace_id, session.id));
            let (workspace, id) = (session.workspace_id.clone(), session.id.clone());
            let (open_workspace, open_id) = (workspace.clone(), id.clone());
            elements.push(
                Reveal::new(
                    row_id,
                    scroll,
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .min_w_0()
                        .px_2()
                        .py_1p5()
                        .rounded_md()
                        .bg(Color::Inset.get(cx))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_xs()
                                .truncate()
                                .child(subject.clone()),
                        )
                        .child(
                            h_flex()
                                .flex_wrap()
                                .justify_end()
                                .gap_1()
                                .child(
                                    Button::new("restore-inbox-completed-session")
                                        .ghost()
                                        .compact()
                                        .xsmall()
                                        .disabled(busy)
                                        .label(tr("恢复会话"))
                                        .accessibility_label(format!(
                                            "{} · {subject}",
                                            tr("恢复会话")
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.restore_completed(&workspace, &id, window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("open-inbox-completed-session")
                                        .ghost()
                                        .compact()
                                        .xsmall()
                                        .disabled(busy)
                                        .label(tr("打开会话"))
                                        .accessibility_label(format!(
                                            "{} · {subject}",
                                            tr("打开会话")
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_completed(
                                                &open_workspace,
                                                &open_id,
                                                window,
                                                cx,
                                            )
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
            );
        }
        if self.task.is_none()
            && let Some(offset) = self.requested_offset()
        {
            elements.push(
                Reveal::new(
                    "inbox-completed-more-reveal",
                    scroll,
                    Button::new("inbox-completed-more")
                        .outline()
                        .compact()
                        .xsmall()
                        .label(tr(if self.error.is_some() {
                            "重试"
                        } else {
                            "加载更多"
                        }))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.load_completed(offset, cx)),
                        ),
                )
                .into_any_element(),
            );
        }
        elements
    }
}

#[cfg(test)]
mod tests {
    use super::{Completed, Scope};
    use crate::{protocol::SessionHit, sessions::search::Page};
    use serde_json::json;
    use std::collections::BTreeSet;

    fn hit(workspace: &str, id: &str, member: &str) -> SessionHit {
        serde_json::from_value(json!({
            "session":{"id":id,"workspaceId":workspace,"title":"Same title","archived":true,
                "originAgentId":member,"identityState":"known"},
            "title":"Same title","workspaceName":"Same project",
        }))
        .unwrap()
    }

    #[test]
    fn rejects_old_scope_and_deduplicates_repeated_session_identities() {
        let scope = |workspace: &str| Scope {
            connection: 4,
            workspace: Some(workspace.to_owned()),
            member: Some("reviewer".into()),
            workspaces: BTreeSet::from([workspace.to_owned()]),
        };
        let mut completed = Completed::default();
        completed.change_scope(Some(scope("atlas")));
        let old = completed.generation;
        completed.change_scope(Some(scope("beacon")));
        let generation = completed.generation;
        let beacon = hit("beacon", "first", "reviewer");
        let atlas = hit("atlas", "first", "reviewer");
        assert!(scope("beacon").contains(&beacon.session));
        assert!(!scope("beacon").contains(&atlas.session));
        assert!(!scope("beacon").contains(&hit("beacon", "other", "wrong").session));
        let page = |matches, total| {
            Ok(Page {
                matches,
                total,
                ..Page::default()
            })
        };
        assert!(!completed.apply(old, 0, page(vec![atlas], 1)));
        assert!(completed.apply(generation, 0, page(vec![beacon.clone()], 3)));
        assert_eq!(completed.next, Some(1));
        let next = hit("beacon", "next", "reviewer");
        assert!(completed.apply(generation, 1, page(vec![beacon, next], 3)));
        assert_eq!(completed.rows.len(), 2);
        assert!(completed.has("atlas", "first").is_none());
        assert!(completed.has("beacon", "first").is_some());
        assert_eq!(completed.next, None);
        completed.invalidate();
        assert!(!completed.apply(generation, 0, page(Vec::new(), 0)));
        assert_eq!(completed.rows.len(), 2);
    }
}
