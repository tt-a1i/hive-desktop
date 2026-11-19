use crate::artifacts::ArtifactSource;
use crate::language::tr;
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::Path;

pub(crate) fn supported_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("html") || value.eq_ignore_ascii_case("htm")
        })
}

pub(crate) struct HtmlPreview {
    source: ArtifactSource,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    _webview: Option<wry::WebView>,
    #[cfg(target_os = "windows")]
    _profile: Option<tempfile::TempDir>,
    error: Option<String>,
}

impl HtmlPreview {
    pub(crate) fn focus_existing(cx: &mut App, source: &ArtifactSource) -> bool {
        cx.windows()
            .into_iter()
            .filter_map(|window| window.downcast::<Self>())
            .find(|window| window.read(cx).is_ok_and(|preview| &preview.source == source))
            .is_some_and(|window| {
                window
                    .update(cx, |_, window, _| window.activate_window())
                    .is_ok()
            })
    }

    pub(crate) fn open(
        cx: &mut App,
        source: ArtifactSource,
        html: String,
        saved_path: bool,
    ) -> Result<(), String> {
        // The original viewer may have closed while its snapshot request was in flight.
        if Self::focus_existing(cx, &source) {
            return Ok(());
        }
        let name = Path::new(&source.path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&source.path);
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(format!("{} — {name}", tr(if saved_path {
                    "旧版路径预览"
                } else {
                    "隔离 HTML 预览"
                })).into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::centered(size(px(880.), px(660.)), cx)),
            window_min_size: Some(size(px(360.), px(260.))),
            ..Default::default()
        };
        cx.open_window(options, move |window, cx| {
            let preview = Self::build(window, source, html);
            cx.new(|_| preview)
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn build(_: &Window, source: ArtifactSource, _: String) -> Self {
        Self {
            source,
            error: Some(tr("此平台暂不支持隔离 HTML 预览。").to_owned()),
        }
    }
}

impl Render for HtmlPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when_some(self.error.as_ref(), |view, detail| {
                view.p_5()
                    .child(format!("{}\n{detail}", tr("无法打开隔离 HTML 预览")))
            })
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod engine {
    use super::*;
    use std::{borrow::Cow, cell::Cell, sync::Mutex};
    use wry::{NewWindowResponse, PermissionResponse, WebView, WebViewBuilder, http};

    const PROTOCOL: &str = "hive-artifact";
    const DOCUMENT: &str = "hive-artifact://preview.invalid/document";
    #[cfg(target_os = "macos")]
    const NAVIGATION: &str = DOCUMENT;
    #[cfg(target_os = "windows")]
    const NAVIGATION: &str = "https://hive-artifact.preview.invalid/document";

    const OFFLINE_CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; worker-src 'none'";
    const FRAME_PERMISSIONS: &str = "camera 'none'; microphone 'none'; geolocation 'none'; clipboard-read 'none'; clipboard-write 'none'; display-capture 'none'; payment 'none'; usb 'none'; serial 'none'; hid 'none'; storage-access 'none'";
    const DENY_HOST_APIS: &str = r#"
        (() => {
            for (const name of [
                'RTCPeerConnection', 'webkitRTCPeerConnection', 'WebTransport', 'WebSocket', 'EventSource',
                'showOpenFilePicker', 'showSaveFilePicker', 'showDirectoryPicker',
            ]) {
                Object.defineProperty(globalThis, name, {value: undefined, writable: false, configurable: false});
            }
            const showPicker = HTMLInputElement.prototype.showPicker;
            if (showPicker) Object.defineProperty(HTMLInputElement.prototype, 'showPicker', {
                value: function () {
                    if (this.type === 'file') throw new DOMException('Host file selection is unavailable', 'NotAllowedError');
                    return showPicker.call(this);
                },
            });
            window.addEventListener('click', event => {
                if (event.target instanceof HTMLInputElement && event.target.type === 'file') {
                    event.preventDefault();
                    event.stopImmediatePropagation();
                }
            }, true);
            for (const eventName of ['dragover', 'drop', 'paste']) {
                window.addEventListener(eventName, event => {
                    const transfer = event.dataTransfer || event.clipboardData;
                    if (transfer && Array.from(transfer.types).includes('Files')) {
                        event.preventDefault();
                        event.stopImmediatePropagation();
                    }
                }, true);
            }
        })();
    "#;

    fn frame_document(html: &str) -> Vec<u8> {
        let mut page = format!(
            r#"<!doctype html><meta charset="utf-8"><meta http-equiv="x-dns-prefetch-control" content="off"><style>html,body,iframe{{margin:0;width:100%;height:100%;overflow:hidden;border:0;display:block}}</style><iframe title="HTML artifact" sandbox="allow-scripts" allow="{FRAME_PERMISSIONS}" referrerpolicy="no-referrer" srcdoc=""#
        );
        let policy = format!(
            r#"<!doctype html><meta charset="utf-8"><meta http-equiv="x-dns-prefetch-control" content="off"><meta http-equiv="Content-Security-Policy" content="{OFFLINE_CSP}; frame-src 'none'">"#
        );
        for character in policy.chars().chain(html.chars()) {
            match character {
                '&' => page.push_str("&amp;"),
                '"' => page.push_str("&quot;"),
                '<' => page.push_str("&lt;"),
                '>' => page.push_str("&gt;"),
                other => page.push(other),
            }
        }
        page.push_str("\"></iframe>");
        page.into_bytes()
    }

    fn internal_frame(uri: &str) -> bool {
        matches!(uri.split('#').next(), Some("about:blank" | "about:srcdoc"))
    }

    fn builder(html: String, builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
        // No IPC, asset-directory mapping, shared storage, or host navigation.
        let page = Mutex::new(Some(frame_document(&html)));
        let loaded = Cell::new(false);
        builder
            .with_incognito(true)
            .with_autoplay(false)
            .with_devtools(false)
            .with_clipboard(false)
            .with_general_autofill_enabled(false)
            .with_back_forward_navigation_gestures(false)
            .with_initialization_script_for_main_only(DENY_HOST_APIS, false)
            .with_permission_handler(|_| PermissionResponse::Deny)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_navigation_handler(move |uri| {
                internal_frame(&uri) || uri == NAVIGATION && !loaded.replace(true)
            })
            .with_custom_protocol(PROTOCOL.to_owned(), move |_, request| {
                if request.method() != http::Method::GET || request.uri().to_string() != DOCUMENT {
                    return http::Response::builder()
                        .status(404)
                        .body(Cow::Borrowed(&[][..]))
                        .unwrap();
                }
                let Some(page) = page.lock().ok().and_then(|mut bytes| bytes.take()) else {
                    return http::Response::builder()
                        .status(410)
                        .body(Cow::Borrowed(&[][..]))
                        .unwrap();
                };
                http::Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .header(
                        "Content-Security-Policy",
                        format!("{OFFLINE_CSP}; frame-src 'self' about:; frame-ancestors 'none'"),
                    )
                    .header("X-DNS-Prefetch-Control", "off")
                    .header("X-Content-Type-Options", "nosniff")
                    .header("Cache-Control", "no-store")
                    .body(Cow::Owned(page))
                    .unwrap()
            })
    }

    #[cfg(target_os = "macos")]
    fn configure(window: &Window, html: String) -> wry::Result<WebView> {
        builder(html, WebViewBuilder::new())
            .with_url(DOCUMENT)
            .build(window)
    }

    #[cfg(target_os = "windows")]
    fn configure(
        window: &Window,
        html: String,
        profile: &tempfile::TempDir,
    ) -> Result<WebView, String> {
        use wry::{WebContext, WebViewBuilderExtWindows};
        let mut context = WebContext::new(Some(profile.path().to_owned()));
        let view = builder(html, WebViewBuilder::new_with_web_context(&mut context))
            .with_https_scheme(true)
            .with_default_context_menus(false)
            .with_browser_accelerator_keys(false)
            .with_additional_browser_args("--disable-background-networking --force-webrtc-ip-handling-policy=disable_non_proxied_udp")
            .build(window)
            .map_err(|error| error.to_string())?;
        block_windows_requests(&view).map_err(|error| error.to_string())?;
        view.load_url(NAVIGATION)
            .map_err(|error| error.to_string())?;
        Ok(view)
    }

    #[cfg(target_os = "windows")]
    fn block_windows_requests(view: &WebView) -> windows_core::Result<()> {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL, ICoreWebView2_22,
            ICoreWebView2_4, ICoreWebView2Frame2,
        };
        use webview2_com::{
            FrameCreatedEventHandler, FrameNavigationStartingEventHandler,
            NavigationStartingEventHandler, WebResourceRequestedEventHandler, take_pwstr,
        };
        use windows_core::{HSTRING, Interface, PWSTR};
        use wry::WebViewExtWindows;
        let native = view.webview();
        let environment = view.environment();
        let mut token = Default::default();
        unsafe {
            let mut version = PWSTR::null();
            match environment.BrowserVersionString(&mut version) {
                Ok(()) => eprintln!("[html-preview] WebView2 runtime {}", take_pwstr(version)),
                Err(error) => eprintln!("[html-preview] runtime version unavailable: {error}"),
            }
            native
                .cast::<ICoreWebView2_22>()?
                .AddWebResourceRequestedFilterWithRequestSourceKinds(
                    &HSTRING::from("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )?;
            native.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut uri = PWSTR::null();
                    args.Request()?.Uri(&mut uri)?;
                    let uri = take_pwstr(uri);
                    if uri != NAVIGATION
                        && !internal_frame(&uri)
                        && !uri.starts_with("data:")
                        && !uri.starts_with("blob:")
                    {
                        let denied = environment.CreateWebResourceResponse(
                            None,
                            403,
                            &HSTRING::from("Offline preview"),
                            &HSTRING::new(),
                        )?;
                        args.SetResponse(&denied)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            // Install the frame-scoped event before loading the document. WebView2
            // runs it after the older WebView-level FrameNavigationStarting event.
            // Registration and callbacks stay on the owning WebView's UI STA.
            native.cast::<ICoreWebView2_4>()?.add_FrameCreated(
                &FrameCreatedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let result = (|| {
                        let frame = args.Frame()?.cast::<ICoreWebView2Frame2>()?;
                        let mut frame_token = Default::default();
                        // COM retains the handler until the frame is destroyed.
                        // It captures neither the frame nor the preview, so there
                        // is no reference cycle or cross-thread view access.
                        frame.add_NavigationStarting(
                            &FrameNavigationStartingEventHandler::create(Box::new(|_, args| {
                                cancel_windows_frame_navigation(args, "frame")
                            })),
                            &mut frame_token,
                        )
                    })();
                    if let Err(error) = &result {
                        eprintln!("[html-preview] frame navigation guard registration failed: {error}");
                    }
                    result
                })),
                &mut token,
            )?;
            // Keep the existing WebView-level guard as an additional boundary.
            native.add_FrameNavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    cancel_windows_frame_navigation(args, "webview fallback")
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn cancel_windows_frame_navigation(
        args: Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2NavigationStartingEventArgs>,
        scope: &str,
    ) -> windows_core::Result<()> {
        use webview2_com::take_pwstr;
        use windows_core::PWSTR;
        if let Some(args) = args {
            let mut uri = PWSTR::null();
            unsafe {
                args.Uri(&mut uri)?;
                if !internal_frame(&take_pwstr(uri)) {
                    // Only strengthen cancellation; never undo another guard.
                    args.SetCancel(true)?;
                    eprintln!("[html-preview] cancelled frame navigation ({scope})");
                }
            }
        }
        Ok(())
    }

    impl HtmlPreview {
        pub(super) fn build(window: &Window, source: ArtifactSource, html: String) -> Self {
            #[cfg(target_os = "macos")]
            let result = configure(window, html)
                .map(|view| (view, ()))
                .map_err(|error| error.to_string());
            #[cfg(target_os = "windows")]
            let result = tempfile::tempdir()
                .map_err(|error| error.to_string())
                .and_then(|profile| Ok((configure(window, html, &profile)?, profile)));
            match result {
                Ok((webview, _profile)) => Self {
                    source,
                    _webview: Some(webview),
                    #[cfg(target_os = "windows")]
                    _profile: Some(_profile),
                    error: None,
                },
                Err(error) => Self {
                    source,
                    _webview: None,
                    #[cfg(target_os = "windows")]
                    _profile: None,
                    error: Some(error),
                },
            }
        }
    }
}
