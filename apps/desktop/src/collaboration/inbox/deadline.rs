use super::{InboxBadge, Snoozes, now_ms};
use crate::app::HiveApp;
use gpui_kit::{Context, Task};
use std::{collections::HashMap, io, path::PathBuf, time::Duration};

const RETRY_MS: u64 = 2_000;
const CLOCK_CHECK_MS: u64 = 30_000;

#[derive(Default)]
pub(super) struct DeadlineWake {
    root: Option<PathBuf>,
    scheduled: Option<u64>,
    generation: u64,
    task: Option<Task<()>>,
    retry: HashMap<String, u64>,
}

fn next_attempt(saved: &Snoozes, retry: Option<&u64>) -> Option<u64> {
    saved
        .next_expiry()
        .map(|time| time.max(retry.copied().unwrap_or(0)))
}

impl HiveApp {
    pub(in crate::collaboration::inbox) fn sync_inbox_deadlines(&mut self, cx: &mut Context<Self>) {
        let badge = &mut self.collaboration.inbox_badge;
        if badge.wake.root != self.data_dir {
            badge.wake = DeadlineWake {
                root: self.data_dir.clone(),
                generation: badge.wake.generation.wrapping_add(1),
                ..DeadlineWake::default()
            };
            badge.reminders.clear();
        }

        let next = if let Some(root) = &self.data_dir
            && !self.closing
        {
            let available = |id: &str| self.workspaces.iter().any(|workspace| workspace.id == id);
            badge.reminders.retain(|id, _| available(id));
            badge.wake.retry.retain(|id, _| available(id));
            for workspace in &self.workspaces {
                if !badge.reminders.contains_key(&workspace.id)
                    && let Ok(saved) = Snoozes::load(root, &workspace.id)
                {
                    badge.reminders.insert(workspace.id.clone(), saved);
                }
            }
            badge
                .reminders
                .iter()
                .filter(|(id, _)| self.workspace_removing.as_ref() != Some(*id))
                .filter_map(|(id, saved)| next_attempt(saved, badge.wake.retry.get(id)))
                .min()
        } else {
            None
        };
        if badge.wake.scheduled == next {
            return;
        }
        badge.wake.scheduled = next;
        badge.wake.generation = badge.wake.generation.wrapping_add(1);
        let generation = badge.wake.generation;
        badge.wake.task = next.map(|time| {
            cx.spawn(async move |this, cx| {
                let ms = time.saturating_sub(now_ms()).min(CLOCK_CHECK_MS);
                smol::Timer::after(Duration::from_millis(ms)).await;
                let _ = this.update(cx, |app, cx| app.wake_inbox_deadlines(generation, cx));
            })
        });
    }

    fn wake_inbox_deadlines(&mut self, generation: u64, cx: &mut Context<Self>) {
        let badge = &mut self.collaboration.inbox_badge;
        if badge.wake.generation != generation || badge.wake.root != self.data_dir || self.closing {
            return;
        }
        badge.wake.scheduled = None;
        let Some(root) = self.data_dir.clone() else {
            return;
        };
        let now = now_ms();
        let due: Vec<String> = badge
            .reminders
            .iter()
            .filter(|(id, saved)| {
                self.workspace_removing.as_ref() != Some(*id)
                    && self.workspaces.iter().any(|workspace| &workspace.id == *id)
                    && next_attempt(saved, badge.wake.retry.get(*id)).is_some_and(|at| at <= now)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for workspace in due.iter() {
            let outcome = Snoozes::expire(&root, workspace, now);
            self.publish_inbox_deadline(workspace, outcome.as_ref(), now, cx);
        }
        if !due.is_empty() {
            cx.notify();
        }
        self.sync_inbox_deadlines(cx);
    }

    fn publish_inbox_deadline(
        &mut self,
        workspace: &str,
        outcome: Result<&Snoozes, &io::Error>,
        now: u64,
        cx: &mut Context<Self>,
    ) {
        let badge: &mut InboxBadge = &mut self.collaboration.inbox_badge;
        match outcome {
            Ok(saved) => {
                badge.remember_reminders(workspace, saved);
                badge.wake.retry.remove(workspace);
            }
            Err(_) => {
                badge
                    .wake
                    .retry
                    .insert(workspace.to_owned(), now.saturating_add(RETRY_MS));
            }
        }
        if self.collaboration.inbox.workspace.as_deref() == Some(workspace) {
            self.collaboration.inbox.reminders.apply_expiry(outcome);
        }
        if let Some(dialog) = badge.dialog.clone() {
            let _ = dialog.update(cx, |dialog, cx| {
                dialog.apply_local_expiry(workspace, outcome, self, cx)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::next_attempt;
    use crate::collaboration::inbox::snoozes::{Reminder, Snoozes};

    #[test]
    fn deadline_retry_does_not_replace_another_workspaces_exact_deadline() {
        let root = tempfile::tempdir().unwrap();
        let first = "a3b2cb99-3959-4adc-a770-b70f9d2a4cb9";
        let second = "c7761168-b30c-4020-80bb-23c1c357e111";
        let save = |workspace, expires_at| {
            Snoozes::set(
                root.path(),
                workspace,
                "dispatch",
                Some(Reminder {
                    expires_at,
                    ..Reminder::default()
                }),
            )
            .unwrap()
        };
        let overdue = save(first, Some(20));
        let another = save(second, Some(30));
        let next = |a: &Snoozes, b: &Snoozes, retry| {
            next_attempt(a, retry)
                .into_iter()
                .chain(next_attempt(b, None))
                .min()
        };
        assert_eq!(next(&overdue, &another, None), Some(20));
        assert_eq!(next(&overdue, &another, Some(&40)), Some(30));
        assert_eq!(next(&save(first, Some(45)), &another, Some(&40)), Some(30));
        assert_eq!(
            next(&save(first, Some(45)), &save(second, None), None),
            Some(45)
        );
    }
}
