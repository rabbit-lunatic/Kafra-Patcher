//! Background monitoring thread for Discord Rich Presence.
//!
//! Spawns a daemon thread that monitors the game process and updates
//! Discord Rich Presence with the player's current game state.

#![cfg(windows)]

use std::sync::Once;
use std::thread;
use std::time::Duration;

use crate::discord_rpc::DiscordRpcManager;
use crate::memory_reader;
use crate::patcher::DiscordConfiguration;

/// Interval between process discovery attempts when the game is not running.
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(5);

/// Interval between Rich Presence updates when the game is running.
const UPDATE_INTERVAL: Duration = Duration::from_secs(10);

/// Ensures only one monitoring thread is spawned across multiple "Play" clicks.
static SPAWN_ONCE: Once = Once::new();

/// Spawn the Rich Presence monitoring thread (idempotent — only runs once).
///
/// Extracts the executable basename from `exe_path` (e.g., `"client/ragexe.exe"` → `"ragexe.exe"`)
/// and uses it to locate the running game process.
pub fn spawn_rich_presence_thread(exe_path: String, discord_config: DiscordConfiguration) {
    SPAWN_ONCE.call_once(move || {
        let result = thread::Builder::new()
            .name("rich-presence-monitor".into())
            .spawn(move || {
                rich_presence_loop(&exe_path, discord_config);
            });
        if let Err(e) = result {
            log::error!("[RichPresence] Failed to spawn monitor thread: {}", e);
        }
    });
}

/// Main monitoring loop. Runs until the patcher process exits.
///
/// State machine:
/// 1. Wait for game process to appear
/// 2. Attach to process, get base address
/// 3. Connect to Discord
/// 4. Poll game memory and update presence every UPDATE_INTERVAL
/// 5. If game exits, clear presence and go back to step 1
fn rich_presence_loop(exe_name: &str, discord_config: DiscordConfiguration) {
    log::info!("[RichPresence] Monitor thread started for '{}'", exe_name);

    loop {
        // Phase 1: Wait for the game process
        let pid = loop {
            if let Some(pid) = memory_reader::find_game_process(exe_name) {
                log::info!("[RichPresence] Found game process PID: {}", pid);
                break pid;
            }
            thread::sleep(DISCOVERY_INTERVAL);
        };

        // Phase 2: Open process and get base address
        let proc_handle = match memory_reader::open_process(pid) {
            Some(h) => h,
            None => {
                log::warn!("[RichPresence] Failed to open process PID {}", pid);
                thread::sleep(DISCOVERY_INTERVAL);
                continue;
            }
        };

        let base_address = match memory_reader::get_module_base(&proc_handle, exe_name) {
            Some(base) => {
                log::info!("[RichPresence] Module base address: 0x{:X}", base);
                base
            }
            None => {
                log::warn!("[RichPresence] Failed to get module base for PID {}", pid);
                thread::sleep(DISCOVERY_INTERVAL);
                continue;
            }
        };

        // Phase 3: Connect to Discord
        let mut rpc = DiscordRpcManager::new(discord_config.clone());

        if !rpc.connect() {
            log::warn!("[RichPresence] Could not connect to Discord, will retry");
            thread::sleep(DISCOVERY_INTERVAL);
            continue;
        }

        // Phase 4: Update loop
        log::info!("[RichPresence] Entering update loop");
        loop {
            match memory_reader::read_game_data(&proc_handle, base_address) {
                Some(game_data) => {
                    log::info!(
                        "[RichPresence] Read: '{}' on '{}' Lv {}/{}",
                        game_data.player_name,
                        game_data.map_name,
                        game_data.base_level,
                        game_data.job_level
                    );
                    rpc.update_presence(&game_data);
                }
                None => {
                    // Process likely closed
                    log::info!("[RichPresence] Game process closed, clearing presence");
                    rpc.clear_presence();
                    rpc.disconnect();
                    break;
                }
            }
            thread::sleep(UPDATE_INTERVAL);
        }

        // Back to phase 1 — wait for game to reopen
        log::info!("[RichPresence] Waiting for game to restart...");
    }
}
