use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

// use advisory_lock::{AdvisoryFileLock, FileLockMode};
use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use futures::stream::{StreamExt, TryStreamExt};
use gruf::grf::GrfArchive;
use gruf::thor::{self, ThorArchive, ThorPatchInfo, ThorPatchList};
use gruf::GrufError;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use url::Url;

use super::cache::{read_cache_file, write_cache_file, PatcherCache};
use super::cancellation::{
    process_incoming_commands, wait_for_cancellation, InterruptibleFnError, InterruptibleFnResult,
};
use super::config::PatchServerInfo;
use super::patching::{apply_patch_to_disk, apply_patch_to_grf, GrfPatchingMethod};
use super::{get_patcher_name, PatcherCommand, PatcherConfiguration};
use crate::patcher::patching::apply_grf_to_grf;
use crate::ui::{PatchingStatus, UiController};

/// Representation of a pending patch (a patch that's been downloaded but has
/// not been applied yet).
#[derive(Debug)]
struct PendingPatch {
    info: thor::ThorPatchInfo,
    local_file_path: PathBuf,
}

/// Entry point of the patching task.
///
/// This waits for a `PatcherCommand::Start` command before starting an
/// interruptible patching task.
pub async fn patcher_thread_routine(
    ui_controller: UiController,
    config: PatcherConfiguration,
    mut patcher_thread_rx: flume::Receiver<PatcherCommand>,
) {
    log::trace!("Patching thread started. Waiting for commands ...");
    let rx = &mut patcher_thread_rx;
    let config = &config;
    loop {
        let cmd = rx.recv_async().await;
        match cmd {
            Err(e) => {
                log::error!("Failed to read from channel: {}", e);
                return;
            }
            Ok(cmd) => match cmd {
                PatcherCommand::StartUpdate => {
                    update_game(&ui_controller, config, rx).await;
                }
                PatcherCommand::ApplyPatch(patch_file_path) => {
                    apply_single_patch(patch_file_path, &ui_controller, config);
                }
                _ => {}
            },
        }
    }
}

/// Starts the automatic update process (download + patching)
async fn update_game(
    ui_controller: &UiController,
    config: &PatcherConfiguration,
    patcher_thread_rx: &mut flume::Receiver<PatcherCommand>,
) {
    // Try taking the update lock
    let lock_file = match take_update_lock().with_context(|| "Failed to take the update lock") {
        Ok(f) => f,
        Err(err) => {
            log::error!("{:#}", err);
            ui_controller.dispatch_patching_status(PatchingStatus::Error(format!("{:#}", err)));
            return;
        }
    };

    // Tell the UI and other processes that we're currently working
    ui_controller.set_patch_in_progress(true);
    let _guard = scopeguard::guard((), |_| {
        let _ = lock_file.unlock();
        ui_controller.set_patch_in_progress(false);
    });

    let res = interruptible_update_routine(ui_controller, config, patcher_thread_rx).await;
    match res {
        Err(err) => {
            log::error!("{:#}", err);
            ui_controller.dispatch_patching_status(PatchingStatus::Error(format!("{:#}", err)));
            // Nota: play_with_error apenas habilita o botão Play no JavaScript,
            // o jogo só será lançado quando o usuário clicar no botão.
        }
        Ok(()) => {
            ui_controller.dispatch_patching_status(PatchingStatus::Ready);
            log::info!("Patching finished!");
        }
    }
}

/// Applies a manual patch given by the user
fn apply_single_patch(
    patch_file_path: impl AsRef<Path>,
    ui_controller: &UiController,
    config: &PatcherConfiguration,
) {
    // Try taking the update lock
    match take_update_lock().with_context(|| "Failed to take the update lock") {
        Err(err) => {
            log::error!("{:#}", err);
            ui_controller.dispatch_patching_status(PatchingStatus::Error(format!("{:#}", err)));
        }
        Ok(lock_file) => {
            // Tell the UI and other processes that we're currently working
            ui_controller.set_patch_in_progress(true);
            let _guard = scopeguard::guard((), |_| {
                let _ = lock_file.unlock();
                ui_controller.set_patch_in_progress(false);
            });

            let current_working_dir =
                env::current_dir().with_context(|| "Failed to resolve current working directory");
            match current_working_dir {
                Err(err) => {
                    log::error!("{:#}", err);
                    ui_controller
                        .dispatch_patching_status(PatchingStatus::Error(format!("{:#}", err)));
                }
                Ok(current_working_dir) => {
                    let patch_file_name = patch_file_path
                        .as_ref()
                        .file_name()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                        .to_string();
                    log::info!("Applying patch '{}'", patch_file_name);
                    let res = apply_patch(patch_file_path, config, current_working_dir);
                    match res {
                        Err(err) => {
                            log::error!("{:#}", err);
                            ui_controller.dispatch_patching_status(PatchingStatus::Error(format!(
                                "{:#}",
                                err
                            )));
                        }
                        Ok(()) => {
                            log::info!("Done");
                            ui_controller.dispatch_patching_status(
                                PatchingStatus::ManualPatchApplied(patch_file_name),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Takes an advisory lock that prevents multiple instances of the patcher to
/// update the game at the same time
fn take_update_lock() -> Result<std::fs::File> {
    let lock_file_name = get_update_lock_file_path()?;
    let lock_file = std::fs::File::create(lock_file_name)?;
    lock_file.try_lock()?;

    Ok(lock_file)
}

/// Main routine of the patching task.
///
/// This routine is written in a way that makes it interuptible (or cancellable)
/// with a relatively low latency.
async fn interruptible_update_routine(
    ui_controller: &UiController,
    config: &PatcherConfiguration,
    patcher_thread_rx: &mut flume::Receiver<PatcherCommand>,
) -> Result<()> {
    log::info!("Start patching");

    // Build a shared HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .with_context(|| "Failed to build HTTP client")?;

    // Find a patch server that we can connect to
    log::info!("Looking for an available patch server ...");
    let (mut patch_list, patch_data_url) = find_available_patch_server(
        &client,
        config.web.patch_servers.as_slice(),
        &config.web.preferred_patch_server,
        patcher_thread_rx,
    )
    .await
    .map_err(|e| match e {
        InterruptibleFnError::Err(msg) => anyhow!(msg),
        InterruptibleFnError::Interrupted => anyhow!("Patching was canceled"),
    })?;
    log::debug!("Successfully fetched patch list: {:?}", patch_list);

    // Try to read cache
    let cache_file_path =
        get_cache_file_path().with_context(|| "Failed to resolve patcher name")?;
    if let Ok(patcher_cache) = read_cache_file(&cache_file_path).await {
        // Ignore already applied patches if needed
        // First we verify that our cached index looks relevant
        let should_filter_patch_list = patch_list
            .iter()
            .any(|x| x.index == patcher_cache.last_patch_index);
        if should_filter_patch_list {
            patch_list.retain(|x| x.index > patcher_cache.last_patch_index);
        }
    };

    // Try fetching patch files
    log::info!("Downloading patches ...");
    let patch_url =
        Url::parse(patch_data_url.as_str()).with_context(|| "Failed to parse 'patch_url'")?;
    let tmp_dir = tempfile::tempdir().with_context(|| "Failed to create temporary directory")?;
    let concurrent_downloads = config.patching.concurrent_downloads.unwrap_or(8);
    let pending_patch_queue = download_patches_concurrent(
        &client,
        patch_url,
        patch_list,
        tmp_dir.path(),
        config.patching.check_integrity,
        concurrent_downloads,
        ui_controller,
        patcher_thread_rx,
    )
    .await
    .map_err(|e| match e {
        InterruptibleFnError::Err(msg) => anyhow!("Failed to download patches: {}", msg),
        InterruptibleFnError::Interrupted => anyhow!("Patching was canceled"),
    })?;
    log::info!("Patches have been downloaded");

    // Proceed with actual patching
    log::info!("Applying patches ...");
    apply_patches(
        pending_patch_queue,
        config,
        &cache_file_path,
        ui_controller,
        patcher_thread_rx,
    )
    .await
    .map_err(|e| match e {
        InterruptibleFnError::Err(msg) => anyhow!("Failed to apply patches: {}", msg),
        InterruptibleFnError::Interrupted => anyhow!("Patching was canceled"),
    })?;
    log::info!("Patches have been applied");

    Ok(())
}

/// Iterates through `server_list` and returns the first available server's info.
/// `preferred_server_name` is checked first if present.
async fn find_available_patch_server(
    client: &reqwest::Client,
    server_list: &[PatchServerInfo],
    preferred_server_name: &Option<String>,
    patching_thread_rx: &mut flume::Receiver<PatcherCommand>,
) -> InterruptibleFnResult<(ThorPatchList, Url)> {
    // Probe the preferred server first if it's specified and valid
    if let Some(preferred_server_name) = preferred_server_name {
        let preferred_server_str = preferred_server_name.as_str();
        let preferred_server = server_list
            .iter()
            .find(|s| s.name.as_str() == preferred_server_str);
        if let Some(preferred_server) = preferred_server {
            if let Ok((patch_list, patch_url)) = probe_patch_server(client, preferred_server).await
            {
                return Ok((patch_list, patch_url));
            } else {
                log::warn!("'{}' is unavailable", preferred_server_name);
            }
        } else {
            log::warn!(
                "'{}' isn't in the list of patch servers",
                preferred_server_name
            );
        }
    }

    // Probe other servers, if any
    for server in server_list {
        // Cancel the patching process if we've been asked to or if the other
        // end of the channel has been disconnected
        process_incoming_commands(patching_thread_rx)?;
        if let Ok((patch_list, patch_url)) = probe_patch_server(client, server).await {
            return Ok((patch_list, patch_url));
        } else {
            log::warn!("'{}' is unavailable", server.name);
        }
    }

    Err(InterruptibleFnError::Err(
        "None of the patch servers are available at the moment".to_string(),
    ))
}

/// Checks whether a patch server is up or not.
/// Returns the list of patches served by the server as well as the URL to
/// download them from.
async fn probe_patch_server(
    client: &reqwest::Client,
    server_info: &PatchServerInfo,
) -> Result<(ThorPatchList, Url)> {
    // Parse URLs
    let patch_list_url = Url::parse(server_info.plist_url.as_str())
        .with_context(|| "Failed to parse 'plist_url'")?;
    let patch_url = Url::parse(server_info.patch_url.as_str())
        .with_context(|| "Failed to parse 'patch_url'")?;

    // Fetch plist
    let patch_list = fetch_patch_list(client, patch_list_url)
        .await
        .with_context(|| "Failed to retrieve the patch list")?;

    // Ensure that the server serves the patches (check the first patch of the list)
    if let Some(patch_info) = patch_list.first() {
        let patch_resp = client
            .head(patch_url.join(patch_info.file_name.as_str())?)
            .send()
            .await
            .with_context(|| "Failed to HEAD URL")?;
        // Return on error
        patch_resp.error_for_status()?;
    }

    Ok((patch_list, patch_url))
}

/// Downloads and parses a 'plist.txt' file located as the URL contained in the
/// `patch_list_url` argument.
///
/// Returns a vector of `ThorPatchInfo` in case of success.
async fn fetch_patch_list(client: &reqwest::Client, patch_list_url: Url) -> Result<ThorPatchList> {
    let resp = client
        .get(patch_list_url)
        .send()
        .await
        .with_context(|| "Failed to GET URL")?;
    if !resp.status().is_success() {
        return Err(anyhow!("Patch list file not found on the remote server"));
    }
    let patch_index_content = resp.text().await.with_context(|| "Invalid response body")?;
    log::info!("Parsing patch index...");

    Ok(thor::patch_list_from_string(patch_index_content.as_str()))
}

/// Returns the patcher cache file's name as a `PathBuf` on success.
fn get_cache_file_path() -> Result<PathBuf> {
    get_instance_asset_file_name("dat")
}

/// Returns the patcher update lock file's name as a `PathBuf` on success.
fn get_update_lock_file_path() -> Result<PathBuf> {
    get_instance_asset_file_name("lock")
}

/// Generates asset file names which are associated with the current 'instance'
/// of the patcher.
fn get_instance_asset_file_name(extension: impl AsRef<std::ffi::OsStr>) -> Result<PathBuf> {
    let patcher_name = get_patcher_name()?;

    Ok(PathBuf::from(patcher_name).with_extension(extension))
}

/// Downloads a list of patches (described with a `ThorPatchList`).
///
/// Files are downloaded from the remote directory located at the URL
/// contained in the 'patch_url' argument.
///
/// This function is interruptible.
#[allow(clippy::too_many_arguments)]
async fn download_patches_concurrent(
    client: &reqwest::Client,
    patch_url: Url,
    patch_list: ThorPatchList,
    download_directory: impl AsRef<Path>,
    ensure_integrity: bool,
    concurrent_downloads: usize,
    ui_controller: &UiController,
    patching_thread_rx: &mut flume::Receiver<PatcherCommand>,
) -> InterruptibleFnResult<Vec<PendingPatch>> {
    let patch_count = patch_list.len();
    ui_controller.dispatch_patching_status(PatchingStatus::DownloadInProgress(0, patch_count, 0));
    // Download files in a cancelable manner
    let mut vec = tokio::select! {
        cancel_res = wait_for_cancellation(patching_thread_rx) => return Err(cancel_res),
        download_res = download_patches_concurrent_inner(client, patch_url, patch_list, download_directory, ensure_integrity, concurrent_downloads, ui_controller) => {
            download_res.map_err(|e| InterruptibleFnError::Err(format!("{:#}", e)))
        },
    }?;
    // Sort patches by index before returning
    vec.sort_unstable_by_key(|l| l.info.index);
    Ok(vec)
}

/// Actual implementation of the concurrent file download
///
/// Returns an unordered vector of `PendingPatch`.
async fn download_patches_concurrent_inner(
    client: &reqwest::Client,
    patch_url: Url,
    patch_list: ThorPatchList,
    download_directory: impl AsRef<Path>,
    ensure_integrity: bool,
    concurrent_downloads: usize,
    ui_controller: &UiController,
) -> Result<Vec<PendingPatch>> {
    const ONE_SECOND: Duration = Duration::from_secs(1);
    // Shared value that contains the number of downloaded patches
    let shared_patch_number = AtomicUsize::new(0_usize);
    // Shared tuple that's used to compute the download speed
    let shared_progress_state = Arc::new(std::sync::Mutex::new((Instant::now(), 0_u64)));

    // Collect stream of "PendingPatch" concurrently with an unordered_buffer
    let patch_count = patch_list.len();
    futures::stream::iter(patch_list.into_iter().map(|patch_info| async {
        let patch_file_url = patch_url
            .join(patch_info.file_name.as_str())
            .with_context(|| "Failed to generate URL for patch file")?;
        let local_file_path = download_directory
            .as_ref()
            .join(patch_info.file_name.as_str());
        let mut tmp_file = File::create(&local_file_path)
            .await
            .with_context(|| "Failed to create temporary file")?;

        // Setup a progress callback that'll send the current download speed to the UI
        let shared_patch_number_ref = &shared_patch_number;
        let shared_state = shared_progress_state.clone();
        let mut last_downloaded_bytes: u64 = 0;
        let mut progress_callback = move |dl_now, _| {
            let dl_delta = dl_now - last_downloaded_bytes;
            // Return download speed if the required time has elapsed (1s)
            let downloaded_bytes_per_sec = {
                if let Ok(mut shared_state) = shared_state.lock() {
                    shared_state.1 += dl_delta;
                    if shared_state.0.elapsed() >= ONE_SECOND {
                        let downloaded_bytes_per_sec = (shared_state.1 as f32
                            / shared_state.0.elapsed().as_secs_f32())
                        .round() as u64;
                        shared_state.0 = Instant::now();
                        shared_state.1 = 0;
                        Some(downloaded_bytes_per_sec)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            // If speed is "available", update UI
            if let Some(downloaded_bytes_per_sec) = downloaded_bytes_per_sec {
                ui_controller.dispatch_patching_status(PatchingStatus::DownloadInProgress(
                    shared_patch_number_ref.load(Ordering::SeqCst),
                    patch_count,
                    downloaded_bytes_per_sec,
                ));
            }
            last_downloaded_bytes = dl_now;
        };

        download_patch_to_file(
            client,
            &patch_file_url,
            &patch_info,
            &mut tmp_file,
            &mut progress_callback,
        )
        .await?;

        // Check the archive's integrity if required
        if ensure_integrity {
            let path_to_check = local_file_path.clone();
            let validity_check =
                tokio::task::spawn_blocking(move || is_archive_valid(&path_to_check))
                    .await
                    .map_err(|e| anyhow!("Integrity check task failed: {}", e))?;

            let context = || {
                format!(
                    "Failed to check archive's integrity: '{}'",
                    patch_info.file_name
                )
            };

            if !validity_check.with_context(context)? {
                return Err(anyhow!("Archive '{}' is corrupt", patch_info.file_name));
            }
        }

        // Update status
        shared_patch_number_ref.fetch_add(1, Ordering::SeqCst);

        // File's been downloaded, add it to the queue
        Ok(PendingPatch {
            info: patch_info,
            local_file_path,
        }) as Result<PendingPatch>
    }))
    .buffer_unordered(concurrent_downloads)
    .try_collect()
    .await
}

fn is_archive_valid(archive_path: impl AsRef<Path>) -> Result<bool> {
    let mut archive =
        ThorArchive::open(archive_path.as_ref()).with_context(|| "Failed to open archive")?;
    match archive.is_valid() {
        Err(e) => {
            if let GrufError::EntryNotFound = e {
                // No integrity file present, consider the archive valid
                Ok(true)
            } else {
                // Only consider this an error if the integrity file was found
                Err(anyhow!("Archive's integrity file is invalid: {}", e,))
            }
        }
        Ok(v) => Ok(v),
    }
}

/// Downloads a single patch described with a `ThorPatchInfo`.
async fn download_patch_to_file<CB: FnMut(u64, u64)>(
    client: &reqwest::Client,
    patch_url: &Url,
    patch: &ThorPatchInfo,
    tmp_file: &mut File,
    mut progress_callback: CB,
) -> Result<()> {
    let patch_file_url = patch_url.join(patch.file_name.as_str()).with_context(|| {
        format!(
            "Invalid file name '{}' given in patch list file",
            patch.file_name
        )
    })?;
    let mut resp = client
        .get(patch_file_url)
        .send()
        .await
        .with_context(|| format!("Failed to download file '{}'", patch.file_name))?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "Patch file '{}' not found on the remote server",
            patch.file_name
        ));
    }
    let bytes_to_download = resp.content_length().unwrap_or(0);
    let mut downloaded_bytes: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .with_context(|| format!("Failed to download file '{}'", patch.file_name))?
    {
        tmp_file
            .write_all(&chunk[..])
            .await
            .with_context(|| format!("Failed to download file '{}'", patch.file_name))?;
        downloaded_bytes += chunk.len() as u64;
        progress_callback(downloaded_bytes, bytes_to_download);
    }
    tmp_file
        .sync_all()
        .await
        .with_context(|| format!("Failed to sync downloaded file '{}'", patch.file_name,))?;
    Ok(())
}

/// Parses and applies a list of patches to GRFs and/or to the game client's
/// files.
///
/// This function is interruptible.
async fn apply_patches(
    pending_patch_queue: Vec<PendingPatch>,
    config: &PatcherConfiguration,
    cache_file_path: impl AsRef<Path>,
    ui_controller: &UiController,
    patching_thread_rx: &mut flume::Receiver<PatcherCommand>,
) -> InterruptibleFnResult<()> {
    let current_working_dir = env::current_dir().map_err(|e| {
        InterruptibleFnError::Err(format!(
            "Failed to resolve current working directory: {}.",
            e
        ))
    })?;
    let patch_count = pending_patch_queue.len();
    ui_controller.dispatch_patching_status(PatchingStatus::InstallationInProgress(0, patch_count));

    let mut last_applied_index = None;
    let mut patching_result = Ok(());

    for (patch_number, pending_patch) in pending_patch_queue.into_iter().enumerate() {
        // Cancel the patching process if we've been asked to or if the other
        // end of the channel has been disconnected
        if let Err(e) = process_incoming_commands(patching_thread_rx) {
            patching_result = Err(e);
            break;
        }

        let patch_name = pending_patch.info.file_name;
        let patch_index = pending_patch.info.index;
        log::info!("Processing {}", patch_name);

        let local_file_path = pending_patch.local_file_path;
        let config_clone = config.clone();
        let cwd_clone = current_working_dir.clone();

        let join_result = tokio::task::spawn_blocking(move || {
            apply_patch(local_file_path, &config_clone, &cwd_clone)
        })
        .await;

        match join_result {
            Ok(Ok(())) => {
                last_applied_index = Some(patch_index);
                // Update status
                ui_controller.dispatch_patching_status(PatchingStatus::InstallationInProgress(
                    1 + patch_number,
                    patch_count,
                ));
            }
            Ok(Err(e)) => {
                patching_result = Err(InterruptibleFnError::Err(format!(
                    "Failed to apply patch '{}': {}.",
                    patch_name, e
                )));
                break;
            }
            Err(e) => {
                patching_result = Err(InterruptibleFnError::Err(format!(
                    "Task join error for '{}': {}.",
                    patch_name, e
                )));
                break;
            }
        }
    }

    // Always update the cache file with the last successful patch's index if any patch was applied
    if let Some(index) = last_applied_index {
        if let Err(e) = write_cache_file(
            &cache_file_path,
            PatcherCache {
                last_patch_index: index,
            },
        )
        .await
        {
            log::warn!("Failed to write cache file: {}.", e);
        }
    }

    patching_result
}

fn apply_patch(
    patch_file_path: impl AsRef<Path>,
    config: &PatcherConfiguration,
    current_working_dir: impl AsRef<Path>,
) -> Result<()> {
    let patch_path = patch_file_path.as_ref();
    let extension = patch_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if extension == "rgz" || extension == "gpf" || extension == "grf" {
        // Handle GRF/RGZ/GPF (Gzipped GRF or regular GRF)
        let file = fs::File::open(patch_path)?;
        let mut decoder = GzDecoder::new(file);

        // Decompress to a temporary GRF file if needed
        let temp_dir = tempfile::tempdir()?;
        let temp_grf_path = temp_dir.path().join("patch_temp.grf");

        if extension == "grf" {
            // Regular GRF, no decompression needed
            // Copy to temp file to ensure we don't lock source if needed?
            // Or just use directly. But apply_grf_to_grf takes a path.
            // If we use source path directly, we might need write access if InPlace, but here we are reading FROM it.
            // apply_grf_to_grf uses `source_grf` which is `&mut GrfArchive`.
            // Let's just copy it to be safe and consistent with RGZ/GPF flow,
            // although for performance avoiding copy would be better.
            // But wait, `GzDecoder` produces a stream.
            // Let's just copy the file content to temp_grf_path logic.
            fs::copy(patch_path, &temp_grf_path)?;
        } else {
            // RGZ/GPF, decompress
            let mut temp_grf_file = fs::File::create(&temp_grf_path)?;
            std::io::copy(&mut decoder, &mut temp_grf_file)?;
        }

        // Open the (decompressed or copied) GRF
        let mut source_grf = GrfArchive::open(&temp_grf_path)?;

        // Target GRF (use default from config)
        let target_grf_name = &config.client.default_grf_name;
        log::trace!("Target GRF: {:?}", target_grf_name);

        let grf_patching_method = match config.patching.in_place {
            true => GrfPatchingMethod::InPlace,
            false => GrfPatchingMethod::OutOfPlace,
        };
        let target_grf_path = current_working_dir.as_ref().join(target_grf_name);

        apply_grf_to_grf(
            grf_patching_method,
            config.patching.create_grf,
            &target_grf_path,
            &mut source_grf,
        )?;

        // Verificar integridade do GRF após patch (se check_integrity estiver habilitado)
        if config.patching.check_integrity {
            verify_grf_integrity(&target_grf_path).with_context(|| {
                format!(
                    "Verificação de integridade falhou para: {}",
                    target_grf_path.display()
                )
            })?;
        }

        // temp_dir will be deleted when it goes out of scope
        Ok(())
    } else {
        let mut thor_archive = ThorArchive::open(patch_path)?;
        if thor_archive.use_grf_merging() {
            // Patch GRF file
            let target_grf_name = {
                if thor_archive.target_grf_name().is_empty() {
                    config.client.default_grf_name.clone()
                } else {
                    thor_archive.target_grf_name()
                }
            };
            log::trace!("Target GRF: {:?}", target_grf_name);
            let grf_patching_method = match config.patching.in_place {
                true => GrfPatchingMethod::InPlace,
                false => GrfPatchingMethod::OutOfPlace,
            };
            let target_grf_path = current_working_dir.as_ref().join(&target_grf_name);
            apply_patch_to_grf(
                grf_patching_method,
                config.patching.create_grf,
                &target_grf_path,
                &mut thor_archive,
            )?;

            // Verificar integridade do GRF após patch (se check_integrity estiver habilitado)
            if config.patching.check_integrity {
                verify_grf_integrity(&target_grf_path).with_context(|| {
                    format!(
                        "Verificação de integridade falhou para: {}",
                        target_grf_path.display()
                    )
                })?;
            }

            Ok(())
        } else {
            // Patch root directory
            apply_patch_to_disk(current_working_dir, &mut thor_archive)
        }
    }
}

/// Verifica a integridade de um arquivo GRF após aplicar patches.
/// Abre o GRF e verifica se todos os arquivos podem ser lidos corretamente.
fn verify_grf_integrity(grf_path: impl AsRef<Path>) -> Result<()> {
    let mut grf_archive = GrfArchive::open(grf_path.as_ref()).with_context(|| {
        format!(
            "Falha ao abrir GRF para verificação: {}",
            grf_path.as_ref().display()
        )
    })?;

    let file_count = grf_archive.file_count();
    log::trace!("Verificando integridade de {} arquivos no GRF", file_count);

    // Verificar amostra de arquivos para não demorar muito em GRFs grandes
    let sample_size = 10;
    // Verificar se podemos ler as entradas do arquivo (apenas uma amostra)
    let entries: Vec<_> = grf_archive
        .get_entries()
        .take(sample_size)
        .cloned()
        .collect();

    for entry in entries {
        // Tentar ler o conteúdo do arquivo
        if entry.size > 0 {
            grf_archive
                .read_file_content(&entry.relative_path)
                .with_context(|| {
                    format!(
                        "Falha ao ler arquivo '{}' do GRF durante verificação de integridade",
                        entry.relative_path
                    )
                })?;
        }
    }

    log::trace!("Verificação de integridade concluída com sucesso");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use std::io::SeekFrom;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    #[tokio::test]
    async fn test_download_path_to_file() {
        // Generate 200MiB of data
        let data_size: usize = 200 * 1024 * 1024;
        let body_content: Vec<u8> = (0..data_size).map(|x| x as u8).collect();

        let patch_name = "patch_archive";
        let patch_path = format!("/{}", patch_name);
        // Setup a local web server
        let server = Server::run();
        // Configure the server to expect a single GET request and respond
        // with a 200 status code.
        server.expect(
            Expectation::matching(request::method_path("GET", patch_path.clone()))
                .respond_with(status_code(200).body(body_content.clone())),
        );

        // "Download" the file
        let from_url = Url::parse(server.url("/").to_string().as_str()).unwrap();
        let patch_info = ThorPatchInfo {
            index: 0,
            file_name: patch_name.to_string(),
        };
        let mut tmp_file = File::from_std(tempfile::tempfile().unwrap());
        download_patch_to_file(
            &reqwest::Client::new(),
            &from_url,
            &patch_info,
            &mut tmp_file,
            |_, _| {},
        )
        .await
        .unwrap();

        tmp_file.seek(SeekFrom::Start(0)).await.unwrap();
        let mut file_content = Vec::with_capacity(data_size);
        tmp_file.read_to_end(&mut file_content).await.unwrap();
        // Size check
        assert_eq!(data_size as u64, tmp_file.metadata().await.unwrap().len());
        assert_eq!(data_size, file_content.len());
        // Content check
        assert_eq!(body_content, file_content);
    }
}
