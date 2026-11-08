use super::*;

impl HiveApp {
    pub fn start_backend(&mut self, settings_window: AnyWindowHandle, cx: &mut Context<Self>) {
        let connection = self.connection_generation;
        self.events = cx.spawn(async move |this, cx| {
            let runtime = match smol::unblock(Runtime::launch).await {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if this.connection_generation != connection {
                            return;
                        }
                        this.loading = false;
                        this.restarting = false;
                        this.error = Some(error);
                        if this.closing {
                            cx.quit();
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let adopted = this.update(cx, |this, cx| {
                this.runtime = Some(runtime.clone());
                if this.closing {
                    this.shutdown_and_quit(cx);
                    return false;
                }
                cx.notify();
                true
            });
            match adopted {
                Ok(true) => (),
                Ok(false) => return,
                Err(_) => {
                    runtime.shutdown().await;
                    return;
                }
            }
            let events = runtime.events.clone();
            let result = runtime.request("initialize", json!({})).await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.runtime = Some(runtime);
                this.loading = false;
                this.restarting = false;
                if this.closing {
                    this.shutdown_and_quit(cx);
                    return;
                }
                match result {
                    Ok(info) => {
                        if let Some(root) = info["dataDir"].as_str() {
                            crate::language::restore(std::path::Path::new(root), cx);
                            crate::appearance::restore(std::path::Path::new(root), cx);
                            crate::composer::restore(std::path::Path::new(root), cx);
                        }
                        this.remote.commands = info["remoteCommands"].clone();
                        this.pi_agent_dir =
                            info["piAgentDir"].as_str().map(std::path::PathBuf::from);
                        if !this.backend_initialized
                            && let Some(value) = info["navigation"].as_str()
                        {
                            match serde_json::from_str(value) {
                                Ok(selection) => this.navigation = selection,
                                Err(error) => {
                                    this.error = Some(format!("无法恢复上次的会话位置：{error}"))
                                }
                            }
                        }
                        if !this.backend_initialized
                            && let Some(root) = info["dataDir"].as_str()
                            && let Err(error) = this.restore_drafts(std::path::Path::new(root))
                        {
                            this.error =
                                Some(format!("无法读取本地草稿，本次编辑仅在窗口中保留：{error}"));
                        }
                        let recovering = this.backend_initialized;
                        this.backend_initialized = true;
                        this.connected = true;
                        this.status = "后台已连接".into();
                        this.load_workspaces(cx);
                        if recovering {
                            this.restore_backend_view(settings_window, cx);
                        }
                        if this.settings_panel {
                            this.load_preferences(settings_window, cx);
                        }
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
            // This task holds no Runtime Arc while awaiting notifications, allowing app drop to shut it down.
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| {
                        if this.connection_generation != connection {
                            return;
                        }
                        match event {
                            RuntimeEvent::Failed(error) => {
                                this.connected = false;
                                this.reset_pi_accounts(settings_window, cx);
                                this.pi_models.reset_connection();
                                this.terminal_disconnected(cx);
                                this.loading = false;
                                this.interactions.sessions.clear();
                                this.status = "后台连接已断开".into();
                                this.error = Some(error);
                                this.busy.clear();
                            }
                            RuntimeEvent::Agent(event) => this.agent_event(event, cx),
                            RuntimeEvent::Terminal(event) => this.terminal_wire_event(event, cx),
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
    }
}

impl HiveApp {
    pub fn restart_backend(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.connected || self.restarting || self.closing || self.loading {
            return;
        }
        let previous_failed = self.failed_drafts.clone();
        for (id, text) in &self.inflight_inputs {
            self.failed_drafts
                .entry(id.clone())
                .or_default()
                .push(text.clone());
        }
        if let Err(error) = self.flush_drafts(cx) {
            self.failed_drafts = previous_failed;
            self.error = Some(format!("无法保存草稿，请复制内容后重试：{error}"));
            cx.notify();
            return;
        }
        self.inflight_inputs.clear();
        window.close_all_dialogs(cx);
        self.connection_generation += 1;
        self.remote = Default::default();
        self.restarting = true;
        self.loading = true;
        self.error = None;
        self.status = "正在清理旧后台…".into();
        self.events = Task::ready(());
        self.open_request += 1;
        self.conversation_ready = false;
        self.busy.clear();
        self.interactions.sessions.clear();
        self.terminal_disconnected(cx);
        self.list_refreshing = false;
        self.list_dirty = false;
        self.member_loading = false;
        self.member_catalog_loading = false;
        self.member_stopping = None;
        self.workspace_removing = None;
        self.session_name_loading = false;
        self.session_pin_loading.clear();
        self.session_archive_loading.clear();
        self.reasoning_picker = Default::default();
        self.command_picker = Default::default();
        self.reset_usage();
        self.attachments.reset_connection();
        self.pinned_sessions.loading = false;
        self.pinned_sessions.dirty = false;
        self.refreshing_models = false;
        self.settings_loading = false;
        self.agent_diagnostic = None;
        self.program_checks.clear();
        self.reset_pi_accounts(window.window_handle(), cx);
        self.pi_models.reset_connection();
        if self.notification_loading {
            self.notification_message =
                Some("连接已中断，地址草稿已保留，请重新读取以核对保存结果。".into());
        }
        self.notification_loading = false;
        self.team_autostaff_loading = false;
        self.reset_memory_connection();
        self.reset_collaboration_connection();
        self.invalidate_workflows();
        let roles_visible = self.roles.visible;
        self.close_roles();
        self.roles.visible = roles_visible;
        let runtime = self.runtime.clone();
        let handle = window.window_handle();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match runtime {
                Some(runtime) => runtime
                    .shutdown_checked(std::time::Duration::from_secs(3))
                    .await
                    .map(|_| ()),
                None => Ok(()),
            };
            let _ = this.update(cx, |this, cx| {
                if this.closing {
                    return;
                }
                match result {
                    Ok(()) => {
                        this.runtime = None;
                        this.status = "正在重新启动后台…".into();
                        this.start_backend(handle, cx);
                    }
                    Err(error) => {
                        this.restarting = false;
                        this.loading = false;
                        this.error = Some(format!(
                            "旧后台清理未确认，尚未启动替代进程，请重试：{error}"
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn restore_backend_view(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.background_sequences.clear();
        self.pending_events.clear();
        self.pending.clear();
        if let Some(id) = self.session.as_ref().map(|session| session.id.clone()) {
            let views = (
                self.memory.visible,
                self.roles.visible,
                self.settings_panel,
                self.member_panel,
                self.workflows.visible,
            );
            self.show_session(id, cx);
            (
                self.memory.visible,
                self.roles.visible,
                self.settings_panel,
                self.member_panel,
                self.workflows.visible,
            ) = views;
        }
        self.refresh_sessions(cx);
        self.refresh_members(cx);
        if self.workflows.visible { self.refresh_workflows(cx); }
        if self.collaboration.visible {
            if self.collaboration.selected.is_some() {
                self.refresh_dispatch(cx);
            } else {
                self.load_tasks(cx);
                self.load_dispatches(cx);
            }
        }
        if self.memory.visible {
            self.load_memory(cx);
        }
        if self.roles.visible {
            self.load_roles(cx);
        }
        self.refresh_executable_snapshot(window, cx);
        self.refresh_terminal_after_reconnect(cx);
    }
}
