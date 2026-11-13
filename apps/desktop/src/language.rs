use gpui_kit::{App, Global, component};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Clone, Copy, Default, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    Zh,
    En,
}
impl Locale {
    fn component_locale(self) -> &'static str {
        match self {
            Self::Zh => "zh-CN",
            Self::En => "en",
        }
    }
}
enum Notice {
    LoadedError(String),
    Saved,
    SaveError(String),
}
#[derive(Default)]
pub struct Language {
    pub locale: Locale,
    root: Option<PathBuf>,
    notice: Option<Notice>,
}
impl Global for Language {}

pub fn english() -> bool {
    component::locale().starts_with("en")
}

// Only explicitly marked application-owned static text is translated. Runtime
// messages, user content, tools and diagnostics never pass through this lookup.
pub fn tr(chinese: &'static str) -> &'static str {
    if !english() {
        return chinese;
    }
    static ENGLISH: OnceLock<HashMap<String, String>> = OnceLock::new();
    ENGLISH
        .get_or_init(|| {
            serde_json::from_str(include_str!("locales/en.json"))
                .expect("embedded English catalogue")
        })
        .get(chinese)
        .map(String::as_str)
        .unwrap_or(chinese)
}

// Closed-set localization for Hive Desktop's own error wrappers and messages
// emitted across the non-UI supervisor/runtime boundary. Unrecognized OS, SDK,
// CLI, or user strings are returned verbatim.
pub(crate) fn localize_owned_error(error: &str) -> String {
    if !english() {
        return error.to_owned();
    }
    const OWNED_ERRORS: &[&str] = &[
        "界面字号须在 12–20 之间",
        "文件路径无效",
        "不支持远程或设备路径",
        "图片内容无法解码",
        "不支持的图片格式。",
        "附件没有返回内容。",
        "草稿标识无效",
        "草稿标识与文件名不符",
        "本地草稿存储尚未可用，请复制任务草稿或丢弃改动后退出",
        "保存路径无效",
        "找不到该成员的活动运行，请刷新状态",
        "等待停止确认超时；请求可能已送达，请刷新后查看状态",
        "应用路径没有父目录",
        "文件路径无法显示为文本",
        "模型草稿版本不受支持",
        "模型草稿存储尚未可用，请保留窗口或丢弃改动",
        "Token 数量需要正整数，或留空使用默认值",
        "后台没有 stdin",
        "后台没有 stdout",
        "后台请求失败，请查看后台日志。",
        "后台连接已关闭；请重新启动 Hive。详细错误见后台日志。",
        "后台已退出，请重新打开 Hive；未收到结果的操作请先核对执行情况。",
        "后台正在关闭或已退出",
        "后台写入线程已停止",
        "后台响应通道已关闭",
        "工作区模型查询未正常结束，无法确认其原生进程已经退出。",
        "Agent 未返回有效的用量对象。",
        "Agent 返回的 Token 计数无效。",
        "Agent 返回的上下文窗口无效。",
        "Agent 返回的上下文比例无效。",
        "此 Claude SDK 没有原生配置读取能力，请更新已配置的版本。",
        "Claude 未返回有效的会话模型和推理设置。",
        "当前 Claude 模型不支持此推理强度。",
        "Codex 未返回有效的推理强度。",
        "当前 Codex 模型不支持此推理强度。",
        "外部 Pi 未返回有效的推理强度信息。",
        "当前外部 Pi 模型未提供图片输入能力。",
        "当前模型不支持此推理强度。",
        "当前 Pi 模型不支持图片输入。",
        "原先的 Pi 模型不可用，请重新选择。",
        "搜索内容需为 200 个字符以内的文本。",
        "会话名称需为 1–120 个字符，且不能包含换行或控制字符。",
        "会话仍有执行或待处理消息，请处理完成后再归档。",
        "成员仍有执行或未结派单，请处理完成后再归档会话。",
        "会话刚刚更新，请核对新内容后重新归档。",
        "无法保存归档状态，历史与草稿仍保留，请稍后重试。",
        "保存的推理档位已不在当前原生模型能力中，请重新选择。",
        "用户附件来源与投递内容不匹配。",
        "不触发模型回合的投递暂不支持图片。",
        "当前主会话仍有执行或未结派单，请先完成或停止相关任务，再切换主会话。",
        "请先为内置 Pi 选择可用模型，再发送消息。",
        "当前模型不支持图片，请切换模型后重试。",
        "请在当前回复结束后刷新模型",
        "推理强度必须是文本。",
        "此后端尚未接入推理强度设置。",
        "请在当前回复结束后调整推理强度。",
        "原生会话未提供可保存推理设置的模型身份。",
        "工作区会话未正常打开，无法确认其清理完成。",
        "未能确认工作区中的所有成员运行已经退出。",
        "工作区会话清理失败。",
        "工作区的工作流仍在启动、运行或清理中；请等待其实际执行与清理结束后再移除。",
        "工作区正在移除，暂时不能执行此操作。",
        "附件标识必须为最多 10 个不同的上传 ID。",
        "附件元数据无效。",
        "附件元数据不是规范格式。",
        "请选择本地文件的绝对路径。",
        "所选路径不是普通文件。",
        "单个文件不能超过 100 MB。",
        "文件在读取时变大，请保存文件后重新选择。",
        "图片文件内容与格式不匹配，请检查文件后重试。",
        "文件保存后无法定位，请重新选择。",
        "附件不存在或不属于当前工作区，请重新选择。",
        "附件存储未提供有效的本地路径。",
        "图片附件无法读取，请重新选择。",
        "请使用图片查看器查看图片。",
        "图片列表最多包含 10 项。",
        "图片需包含类型、MIME 与 base64 内容。",
        "单张图片的 base64 最多 5 MB，单次图片合计最多 20 MB。",
        "图片内容不是规范的 base64。",
        "图片格式与内容不匹配，仅支持 PNG、JPEG、GIF、WebP。",
        "ID 不能为空或包含控制字符",
        "提供商 models 字段应为数组",
        "模型缺少有效 ID，请先修复配置",
        "提供商包含重复模型 ID，请先修复配置",
        "模型配置正在关闭",
        "请先读取当前配置",
        "新提供商 ID 请使用字母、数字、点、下划线或连字符",
        "新接口需要类型、地址和首个模型 ID",
        "请选择支持的接口类型",
        "请输入有效的 HTTP 或 HTTPS 接口地址",
        "接口地址需要 HTTP 或 HTTPS，且不能包含用户名或密码",
        "提供商配置已不存在，请重新读取",
        "模型配置已不存在，请重新读取",
        "配置校验不能修改凭据",
        "配置校验不能删除凭据",
        "配置应为 JSON 对象",
        "配置包含重复字段，请先在 Pi 配置文件中消除重复项",
        "models.json 无法解析，原文件未改动",
        "models.json 的链接目标不存在，请先修复链接",
        "models.json 不是普通文件",
        "配置未通过 Pi 校验，未保存。请检查接口类型、地址、模型参数以及现有高级配置。",
    ];
    if let Some(key) = OWNED_ERRORS.iter().find(|key| **key == error) {
        return tr(key).to_owned();
    }
    const OWNED_PREFIXES: &[(&str, &str)] = &[
        ("无法读取工作区：", "无法读取工作区"),
        ("文件位于工作区之外：", "文件位于工作区之外"),
        ("无法读取图片：", "无法读取图片"),
        ("运行列表格式错误：", "运行列表格式错误"),
        ("会话列表格式错误：", "会话列表格式错误"),
        ("工作区列表格式错误：", "工作区列表格式错误"),
        ("停止未确认：", "停止未确认"),
        ("无法读取应用路径：", "无法读取应用路径"),
        ("无法设置后台 PATH：", "无法设置后台 PATH"),
        ("启动/监督 Node 后台失败：", "启动/监督 Node 后台失败"),
        ("写入后台失败：", "写入后台失败"),
        ("后台协议错误：", "后台协议错误"),
        ("终端事件格式错误：", "终端事件格式错误"),
        ("Agent 事件格式错误：", "Agent 事件格式错误"),
        ("无法检查后台进程状态：", "无法检查后台进程状态"),
        ("后台进程清理未确认：", "后台进程清理未确认"),
        ("找不到已保存的 Pi 会话文件。恢复文件后可重新打开会话：", "找不到已保存的 Pi 会话文件。恢复文件后可重新打开会话"),
        ("原推理设置未能恢复，请重新选择强度后发送：", "原推理设置未能恢复，请重新选择强度后发送"),
        ("原生会话设置更新失败，会话已暂停；修复问题后可重新读取并重试。原因：", "原生会话设置更新失败，会话已暂停；修复问题后可重新读取并重试。原因"),
        ("Pi 会话变更未能保存，请修复问题后重试：", "Pi 会话变更未能保存，请修复问题后重试"),
        ("工作区已移除，但桌面记录或后续清理未完成：", "工作区已移除，但桌面记录或后续清理未完成"),
        ("移除失败，暂时无法确认工作区是否仍存在；请刷新工作区列表：", "移除失败，暂时无法确认工作区是否仍存在；请刷新工作区列表"),
        ("工作区尚未移除，部分会话可能已停止；原有会话记录和待投递数据保留：", "工作区尚未移除，部分会话可能已停止；原有会话记录和待投递数据保留"),
    ];
    for (prefix, key) in OWNED_PREFIXES {
        if let Some(rest) = error.strip_prefix(prefix) {
            return format!("{}: {}", tr(key), localize_owned_error(rest));
        }
    }
    if let Some((reported, cause)) = error
        .strip_prefix("无法定位文件 ")
        .and_then(|rest| rest.split_once('：'))
    {
        return format!(
            "Could not locate file {reported}: {}",
            localize_owned_error(cause)
        );
    }
    if let Some(path) = error
        .strip_prefix("找不到后台 ")
        .and_then(|rest| rest.strip_suffix("，请重新安装应用；源码运行请先执行 pnpm build:runtime"))
    {
        return format!(
            "Runtime {path} was not found. Reinstall the app, or run pnpm build:runtime from source."
        );
    }
    if let Some(path) = error
        .strip_prefix("找不到应用内的 Node ")
        .and_then(|rest| rest.strip_suffix("，请重新安装应用"))
    {
        return format!("Bundled Node {path} was not found. Reinstall the app.");
    }
    if let Some(offset) = error
        .strip_prefix("models.json 无法解析（字符位置 ")
        .and_then(|rest| rest.strip_suffix("），原文件未改动"))
    {
        return format!(
            "Could not parse models.json (character offset {offset}); the original file was not changed"
        );
    }
    error.to_owned()
}
fn load(root: &Path) -> Result<Locale, String> {
    match fs::File::open(root.join("desktop-language.json")) {
        Ok(file) => serde_json::from_reader(file).map_err(|e| e.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Locale::default()),
        Err(error) => Err(error.to_string()),
    }
}
fn save(root: &Path, locale: Locale) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    crate::file_save::save_file(
        &root.join("desktop-language.json"),
        &serde_json::to_vec(&locale).map_err(|e| e.to_string())?,
    )
}
pub fn restore(root: &Path, cx: &mut App) {
    if cx.global::<Language>().root.is_some() {
        return;
    }
    let result = load(root);
    let state = cx.global_mut::<Language>();
    state.root = Some(root.to_owned());
    match result {
        Ok(locale) => state.locale = locale,
        Err(error) => state.notice = Some(Notice::LoadedError(error)),
    }
    component::set_locale(state.locale.component_locale());
    cx.refresh_windows();
}
pub fn choose(locale: Locale, cx: &mut App) -> bool {
    let Some(root) = cx.global::<Language>().root.clone() else {
        return false;
    };
    match save(&root, locale) {
        Ok(()) => {
            let state = cx.global_mut::<Language>();
            state.locale = locale;
            state.notice = Some(Notice::Saved);
            component::set_locale(locale.component_locale());
            cx.refresh_windows();
            true
        }
        Err(error) => {
            cx.global_mut::<Language>().notice = Some(Notice::SaveError(error));
            cx.refresh_windows();
            false
        }
    }
}
pub fn available(cx: &App) -> bool {
    cx.global::<Language>().root.is_some()
}
pub fn message(cx: &App) -> Option<String> {
    cx.global::<Language>()
        .notice
        .as_ref()
        .map(|notice| match notice {
            Notice::Saved => tr("语言已保存").to_owned(),
            Notice::LoadedError(error) => {
                format!("{}: {}", tr("读取语言设置失败"), localize_owned_error(error))
            }
            Notice::SaveError(error) => {
                format!("{}: {}", tr("保存语言设置失败"), localize_owned_error(error))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_preferences_reopen_and_preserve_invalid_or_failed_targets() {
        let previous = component::locale().to_string();
        let connected = crate::app::Status::from("后台已连接");
        let disconnected = crate::app::Status::from("后台连接已断开");
        let notice = crate::app::ErrorNotice::from("后台尚未连接");
        let original = "native diagnostic: 中文\nC:\\work\\file";
        let raw = crate::app::ErrorNotice::from(original.to_owned());
        let owned = crate::app::ErrorNotice::from("请先为内置 Pi 选择可用模型，再发送消息。".to_owned());
        let prefixed = crate::app::ErrorNotice::from(format!("写入后台失败：{original}"));
        let explained = crate::app::ErrorNotice::Context("未能确认后台进程已退出",Box::new(original.to_owned().into()),"请重试退出；当前不会启动新的后台。");
        let batch = crate::app::ErrorNotice::Batch(vec!["后台尚未连接".into(),original.to_owned().into()]);
        let report = crate::app::DeliveryCard { dispatch_id:None,report:true,worker_id:None,artifacts:vec![],heading:"成员汇报",worker_name:Some("成员汇报".to_owned()),raw:original.to_owned(),expanded:false,missing_provenance:false };
        component::set_locale(Locale::En.component_locale());
        assert_eq!(tr("设置"), "Settings");
        assert_eq!(connected.text(), "Backend connected");
        assert_eq!(notice.text(), "Backend not connected");
        assert_eq!(owned.text(), "Select an available model for built-in Pi before sending a message.");
        assert_eq!(prefixed.text(), format!("Could not write to runtime: {original}"));
        assert_eq!(explained.text(),format!("Could not confirm the runtime exited: {original}\nRetry quitting. No new runtime will be started while cleanup is unconfirmed."));
        assert_eq!(batch.text(),format!("Backend not connected\n{original}"));
        assert_eq!(report.heading_text(), "成员汇报 · Worker report");
        assert_eq!(report.raw, original);
        assert!(connected.is_background_connected());
        assert!(!disconnected.is_background_connected());
        assert_eq!(raw.text(), original);
        component::set_locale(Locale::Zh.component_locale());
        assert_eq!(tr("设置"), "设置");
        assert_eq!(connected.text(), "后台已连接");
        assert_eq!(notice.text(), "后台尚未连接");
        assert_eq!(owned.text(), "请先为内置 Pi 选择可用模型，再发送消息。");
        assert_eq!(prefixed.text(), format!("写入后台失败：{original}"));
        assert_eq!(explained.text(),format!("未能确认后台进程已退出: {original}\n请重试退出；当前不会启动新的后台。"));
        assert_eq!(batch.text(),format!("后台尚未连接\n{original}"));
        assert_eq!(report.heading_text(), "成员汇报 · 成员汇报");
        assert_eq!(report.raw, original);
        assert!(connected.is_background_connected());
        assert!(!disconnected.is_background_connected());
        assert_eq!(raw.text(), original);
        component::set_locale(&previous);
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap(), Locale::Zh);
        for locale in [Locale::En, Locale::Zh, Locale::En] {
            save(root.path(), locale).unwrap();
            assert_eq!(load(root.path()).unwrap(), locale);
        }
        let file = root.path().join("desktop-language.json");
        fs::write(&file, b"\"unsupported\"").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"\"unsupported\"");
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        fs::write(file.join("preserved"), b"owned target").unwrap();
        assert!(save(root.path(), Locale::En).is_err());
        assert_eq!(fs::read(file.join("preserved")).unwrap(), b"owned target");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
