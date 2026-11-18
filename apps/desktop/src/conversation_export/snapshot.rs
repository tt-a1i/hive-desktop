use crate::language::{english,tr};
use crate::{
    app::HiveApp,
    protocol::{Session, Workspace},
};
use std::io::{self, Write};

const PREVIEW_CHARS: usize = 10_000;

pub(super) struct Preview {
    pub text: String,
    pub truncated: bool,
}

struct PreviewWriter {
    remaining: usize,
    text: String,
}

impl Write for PreviewWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // The formatter writes complete UTF-8 strings. Count their leading bytes
        // so even a single large message is only decoded up to the preview limit.
        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 0b1100_0000 == 0b1000_0000 {
                continue;
            }
            if self.remaining == 0 {
                self.text.push_str(std::str::from_utf8(&bytes[..index]).unwrap());
                return Err(io::ErrorKind::WriteZero.into());
            }
            self.remaining -= 1;
        }
        self.text.push_str(std::str::from_utf8(bytes).unwrap());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) struct Snapshot {
    pub title: String,
    workspace: String,
    backend: String,
    session: String,
    running: bool,
    english: bool,
    entries: Vec<Entry>,
}
struct Entry {
    heading: String,
    body: String,
    thinking: Vec<String>,
    tool: bool,
    input: Option<String>,
    patch: Option<String>,
    files: Vec<String>,
    failure: Option<String>,
}
impl Snapshot {
    pub fn capture(app: &HiveApp, session: &Session, workspace: Option<&Workspace>) -> Self {
        Self {
            title: app.session_title(session).to_owned(),
            workspace: workspace
                .map(|item| item.name.clone())
                .unwrap_or_else(|| tr("未提供").into()),
            backend: tr(session.backend.label()).into(),
            session: session.id.clone(),
            running: app.busy.contains(&session.id),
            english: english(),
            entries: app
                .messages
                .iter()
                .map(|message| Entry {
                    heading: if let Some(tool) = &message.tool {
                        format!("{} · {} · {}",tr("工具"),tool.name,tr(tool.status))
                    } else if let Some(delivery) = &message.delivery {
                        delivery.heading_text()
                    } else {
                        match message.role.as_str() {
                            "user" => tr("用户"),
                            "assistant" => "Agent",
                            "system" => tr("会话说明"),
                            _ => &message.role,
                        }
                        .to_owned()
                    },
                    body: message.display_body().to_owned(),
                    thinking: message
                        .thinking
                        .iter()
                        .map(|block| {
                            if block.redacted {
                                format!("[{}]",tr("原生后端未提供此段思考正文"))
                            } else if block.interrupted {
                                format!("[{}]\n{}",tr("已中断"),block.text)
                            } else if block.running {
                                format!("[{}]\n{}",tr("进行中"),block.text)
                            } else {
                                block.text.clone()
                            }
                        })
                        .collect(),
                    tool: message.tool.is_some(),
                    input: message
                        .tool
                        .as_ref()
                        .and_then(|_| app.tool_inputs.get(&message.id))
                        .filter(|input| !input.is_null())
                        .map(|input| serde_json::to_string_pretty(input).unwrap()),
                    patch: message.tool.as_ref().and_then(|tool| tool.patch.clone()),
                    files: {
                        let mut files: Vec<_> = message
                            .files
                            .iter()
                            .map(|file| {
                                format!(
                                    "{} · {} · {} · {} {}",
                                    file.name,
                                    file.mime_type,
                                    file.size_label(),tr("附件编号"),
                                    file.id
                                )
                            })
                            .collect();
                        if let Some(delivery) = &message.delivery {
                            files.extend(
                                delivery
                                    .artifacts
                                    .iter()
                                    .map(|path| format!("{}: {path}",tr("汇报产物"))),
                            );
                        }
                        let images = message.history_images.as_ref().map_or_else(
                            || message.image_sources.len().max(message.images.len()),
                            |images| images.count,
                        );
                        if images > 0 {
                            files.push(if english() {format!("{images} native images · filenames unavailable in this chat projection · image data excluded")} else {format!("原生图片 {images} 张 · 此聊天投影未提供文件名 · 不含图片本体")});
                        } else if message.images_loading {
                            files.push(tr("原生图片预览读取中 · 不含图片本体").into());
                        } else if message.images_error.is_some() {
                            files.push(tr("原生图片预览不可用 · 不含图片本体").into());
                        }
                        files
                    },
                    failure: message
                        .failure
                        .as_ref()
                        .map(|failure| failure.display_message().into_owned()),
                })
                .collect(),
        }
    }
    pub fn filename(&self) -> String {
        let title: String = self
            .title
            .chars()
            .filter(|c| !c.is_control())
            .map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c })
            .take(60)
            .collect();
        let title = title
            .trim()
            .trim_end_matches(|c: char| c == '.' || c.is_whitespace());
        format!("Hive-{}.md", if title.is_empty() { if self.english { "conversation" } else { "对话" } } else { title })
    }
    pub fn count(&self) -> usize {
        self.entries.len()
    }
    pub fn preview(&self, thinking: bool, tools: bool) -> Preview {
        let mut writer = PreviewWriter {
            remaining: PREVIEW_CHARS,
            text: String::new(),
        };
        let truncated = match self.write_markdown(&mut writer, thinking, tools) {
            Ok(()) => false,
            Err(error) if error.kind() == io::ErrorKind::WriteZero => true,
            Err(error) => unreachable!("Preview writer failed: {error}"),
        };
        Preview {
            text: writer.text,
            truncated,
        }
    }

    pub fn markdown(&self, thinking: bool, tools: bool) -> String {
        let mut bytes = Vec::new();
        self.write_markdown(&mut bytes, thinking, tools)
            .expect("Writing to a vector cannot fail");
        String::from_utf8(bytes).expect("Markdown is written in UTF-8 chunks")
    }

    pub fn write_markdown(
        &self,
        out: &mut impl Write,
        thinking: bool,
        tools: bool,
    ) -> io::Result<()> {
        if self.english {
            writeln!(out, "# {}\n\nWorkspace: {}\n\nBackend: {}\n\nSession ID: {}\n\n> Snapshot of the loaded branch when the export window opened. Attachments include names and metadata; images include available counts. File data, unsent drafts and hidden system context are excluded.", self.title, self.workspace, self.backend, self.session)?;
        } else {
            writeln!(out,
            "# {}\n\n工作区：{}\n\n后端：{}\n\n会话编号：{}\n\n> 打开导出窗口时的已加载分支快照。普通附件仅记录名称与元数据，图片仅记录可用的数量，不含文件本体；未发送草稿和隐藏系统上下文不在此文档中。",
            self.title, self.workspace, self.backend, self.session
        )?;
        }
        if self.running {
            out.write_all(if self.english {b"\n> The Agent was responding when captured. The current turn may be incomplete.\n"} else {"\n> 捕获时 Agent 正在回复，当前回合可能尚未完整。\n".as_bytes()})?;
        }
        for entry in &self.entries {
            if entry.tool && !tools {
                continue;
            }
            if !entry.tool
                && entry.body.trim().is_empty()
                && entry.files.is_empty()
                && entry.failure.is_none()
                && (!thinking || entry.thinking.is_empty())
            {
                continue;
            }
            write!(out, "\n---\n\n## {}\n\n", entry.heading)?;
            if thinking {
                for text in &entry.thinking {
                    out.write_all(if self.english { "### Thinking\n\n" } else { "### 思考过程\n\n" }.as_bytes())?;
                    fenced(out, text, "text")?;
                }
            }
            if entry.tool {
                if let Some(input) = &entry.input {
                    out.write_all(if self.english { "### Input\n\n" } else { "### 输入\n\n" }.as_bytes())?;
                    fenced(out, input, "json")?;
                }
                out.write_all(if self.english { "### Result\n\n" } else { "### 结果\n\n" }.as_bytes())?;
                fenced(out, &entry.body, "text")?;
                if let Some(patch) = &entry.patch {
                    out.write_all(if self.english { "### Patch\n\n" } else { "### 补丁\n\n" }.as_bytes())?;
                    fenced(out, patch, "diff")?;
                }
            } else {
                write!(out, "{}\n\n", entry.body)?;
            }
            if !entry.files.is_empty() {
                out.write_all(if self.english { "### Attachments\n\n" } else { "### 附件\n\n" }.as_bytes())?;
                for file in &entry.files {
                    fenced(out, file, "text")?;
                }
            }
            if let Some(failure) = &entry.failure {
                out.write_all(if self.english { "### Diagnostics\n\n" } else { "### 诊断\n\n" }.as_bytes())?;
                fenced(out, failure, "text")?;
            }
        }
        Ok(())
    }
}
fn fenced(out: &mut impl Write, text: &str, language: &str) -> io::Result<()> {
    let width = text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
        .max(2)
        + 1;
    // Formatting uses bounded chunks even when a long run of backticks is present.
    write!(out, "{:`>width$}{language}\n{text}\n{:`>width$}\n\n", "", "")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_preserves_selected_content_and_fences_tool_output() {
        let snapshot = Snapshot {
            title: "讨论/中文".into(),
            workspace: "项目".into(),
            backend: "Pi".into(),
            session: "actual-session".into(),
            running: true,
            english: false,
            entries: vec![
                Entry {
                    heading: "用户".into(),
                    body: "第一行\n第二行".into(),
                    thinking: vec!["原生思考".into()],
                    tool: false,
                    input: None,
                    patch: None,
                    files: vec!["说明.md · text/markdown · 12 B".into()],
                    failure: None,
                },
                Entry {
                    heading: "工具 · bash · 失败".into(),
                    body: "result\n```\nend".into(),
                    thinking: vec![],
                    tool: true,
                    input: Some("{\"command\":\"实际命令\"}".into()),
                    patch: Some("-old\n+new".into()),
                    files: vec![],
                    failure: Some("实际失败".into()),
                },
            ],
        };
        let body = snapshot.markdown(false, false);
        assert!(body.contains("第一行\n第二行"));
        assert!(body.contains("说明.md"));
        assert!(body.contains("Agent 正在回复"));
        assert!(!body.contains("原生思考"));
        assert!(!body.contains("实际命令"));
        let tools = snapshot.markdown(false, true);
        assert!(tools.contains("实际命令"));
        assert!(!tools.contains("原生思考"));
        let thinking = snapshot.markdown(true, false);
        assert!(thinking.contains("原生思考"));
        assert!(!thinking.contains("实际命令"));
        let full = snapshot.markdown(true, true);
        assert!(full.contains("原生思考"));
        assert!(full.contains("实际命令"));
        assert!(full.contains("````text\nresult\n```\nend\n````"));
        assert!(full.contains("-old\n+new"));
        assert!(full.contains("实际失败"));
        let preview = snapshot.preview(true, true);
        assert!(!preview.truncated);
        assert_eq!(preview.text, full);
        assert_eq!(snapshot.filename(), "Hive-讨论_中文.md");
        let english_snapshot = Snapshot { english: true, ..snapshot };
        let english_full = english_snapshot.markdown(true,true);
        assert!(english_full.contains("Workspace: 项目"));
        assert!(english_full.contains("### Input\n\n"));
        assert!(english_full.contains("### Thinking\n\n"));
        assert!(english_full.contains("The Agent was responding when captured"));
        assert!(english_full.contains("第一行\n第二行"));
        assert!(english_full.contains("实际命令"));
        assert!(english_full.contains("````text\nresult\n```\nend\n````"));
        assert!(english_full.contains("-old\n+new"));
        assert!(english_full.contains("实际失败"));
        let reserved = Snapshot {
            title: "CON. .  ".into(),
            entries: vec![],
            ..english_snapshot
        };
        assert_eq!(reserved.filename(), "Hive-CON.md");
    }

    #[test]
    fn preview_stops_at_a_complete_unicode_character_and_keeps_the_export_complete() {
        let mut snapshot = Snapshot {
            title: "Export".into(),
            workspace: "Project".into(),
            backend: "Codex".into(),
            session: "test-session".into(),
            running: false,
            english: true,
            entries: vec![Entry {
                heading: "Agent".into(),
                body: "文🦀".repeat(PREVIEW_CHARS),
                thinking: Vec::new(),
                tool: false,
                input: None,
                patch: None,
                files: Vec::new(),
                failure: None,
            }],
        };
        snapshot.entries[0].body.push_str("END");
        let full = snapshot.markdown(false, false);
        let expected: String = full.chars().take(PREVIEW_CHARS).collect();
        let preview = snapshot.preview(false, false);
        assert!(preview.truncated);
        assert_eq!(preview.text, expected);
        assert!(full.ends_with("END\n\n"));

        let excess = full.chars().count() - PREVIEW_CHARS;
        let end = snapshot.entries[0].body.char_indices().rev().nth(excess - 1).unwrap().0;
        snapshot.entries[0].body.truncate(end);
        let preview = snapshot.preview(false, false);
        assert!(!preview.truncated);
        assert_eq!(preview.text.chars().count(), PREVIEW_CHARS);
    }
}
