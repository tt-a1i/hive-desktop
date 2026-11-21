use crate::{
    app::{ErrorNotice, HiveApp},
    language::tr,
    palette::Color,
    protocol::{Backend, Model, ModelIdentity},
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Icon,
        button::{Button, ButtonVariants},
        input::{Escape, Input, MoveDown, MoveUp},
        popover::Popover,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs, io,
    ops::Range,
    path::{Path, PathBuf},
};

const FAVORITES_FILE: &str = "desktop-model-favorites.json";

#[derive(Default)]
pub(crate) enum ProjectModelDefaultState {
    #[default]
    Idle,
    Loading,
    Ready(Option<ModelIdentity>),
    Failed(String),
}

#[derive(Deserialize)]
struct ProjectModelDefaultResponse {
    model: Option<ModelIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct ModelFavoriteKey {
    pub provider: String,
    pub id: String,
}

#[derive(Serialize, Deserialize)]
struct StoredModelFavorites {
    version: u32,
    favorites: Vec<ModelFavoriteKey>,
}

pub(crate) struct ModelFavoritesStore {
    items: BTreeSet<ModelFavoriteKey>,
    root: Option<PathBuf>,
    preview_only: bool,
}

impl Default for ModelFavoritesStore {
    fn default() -> Self {
        Self {
            items: BTreeSet::new(),
            root: None,
            preview_only: cfg!(feature = "ui-preview")
                && std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN")
                    .is_ok_and(|screen| !screen.trim().is_empty()),
        }
    }
}

impl ModelFavoritesStore {
    #[cfg(test)]
    pub(crate) fn for_test(root: Option<PathBuf>, preview_only: bool) -> Self {
        Self {
            items: BTreeSet::new(),
            root,
            preview_only,
        }
    }

    pub(crate) fn restore(&mut self, root: &Path) {
        if self.root.is_some() {
            return;
        }
        self.root = Some(root.to_path_buf());
        let path = root.join(FAVORITES_FILE);
        let Ok(file) = fs::File::open(&path) else {
            return;
        };
        if let Ok(stored) = serde_json::from_reader::<_, StoredModelFavorites>(file)
            && stored.version == 1
        {
            self.items = stored.favorites.into_iter().collect();
        }
    }

    pub(crate) fn contains(&self, provider: &str, id: &str) -> bool {
        self.items
            .iter()
            .any(|item| item.provider == provider && item.id == id)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub(crate) fn toggle(&mut self, provider: &str, id: &str) -> io::Result<()> {
        let key = ModelFavoriteKey {
            provider: provider.to_owned(),
            id: id.to_owned(),
        };
        let mut next = self.items.clone();
        if !next.remove(&key) {
            next.insert(key);
        }
        if !self.preview_only {
            let root = self
                .root
                .as_ref()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, tr("后台尚未连接")))?;
            save_favorites(root, &next)?;
        }
        self.items = next;
        Ok(())
    }
}

fn save_favorites(root: &Path, items: &BTreeSet<ModelFavoriteKey>) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    serde_json::to_writer(
        file.as_file_mut(),
        &StoredModelFavorites {
            version: 1,
            favorites: items.iter().cloned().collect(),
        },
    )?;
    file.as_file().sync_all()?;
    file.persist(root.join(FAVORITES_FILE))?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum ModelPickerTab {
    Favorites,
    #[default]
    All,
    Provider(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ModelPickerFocusPane {
    ProviderRail,
    #[default]
    ModelList,
    RightColumn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RightColumnItem {
    Reasoning(String),
    Mode { permission: bool, id: String },
}

#[derive(Default)]
pub(crate) struct ModelSearchNavigation {
    active: Option<(String, String)>,
    enter: Option<(String, String)>,
}

impl ModelSearchNavigation {
    pub(crate) fn cancel_enter(&mut self) {
        self.enter = None;
    }

    fn active_index(&self, models: &[&Model]) -> usize {
        self.active
            .as_ref()
            .and_then(|(provider, id)| {
                models
                    .iter()
                    .position(|model| model.provider == *provider && model.id == *id)
            })
            .unwrap_or(0)
    }
}

pub(crate) struct ModelPickerState {
    pub tab: ModelPickerTab,
    pub focus_pane: ModelPickerFocusPane,
    pub rail_focus: FocusHandle,
    pub right_focus: FocusHandle,
    pub right_index: usize,
    pub had_right_column: bool,
    pub focus_search_on_render: bool,
    pub model_scroll: ScrollHandle,
    pub right_scroll: ScrollHandle,
    pub navigation: ModelSearchNavigation,
    pub favorites: ModelFavoritesStore,
}

impl ModelPickerState {
    pub(crate) fn new(cx: &mut Context<HiveApp>) -> Self {
        Self {
            tab: ModelPickerTab::All,
            focus_pane: ModelPickerFocusPane::ModelList,
            rail_focus: cx.focus_handle(),
            right_focus: cx.focus_handle(),
            right_index: 0,
            had_right_column: false,
            focus_search_on_render: false,
            model_scroll: ScrollHandle::new(),
            right_scroll: ScrollHandle::new(),
            navigation: ModelSearchNavigation::default(),
            favorites: ModelFavoritesStore::default(),
        }
    }

    pub(crate) fn reset_ephemeral(&mut self) {
        self.tab = ModelPickerTab::All;
        self.focus_pane = ModelPickerFocusPane::ModelList;
        self.right_index = 0;
        self.had_right_column = false;
        self.focus_search_on_render = false;
        self.navigation = ModelSearchNavigation::default();
        self.model_scroll.set_offset(point(px(0.), px(0.)));
        self.right_scroll.set_offset(point(px(0.), px(0.)));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NarrowModelPickerLayout {
    pub body_max_h: f32,
    pub list_max_h: f32,
    pub right_max_h: f32,
}

pub(crate) fn narrow_model_picker_layout(
    viewport_height: f32,
    has_right_column: bool,
    show_footer: bool,
) -> NarrowModelPickerLayout {
    const SEARCH_HEADER_H: f32 = 36.0;
    const RIGHT_BORDER_H: f32 = 1.0;
    const POPOVER_BORDER_H: f32 = 2.0;
    const FOOTER_H: f32 = 36.0;
    const VIEWPORT_MARGIN_H: f32 = 24.0;

    let max_popover_h = (viewport_height - VIEWPORT_MARGIN_H).max(160.0);
    let overhead_h = POPOVER_BORDER_H + if show_footer { FOOTER_H } else { 0.0 };
    let body_max_h = (max_popover_h - overhead_h).max(120.0);
    if has_right_column {
        let panes_budget = (body_max_h - SEARCH_HEADER_H - RIGHT_BORDER_H).max(120.0);
        let right_max_h = (panes_budget * 0.45).floor().min(190.0).max(80.0);
        let list_max_h = (panes_budget - right_max_h).floor().min(220.0).max(60.0);
        NarrowModelPickerLayout {
            body_max_h,
            list_max_h,
            right_max_h,
        }
    } else {
        let list_max_h = (body_max_h - SEARCH_HEADER_H).floor().min(220.0).max(80.0);
        NarrowModelPickerLayout {
            body_max_h,
            list_max_h,
            right_max_h: 0.0,
        }
    }
}

pub(crate) fn resolve_right_column_after_load(
    still_loading: bool,
    has_confirmed_right: bool,
    had_right_column: bool,
    focus_pane: ModelPickerFocusPane,
) -> (bool, ModelPickerFocusPane, bool) {
    if still_loading {
        return (had_right_column, focus_pane, false);
    }
    if !has_confirmed_right && focus_pane == ModelPickerFocusPane::RightColumn {
        (false, ModelPickerFocusPane::ModelList, true)
    } else {
        (has_confirmed_right, focus_pane, false)
    }
}

fn model_key(model: &Model) -> (String, String) {
    (model.provider.clone(), model.id.clone())
}

pub(crate) fn available_providers(models: &[Model]) -> Vec<String> {
    let mut providers = Vec::new();
    for model in models {
        if model.available && !providers.iter().any(|p| p == &model.provider) {
            providers.push(model.provider.clone());
        }
    }
    providers
}

pub(crate) fn should_show_provider_rail(providers: &[String], has_favorites: bool) -> bool {
    providers.len() >= 2 || has_favorites
}

fn rail_tabs(providers: &[String]) -> Vec<ModelPickerTab> {
    let mut tabs = Vec::with_capacity(providers.len() + 2);
    tabs.push(ModelPickerTab::Favorites);
    tabs.push(ModelPickerTab::All);
    for provider in providers {
        tabs.push(ModelPickerTab::Provider(provider.clone()));
    }
    tabs
}

fn rank_matching_models<'a>(
    models: &'a [Model],
    query: &str,
    tab: &ModelPickerTab,
    current: Option<&ModelIdentity>,
    favorites: &ModelFavoritesStore,
) -> Vec<&'a Model> {
    let query = query.trim().to_lowercase();
    let mut matching: Vec<(usize, &'a Model)> = models
        .iter()
        .enumerate()
        .filter(|(_, model)| {
            if !model.available {
                return false;
            }
            if query.is_empty() {
                match tab {
                    ModelPickerTab::Favorites => favorites.contains(&model.provider, &model.id),
                    ModelPickerTab::All => true,
                    ModelPickerTab::Provider(provider) => &model.provider == provider,
                }
            } else {
                format!("{} {} {}", model.provider, model.id, model.name)
                    .to_lowercase()
                    .contains(&query)
            }
        })
        .collect();

    matching.sort_by_key(|(idx, model)| {
        let id_lower = model.id.to_lowercase();
        let name_lower = model.name.to_lowercase();
        let exact = !query.is_empty() && (id_lower == query || name_lower == query);
        let prefix =
            !query.is_empty() && (id_lower.starts_with(&query) || name_lower.starts_with(&query));
        let fav = favorites.contains(&model.provider, &model.id);
        let selected = current.is_some_and(|selected| {
            selected.provider == model.provider && selected.id == model.id
        });
        (!exact, !prefix, !fav, !selected, *idx)
    });

    matching.into_iter().map(|(_, model)| model).collect()
}

pub(crate) fn should_switch_pane_from_search(
    key: &str,
    modifiers: Modifiers,
    composing: bool,
    value: &str,
    selection: Range<usize>,
    show_rail: bool,
    has_right: bool,
) -> Option<ModelPickerFocusPane> {
    if composing || modifiers.modified() {
        return None;
    }
    match key {
        "left" if show_rail && value.is_empty() && selection == (0..0) => {
            Some(ModelPickerFocusPane::ProviderRail)
        }
        "right" if has_right && selection.is_empty() && selection.end == value.len() => {
            Some(ModelPickerFocusPane::RightColumn)
        }
        _ => None,
    }
}

const OPENAI_LOGO_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z"/></svg>"#;

const CLAUDE_LOGO_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="12" y1="3" x2="12" y2="21"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="5.64" y1="5.64" x2="18.36" y2="18.36"/><line x1="18.36" y1="5.64" x2="5.64" y2="18.36"/></svg>"#;

const GEMINI_LOGO_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C12.35 7.45 16.55 11.65 22 12C16.55 12.35 12.35 16.55 12 22C11.65 16.55 7.45 12.35 2 12C7.45 11.65 11.65 7.45 12 2Z"/></svg>"#;

fn model_provider_icon(
    provider: Option<&str>,
    model_id: Option<&str>,
    backend: Option<Backend>,
) -> Icon {
    let p = provider.unwrap_or("").to_ascii_lowercase();
    let m = model_id.unwrap_or("").to_ascii_lowercase();
    if p.contains("openai")
        || p.contains("codex")
        || m.starts_with("gpt")
        || m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4")
        || (p.is_empty() && matches!(backend, Some(Backend::Codex)))
    {
        Icon::new(Glyph::Sparkles).data(OPENAI_LOGO_SVG)
    } else if p.contains("anthropic")
        || p.contains("claude")
        || m.contains("claude")
        || (p.is_empty() && matches!(backend, Some(Backend::Claude)))
    {
        Icon::new(Glyph::Sparkles).data(CLAUDE_LOGO_SVG)
    } else if p.contains("google") || p.contains("gemini") || m.contains("gemini") {
        Icon::new(Glyph::Sparkles).data(GEMINI_LOGO_SVG)
    } else if p.contains("deepseek")
        || p.contains("qwen")
        || p.contains("xai")
        || p.contains("grok")
        || p.contains("mistral")
        || p.contains("meta")
        || p.contains("llama")
        || m.contains("deepseek")
        || m.contains("qwen")
        || m.contains("grok")
    {
        Icon::new(Glyph::Orbit)
    } else {
        Icon::new(
            backend
                .map(crate::composer_controls::backend_glyph)
                .unwrap_or(Glyph::Sparkles),
        )
    }
}

fn short_model_name(raw_name: &str) -> String {
    ["Codex", "Claude", "Pi", "外部 Pi", tr("外部 Pi")]
        .into_iter()
        .find_map(|tag| raw_name.strip_suffix(&format!(" · {tag}")))
        .map(str::to_owned)
        .unwrap_or_else(|| raw_name.to_owned())
}

impl HiveApp {
    fn project_model_scope(&self) -> Option<(&str, &str, &crate::agent_profiles::ProfileRef)> {
        let session = self.session.as_ref()?;
        let lead = format!("{}:orchestrator", session.workspace_id);
        if self.workspace.as_ref() != Some(&session.workspace_id)
            || session.backend == Backend::Acp
            || session.scratch
            || session.archived
            || session.worktree_released
            || !matches!(session.identity_state.as_deref(), Some("new" | "known"))
            || session.agent_id.as_ref().is_some_and(|id| id != &lead)
            || session.origin_agent_id.as_ref().is_some_and(|id| id != &lead)
        {
            return None;
        }
        Some((&session.workspace_id, &session.id, session.profile.as_ref()?))
    }

    fn confirmed_project_model(&self) -> Option<&ModelIdentity> {
        if self
            .current_history_access()
            .is_some_and(|access| access.saved_history_passive)
            || self.unavailable_model.is_some()
        {
            return None;
        }
        let model = self.model.as_ref()?;
        self.models
            .iter()
            .any(|candidate| {
                candidate.available && candidate.provider == model.provider && candidate.id == model.id
            })
            .then_some(model)
    }

    pub(crate) fn unavailable_project_draft_model(&self) -> Option<&ModelIdentity> {
        self.session.as_ref()?.project_default_draft_model.then_some(())?;
        self.unavailable_inherited_model()
    }

    pub(crate) fn load_model_picker_contents(&mut self, cx: &mut Context<Self>) {
        self.request_project_model_default(None, cx);
        if self.unavailable_project_draft_model().is_none() {
            self.load_reasoning(None, cx);
            self.load_mode(None, cx);
            self.load_permission_mode(None, cx);
        }
        self.sync_model_picker_right_column_after_load();
    }

    pub(crate) fn model_picker_right_loading(&self) -> bool {
        self.reasoning_picker.loading || self.mode_picker.is_loading()
    }

    pub(crate) fn has_confirmed_model_picker_right_column(&self) -> bool {
        let reasoning_has = self.reasoning_picker.levels().len() >= 2
            || self.reasoning_picker.has_error()
            || self.reasoning_picker.has_recovery_notice();
        let mode_has = self
            .mode_picker
            .has_visible_right_column_content(self.session.as_ref().map(|s| s.backend));
        reasoning_has || mode_has
    }

    pub(crate) fn sync_model_picker_right_column_after_load(&mut self) {
        let (next_had_right, next_focus_pane, needs_search_focus) = resolve_right_column_after_load(
            self.model_picker_right_loading(),
            self.has_confirmed_model_picker_right_column(),
            self.model_picker.had_right_column,
            self.model_picker.focus_pane,
        );
        self.model_picker.had_right_column = next_had_right;
        if next_focus_pane != self.model_picker.focus_pane {
            self.model_picker.focus_pane = next_focus_pane;
            self.model_picker.right_index = 0;
        }
        if needs_search_focus && self.choosing_model {
            self.model_picker.focus_search_on_render = true;
        }
    }

    pub(crate) fn has_model_picker_right_column(&self, running: bool, cx: &Context<Self>) -> bool {
        self.render_reasoning_settings(running, cx).is_some()
            || self.render_mode_settings(running, cx).is_some()
            || (self.model_picker.had_right_column && self.model_picker_right_loading())
    }

    fn right_column_items(&self) -> Vec<RightColumnItem> {
        let mut items = Vec::new();
        let levels = self.reasoning_picker.levels();
        if levels.len() >= 2 {
            for level in levels {
                items.push(RightColumnItem::Reasoning(level.clone()));
            }
        }
        for (permission, id) in self.mode_right_column_items() {
            items.push(RightColumnItem::Mode { permission, id });
        }
        items
    }

    fn focused_right_column_item(&self) -> Option<RightColumnItem> {
        if !self.choosing_model
            || self.model_picker.focus_pane != ModelPickerFocusPane::RightColumn
        {
            return None;
        }
        self.right_column_items()
            .get(self.model_picker.right_index)
            .cloned()
    }

    pub(crate) fn model_picker_focused_reasoning(&self) -> Option<&str> {
        if !self.choosing_model
            || self.model_picker.focus_pane != ModelPickerFocusPane::RightColumn
        {
            return None;
        }
        let levels = self.reasoning_picker.levels();
        (levels.len() >= 2)
            .then(|| levels.get(self.model_picker.right_index).map(String::as_str))
            .flatten()
    }

    pub(crate) fn model_picker_focused_mode(&self, permission: bool) -> Option<String> {
        if !self.choosing_model
            || self.model_picker.focus_pane != ModelPickerFocusPane::RightColumn
        {
            return None;
        }
        let levels_offset = if self.reasoning_picker.levels().len() >= 2 {
            self.reasoning_picker.levels().len()
        } else {
            0
        };
        let mode_items = self.mode_right_column_items();
        let idx = self.model_picker.right_index.checked_sub(levels_offset)?;
        let (item_perm, id) = mode_items.get(idx)?;
        (*item_perm == permission).then(|| id.clone())
    }

    fn toggle_model_favorite(&mut self, provider: &str, id: &str, cx: &mut Context<Self>) {
        if let Err(error) = self.model_picker.favorites.toggle(provider, id) {
            self.error = Some(ErrorNotice::Detail("无法保存收藏模型", error.to_string()));
        }
        cx.notify();
    }

    fn request_project_model_default(&mut self, change: Option<Value>, cx: &mut Context<Self>) {
        if !self.connected
            || self.closing
            || !self.conversation_ready
            || matches!(self.project_model_default, ProjectModelDefaultState::Loading)
            || change.is_some() && !self.workspace_writes_allowed()
        {
            return;
        }
        let (Some(runtime), Some((workspace, session, profile))) =
            (self.runtime.clone(), self.project_model_scope())
        else {
            return;
        };
        let mut params =
            json!({"workspaceId": workspace, "sessionId": session, "profile": profile});
        let (method, error_label) = if let Some(model) = change {
            params["model"] = model;
            ("chat.setProjectModelDefault", "无法保存项目默认模型")
        } else {
            ("chat.projectModelDefault", "无法读取项目默认模型")
        };
        let opened = self.open_request;
        let connection = self.connection_generation;
        self.project_model_default = ProjectModelDefaultState::Loading;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await.and_then(|value| {
                serde_json::from_value::<ProjectModelDefaultResponse>(value)
                    .map_err(|error| error.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != opened
                    || this.closing
                {
                    return;
                }
                this.project_model_default = match result {
                    Ok(result) => ProjectModelDefaultState::Ready(result.model),
                    Err(error) => {
                        ProjectModelDefaultState::Failed(format!("{}\n{error}", tr(error_label)))
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn set_current_project_model_default(&mut self, cx: &mut Context<Self>) {
        if let Some(model) = self.confirmed_project_model() {
            self.request_project_model_default(
                Some(json!({"provider": model.provider, "id": model.id})),
                cx,
            );
        }
    }

    fn project_model_default_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        use ProjectModelDefaultState::{Failed, Idle, Loading, Ready};
        let saved = match &self.project_model_default {
            Ready(model) => model.as_ref(),
            _ => None,
        };
        let current = self.confirmed_project_model();
        let blocked =
            matches!(self.project_model_default, Idle | Loading) || !self.workspace_writes_allowed();
        let (status_text, status_error) = match &self.project_model_default {
            Failed(error) => (error.clone(), true),
            Ready(Some(model)) => {
                let identity = format!("{} / {}", model.provider, model.id);
                let missing = self.unavailable_project_draft_model().is_some_and(|stale| {
                    stale.provider == model.provider && stale.id == model.id
                }) || (!self.models.is_empty()
                    && !self.models.iter().any(|candidate| {
                        candidate.available
                            && candidate.provider == model.provider
                            && candidate.id == model.id
                    }));
                if missing {
                    (
                        tr("项目默认模型 {model} 暂不可用。").replace("{model}", &identity),
                        true,
                    )
                } else {
                    (format!("{}：{}", tr("项目默认"), identity), false)
                }
            }
            Ready(None) => (tr("未设项目默认").to_owned(), false),
            Idle | Loading => (tr("项目默认").to_owned(), false),
        };
        h_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_1p5()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Inset.get(cx).opacity(0.32))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.5))
                    .text_color(if status_error {
                        Color::Error.get(cx)
                    } else {
                        Color::Muted.get(cx)
                    })
                    .child(status_text),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("set-project-default-model")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(22.))
                            .px_2()
                            .rounded_md()
                            .label(tr("设为项目默认"))
                            .tooltip(tr("使用当前模型作为项目默认"))
                            .accessibility_label(tr("使用当前模型作为项目默认"))
                            .disabled(
                                blocked
                                    || current.is_none_or(|model| {
                                        saved.is_some_and(|saved| {
                                            saved.provider == model.provider
                                                && saved.id == model.id
                                        })
                                    }),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_current_project_model_default(cx)
                            })),
                    )
                    .when(
                        saved.is_some() || matches!(self.project_model_default, Failed(_)),
                        |row| {
                            row.child(
                                Button::new("reset-project-default-model")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("重置"))
                                    .tooltip(tr("重置项目默认模型"))
                                    .accessibility_label(tr("重置项目默认模型"))
                                    .disabled(blocked)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_project_model_default(Some(Value::Null), cx)
                                    })),
                            )
                        },
                    ),
            )
    }

    fn matching_picker_models(&self, cx: &App) -> Vec<&Model> {
        let query = self.model_search.read(cx).value();
        rank_matching_models(
            &self.models,
            &query,
            &self.model_picker.tab,
            self.model.as_ref(),
            &self.model_picker.favorites,
        )
    }

    pub(crate) fn reset_model_picker_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let providers = available_providers(&self.models);
        let default_tab = if providers.len() >= 2 {
            if let Some(current) = &self.model
                && providers.iter().any(|p| p == &current.provider)
            {
                ModelPickerTab::Provider(current.provider.clone())
            } else {
                ModelPickerTab::All
            }
        } else {
            ModelPickerTab::All
        };
        self.model_picker.tab = default_tab;
        self.model_picker.focus_pane = ModelPickerFocusPane::ModelList;
        self.model_picker.right_index = 0;
        self.model_picker.had_right_column = false;
        self.model_picker.focus_search_on_render = false;
        self.model_picker_search_changed(cx);
        self.model_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.model_picker.right_scroll.set_offset(point(px(0.), px(0.)));
    }

    pub(crate) fn model_picker_search_changed(&mut self, cx: &mut Context<Self>) {
        self.model_picker.navigation = Default::default();
        self.model_picker.model_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn model_search_is_composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.model_search.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        })
    }

    fn move_model_search(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_search_is_composing(window, cx) {
            return;
        }
        cx.stop_propagation();
        let matching = self.matching_picker_models(cx);
        if matching.is_empty() {
            return;
        }
        let current = self.model_picker.navigation.active_index(&matching);
        let index = current.saturating_add_signed(delta).min(matching.len() - 1);
        let key = model_key(matching[index]);
        self.model_picker.navigation.active = Some(key);
        self.model_picker.navigation.cancel_enter();
        self.model_picker.model_scroll.scroll_to_item(index);
        cx.notify();
    }

    fn focus_model_picker_pane(
        &mut self,
        pane: ModelPickerFocusPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model_picker.focus_pane = pane;
        match pane {
            ModelPickerFocusPane::ProviderRail => {
                self.model_picker.rail_focus.focus(window, cx);
            }
            ModelPickerFocusPane::ModelList => {
                self.model_search.read(cx).focus_handle(cx).focus(window, cx);
            }
            ModelPickerFocusPane::RightColumn => {
                let items = self.right_column_items();
                if let Some(current_level) = self.reasoning_picker.current_level()
                    && let Some(pos) = items.iter().position(|item| {
                        matches!(item, RightColumnItem::Reasoning(l) if l == current_level)
                    })
                {
                    self.model_picker.right_index = pos;
                } else if self.model_picker.right_index >= items.len() {
                    self.model_picker.right_index = 0;
                }
                self.model_picker.right_focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    fn model_search_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let composing = self.model_search_is_composing(window, cx);
        let value = self.model_search.read(cx).value().to_string();
        let selection = self.model_search.read(cx).selected_range();
        let providers = available_providers(&self.models);
        let show_rail =
            should_show_provider_rail(&providers, !self.model_picker.favorites.is_empty());
        let has_right = self.has_model_picker_right_column(false, cx);
        if let Some(target_pane) = should_switch_pane_from_search(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers,
            composing,
            &value,
            selection,
            show_rail,
            has_right,
        ) {
            cx.stop_propagation();
            self.focus_model_picker_pane(target_pane, window, cx);
            return;
        }
        self.model_search_enter_down(event, window, cx);
    }

    fn model_search_enter_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "enter"
            || event.keystroke.modifiers.modified()
            || self.model_search_is_composing(window, cx)
        {
            return;
        }
        cx.stop_propagation();
        if event.is_held
            || self.reasoning_picker.loading
            || self.model_picker.navigation.enter.is_some()
        {
            return;
        }
        // Keep focus in the search until Enter is released. Otherwise a held key
        // can land in the composer as soon as the popover closes and send its draft.
        let matching = self.matching_picker_models(cx);
        let index = self.model_picker.navigation.active_index(&matching);
        self.model_picker.navigation.enter = matching.get(index).map(|model| model_key(model));
    }

    fn model_search_enter_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "enter" {
            return;
        }
        let Some((provider, id)) = self.model_picker.navigation.enter.take() else {
            return;
        };
        cx.stop_propagation();
        if event.keystroke.modifiers.modified()
            || !self.choosing_model
            || self.reasoning_picker.loading
            || self.model_search_is_composing(window, cx)
        {
            return;
        }
        if let Some(model) = self
            .matching_picker_models(cx)
            .into_iter()
            .find(|model| model.provider == provider && model.id == id)
        {
            self.set_model(model.clone(), cx);
        }
    }

    fn model_search_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.model_picker.navigation.cancel_enter();
        if self.model_search_is_composing(window, cx) {
            self.model_search
                .update(cx, |input, cx| input.unmark_text(window, cx));
            return;
        }
        self.choosing_model = false;
        self.input.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn model_rail_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let providers = available_providers(&self.models);
        let tabs = rail_tabs(&providers);
        if tabs.is_empty() {
            return;
        }
        let current_idx = tabs
            .iter()
            .position(|tab| tab == &self.model_picker.tab)
            .unwrap_or(0);
        match event.keystroke.key.as_str() {
            "up" => {
                cx.stop_propagation();
                let next = current_idx.saturating_sub(1);
                self.model_picker.tab = tabs[next].clone();
                self.model_picker_search_changed(cx);
            }
            "down" => {
                cx.stop_propagation();
                let next = (current_idx + 1).min(tabs.len() - 1);
                self.model_picker.tab = tabs[next].clone();
                self.model_picker_search_changed(cx);
            }
            "right" | "enter" => {
                cx.stop_propagation();
                self.focus_model_picker_pane(ModelPickerFocusPane::ModelList, window, cx);
            }
            "escape" => {
                cx.stop_propagation();
                self.choosing_model = false;
                self.input.read(cx).focus_handle(cx).focus(window, cx);
                cx.notify();
            }
            _ => {}
        }
    }

    fn model_right_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let items = self.right_column_items();
        match event.keystroke.key.as_str() {
            "up" if !items.is_empty() => {
                cx.stop_propagation();
                self.model_picker.right_index = self.model_picker.right_index.saturating_sub(1);
                self.model_picker
                    .right_scroll
                    .scroll_to_item(self.model_picker.right_index);
                cx.notify();
            }
            "down" if !items.is_empty() => {
                cx.stop_propagation();
                self.model_picker.right_index =
                    (self.model_picker.right_index + 1).min(items.len() - 1);
                self.model_picker
                    .right_scroll
                    .scroll_to_item(self.model_picker.right_index);
                cx.notify();
            }
            "enter" | "space" if !event.is_held => {
                cx.stop_propagation();
                if let Some(item) = self.focused_right_column_item() {
                    match item {
                        RightColumnItem::Reasoning(level) => {
                            self.load_reasoning(Some(level), cx);
                        }
                        RightColumnItem::Mode { permission, id } => {
                            self.load_mode_axis(permission, Some(id), cx);
                        }
                    }
                }
            }
            "left" => {
                cx.stop_propagation();
                self.focus_model_picker_pane(ModelPickerFocusPane::ModelList, window, cx);
            }
            "escape" => {
                cx.stop_propagation();
                self.choosing_model = false;
                self.input.read(cx).focus_handle(cx).focus(window, cx);
                cx.notify();
            }
            _ => {}
        }
    }

    fn render_model_picker_right_column(
        &self,
        model_name: &str,
        running: bool,
        narrow_max_h: Option<f32>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let reasoning = self.render_reasoning_settings(running, cx);
        let mode = self.render_mode_settings(running, cx);
        let keep_loading =
            self.model_picker.had_right_column && self.model_picker_right_loading();
        if reasoning.is_none() && mode.is_none() && !keep_loading {
            return None;
        }
        let has_reasoning = reasoning.is_some();
        let header_title = match (reasoning.is_some(), mode.is_some()) {
            (true, true) => tr("推理与模式"),
            (true, false) => tr("推理强度"),
            (false, true) => tr("会话模式"),
            (false, false) => tr("推理与模式"),
        };
        let panel = v_flex()
            .id("model-picker-right-column")
            .track_focus(&self.model_picker.right_focus)
            .on_key_down(cx.listener(Self::model_right_key_down))
            .w_full()
            .when(narrow_max_h.is_none(), |panel| panel.h_full())
            .when_some(narrow_max_h, |panel, max_h| panel.max_h(px(max_h)))
            .min_w_0()
            .min_h_0()
            .bg(Color::Inset.get(cx).opacity(0.28))
            .child(
                h_flex()
                    .h(px(36.))
                    .flex_shrink_0()
                    .px_2p5()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Secondary.get(cx))
                            .child(header_title),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(Color::Muted.get(cx))
                            .child(model_name.to_owned()),
                    ),
            )
            .child(
                v_flex()
                    .id("model-picker-right-scroll")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .p_1()
                    .gap_1p5()
                    .overflow_y_scroll()
                    .track_scroll(&self.model_picker.right_scroll)
                    .vertical_scrollbar(&self.model_picker.right_scroll)
                    .when(reasoning.is_none() && mode.is_none() && keep_loading, |col| {
                        col.child(
                            div()
                                .px_2()
                                .py_1p5()
                                .text_size(px(11.5))
                                .text_color(Color::Muted.get(cx))
                                .child(tr("正在读取…")),
                        )
                    })
                    .children(reasoning)
                    .when_some(mode, |col, mode_el| {
                        col.child(
                            div()
                                .w_full()
                                .min_w_0()
                                .when(has_reasoning, |wrap| {
                                    wrap.border_t_1()
                                        .border_color(Color::Divider.get(cx))
                                        .pt_1()
                                })
                                .child(mode_el),
                        )
                    }),
            );
        Some(panel.into_any_element())
    }

    pub(crate) fn model_picker(
        &self,
        window: &Window,
        running: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let current = self.model.as_ref();
        let selected = current.and_then(|current| {
            self.models
                .iter()
                .find(|model| model.provider == current.provider && model.id == current.id)
        });
        let raw_name = selected
            .filter(|model| !model.name.trim().is_empty())
            .map(|model| model.name.clone())
            .or_else(|| {
                current.and_then(|model| {
                    model
                        .name
                        .as_ref()
                        .filter(|name| !name.trim().is_empty())
                        .cloned()
                })
            })
            .or_else(|| current.map(|model| model.id.clone()))
            .unwrap_or_else(|| tr("选择模型").into());
        let name = short_model_name(&raw_name);
        let effort = self.reasoning_label().filter(|level| !level.trim().is_empty());
        let label = effort
            .as_ref()
            .map(|level| format!("{name} · {level}"))
            .unwrap_or_else(|| name.clone());
        let identity = current
            .map(|model| format!("{} / {}", model.provider, model.id))
            .unwrap_or_else(|| tr("选择会话模型").into());
        let unavailable =
            !self.connected || !self.conversation_ready || running || self.loading || self.closing;
        let backend_icon = model_provider_icon(
            current.map(|m| m.provider.as_str()),
            current.map(|m| m.id.as_str()),
            self.session.as_ref().map(|session| session.backend),
        );
        let profile_note = self
            .session
            .as_ref()
            .and_then(|s| s.profile.as_ref())
            .map(|profile| format!("\n{}", self.profile_label(profile)))
            .unwrap_or_default();
        let picker_label = tr("选择会话模型与推理强度");
        let accessibility = if self.session.as_ref().is_some_and(|session| {
            session.inherited_draft_model || session.inherited_draft_reasoning.is_some()
        }) {
            format!("{picker_label} · {label}")
        } else {
            picker_label.to_owned()
        };
        let trigger = Button::new("choose-model")
            .accessibility_label(accessibility)
            .custom(crate::palette::quiet_icon_variant(cx))
            .compact()
            .h(px(28.))
            .px_2()
            .rounded_lg()
            .selected(self.choosing_model)
            .min_w_0()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        backend_icon
                            .size(px(14.5))
                            .text_color(Color::Secondary.get(cx))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .max_w(px(180.))
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Text.get(cx))
                            .child(name.clone()),
                    )
                    .when_some(effort, |row, level| {
                        row.child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(12.5))
                                .text_color(Color::Muted.get(cx))
                                .child(format!("· {level}")),
                        )
                    }),
            )
            .tooltip(format!(
                "{label}\n{identity}{profile_note}\n{}\n{}",
                tr("只影响当前会话，不修改 Agent 全局默认"),
                crate::keybindings::hint(crate::keybindings::Target::ChooseModel, "快捷键", cx),
            ))
            .disabled(unavailable);
        if unavailable {
            return trigger.into_any_element();
        }
        let mut trigger_layout = div().min_w_0();
        let trigger_style = trigger_layout.style().clone();

        let viewport = window.viewport_size();
        let narrow = viewport.width < px(600.);
        let show_footer = self.project_model_scope().is_some();
        let providers = available_providers(&self.models);
        let show_provider_rail =
            should_show_provider_rail(&providers, !self.model_picker.favorites.is_empty());
        let has_right_hint =
            self.choosing_model && self.has_model_picker_right_column(running, cx);
        let narrow_layout = narrow.then(|| {
            narrow_model_picker_layout(f32::from(viewport.height), has_right_hint, show_footer)
        });
        let right_column = if self.choosing_model {
            self.render_model_picker_right_column(
                &name,
                running,
                narrow_layout.map(|layout| layout.right_max_h),
                cx,
            )
        } else {
            None
        };
        let has_right_column = right_column.is_some();
        if self.choosing_model
            && !has_right_column
            && (self.model_picker.focus_search_on_render
                || self.model_picker.focus_pane == ModelPickerFocusPane::RightColumn
                || self.model_picker.right_focus.is_focused(window))
        {
            let weak = cx.entity().downgrade();
            window.on_next_frame(move |window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    let needs_focus = std::mem::take(&mut this.model_picker.focus_search_on_render);
                    if this.choosing_model
                        && !this.has_model_picker_right_column(false, cx)
                        && (needs_focus
                            || this.model_picker.focus_pane == ModelPickerFocusPane::RightColumn
                            || this.model_picker.right_focus.is_focused(window))
                    {
                        this.focus_model_picker_pane(ModelPickerFocusPane::ModelList, window, cx);
                    }
                });
            });
        }
        let popover_width = if narrow {
            (viewport.width - px(24.)).max(px(280.))
        } else {
            let left_w = if show_provider_rail { 364. } else { 328. };
            let right_w = if has_right_column { 192. } else { 0. };
            px(left_w + right_w).min(viewport.width - px(24.))
        };

        Popover::new("session-model-picker")
            .anchor(Anchor::BottomRight)
            .trigger_style(trigger_style)
            .p_0()
            .rounded_xl()
            .border_1()
            .border_color(Color::BorderStrong.get(cx))
            .bg(Color::Overlay.get(cx))
            .shadow(crate::palette::overlay_shadow(cx))
            .overflow_hidden()
            .w(popover_width)
            .open(self.choosing_model)
            .track_focus(&self.model_search.read(cx).focus_handle(cx))
            .trigger(trigger)
            .on_open_change(cx.listener(|this, open, window, cx| {
                if *open
                    && this.unavailable_project_draft_model().is_none()
                    && this.connect_saved_session(crate::sessions::NativeSessionPanel::Models, cx)
                {
                    return;
                }
                this.choosing_model = *open;
                if *open {
                    this.reset_model_picker_search(window, cx);
                    this.load_model_picker_contents(cx);
                }
                cx.notify();
            }))
            .when(self.choosing_model, |popover| {
                let matching = self.matching_picker_models(cx);
                let active = self.model_picker.navigation.active_index(&matching);
                let search_focused = self
                    .model_search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window);
                let query_non_empty = !self.model_search.read(cx).value().trim().is_empty();
                let empty = matching.is_empty();
                let show_provider_tag = query_non_empty
                    || matches!(
                        self.model_picker.tab,
                        ModelPickerTab::Favorites | ModelPickerTab::All
                    );
                let model_scroll = &self.model_picker.model_scroll;
                let session_backend = self.session.as_ref().map(|s| s.backend);
                let is_pi_backend = matches!(session_backend, Some(Backend::Pi | Backend::PiRpc));
                let can_refresh_pi = is_pi_backend
                    && !self.conversation_is_read_only()
                    && self.workspace_writes_allowed();
                let empty_favorites = matches!(self.model_picker.tab, ModelPickerTab::Favorites)
                    && !query_non_empty
                    && self.model_picker.favorites.is_empty();
                let body_h = px(346.).min(viewport.height - px(160.)).max(px(220.));

                let rail_el = show_provider_rail.then(|| {
                    let active_tab = self.model_picker.tab.clone();
                    let fav_selected = matches!(active_tab, ModelPickerTab::Favorites);
                    let all_selected = matches!(active_tab, ModelPickerTab::All);
                    v_flex()
                        .id("model-provider-rail")
                        .role(Role::Group)
                        .aria_label(tr("模型提供商"))
                        .track_focus(&self.model_picker.rail_focus)
                        .on_key_down(cx.listener(Self::model_rail_key_down))
                        .w(px(44.))
                        .flex_shrink_0()
                        .min_h_0()
                        .when(!narrow, |rail| rail.h_full())
                        .when_some(narrow_layout, |rail, layout| {
                            rail.max_h(px(36.0 + layout.list_max_h))
                        })
                        .overflow_y_scroll()
                        .bg(Color::Sidebar.get(cx).opacity(0.6))
                        .border_r_1()
                        .border_color(Color::Divider.get(cx))
                        .p_1()
                        .gap_1()
                        .items_center()
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_center()
                                .relative()
                                .when(fav_selected, |row| {
                                    row.child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .h(px(20.))
                                            .w(px(3.))
                                            .rounded_full()
                                            .bg(Color::Accent.get(cx)),
                                    )
                                })
                                .child(
                                    Button::new("model-rail-favorites")
                                        .custom(crate::palette::sidebar_item_variant(
                                            fav_selected,
                                            cx,
                                        ))
                                        .compact()
                                        .w(px(32.))
                                        .h(px(32.))
                                        .rounded_md()
                                        .icon(if fav_selected || !self.model_picker.favorites.is_empty() {
                                            Glyph::StarFill
                                        } else {
                                            Glyph::Star
                                        })
                                        .tooltip(tr("收藏"))
                                        .accessibility_label(tr("收藏"))
                                        .selected(fav_selected)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.model_picker.tab = ModelPickerTab::Favorites;
                                            this.model_picker_search_changed(cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .w(px(24.))
                                .h(px(1.))
                                .bg(Color::Divider.get(cx)),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_center()
                                .relative()
                                .when(all_selected, |row| {
                                    row.child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .h(px(20.))
                                            .w(px(3.))
                                            .rounded_full()
                                            .bg(Color::Accent.get(cx)),
                                    )
                                })
                                .child(
                                    Button::new("model-rail-all")
                                        .custom(crate::palette::sidebar_item_variant(
                                            all_selected,
                                            cx,
                                        ))
                                        .compact()
                                        .w(px(32.))
                                        .h(px(32.))
                                        .rounded_md()
                                        .icon(Glyph::Layers)
                                        .tooltip(tr("全部"))
                                        .accessibility_label(tr("全部"))
                                        .selected(all_selected)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.model_picker.tab = ModelPickerTab::All;
                                            this.model_picker_search_changed(cx);
                                        })),
                                ),
                        )
                        .children(providers.iter().enumerate().map(|(i, provider)| {
                            let is_active =
                                matches!(&active_tab, ModelPickerTab::Provider(p) if p == provider);
                            let provider_name = provider.clone();
                            let count = self
                                .models
                                .iter()
                                .filter(|m| m.available && &m.provider == provider)
                                .count();
                            let icon = model_provider_icon(Some(provider), None, session_backend);
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_center()
                                .relative()
                                .when(is_active, |row| {
                                    row.child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .h(px(20.))
                                            .w(px(3.))
                                            .rounded_full()
                                            .bg(Color::Accent.get(cx)),
                                    )
                                })
                                .child(
                                    Button::new(("model-rail-provider", i))
                                        .custom(crate::palette::sidebar_item_variant(
                                            is_active, cx,
                                        ))
                                        .compact()
                                        .w(px(32.))
                                        .h(px(32.))
                                        .rounded_md()
                                        .tooltip(format!("{provider} ({count})"))
                                        .accessibility_label(provider.clone())
                                        .selected(is_active)
                                        .child(
                                            icon.size(px(15.)).text_color(if is_active {
                                                Color::Text.get(cx)
                                            } else {
                                                Color::Secondary.get(cx)
                                            }),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.model_picker.tab =
                                                ModelPickerTab::Provider(provider_name.clone());
                                            this.model_picker_search_changed(cx);
                                        })),
                                )
                        }))
                });

                let list_col = v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .h_full()
                    .child(
                        h_flex()
                            .items_center()
                            .h(px(36.))
                            .flex_shrink_0()
                            .px_2p5()
                            .gap_1()
                            .border_b_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                                        this.move_model_search(-1, window, cx)
                                    }))
                                    .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                                        this.move_model_search(1, window, cx)
                                    }))
                                    .capture_action(cx.listener(Self::model_search_escape))
                                    .capture_key_down(cx.listener(Self::model_search_key_down))
                                    .capture_key_up(cx.listener(Self::model_search_enter_up))
                                    .child(
                                        Input::new(&self.model_search)
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .cleanable(true)
                                            .prefix(
                                                Icon::new(Glyph::Search)
                                                    .size(px(13.5))
                                                    .text_color(Color::Muted.get(cx)),
                                            )
                                            .aria_label(tr("搜索会话模型")),
                                    ),
                            )
                            .when(can_refresh_pi, |bar| {
                                bar.child(
                                    Button::new("model-picker-refresh-pi")
                                        .custom(crate::palette::quiet_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .h(px(24.))
                                        .w(px(24.))
                                        .rounded_md()
                                        .icon(Glyph::RefreshCw)
                                        .tooltip(tr("刷新模型"))
                                        .accessibility_label(tr("刷新模型"))
                                        .disabled(self.refreshing_models || running)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.refresh_pi_models(cx)
                                        })),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .w_full()
                            .min_w_0()
                            .min_h_0()
                            .when_some(narrow_layout, |box_el, layout| {
                                box_el.max_h(px(layout.list_max_h))
                            })
                            .child(crate::focus_scroll::Viewport::new(
                                "model-picker-viewport",
                                model_scroll,
                                v_flex()
                                    .id("model-list")
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .p_1p5()
                                    .gap_0p5()
                                    .overflow_y_scroll()
                                    .track_scroll(model_scroll)
                                    .vertical_scrollbar(model_scroll)
                                    .when(empty, |list| {
                                        list.child(
                                            div()
                                                .py_5()
                                                .px_3()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    crate::text_reading::selectable_text(
                                                        "model-picker-empty",
                                                        if empty_favorites {
                                                            tr("暂无收藏模型")
                                                        } else {
                                                            tr("没有匹配的可用模型")
                                                        },
                                                    )
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx)),
                                                ),
                                        )
                                    })
                                    .children(matching.into_iter().enumerate().map(|(i, model)| {
                                        let selected = self.model.as_ref().is_some_and(|m| {
                                            m.id == model.id && m.provider == model.provider
                                        });
                                        let favorited = self
                                            .model_picker
                                            .favorites
                                            .contains(&model.provider, &model.id);
                                        let fav_provider = model.provider.clone();
                                        let fav_id = model.id.clone();
                                        let fav_name = model.name.clone();
                                        let model = model.clone();
                                        let provider_icon = model_provider_icon(
                                            Some(&model.provider),
                                            Some(&model.id),
                                            session_backend,
                                        );
                                        let separator = if crate::language::english() {
                                            ", "
                                        } else {
                                            "，"
                                        };
                                        let mut label = format!(
                                            "{}{separator}{} / {}",
                                            model.name, model.provider, model.id
                                        );
                                        if selected {
                                            label.push_str(separator);
                                            label.push_str(tr("当前生效"));
                                        }
                                        if favorited {
                                            label.push_str(separator);
                                            label.push_str(tr("已收藏"));
                                        }
                                        if i == active
                                            && search_focused
                                            && !self.reasoning_picker.loading
                                        {
                                            label.push_str(separator);
                                            label.push_str(tr("按 Enter 选择"));
                                        }
                                        crate::focus_scroll::Reveal::new(
                                            ("model-option-reveal", i),
                                            model_scroll,
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .flex_shrink_0()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div().flex_1().min_w_0().child(
                                                        Button::new(("model", i))
                                                            .custom(
                                                                crate::palette::sidebar_item_variant(
                                                                    i == active,
                                                                    cx,
                                                                ),
                                                            )
                                                            .compact()
                                                            .justify_start()
                                                            .w_full()
                                                            .min_w_0()
                                                            .h(px(32.))
                                                            .px_2()
                                                            .rounded_lg()
                                                            .disabled(self.reasoning_picker.loading)
                                                            .accessibility_label(label)
                                                            .tooltip(format!(
                                                                "{}\n{} / {}",
                                                                model.name,
                                                                model.provider,
                                                                model.id
                                                            ))
                                                            .child(
                                                                h_flex()
                                                                    .w_full()
                                                                    .min_w_0()
                                                                    .items_center()
                                                                    .gap_2()
                                                                    .child(
                                                                        provider_icon
                                                                            .size(px(14.))
                                                                            .text_color(if selected {
                                                                                Color::Secondary.get(cx)
                                                                            } else {
                                                                                Color::Muted.get(cx)
                                                                            })
                                                                            .flex_shrink_0(),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .flex_1()
                                                                            .min_w_0()
                                                                            .truncate()
                                                                            .text_size(px(13.))
                                                                            .font_weight(if selected {
                                                                                FontWeight::SEMIBOLD
                                                                            } else {
                                                                                FontWeight::NORMAL
                                                                            })
                                                                            .text_color(
                                                                                Color::Text.get(cx),
                                                                            )
                                                                            .child(
                                                                                model.name.clone(),
                                                                            ),
                                                                    )
                                                                    .when(
                                                                        show_provider_tag,
                                                                        |row| {
                                                                            row.child(
                                                                                div()
                                                                                    .flex_shrink_0()
                                                                                    .text_size(px(
                                                                                        10.5,
                                                                                    ))
                                                                                    .text_color(
                                                                                        Color::Muted
                                                                                            .get(cx),
                                                                                    )
                                                                                    .child(
                                                                                        model
                                                                                            .provider
                                                                                            .clone(),
                                                                                    ),
                                                                            )
                                                                        },
                                                                    )
                                                                    .when(selected, |row| {
                                                                        row.child(
                                                                            Icon::new(Glyph::Check)
                                                                                .size(px(13.5))
                                                                                .text_color(
                                                                                    Color::Accent
                                                                                        .get(cx),
                                                                                )
                                                                                .flex_shrink_0(),
                                                                        )
                                                                    }),
                                                            )
                                                            .selected(i == active)
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    this.set_model(
                                                                        model.clone(),
                                                                        cx,
                                                                    )
                                                                },
                                                            )),
                                                    ),
                                                )
                                                .child(
                                                    Button::new(("model-fav", i))
                                                        .custom(crate::palette::quiet_icon_variant(
                                                            cx,
                                                        ))
                                                        .compact()
                                                        .xsmall()
                                                        .flex_shrink_0()
                                                        .h(px(26.))
                                                        .w(px(26.))
                                                        .rounded_md()
                                                        .icon(if favorited {
                                                            Glyph::StarFill
                                                        } else {
                                                            Glyph::Star
                                                        })
                                                        .text_color(if favorited {
                                                            rgb(if Theme::global(cx).is_dark() {
                                                                0xfbbf24
                                                            } else {
                                                                0xd97706
                                                            })
                                                        } else {
                                                            Color::Muted.get(cx).opacity(0.6)
                                                        })
                                                        .tooltip(tr(if favorited {
                                                            "取消收藏"
                                                        } else {
                                                            "加入收藏"
                                                        }))
                                                        .accessibility_label(format!(
                                                            "{}: {}",
                                                            tr(if favorited {
                                                                "取消收藏"
                                                            } else {
                                                                "加入收藏"
                                                            }),
                                                            fav_name
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.toggle_model_favorite(
                                                                    &fav_provider,
                                                                    &fav_id,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                ),
                                        )
                                    })),
                            )),
                    );

                let left_pane = h_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .when(!narrow, |pane| pane.h_full())
                    .when_some(narrow_layout, |pane, layout| {
                        pane.max_h(px(36.0 + layout.list_max_h))
                    })
                    .children(rail_el)
                    .child(list_col);

                let body = if narrow {
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .min_h_0()
                        .when_some(narrow_layout, |col, layout| {
                            col.max_h(px(layout.body_max_h)).overflow_hidden()
                        })
                        .child(left_pane)
                        .when_some(right_column, |col, right| {
                            col.child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .min_h_0()
                                    .border_t_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(right),
                            )
                        })
                        .into_any_element()
                } else {
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .h(body_h)
                        .min_h(px(220.))
                        .child(left_pane)
                        .when_some(right_column, |row, right| {
                            row.child(
                                div()
                                    .w(px(192.))
                                    .flex_shrink_0()
                                    .h_full()
                                    .min_h_0()
                                    .border_l_1()
                                    .border_color(Color::Divider.get(cx))
                                    .child(right),
                            )
                        })
                        .into_any_element()
                };

                popover.child(
                    v_flex()
                        .key_context("HiveModelPicker")
                        .w_full()
                        .min_w_0()
                        .child(body)
                        .when(show_footer, |col| {
                            col.child(self.project_model_default_controls(cx))
                        }),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ModelFavoritesStore, ModelPickerFocusPane, ModelPickerTab, available_providers,
        narrow_model_picker_layout, rank_matching_models, resolve_right_column_after_load,
        should_show_provider_rail, should_switch_pane_from_search,
    };
    use crate::protocol::{Model, ModelIdentity};
    use gpui_kit::Modifiers;

    fn make_model(provider: &str, id: &str, name: &str, available: bool) -> Model {
        Model {
            provider: provider.into(),
            id: id.into(),
            name: name.into(),
            available,
        }
    }

    #[test]
    fn provider_tabs_hide_rail_for_single_provider_without_favorites() {
        let single = vec![
            make_model("openai", "gpt-5", "GPT-5", true),
            make_model("openai", "gpt-5-mini", "GPT-5 Mini", true),
        ];
        let single_providers = available_providers(&single);
        assert_eq!(single_providers, vec!["openai"]);
        assert!(!should_show_provider_rail(&single_providers, false));
        assert!(should_show_provider_rail(&single_providers, true));

        let multi = vec![
            make_model("openai", "gpt-5", "GPT-5", true),
            make_model("anthropic", "claude-4", "Claude 4", true),
            make_model("google", "gemini-2", "Gemini 2", false),
        ];
        let multi_providers = available_providers(&multi);
        assert_eq!(multi_providers, vec!["openai", "anthropic"]);
        assert!(should_show_provider_rail(&multi_providers, false));
    }

    #[test]
    fn matching_models_prioritize_favorites_and_exact_matches() {
        let models = vec![
            make_model("openai", "gpt-5-mini", "GPT-5 Mini", true),
            make_model("openai", "gpt-5", "GPT-5", true),
            make_model("anthropic", "claude-sonnet", "Claude Sonnet", true),
            make_model("anthropic", "claude-opus", "Claude Opus", true),
        ];
        let mut favs = ModelFavoritesStore::for_test(None, true);
        favs.toggle("anthropic", "claude-opus").unwrap();

        let empty_query = rank_matching_models(&models, "", &ModelPickerTab::All, None, &favs);
        assert_eq!(empty_query[0].id, "claude-opus");

        let current = ModelIdentity {
            provider: "openai".into(),
            id: "gpt-5-mini".into(),
            name: None,
        };
        let search_gpt =
            rank_matching_models(&models, "gpt-5", &ModelPickerTab::Provider("anthropic".into()), Some(&current), &favs);
        assert_eq!(search_gpt.len(), 2);
        assert_eq!(search_gpt[0].id, "gpt-5");
        assert_eq!(search_gpt[1].id, "gpt-5-mini");

        let fav_tab =
            rank_matching_models(&models, "", &ModelPickerTab::Favorites, None, &favs);
        assert_eq!(fav_tab.len(), 1);
        assert_eq!(fav_tab[0].id, "claude-opus");
    }

    #[test]
    fn model_favorites_persist_atomically_and_respect_preview_guard() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ModelFavoritesStore::for_test(Some(dir.path().to_path_buf()), false);
        store.toggle("openai", "gpt-5").unwrap();
        assert!(store.contains("openai", "gpt-5"));
        let file = dir.path().join(super::FAVORITES_FILE);
        assert!(file.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let mut reopened = ModelFavoritesStore::for_test(None, false);
        reopened.restore(dir.path());
        assert!(reopened.contains("openai", "gpt-5"));

        reopened.toggle("openai", "gpt-5").unwrap();
        assert!(!reopened.contains("openai", "gpt-5"));

        // preview_only = true must never write to disk.
        let preview_dir = tempfile::tempdir().unwrap();
        let mut preview =
            ModelFavoritesStore::for_test(Some(preview_dir.path().to_path_buf()), true);
        preview.toggle("anthropic", "claude-4").unwrap();
        assert!(preview.contains("anthropic", "claude-4"));
        assert!(!preview_dir.path().join(super::FAVORITES_FILE).exists());
    }

    #[test]
    fn keyboard_pane_navigation_respects_ime_and_cursor_bounds() {
        let none = Modifiers::none();
        let ctrl = Modifiers::control();
        assert_eq!(
            should_switch_pane_from_search("left", none, false, "", 0..0, true, true),
            Some(ModelPickerFocusPane::ProviderRail)
        );
        assert_eq!(
            should_switch_pane_from_search("left", none, false, "", 0..0, false, true),
            None
        );
        assert_eq!(
            should_switch_pane_from_search("left", none, false, "gpt", 0..0, true, true),
            None
        );
        assert_eq!(
            should_switch_pane_from_search("right", none, false, "gpt", 1..1, true, true),
            None
        );
        assert_eq!(
            should_switch_pane_from_search("right", none, false, "gpt", 3..3, true, true),
            Some(ModelPickerFocusPane::RightColumn)
        );
        assert_eq!(
            should_switch_pane_from_search("right", none, true, "gpt", 3..3, true, true),
            None
        );
        assert_eq!(
            should_switch_pane_from_search("right", ctrl, false, "gpt", 3..3, true, true),
            None
        );
        assert_eq!(
            should_switch_pane_from_search("right", none, false, "gpt", 3..3, true, false),
            None
        );
    }

    #[test]
    fn narrow_model_picker_layout_stays_within_viewport_bounds() {
        for viewport_h in [480.0_f32, 440.0, 380.0] {
            for has_right in [false, true] {
                for show_footer in [false, true] {
                    let layout = narrow_model_picker_layout(viewport_h, has_right, show_footer);
                    let footer_h = if show_footer { 36.0 } else { 0.0 };
                    let max_total = 2.0
                        + 36.0
                        + layout.list_max_h
                        + if has_right { 1.0 + layout.right_max_h } else { 0.0 }
                        + footer_h;
                    assert!(
                        layout.body_max_h + 2.0 + footer_h <= viewport_h - 24.0 + f32::EPSILON,
                        "body+overhead exceeded margin at h={viewport_h}, right={has_right}, footer={show_footer}"
                    );
                    assert!(
                        max_total <= viewport_h - 24.0 + f32::EPSILON,
                        "popover total {max_total} exceeded viewport-24 at h={viewport_h}, right={has_right}, footer={show_footer}"
                    );
                    assert!(
                        max_total < viewport_h - 16.0,
                        "popover total {max_total} must stay below Positioner clamp bound"
                    );
                }
            }
        }
    }

    #[test]
    fn right_column_loading_transition_and_focus_fallback() {
        // Still loading keeps previous had_right_column and focus_pane without flagging focus reset.
        assert_eq!(
            resolve_right_column_after_load(true, false, false, ModelPickerFocusPane::ModelList),
            (false, ModelPickerFocusPane::ModelList, false)
        );
        assert_eq!(
            resolve_right_column_after_load(true, false, true, ModelPickerFocusPane::RightColumn),
            (true, ModelPickerFocusPane::RightColumn, false)
        );

        // Finished loading with no right column when focus was in RightColumn falls back to ModelList.
        assert_eq!(
            resolve_right_column_after_load(false, false, true, ModelPickerFocusPane::RightColumn),
            (false, ModelPickerFocusPane::ModelList, true)
        );

        // Finished loading with confirmed right column retains RightColumn focus.
        assert_eq!(
            resolve_right_column_after_load(false, true, true, ModelPickerFocusPane::RightColumn),
            (true, ModelPickerFocusPane::RightColumn, false)
        );
    }
}
