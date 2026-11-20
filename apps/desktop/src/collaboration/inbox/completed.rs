use super::super::{activity_matches, inbox_open_button, session_inbox_activity};
use super::{ActivityRange, GlobalInbox, RowId, display_project, inbox_status};
use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    language::{localize_owned_error, tr},
    palette::Color,
    protocol::{Backend, Session, SessionHit},
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
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeSet, HashSet};

pub(super) const PAGE_SIZE: usize = 20;
const MAX_PAGE: usize = 100;
pub(super) const INVALID_PAGE: &str = "收件箱返回了其他工作区的数据。";
const CHANGED: &str = "收件箱中的会话已变化，请刷新后重试。";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub(super) connection: u64,
    pub(super) workspace: Option<String>,
    pub(super) member: Option<String>,
    pub(super) backend: Option<Backend>,
    pub(super) activity: Option<ActivityRange>,
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
            && self
                .backend
                .is_none_or(|backend| session.backend == backend)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Page {
    matches: Vec<SessionHit>,
    total: usize,
    archive_snapshot: String,
    #[serde(default)]
    archive_snapshot_changed: bool,
}

impl Page {
    pub(super) fn has_older_activity(&self, cutoff: u64) -> bool {
        self.matches
            .iter()
            .any(|hit| !activity_matches(session_inbox_activity(&hit.session), Some(cutoff)))
    }
}

enum ReadError {
    Changed,
    Failed(String),
}

async fn read_page(
    runtime: &Runtime,
    scope: &Scope,
    offset: usize,
    limit: usize,
    snapshot: Option<&str>,
    activity_since: Option<u64>,
) -> Result<Page, ReadError> {
    let mut params =
        json!({"query":"", "archived":true, "navigationOnly":true, "offset":offset, "limit":limit});
    if let Some(workspace) = &scope.workspace {
        params["workspaceId"] = json!(workspace);
    }
    if let Some(member) = &scope.member {
        params["memberId"] = json!(member);
    }
    if let Some(backend) = scope.backend {
        params["backend"] = json!(backend.wire());
    }
    if let Some(snapshot) = snapshot {
        params["archiveSnapshot"] = json!(snapshot);
    }
    if let Some(since) = activity_since {
        params["activitySinceMs"] = json!(since);
    }
    let value = runtime.search(params).await.map_err(ReadError::Failed)?;
    let mut page: Page =
        serde_json::from_value(value).map_err(|error| ReadError::Failed(error.to_string()))?;
    if page.archive_snapshot_changed || snapshot.is_some_and(|token| token != page.archive_snapshot)
    {
        return Err(ReadError::Changed);
    }
    if page.matches.len() != limit.min(page.total.saturating_sub(offset))
        || page.archive_snapshot.len() != 64
        || !page
            .archive_snapshot
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || page.matches.iter().any(|hit| {
            !hit.session.archived
                || !scope.contains(&hit.session)
                || !activity_matches(session_inbox_activity(&hit.session), activity_since)
        })
    {
        return Err(ReadError::Failed(tr(INVALID_PAGE).to_owned()));
    }
    // This is a metadata-only query. Intentionally do not navigate to a
    // history-message offset even if the response carries one.
    for hit in &mut page.matches {
        hit.body_match = None;
    }
    Ok(page)
}

pub(super) async fn read(
    runtime: &Runtime,
    scope: &Scope,
    offset: usize,
    limit: usize,
    snapshot: Option<&str>,
    activity_since: Option<u64>,
) -> Result<Page, (usize, String)> {
    let result = async {
        let mut prefix = read_page(
            runtime,
            scope,
            offset,
            limit.min(MAX_PAGE),
            snapshot,
            activity_since,
        )
        .await?;
        while prefix.matches.len() < limit && offset + prefix.matches.len() < prefix.total {
            let remaining = limit - prefix.matches.len();
            let continuation = read_page(
                runtime,
                scope,
                offset + prefix.matches.len(),
                remaining.min(MAX_PAGE),
                Some(&prefix.archive_snapshot),
                activity_since,
            )
            .await?;
            if continuation.total != prefix.total {
                return Err(ReadError::Changed);
            }
            prefix.matches.extend(continuation.matches);
        }
        Ok(prefix)
    }
    .await;
    result.map_err(|error| match error {
        ReadError::Changed => (0, tr(CHANGED).to_owned()),
        ReadError::Failed(error) => (offset, error),
    })
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
    pub(super) requested_depth: usize,
    pub(super) snapshot: Option<String>,
    pub(super) activity_since: Option<u64>,
    loading_tail_height: Pixels,
    error: Option<(usize, String)>,
}

impl Completed {
    pub(super) fn change_scope(&mut self, scope: Option<Scope>) {
        if self.scope != scope {
            self.scope = scope;
            self.rows.clear();
            self.requested_depth = 0;
            self.loading_tail_height = px(0.);
            self.activity_since = None;
            self.invalidate();
        }
    }

    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.next = None;
        self.snapshot = None;
        self.error = None;
        self.restore_error = None;
        self.task = None;
        self.refresh = self.scope.is_some();
    }

    pub(super) fn refresh_loaded(&mut self, session: &Session) {
        if !session.archived {
            self.rows.retain(|hit| {
                hit.session.workspace_id != session.workspace_id || hit.session.id != session.id
            });
        }
        self.invalidate();
    }

    pub(super) fn refresh_activity(&mut self) {
        self.invalidate();
    }

    pub(super) fn clock_moved_back(&self, cutoff: u64) -> bool {
        self.activity_since.is_some_and(|since| cutoff < since)
    }

    pub(super) fn activity_needs_refresh(&self, cutoff: u64) -> bool {
        self.clock_moved_back(cutoff)
            || self.activity_since.is_some_and(|since| {
                cutoff > since
                    && self.rows.iter().any(|hit| {
                        let activity = session_inbox_activity(&hit.session);
                        activity >= since && activity < cutoff
                    })
            })
    }

    pub(super) fn reserve_loading_tail(&mut self, row_extent: Pixels, footer_height: Pixels) {
        if self.loading_tail_height > px(0.) {
            self.loading_tail_height += row_extent;
        } else if self.next.is_some() {
            // A following page can fill the gap. Preserve the scroll extent
            // until the authoritative prefix arrives, including the old footer.
            self.loading_tail_height = row_extent + footer_height;
        }
    }

    pub(super) fn request_limit(&mut self, offset: usize) -> usize {
        let limit = if offset == 0 {
            self.requested_depth.max(PAGE_SIZE)
        } else {
            PAGE_SIZE
        };
        self.requested_depth = self.requested_depth.max(offset.saturating_add(limit));
        limit
    }

    pub(super) fn requested_offset(&self) -> Option<usize> {
        self.error.as_ref().map(|(offset, _)| *offset).or(self.next)
    }

    pub(super) fn has(&self, workspace: &str, id: &str) -> Option<&SessionHit> {
        self.rows
            .iter()
            .find(|hit| hit.session.workspace_id == workspace && hit.session.id == id)
    }

    pub(super) fn missing_rows(&self, page: &Page) -> HashSet<RowId> {
        let fresh: HashSet<_> = page
            .matches
            .iter()
            .map(|hit| (&hit.session.workspace_id, &hit.session.id))
            .collect();
        self.rows
            .iter()
            .filter(|hit| !fresh.contains(&(&hit.session.workspace_id, &hit.session.id)))
            .map(|hit| RowId::for_completed(&hit.session.workspace_id, &hit.session.id))
            .collect()
    }

    pub(super) fn apply(
        &mut self,
        generation: u64,
        offset: usize,
        result: Result<Page, (usize, String)>,
    ) -> bool {
        if self.generation != generation {
            return false;
        }
        self.task = None;
        self.error = None;
        self.loading_tail_height = px(0.);
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                self.error = Some(error);
                return true;
            }
        };
        if offset == 0 {
            self.rows.clear();
        }
        let next = offset.saturating_add(page.matches.len());
        self.next = (!page.matches.is_empty() && next < page.total).then_some(next);
        self.snapshot = Some(page.archive_snapshot);
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
    ) -> Vec<(Option<RowId>, AnyElement)> {
        let action = tr(if self.expanded {
            "收起已处理会话"
        } else {
            "展开已处理会话"
        });
        let mut elements = vec![(
            None,
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
        )];
        if !self.expanded {
            return elements;
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
            let (workspace, id) = (session.workspace_id.clone(), session.id.clone());
            let (open_workspace, open_id) = (workspace.clone(), id.clone());
            elements.push((
                Some(RowId::for_completed(&workspace, &id)),
                Reveal::new(
                    SharedString::from(format!("completed:{workspace}:{id}")),
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
                            inbox_open_button(
                                "open-inbox-completed-session",
                                busy,
                                cx.listener(move |this, _, window, cx| {
                                    this.open_completed(&open_workspace, &open_id, window, cx)
                                }),
                            )
                            .flex_1()
                            .accessibility_label(format!("{} · {subject}", tr("打开会话")))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .child(subject.clone()),
                            ),
                        )
                        .child(
                            h_flex().flex_wrap().justify_end().gap_1().child(
                                Button::new("restore-inbox-completed-session")
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .disabled(busy)
                                    .label(tr("恢复会话"))
                                    .accessibility_label(format!("{} · {subject}", tr("恢复会话")))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.restore_completed(&workspace, &id, window, cx)
                                    })),
                            ),
                        ),
                )
                .into_any_element(),
            ));
        }
        if let Some(error) = &self.restore_error {
            elements.push((None, inbox_status(error.clone(), cx)));
        }
        let status = if let Some((_, error)) = &self.error {
            Some(format!(
                "{} · {}",
                tr("无法读取已处理会话"),
                localize_owned_error(error)
            ))
        } else if self.task.is_some() || self.scope.is_none() || self.refresh {
            Some(tr("正在读取已处理会话…").to_owned())
        } else if self.rows.is_empty() {
            Some(tr("暂无已处理会话。").to_owned())
        } else {
            None
        };
        if let Some(status) = status {
            elements.push((
                None,
                div()
                    .min_h(self.loading_tail_height)
                    .child(inbox_status(status, cx))
                    .into_any_element(),
            ));
        }
        if self.task.is_none()
            && let Some(offset) = self.requested_offset()
        {
            elements.push((
                None,
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
            ));
        }
        elements
    }
}

#[cfg(test)]
mod tests {
    use super::{Completed, Page, Scope};
    use crate::protocol::SessionHit;
    use gpui_kit::px;
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

    fn page(matches: Vec<SessionHit>, total: usize) -> Result<Page, (usize, String)> {
        Ok(Page {
            matches,
            total,
            archive_snapshot: "a".repeat(64),
            archive_snapshot_changed: false,
        })
    }

    #[test]
    fn rejects_old_scope_and_deduplicates_repeated_session_identities() {
        let scope = |workspace: &str| Scope {
            connection: 4,
            workspace: Some(workspace.to_owned()),
            member: Some("reviewer".into()),
            backend: None,
            activity: None,
            workspaces: BTreeSet::from([workspace.to_owned()]),
        };
        let mut completed = Completed::default();
        completed.change_scope(Some(scope("atlas")));
        let old = completed.generation;
        let mut beacon_scope = scope("beacon");
        beacon_scope.backend = Some(crate::protocol::Backend::Pi);
        completed.change_scope(Some(beacon_scope.clone()));
        let generation = completed.generation;
        let beacon = hit("beacon", "first", "reviewer");
        let atlas = hit("atlas", "first", "reviewer");
        assert!(beacon_scope.contains(&beacon.session));
        assert!(!beacon_scope.contains(&atlas.session));
        assert!(!beacon_scope.contains(&hit("beacon", "other", "wrong").session));
        let mut wrong_backend = beacon.session.clone();
        wrong_backend.backend = crate::protocol::Backend::PiRpc;
        assert!(!beacon_scope.contains(&wrong_backend));
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

    #[test]
    fn restore_keeps_the_loaded_prefix_until_an_authoritative_replacement() {
        let scope = Scope {
            connection: 4,
            workspace: Some("beacon".into()),
            member: None,
            backend: None,
            activity: None,
            workspaces: BTreeSet::from(["beacon".into()]),
        };
        let rows: Vec<_> = (0..45)
            .map(|i| hit("beacon", &format!("session-{i}"), "reviewer"))
            .collect();
        let mut completed = Completed::default();
        completed.change_scope(Some(scope.clone()));
        let initial = completed.generation;
        assert_eq!(completed.request_limit(0), 20);
        completed.apply(initial, 0, page(rows[..20].to_vec(), 45));
        assert_eq!(completed.requested_offset(), Some(20));
        assert_eq!(completed.request_limit(20), 20);
        completed.apply(initial, 20, page(rows[20..40].to_vec(), 45));

        let mut restored = rows[26].session.clone();
        restored.archived = false;
        completed.reserve_loading_tail(px(40.5), px(26.));
        completed.refresh_loaded(&restored);
        let refresh = completed.generation;
        assert_eq!(completed.loading_tail_height, px(66.5));
        assert!(completed.has("beacon", "session-26").is_none());
        assert!(completed.has("beacon", "session-27").is_some());
        assert_eq!(completed.rows.len(), 39);
        assert_eq!(completed.request_limit(0), 40);
        assert!(!completed.apply(initial, 40, page(rows[40..].to_vec(), 45)));
        assert_eq!(completed.loading_tail_height, px(66.5));
        assert!(completed.apply(refresh, 0, Err((0, "stale".into()))));
        assert_eq!(completed.loading_tail_height, px(0.));
        assert_eq!(completed.rows.len(), 39);
        assert_eq!(completed.requested_offset(), Some(0));

        let authoritative: Vec<_> = rows
            .into_iter()
            .filter(|row| row.session.id != restored.id)
            .collect();
        completed.apply(refresh, 0, page(authoritative[..40].to_vec(), 44));
        assert_eq!(completed.requested_offset(), Some(40));
        assert_eq!(completed.rows.last().unwrap().session.id, "session-40");
        completed.apply(refresh, 40, page(authoritative[40..].to_vec(), 44));
        assert_eq!(completed.rows.len(), 44);

        completed.change_scope(Some(Scope {
            backend: Some(crate::protocol::Backend::Codex),
            ..scope
        }));
        assert!(completed.rows.is_empty());
        assert_eq!(completed.request_limit(0), 20);
        assert!(!completed.apply(refresh, 0, page(authoritative, 44)));
    }
}
