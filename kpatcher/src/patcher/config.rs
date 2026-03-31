use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::get_patcher_name;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
    Nonce, // Or `Key`
};
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;

/// Marker to identify embedded config at the end of the EXE
const CONFIG_MARKER: &[u8; 4] = b"KCFG";

/// XOR mask used to obfuscate the encryption key in the binary.
const XOR_MASK: [u8; 32] = [
    0xA7, 0x3B, 0x5C, 0x9E, 0x12, 0xF4, 0x68, 0xD1, 0x83, 0x47, 0xB2, 0x0F, 0xE5, 0x6A, 0x91, 0xC3,
    0x2D, 0x78, 0xF0, 0x14, 0x56, 0xAB, 0x39, 0xE7, 0x04, 0x8C, 0xD5, 0x63, 0xB9, 0x1E, 0x72, 0x4F,
];

/// Obfuscated encryption key (result of original key XOR'd with XOR_MASK).
/// IMPORTANT: Keep in sync with mkpatch/src/embed.rs
const OBFUSCATED_KEY: [u8; 32] = {
    let key = b"kpatcher_secret_key_32_bytes!!!!";
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = key[i] ^ XOR_MASK[i];
        i += 1;
    }
    out
};

/// Derives the encryption key by reversing the XOR obfuscation.
fn derive_encryption_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    for i in 0..32 {
        key[i] = OBFUSCATED_KEY[i] ^ XOR_MASK[i];
    }
    key
}

#[derive(Deserialize, Clone)]
pub struct PatcherConfiguration {
    pub window: WindowConfiguration,
    pub play: PlayConfiguration,
    pub setup: SetupConfiguration,
    pub web: WebConfiguration,
    pub client: ClientConfiguration,
    pub patching: PatchingConfiguration,
    #[serde(default)]
    pub discord: Option<DiscordConfiguration>,
}

#[derive(Deserialize, Clone)]
pub struct WindowConfiguration {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub resizable: bool,
    pub frameless: Option<bool>,
    pub dllwebview: Option<String>,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub border_radius: Option<i32>,
}

#[derive(Deserialize, Clone)]
pub struct PlayConfiguration {
    pub path: String,
    pub arguments: Vec<String>,
    pub exit_on_success: Option<bool>,
    pub play_with_error: Option<bool>,
    pub minimize_on_start: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct SetupConfiguration {
    pub path: String,
    pub arguments: Vec<String>,
    pub exit_on_success: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct WebConfiguration {
    pub index_url: String, // URL of the index file implementing the UI
    pub preferred_patch_server: Option<String>, // Name of the patch server to use in priority
    pub patch_servers: Vec<PatchServerInfo>,
}

#[derive(Deserialize, Clone)]
pub struct PatchServerInfo {
    pub name: String,      // Name of that identifies the patch server
    pub plist_url: String, // URL of the plist.txt file
    pub patch_url: String, // URL of the directory containing .thor files
}

#[derive(Deserialize, Clone)]
pub struct ClientConfiguration {
    pub default_grf_name: String, // GRF file to patch by default
}

#[derive(Deserialize, Clone)]
pub struct PatchingConfiguration {
    pub in_place: bool,        // In-place GRF patching
    pub check_integrity: bool, // Check THOR archives' integrity
    pub create_grf: bool,      // Create new GRFs if they don't exist
    #[serde(default)]
    pub concurrent_downloads: Option<usize>, // Max concurrent downloads (default: 8)
}

#[derive(Deserialize, Clone)]
pub struct DiscordConfiguration {
    pub client_id: String, // Discord Application ID
    #[serde(default = "default_large_image")]
    pub large_image: String, // Large image asset key
    #[serde(default = "default_large_text")]
    pub large_text: String, // Tooltip for large image
    #[serde(default = "default_small_image")]
    pub small_image: String, // Small image asset key
    #[serde(default = "default_small_text")]
    pub small_text: String, // Tooltip for small image
    #[serde(default)]
    pub custom_maps: HashMap<String, String>, // User-defined custom map translations
}

fn default_large_image() -> String {
    "logo".to_string()
}
fn default_large_text() -> String {
    "Ragnarok Online".to_string()
}
fn default_small_image() -> String {
    "classe_icon".to_string()
}
fn default_small_text() -> String {
    "Jogando".to_string()
}

pub fn retrieve_patcher_configuration(
    config_file_path: Option<PathBuf>,
) -> Result<PatcherConfiguration> {
    // Primeiro, tentar extrair config embutido do próprio executável
    if let Ok(config) = extract_embedded_config() {
        log::info!("Using embedded configuration");
        return Ok(config);
    }

    // Fallback: carregar de arquivo externo
    let patcher_name = get_patcher_name()?;
    let config_file_path =
        config_file_path.unwrap_or_else(|| PathBuf::from(patcher_name).with_extension("yml"));
    log::info!("Loading configuration from: {}", config_file_path.display());
    parse_configuration(config_file_path)
}

/// Tenta extrair configuração embutida no final do executável.
///
/// Formato esperado: [EXE] + [YAML gzip] + [tamanho u32 LE] + [marcador "KCFG"]
fn extract_embedded_config() -> Result<PatcherConfiguration> {
    let exe_path = env::current_exe().context("Failed to get current executable path")?;
    let mut file = File::open(&exe_path).context("Failed to open executable")?;

    let file_size = file.metadata()?.len();
    if file_size < 8 {
        anyhow::bail!("File too small to contain embedded config");
    }

    // Ler os últimos 8 bytes (tamanho + marcador)
    file.seek(SeekFrom::End(-8))?;
    let mut footer = [0u8; 8];
    file.read_exact(&mut footer)?;

    // Verificar marcador
    if &footer[4..8] != CONFIG_MARKER {
        anyhow::bail!("No embedded config marker found");
    }

    // Tamanho total do bundle (Nonce + Ciphertext)
    let bundle_size = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]) as u64;

    // Validar tamanho
    if bundle_size == 0 || bundle_size > file_size - 8 {
        anyhow::bail!("Invalid embedded config size");
    }

    // Posicionar no início do bundle
    let bundle_start = file_size - 8 - bundle_size;
    file.seek(SeekFrom::Start(bundle_start))?;

    // Ler o bundle completo
    let mut bundle_data = vec![0u8; bundle_size as usize];
    file.read_exact(&mut bundle_data)?;

    if bundle_data.len() < 12 {
        anyhow::bail!("Embedded config too short to contain nonce");
    }

    // Separar Nonce e Ciphertext
    let (nonce_bytes, ciphertext) = bundle_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Descriptografar
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key));
    let compressed_data = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Failed to decrypt embedded config: {}", e))?;

    // Descomprimir
    let mut decoder = GzDecoder::new(&compressed_data[..]);
    let mut yaml_content = String::new();
    decoder
        .read_to_string(&mut yaml_content)
        .context("Failed to decompress embedded config")?;

    // Parse YAML
    serde_yaml::from_str(&yaml_content).context("Invalid embedded configuration")
}

fn parse_configuration(config_file_path: impl AsRef<Path>) -> Result<PatcherConfiguration> {
    let config_file = File::open(config_file_path)?;
    let config_reader = BufReader::new(config_file);
    serde_yaml::from_reader(config_reader).context("Invalid configuration")
}
