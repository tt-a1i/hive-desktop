use crate::{language::tr, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    *,
};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

const MARKER: &str = "desktop-inbox-guide-seen";

#[derive(Clone, Default)]
pub(super) enum IntroProgress {
    #[default]
    Unseen,
    WaitingForDirectory,
    SeenIn(PathBuf),
}

impl IntroProgress {
    pub(super) fn should_show(&self, root: Option<&Path>) -> bool {
        match (self, root) {
            (Self::WaitingForDirectory, _) => false,
            (Self::SeenIn(seen), Some(root)) if seen == root => false,
            (_, Some(root)) => !root.join(MARKER).is_file(),
            _ => true,
        }
    }

    pub(super) fn acknowledge(&mut self, root: Option<&Path>) -> io::Result<()> {
        *self = Self::WaitingForDirectory;
        self.save_pending(root)
    }

    pub(super) fn save_pending(&mut self, root: Option<&Path>) -> io::Result<()> {
        if let (Self::WaitingForDirectory, Some(root)) = (&*self, root) {
            let result = write_marker(root);
            *self = Self::SeenIn(root.to_owned());
            result?;
        }
        Ok(())
    }
}

fn write_marker(root: &Path) -> io::Result<()> {
    let marker = root.join(MARKER);
    if marker.is_file() {
        return Ok(());
    }
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    file.write_all(b"1\n")?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(&marker) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists && marker.is_file() => {
            Ok(())
        }
        Err(error) => Err(error.error),
    }
}

#[derive(Clone, Copy)]
pub(super) enum IntroMode {
    FirstUse,
    Replay,
}

pub(super) fn guide(
    focus: &FocusHandle,
    on_start: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    v_flex()
        .min_w_0()
        .gap_4()
        .py_2()
        .child(
            div()
                .id("inbox-guide-heading")
                .role(Role::Heading)
                .aria_label(tr("了解收件箱"))
                .text_lg()
                .font_semibold()
                .child(tr("了解收件箱")),
        )
        .children(
            [
                (
                    0usize,
                    "需要你关注",
                    "查看等待你回复的问题、未读报告和需要处理的问题。",
                ),
                (
                    1,
                    "暂缓事项",
                    "稍后再处理时，选择恢复时间。暂缓期间有新的问题或报告时，事项会提前恢复。",
                ),
                (
                    2,
                    "查看新事项",
                    "收件箱上的绿点表示有新事项。选择“查看新事项”，即可跨项目查看它们。",
                ),
            ]
            .map(|(id, title, description)| {
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .id(("inbox-guide-point", id))
                            .role(Role::Heading)
                            .aria_label(tr(title))
                            .text_sm()
                            .font_semibold()
                            .child(tr(title)),
                    )
                    .child(
                        div()
                            .id(("inbox-guide-description", id))
                            .role(Role::Label)
                            .aria_label(tr(description))
                            .text_sm()
                            .text_color(Color::Muted.get(cx))
                            .child(tr(description)),
                    )
            }),
        )
        .child(
            h_flex().justify_end().child(
                Button::new("inbox-guide-start")
                    .primary()
                    .small()
                    .label(tr("开始使用"))
                    .track_focus(focus)
                    .on_click(on_start),
            ),
        )
}

#[cfg(test)]
mod tests {
    use super::{IntroProgress, MARKER};

    #[test]
    fn inbox_guide_completion_survives_reopen_only_for_that_data_directory() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut state = IntroProgress::default();
        assert!(state.should_show(Some(first.path())));
        assert!(!first.path().join(MARKER).exists());

        state.acknowledge(Some(first.path())).unwrap();
        assert!(!state.should_show(Some(first.path())));
        assert!(!IntroProgress::default().should_show(Some(first.path())));
        assert!(state.should_show(Some(second.path())));
        assert!(IntroProgress::default().should_show(Some(second.path())));
    }

    #[test]
    fn inbox_guide_keeps_early_completion_until_the_data_directory_arrives() {
        let root = tempfile::tempdir().unwrap();
        let mut state = IntroProgress::default();
        assert!(state.should_show(None));
        state.acknowledge(None).unwrap();
        assert!(!state.should_show(None));
        state.save_pending(Some(root.path())).unwrap();
        assert!(!IntroProgress::default().should_show(Some(root.path())));
    }
}
