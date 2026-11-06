use crate::backend::probe_local;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    text::{TextView, TextViewState},
    *,
};
use gpui_kit::*;
use std::time::Duration;

const DEMO: &str = "## UI 演示 · 非 Agent 输出\n\n这段内容由本地定时器逐段追加，用于验证中文、**Markdown**、代码块与滚动。\n\n```rust\n// UI rendering probe, not a backend response\nlet selected_member = \"Orchestrator\";\nprintln!(\"你好，Hive\");\n```\n\n真实协作消息接线尚未实现。后台探测按钮只执行 GET，不创建或启动 Agent。\n\n";

pub struct Chat {
    input: Entity<TextareaState>,
    url: Entity<InputState>,
    markdown: Entity<TextViewState>,
    scroll: ScrollHandle,
    selected: usize,
    status: SharedString,
    streaming: bool,
    _subscriptions: Vec<Subscription>,
    _stream: Task<()>,
    _request: Task<()>,
}
impl Chat {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 6)
                .placeholder("输入中文或英文；Enter 换行，点击发送到本地演示…")
        });
        let endpoint =
            std::env::var("HIVE_PROBE_URL").unwrap_or_else(|_| "http://127.0.0.1:58591".into());
        let url = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("http://127.0.0.1:PORT");
            state.set_value(endpoint, window, cx);
            state
        });
        let markdown = cx.new(|cx| TextViewState::markdown(&format!("# 统一聊天 · GPUI 选型原型\n\n**全部预置内容均为 UI 演示，不是 Agent 输出。**\n\n{}", DEMO.repeat(8)), cx));
        let subscriptions =
            vec![
                cx.subscribe_in(&input, window, |_, state, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        println!(
                            "[probe] input changed: {} Unicode chars, {} lines",
                            state.read(cx).value().chars().count(),
                            state.read(cx).value().split('\n').count()
                        );
                    }
                }),
            ];
        Self {
            input,
            url,
            markdown,
            scroll: ScrollHandle::new(),
            selected: 0,
            status: "尚未连接后台 · 预置消息仅演示".into(),
            streaming: false,
            _subscriptions: subscriptions,
            _stream: Task::ready(()),
            _request: Task::ready(()),
        }
    }
    fn stream(&mut self, cx: &mut Context<Self>) {
        if self.streaming {
            return;
        }
        self.streaming = true;
        println!("[probe] demo stream started");
        let chunks: Vec<String> = DEMO
            .chars()
            .collect::<Vec<_>>()
            .chunks(10)
            .map(|s| s.iter().collect())
            .collect();
        self._stream = cx.spawn(async move |this, cx| {
            for chunk in chunks {
                smol::Timer::after(Duration::from_millis(45)).await;
                if this
                    .update(cx, |this, cx| {
                        this.markdown
                            .update(cx, |state, cx| state.push_str(&chunk, cx));
                        this.scroll.scroll_to_bottom();
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            _ = this.update(cx, |this, cx| {
                this.streaming = false;
                println!("[probe] demo stream complete");
                cx.notify();
            });
        });
    }
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value();
        if value.trim().is_empty() {
            return;
        }
        println!(
            "[probe] local demo send: {} Unicode chars",
            value.chars().count()
        );
        // Preserve entered text literally; it is local UI content, not an Agent request.
        let entry = format!("\n\n---\n\n### 你 · 本地演示\n\n{}\n\n", value);
        self.markdown
            .update(cx, |state, cx| state.push_str(&entry, cx));
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.scroll.scroll_to_bottom();
        self.stream(cx);
        cx.notify();
    }
    fn probe(&mut self, cx: &mut Context<Self>) {
        let endpoint = self.url.read(cx).value().to_string();
        self.status = "正在执行真实 GET…".into();
        println!("[probe] GET {endpoint}");
        self._request = cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || probe_local(&endpoint)).await;
            _ = this.update(cx, |this, cx| {
                println!("[probe] {result}");
                this.status = result.into();
                cx.notify();
            });
        });
        cx.notify();
    }
}
impl Render for Chat {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().size_full().bg(rgb(0xf5f5f4)).text_color(rgb(0x252525))
            .child(v_flex().w(px(228.)).h_full().p_4().gap_4().bg(rgb(0xecece9))
                .child(div().text_xl().child("Hive / 实验"))
                .child(div().text_sm().child("会话与成员 · UI 演示"))
                .children(["统一聊天", "Orchestrator · Pi", "Coder · Codex", "Reviewer · Claude"].into_iter().enumerate().map(|(i, name)| {
                    Button::new(("member", i)).label(name).selected(self.selected == i).on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i; println!("[probe] selected UI member {i}"); cx.notify();
                    }))
                }))
                .child(div().text_sm().child("成员按钮仅验证选择状态；当前使用同一份演示消息。")))
            .child(v_flex().flex_1().min_w_0().h_full().p_4().gap_3()
                .child(h_flex().gap_3().child(div().flex_1().text_lg().child("聊天体验 / 原生 GPUI"))
                    .child(Button::new("stream").label(if self.streaming { "演示流进行中…" } else { "播放演示流" }).on_click(cx.listener(|this, _, _, cx| this.stream(cx)))))
                .child(h_flex().gap_2().child(div().flex_1().child(Input::new(&self.url)))
                    .child(Button::new("probe").label("真实 GET").on_click(cx.listener(|this, _, _, cx| this.probe(cx)))))
                .child(div().text_sm().child(self.status.clone()))
                .child(div().id("chat-scroll").flex_1().min_h_0().w_full().overflow_y_scroll().track_scroll(&self.scroll).p_4().bg(rgb(0xffffff))
                    .child(TextView::new(&self.markdown).selectable(true).stream_fade(true)))
                .child(h_flex().gap_2().child(div().flex_1().child(Textarea::new(&self.input).aria_label("聊天消息输入")))
                    .child(Button::new("send").primary().disabled(self.streaming).label("发送到演示").on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))))
                .child(div().text_xs().child("选型原型 · 无 Agent 运行/持久化 · 中文输入、复制、流式 Markdown 与长列表滚动验证")))
    }
}
