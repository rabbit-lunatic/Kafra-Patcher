#![windows_subsystem = "windows"]

mod patcher;
mod process;
mod ui;

#[cfg(windows)]
mod memory_reader;
#[cfg(windows)]
mod discord_rpc;
#[cfg(windows)]
mod rich_presence_monitor;


use log::LevelFilter;
use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use simple_logger::SimpleLogger;
use structopt::StructOpt;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tinyfiledialogs as tfd;
use ui::{UiController, UiEvent};

use patcher::{
    patcher_thread_routine, retrieve_patcher_configuration, PatcherCommand, PatcherConfiguration,
};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, version = PKG_VERSION, author = PKG_AUTHORS, about = PKG_DESCRIPTION)]
struct Opt {
    /// Sets a custom working directory
    #[structopt(short, long, parse(from_os_str))]
    working_directory: Option<PathBuf>,
}

fn main() -> Result<()> {
    // Cleanup old executable if it exists
    if let Ok(current_exe) = env::current_exe() {
        let old_exe = current_exe.with_extension("exe.old");
        if old_exe.exists() {
            let _ = fs::remove_file(old_exe);
        }
    }

    SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level(PKG_NAME, LevelFilter::Info)
        .init()
        .with_context(|| "Failed to initalize the logger")?;

    // Parse CLI arguments
    let cli_args = Opt::from_args();
    if let Some(working_directory) = cli_args.working_directory {
        env::set_current_dir(working_directory)
            .with_context(|| "Specified working directory is invalid or inaccessible")?;
    };

    let mut config = match retrieve_patcher_configuration(None) {
        Err(e) => {
            let err_msg = "Failed to retrieve the patcher's configuration";
            // Sanitize error message to avoid issues with double quotes in tinyfiledialogs
            let formatted_error = format!("Error: {}: {:#}.", err_msg, e).replace('"', "'");
            tfd::message_box_ok(
                "Error",
                formatted_error.as_str(),
                tfd::MessageBoxIcon::Error,
            );
            return Err(e);
        }
        Ok(v) => v,
    };

    #[cfg(windows)]
    if let Some(dll_name) = &config.window.dllwebview {
        use std::os::windows::ffi::OsStrExt;
        let wide_name: Vec<u16> = std::ffi::OsStr::new(dll_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { winapi::um::libloaderapi::LoadLibraryW(wide_name.as_ptr()) };
        if handle.is_null() {
            log::warn!("Failed to load custom dllwebview: {}", dll_name);
        } else {
            log::info!("Successfully loaded custom dllwebview: {}", dll_name);
        }
    }

    // Resolve relative path for index_url if it's not a remote URL or absolute file URI
    if !config.web.index_url.starts_with("http://")
        && !config.web.index_url.starts_with("https://")
        && !config.web.index_url.starts_with("file://")
    {
        let current_dir = env::current_dir().context("Failed to get current directory")?;
        let absolute_path = current_dir.join(&config.web.index_url);
        // Convert to slash-based path for file:/// URI
        let path_str = absolute_path.to_string_lossy().replace('\\', "/");
        config.web.index_url = format!("file:///{}", path_str);
        log::info!("Resolved local index URL: {}", config.web.index_url);
    }

    // Event Loop
    let event_loop = EventLoop::<UiEvent>::with_user_event();
    let proxy = event_loop.create_proxy();

    // Create a channel to allow the patcher thread to communicate with the patching thread (which we spawn)
    // Wait, the patching thread receives PatcherCommand from UI.
    // The UI Controller sends UiEvent to Main Thread.
    let (tx, rx) = flume::bounded(32);

    let (webview, patching_in_progress) =
        ui::build_webview(&event_loop, config.clone(), tx, proxy.clone())
            .with_context(|| "Failed to build a web view")?;

    // Spawn a patching thread
    let ui_ctrl = UiController::new(proxy);
    // new_patching_thread returns a JoinHandle, but we can't join it easily in tao loop.
    // We just spawn it and let it run.
    let _patching_thread = new_patching_thread(rx, ui_ctrl, config.clone());

    // Prevent dragging images
    webview
        .evaluate_script(
            r#"
        window.addEventListener('load', function() {
            var style = document.createElement('style');
            style.innerHTML = 'img { -webkit-user-drag: none; user-select: none; }';
            document.head.appendChild(style);
            document.addEventListener('dragstart', function(e) { e.preventDefault(); });
        });
        "#,
        )
        .with_context(|| "Failed to inject drag prevention script")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(ui_event) => match ui_event {
                UiEvent::PatchingStatus(status) => {
                    let script = match status {
                        ui::PatchingStatus::Ready => "patchingStatusReady()".to_string(),
                        ui::PatchingStatus::Error(msg) => {
                            let play_with_error = config.play.play_with_error.unwrap_or(false);
                            format!("patchingStatusError(\"{}\", {})", msg, play_with_error)
                        }
                        ui::PatchingStatus::DownloadInProgress(nb, total, rate) => {
                            format!("patchingStatusDownloading({}, {}, {})", nb, total, rate)
                        }
                        ui::PatchingStatus::InstallationInProgress(nb, total) => {
                            format!("patchingStatusInstalling({}, {})", nb, total)
                        }
                        ui::PatchingStatus::ManualPatchApplied(name) => {
                            format!("patchingStatusPatchApplied(\"{}\")", name)
                        }
                    };
                    if let Err(e) = webview.evaluate_script(&script) {
                        log::warn!("Failed to dispatch patching status: {}.", e);
                    }
                }
                UiEvent::SetPatchInProgress(val) => {
                    patching_in_progress.store(val, std::sync::atomic::Ordering::Relaxed);
                }
                UiEvent::Exit => {
                    ui::save_window_position(webview.window());
                    *control_flow = ControlFlow::Exit;
                }
                UiEvent::RunScript(script) => {
                    if let Err(e) = webview.evaluate_script(&script) {
                        log::warn!("Failed to run script: {}.", e);
                    }
                }
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                ui::save_window_position(webview.window());
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                let script = if focused {
                    "if(typeof mediaResume==='function')mediaResume();"
                } else {
                    "if(typeof mediaPause==='function')mediaPause();"
                };
                let _ = webview.evaluate_script(script);
            }
            _ => (),
        }
    });
}

/// Spawns a new thread that runs a single threaded tokio runtime to execute the patcher routine
fn new_patching_thread(
    rx: flume::Receiver<PatcherCommand>,
    ui_ctrl: UiController,
    config: PatcherConfiguration,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn(move || {
        // Build a tokio runtime that runs a scheduler on the current thread and a reactor
        let tokio_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .with_context(|| "Failed to build a tokio runtime")?;
        // Block on the patching task from our synchronous function
        tokio_rt.block_on(patcher_thread_routine(ui_ctrl, config, rx));

        Ok(())
    })
}
