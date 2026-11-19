use super::super::{Fault, Identity};
use serde::Deserialize;
use serde_json::Value;

/// Bind a response/review to the original navigation, even when IDs return to A.
#[derive(Clone, Copy)]
pub(super) struct RequestStamp {
    open_request: u64,
    generation: u64,
}
impl RequestStamp {
    pub fn new(open_request: u64, generation: u64) -> Self {
        Self {
            open_request,
            generation,
        }
    }
    pub fn matches(self, open_request: u64, generation: u64) -> bool {
        self.open_request == open_request && self.generation == generation
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceVersion {
    #[serde(flatten)]
    pub identity: Identity,
    pub index_path: String,
}
impl WorkspaceVersion {
    pub fn description(&self) -> String {
        format!(
            "{}\nindex: {}",
            self.identity.description(),
            self.index_path
        )
    }
}
#[derive(Deserialize)]
pub(super) struct Remote {
    pub name: String,
    pub url: Option<String>,
    pub error: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct RemoteList {
    pub workspace: WorkspaceVersion,
    pub remotes: Vec<Remote>,
}
#[derive(Clone, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Relation {
    New,
    UpToDate,
    FastForward,
    Diverged,
    UnknownHistory,
}
impl Relation {
    pub fn label(&self) -> &'static str {
        use crate::language::tr;
        match self {
            Self::New => tr("远端新分支"),
            Self::UpToDate => tr("已是最新"),
            Self::FastForward => tr("可快进推送"),
            Self::Diverged => tr("分支已分叉"),
            Self::UnknownHistory => tr("缺少远端历史"),
        }
    }
}
#[derive(Clone, Deserialize)]
pub(super) struct Commit {
    pub oid: String,
    pub subject: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PushPreview {
    pub version: String,
    pub workspace: WorkspaceVersion,
    pub remote: String,
    pub remote_url: String,
    pub branch: String,
    pub head_oid: String,
    pub remote_oid: Option<String>,
    pub relation: Relation,
    #[serde(default)]
    pub can_push: bool,
    pub commits: Vec<Commit>,
    pub commits_complete: bool,
    pub warnings: Vec<String>,
}
impl PushPreview {
    pub fn allowed(&self) -> bool {
        self.can_push
            && matches!(
                self.relation,
                Relation::New | Relation::UpToDate | Relation::FastForward
            )
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PushResult {
    pub ok: bool,
    pub pushed: Value,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub head_oid: String,
    pub remote_oid: Option<String>,
    pub error: Option<Fault>,
}
impl PushResult {
    pub fn confirmed(&self, expected_oid: &str) -> bool {
        self.ok
            && self.pushed == true
            && self.verified
            && self.head_oid == expected_oid
            && self.remote_oid.as_deref() == Some(expected_oid)
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Repository {
    pub host: String,
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub url: String,
}
impl Repository {
    pub fn supported(&self) -> bool {
        let safe = |value: &str| {
            !value.is_empty()
                && value != "."
                && value != ".."
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        };
        self.host == "github.com"
            && safe(&self.owner)
            && safe(&self.name)
            && self.full_name == format!("{}/{}", self.owner, self.name)
            && self.url == format!("https://github.com/{}", self.full_name)
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub head_ref_name: String,
    pub base_ref_name: String,
    pub head_ref_oid: String,
    pub is_draft: bool,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrPreview {
    pub version: String,
    pub workspace: WorkspaceVersion,
    pub repository: Repository,
    pub remote: String,
    pub remote_url: String,
    pub head_branch: String,
    pub base: String,
    pub head_oid: String,
    pub remote_oid: Option<String>,
    pub base_oid: String,
    pub pushed: bool,
    pub existing: Vec<PullRequest>,
    pub same_repository_only: bool,
}
impl PrPreview {
    pub fn allowed(&self) -> bool {
        self.pushed
            && self.same_repository_only
            && self.repository.supported()
            && self.existing.len() <= 1
            && self.existing.iter().all(|pr| {
                pr.head_ref_name == self.head_branch
                    && pr.base_ref_name == self.base
                    && pr.head_ref_oid == self.head_oid
                    && verified_pr_url(&pr.url, &self.repository, pr.number).is_some()
            })
    }
}
#[derive(Deserialize)]
pub(super) struct PrResult {
    pub ok: bool,
    pub applied: Value,
    pub operation: String,
    pub url: Option<String>,
    pub number: Option<u64>,
    pub error: Option<Fault>,
}
impl PrResult {
    pub fn confirmed(
        &self,
        repository: &Repository,
        operation: &str,
        expected_number: Option<u64>,
    ) -> bool {
        self.ok
            && self.applied == true
            && self.operation == operation
            && expected_number.is_none_or(|number| self.number == Some(number))
            && self
                .url
                .as_ref()
                .zip(self.number)
                .is_some_and(|(url, number)| verified_pr_url(url, repository, number).is_some())
    }
}
pub(super) fn verified_pr_url(value: &str, repository: &Repository, number: u64) -> Option<String> {
    if !repository.supported() || number == 0 {
        return None;
    }
    let url = url::Url::parse(value).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .path()
            .eq_ignore_ascii_case(&format!("/{}/pull/{number}", repository.full_name))
    {
        return None;
    }
    Some(url.to_string())
}
pub(super) fn envelope<T: serde::de::DeserializeOwned>(
    value: Value,
    field: Option<&str>,
) -> Result<T, Fault> {
    if value["ok"] == true {
        serde_json::from_value(field.map(|field| value[field].clone()).unwrap_or(value)).map_err(
            |error| Fault {
                code: "INVALID_RESPONSE".into(),
                message: error.to_string(),
            },
        )
    } else {
        Err(
            serde_json::from_value(value["error"].clone()).unwrap_or_else(|_| Fault {
                code: "INVALID_RESPONSE".into(),
                message: value.to_string(),
            }),
        )
    }
}
pub(super) fn write_result<T: serde::de::DeserializeOwned>(
    value: Value,
    marker: &str,
) -> Result<T, Fault> {
    if value.get(marker).is_none() {
        return Err(
            serde_json::from_value(value["error"].clone()).unwrap_or_else(|_| Fault {
                code: "INVALID_RESPONSE".into(),
                message: value.to_string(),
            }),
        );
    }
    if value[marker] != true && value[marker] != false && value[marker] != "unknown" {
        return Err(Fault {
            code: "INVALID_RESPONSE".into(),
            message: "Invalid write outcome".into(),
        });
    }
    serde_json::from_value(value).map_err(|error| Fault {
        code: "INVALID_RESPONSE".into(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PrPreview, PrResult, PushPreview, PushResult, RemoteList, Repository, envelope,
        verified_pr_url, write_result,
    };
    use serde_json::json;
    #[test]
    fn navigation_roundtrip_and_same_id_reopen_reject_late_responses_and_reviews() {
        let response = super::RequestStamp::new(10, 3);
        let review = super::RequestStamp::new(10, 4);
        assert!(response.matches(10, 3));
        // Reopening A keeps its workspace/session IDs, but advances open_request.
        assert!(!response.matches(11, 3));
        assert!(!review.matches(11, 4));
        // A -> B -> A advances twice even if no observer sees the intermediate B.
        assert!(!response.matches(12, 3));
        assert!(!review.matches(12, 4));
        // Editing/refresh invalidates a review without a navigation change.
        assert!(!review.matches(10, 5));
    }
    fn workspace() -> serde_json::Value {
        json!({"projectRoot":"/actual/worktree/project","repositoryRoot":"/actual/worktree","projectPrefix":"project/","linkedWorktree":true,"branch":"feature","headOid":"head","detached":false,"unborn":false,"indexPath":"/actual/git/index","indexFingerprint":"index","configFingerprint":"config"})
    }
    fn repository() -> Repository {
        serde_json::from_value(json!({"host":"github.com","owner":"owner","name":"repo","fullName":"owner/repo","url":"https://github.com/owner/repo"})).unwrap()
    }
    #[test]
    fn only_explicit_safe_push_relations_and_can_push_allow_confirmation() {
        for (relation, allowed) in [
            ("new", true),
            ("up-to-date", true),
            ("fast-forward", true),
            ("diverged", false),
            ("unknown-history", false),
        ] {
            let value = json!({"version":"cas","workspace":workspace(),"remote":"origin","remoteUrl":"https://[redacted]@github.com/owner/repo","branch":"explicit-target","headOid":"head","remoteOid":null,"relation":relation,"canPush":true,"commits":[],"commitsComplete":false,"warnings":["No fetch"],"nativeHooks":true,"nativeTrackingRefsMayUpdate":true});
            let preview: PushPreview =
                envelope(json!({"ok":true,"preview":value.clone()}), Some("preview")).unwrap();
            assert_eq!(preview.allowed(), allowed);
            assert!(!preview.commits_complete);
            assert_eq!(
                preview.workspace.identity.project_root,
                "/actual/worktree/project"
            );
            let mut denied = value;
            denied["canPush"] = json!(false);
            let denied: PushPreview = serde_json::from_value(denied).unwrap();
            assert!(!denied.allowed());
        }
        let list:RemoteList=envelope(json!({"ok":true,"workspace":workspace(),"remotes":[{"name":"broken","error":"configuration not supported"}]}),None).unwrap();
        assert!(list.remotes[0].url.is_none());
        assert!(list.remotes[0].error.is_some());
        assert!(
            envelope::<RemoteList>(
                json!({"ok":false,"error":{"code":"NOT_REPOSITORY","message":"not a repo"}}),
                None
            )
            .is_err()
        );
    }
    #[test]
    fn not_attempted_unknown_and_verified_write_outcomes_stay_distinct() {
        let denied:PushResult=write_result(json!({"ok":false,"pushed":false,"applied":false,"error":{"code":"STALE_PREVIEW","message":"changed"}}),"pushed").unwrap();
        assert_eq!(denied.pushed, false);
        assert!(!denied.confirmed("head"));
        let unknown:PushResult=write_result(json!({"ok":false,"pushed":"unknown","verified":true,"headOid":"head","remoteOid":"head","nativeTrackingRefsMayUpdate":true}),"pushed").unwrap();
        assert_eq!(unknown.pushed, "unknown");
        assert!(!unknown.confirmed("head"));
        let confirmed:PushResult=write_result(json!({"ok":true,"pushed":true,"verified":true,"headOid":"head","remoteOid":"head","nativeTrackingRefsMayUpdate":true}),"pushed").unwrap();
        assert!(confirmed.confirmed("head"));
        assert!(!confirmed.confirmed("another-head"));
        assert!(write_result::<PushResult>(json!({"ok":true,"pushed":"maybe"}), "pushed").is_err());
        let blocked:PrResult=write_result(json!({"ok":false,"applied":false,"operation":"create","error":{"code":"PUSH_REQUIRED","message":"push first"}}),"applied").unwrap();
        assert_eq!(blocked.applied, false);
        assert!(!blocked.confirmed(&repository(), "create", None));
    }
    #[test]
    fn pr_confirmation_and_open_url_require_exact_same_repository_identity() {
        let repository = repository();
        let good = "https://github.com/owner/repo/pull/17";
        assert_eq!(
            verified_pr_url(good, &repository, 17).as_deref(),
            Some(good)
        );
        for bad in [
            "file:///project/pr/17",
            "https://github.com.evil/owner/repo/pull/17",
            "https://user@github.com/owner/repo/pull/17",
            "https://github.com/another/repo/pull/17",
            "https://github.com/owner/repo/pull/18",
            "https://github.com/owner/repo/pull/17?token=x",
            "https://github.com/owner/repo/pull/17#body",
        ] {
            assert!(verified_pr_url(bad, &repository, 17).is_none(), "{bad}");
        }
        let result: PrResult = write_result(
            json!({"ok":true,"applied":true,"operation":"update","number":17,"url":good}),
            "applied",
        )
        .unwrap();
        assert!(result.confirmed(&repository, "update", Some(17)));
        assert!(!result.confirmed(&repository, "update", Some(18)));
        assert!(!result.confirmed(&repository, "create", None));
        let mut preview = json!({"version":"pr-cas","workspace":workspace(),"repository":{"host":"github.com","owner":"owner","name":"repo","fullName":"owner/repo","url":"https://github.com/owner/repo"},"remote":"origin","remoteUrl":"git@github.com:owner/repo.git","headBranch":"feature","base":"main","headOid":"head","remoteOid":"head","baseOid":"base","pushed":false,"existing":[],"sameRepositoryOnly":true});
        assert!(
            !serde_json::from_value::<PrPreview>(preview.clone())
                .unwrap()
                .allowed()
        );
        preview["pushed"] = json!(true);
        assert!(
            serde_json::from_value::<PrPreview>(preview.clone())
                .unwrap()
                .allowed()
        );
        preview["sameRepositoryOnly"] = json!(false);
        assert!(
            !serde_json::from_value::<PrPreview>(preview)
                .unwrap()
                .allowed()
        );
    }
}
