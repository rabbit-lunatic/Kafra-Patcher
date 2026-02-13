//! Módulo para embutir configuração YAML dentro de um executável.
//! 
//! Formato do EXE com config embutido:
//! [EXE original] + [Nonce (12)] + [YAML cifrado (gzip + AES-256-GCM)] + [tamanho total u32 LE] + [marcador "KCFG"]

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce // Or `Key`
};
use rand::Rng;

/// XOR mask used to obfuscate the encryption key in the binary.
/// The actual key is derived by XOR'ing OBFUSCATED_KEY with XOR_MASK at runtime.
const XOR_MASK: [u8; 32] = [
    0xA7, 0x3B, 0x5C, 0x9E, 0x12, 0xF4, 0x68, 0xD1,
    0x83, 0x47, 0xB2, 0x0F, 0xE5, 0x6A, 0x91, 0xC3,
    0x2D, 0x78, 0xF0, 0x14, 0x56, 0xAB, 0x39, 0xE7,
    0x04, 0x8C, 0xD5, 0x63, 0xB9, 0x1E, 0x72, 0x4F,
];

/// Obfuscated encryption key (result of original key XOR'd with XOR_MASK).
/// IMPORTANT: Keep in sync with kpatcher/src/patcher/config.rs
const OBFUSCATED_KEY: [u8; 32] = {
    // Original key: b"kpatcher_secret_key_32_bytes!!!!"
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

/// Marker to identify embedded config at the end of the EXE
pub const CONFIG_MARKER: &[u8; 4] = b"KCFG";

/// Embeds the contents of a YAML file inside an executable.
/// 
/// The result is a new file that contains:
/// - The original executable
/// - O Nonce (12 bytes)
/// - O YAML comprimido e cifrado
/// - 4 bytes com o tamanho total (Nonce + Cifrado) (little-endian)
/// - 4 bytes com o marcador "KCFG"
pub fn embed_config_in_exe(
    exe_path: &Path,
    config_path: &Path,
    output_path: &Path,
) -> Result<()> {
    // Ler o executável original
    let exe_data = fs::read(exe_path)
        .with_context(|| format!("Failed to read executable: {}", exe_path.display()))?;

    // Ler e comprimir o config YAML
    let config_data = fs::read(config_path)
        .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
    
    let compressed_config = compress_data(&config_data)?;
    
    // Criptografar
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, compressed_config.as_ref())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    // Tamanho total = Nonce + Ciphertext
    let total_size = (nonce_bytes.len() + ciphertext.len()) as u32;

    // Criar arquivo de saída
    let mut output_file = File::create(output_path)
        .with_context(|| format!("Failed to create output: {}", output_path.display()))?;

    // Escrever: EXE + Nonce + Ciphertext + Tamanho + Marcador
    output_file.write_all(&exe_data)?;
    output_file.write_all(&nonce_bytes)?;
    output_file.write_all(&ciphertext)?;
    output_file.write_all(&total_size.to_le_bytes())?;
    output_file.write_all(CONFIG_MARKER)?;

    log::info!(
        "Config embedded successfully. Original: {} bytes, Encrypted Bundle: {} bytes",
        exe_data.len(),
        total_size
    );

    Ok(())
}

/// Comprime dados usando gzip
fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish().context("Failed to compress data")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};
    use flate2::read::GzDecoder;

    #[test]
    fn test_compress_encrypt_decrypt_decompress() {
        let original = b"Hello, World! This is a test YAML config.";
        
        // 1. Compress
        let compressed = compress_data(original).unwrap();
        
        // 2. Encrypt
        let key = derive_encryption_key();
        let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, compressed.as_ref()).unwrap();
        
        // 3. Decrypt
        let plaintext_compressed = cipher.decrypt(nonce, ciphertext.as_ref()).unwrap();
        
        // 4. Decompress
        let mut decoder = GzDecoder::new(Cursor::new(plaintext_compressed));
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }
}
