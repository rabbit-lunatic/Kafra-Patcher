use crate::patcher::{get_patcher_name, PatcherCommand, PatcherConfiguration};
use crate::process::start_executable;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event_loop::{EventLoop, EventLoopProxy},
    window::{Window, WindowBuilder},
};
use tinyfiledialogs as tfd;
use wry::webview::{WebView, WebViewBuilder};

const WINDOW_STATE_FILE: &str = "kpatcher_state.json";

/// Coordinates below this threshold are considered invalid (e.g. minimized windows
/// on Windows report positions like -32000).
const MINIMIZED_COORD_THRESHOLD: i32 = -20000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
}

impl WindowState {
    pub fn load() -> Option<WindowState> {
        let content = fs::read_to_string(WINDOW_STATE_FILE).ok()?;
        let state: WindowState = serde_json::from_str(&content).ok()?;
        // Validate coordinates: ignore if they look like minimized values (-32000)
        // or are extremely far off-screen.
        if state.x < MINIMIZED_COORD_THRESHOLD || state.y < MINIMIZED_COORD_THRESHOLD {
            return None;
        }
        Some(state)
    }

    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(WINDOW_STATE_FILE, content);
        }
    }
}

pub fn save_window_position(window: &Window) {
    if window.is_minimized() {
        return;
    }
    if let Ok(pos) = window.outer_position() {
        // Double check for minimized coordinates
        if pos.x < MINIMIZED_COORD_THRESHOLD || pos.y < MINIMIZED_COORD_THRESHOLD {
            return;
        }
        let state = WindowState { x: pos.x, y: pos.y };
        state.save();
    }
}

#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;
#[cfg(windows)]
use winapi::um::wingdi::CreateRoundRectRgn;
#[cfg(windows)]
use winapi::um::winuser::SetWindowRgn;

#[derive(Debug, Clone)]
pub enum UiEvent {
    PatchingStatus(PatchingStatus),
    SetPatchInProgress(bool),
    Exit,
    RunScript(String),
}

#[derive(Clone)]
pub struct UiController {
    proxy: EventLoopProxy<UiEvent>,
}

impl UiController {
    pub fn new(proxy: EventLoopProxy<UiEvent>) -> UiController {
        UiController { proxy }
    }

    pub fn dispatch_patching_status(&self, status: PatchingStatus) {
        let _ = self.proxy.send_event(UiEvent::PatchingStatus(status));
    }

    pub fn set_patch_in_progress(&self, value: bool) {
        let _ = self.proxy.send_event(UiEvent::SetPatchInProgress(value));
    }
}

#[derive(Debug, Clone)]
pub enum PatchingStatus {
    Ready,
    Error(String),
    DownloadInProgress(usize, usize, u64),
    InstallationInProgress(usize, usize),
    ManualPatchApplied(String),
}

/// Builds the Window and WebView, setting up IPC handling.
/// Returns the Window, WebView, and a shared flag for patching status.
pub fn build_webview(
    event_loop: &EventLoop<UiEvent>,
    config: PatcherConfiguration,
    patching_thread_tx: flume::Sender<PatcherCommand>,
    proxy: EventLoopProxy<UiEvent>,
) -> Result<(WebView, Arc<AtomicBool>)> {
    let mut window_builder = WindowBuilder::new()
        .with_title(&config.window.title)
        .with_inner_size(LogicalSize::new(
            config.window.width as f64,
            config.window.height as f64,
        ))
        .with_resizable(config.window.resizable)
        .with_decorations(!config.window.frameless.unwrap_or(false))
        .with_transparent(true);

    // Restore saved window position if available
    if let Some(state) = WindowState::load() {
        window_builder = window_builder.with_position(PhysicalPosition::new(state.x, state.y));
    }

    let window = window_builder
        .build(event_loop)
        .context("Failed to create window")?;

    // Apply border radius if configured (Windows only)
    #[cfg(windows)]
    if let Some(radius) = config.window.border_radius {
        let physical_size = window.inner_size();
        apply_border_radius(
            &window,
            physical_size.width as i32,
            physical_size.height as i32,
            (radius as f64 * window.scale_factor()) as i32,
        );
    }

    // Shared state for IPC handler
    let patching_in_progress = Arc::new(AtomicBool::new(false));
    let pip_clone = patching_in_progress.clone();

    // Capture config and tx for IPC
    let ipc_config = config.clone();
    let ipc_tx = patching_thread_tx.clone();
    let ipc_proxy = proxy.clone();

    // The IPC handler for Wry
    let ipc_handler = move |window: &Window, request: String| {
        match request.as_str() {
            "play" => {
                let args = ipc_config.play.arguments.clone();
                start_game_client(&ipc_config, &args);
                if ipc_config.play.exit_on_success.unwrap_or(true) {
                    let _ = ipc_proxy.send_event(UiEvent::Exit);
                } else if ipc_config.play.minimize_on_start.unwrap_or(false) {
                    window.set_minimized(true);
                }
            }
            "setup" => {
                handle_setup(&ipc_config);
                if ipc_config.setup.exit_on_success.unwrap_or(false) {
                    let _ = ipc_proxy.send_event(UiEvent::Exit);
                }
            }
            "exit" => {
                let _ = ipc_proxy.send_event(UiEvent::Exit);
            }
            "start_update" => {
                if pip_clone.load(Ordering::Relaxed) {
                    let _ = ipc_proxy
                        .send_event(UiEvent::RunScript("notificationInProgress()".to_string()));
                } else {
                    let _ = ipc_tx.send(PatcherCommand::StartUpdate);
                }
            }
            "cancel_update" => {
                let _ = ipc_tx.send(PatcherCommand::CancelUpdate);
            }
            "reset_cache" => {
                handle_reset_cache();
            }
            "manual_patch" => {
                if pip_clone.load(Ordering::Relaxed) {
                    let _ = ipc_proxy
                        .send_event(UiEvent::RunScript("notificationInProgress()".to_string()));
                } else {
                    // We need to open a dialog. tfd::open_file_dialog blocks.
                    // Is it safe on this thread? If this is the UI thread, it blocks UI.
                    // But `web-view` did it in invoke handler.
                    handle_manual_patch(&ipc_tx);
                }
            }
            "minimize" => {
                window.set_minimized(true);
            }
            "start_drag" => {
                let _ = window.drag_window();
            }
            req => {
                handle_json_request(req, &ipc_config, window, &ipc_proxy);
            }
        }
    };

    let webview = WebViewBuilder::new(window)?
        .with_url(&config.web.index_url)?
        .with_transparent(true)
        // Inject polyfill for web-view's `external.invoke`
        .with_initialization_script(
            "window.external = { invoke: function(s) { window.ipc.postMessage(s); } };",
        )
        .with_ipc_handler(ipc_handler)
        .build()?;

    Ok((webview, patching_in_progress))
}

pub fn start_game_client(config: &PatcherConfiguration, args: &[String]) {
    let client_exe = &config.play.path;
    let _ =
        start_executable(client_exe, args).map_err(|e| log::warn!("Failed to start client: {}", e));
}

fn handle_setup(config: &PatcherConfiguration) {
    let setup_exe = &config.setup.path;
    let setup_args = &config.setup.arguments;
    let _ = start_executable(setup_exe, setup_args)
        .map_err(|e| log::warn!("Failed to start setup: {}", e));
}

fn handle_reset_cache() {
    if let Ok(patcher_name) = get_patcher_name() {
        let cache_file_path = PathBuf::from(patcher_name).with_extension("dat");
        let _ = fs::remove_file(cache_file_path);
    }
}

fn handle_manual_patch(tx: &flume::Sender<PatcherCommand>) {
    let opt_path = tfd::open_file_dialog(
        "Select a file",
        "",
        Some((&["*.thor"], "Patch Files (*.thor)")),
    );
    if let Some(path) = opt_path {
        let _ = tx.send(PatcherCommand::ApplyPatch(PathBuf::from(path)));
    }
}

#[derive(Deserialize)]
struct LoginParameters {
    login: String,
    password: String,
}

#[derive(Deserialize)]
struct OpenUrlParameters {
    url: String,
}

fn handle_json_request(
    request: &str,
    config: &PatcherConfiguration,
    _window: &Window,
    _proxy: &EventLoopProxy<UiEvent>,
) {
    if let Ok(json_req) = serde_json::from_str::<Value>(request) {
        if let Some(function_name) = json_req["function"].as_str() {
            match function_name {
                "login" => {
                    if let Ok(params) =
                        serde_json::from_value::<LoginParameters>(json_req["parameters"].clone())
                    {
                        let mut args = vec![
                            format!("-t:{}", params.password),
                            params.login,
                            "server".to_string(),
                        ];
                        args.extend(config.play.arguments.iter().cloned());
                        start_game_client(config, &args);
                    }
                }
                "open_url" => {
                    if let Ok(params) =
                        serde_json::from_value::<OpenUrlParameters>(json_req["parameters"].clone())
                    {
                        let _ = open::that(params.url);
                    }
                }
                _ => log::error!("Unknown function '{}'", function_name),
            }
        }
    }
}

#[cfg(windows)]
fn apply_border_radius(window: &Window, width: i32, height: i32, radius: i32) {
    let hwnd = window.hwnd() as winapi::shared::windef::HWND;
    unsafe {
        let region = CreateRoundRectRgn(0, 0, width, height, radius, radius);
        if !region.is_null() {
            SetWindowRgn(hwnd, region, 1); // 1 = TRUE (redraw)
                                           // Note: SetWindowRgn takes ownership of the region, so we don't delete it
        }
    }
}
