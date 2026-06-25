use std::io::Read;

use crate::{GrufError, Result};
use flate2::read::GzDecoder;

#[derive(Debug)]
pub enum RgzEntry {
    File { path: String, data: Vec<u8> },
    Directory { path: String },
}

pub struct RgzArchive {
    entries: Vec<RgzEntry>,
}

impl RgzArchive {
    pub fn open<R: Read>(reader: R) -> Result<Self> {
        let mut decoder = GzDecoder::new(reader);
        let mut entries = Vec::new();

        loop {
            let mut type_buf = [0u8; 1];
            if decoder.read_exact(&mut type_buf).is_err() {
                break; // End of stream or error
            }
            let entry_type = type_buf[0];

            let mut len_buf = [0u8; 1];
            if decoder.read_exact(&mut len_buf).is_err() {
                return Err(GrufError::parsing_error(
                    "Invalid RGZ: missing filename length",
                ));
            }
            let name_len = len_buf[0] as usize;

            let mut name_buf = vec![0u8; name_len];
            if decoder.read_exact(&mut name_buf).is_err() {
                return Err(GrufError::parsing_error("Invalid RGZ: missing filename"));
            }

            let name = String::from_utf8_lossy(&name_buf)
                .trim_end_matches('\0')
                .to_string();

            match entry_type {
                b'f' => {
                    let mut data_len_buf = [0u8; 4];
                    if decoder.read_exact(&mut data_len_buf).is_err() {
                        return Err(GrufError::parsing_error(
                            "Invalid RGZ: missing file data length",
                        ));
                    }
                    let data_len = u32::from_le_bytes(data_len_buf) as usize;

                    let mut data = vec![0u8; data_len];
                    if decoder.read_exact(&mut data).is_err() {
                        return Err(GrufError::parsing_error("Invalid RGZ: missing file data"));
                    }

                    entries.push(RgzEntry::File { path: name, data });
                }
                b'd' => {
                    entries.push(RgzEntry::Directory { path: name });
                }
                b'e' | b'E' => {
                    break;
                }
                _ => {
                    return Err(GrufError::parsing_error(format!(
                        "Invalid RGZ: unknown entry type '{}'",
                        entry_type as char
                    )))
                }
            }
        }

        Ok(Self { entries })
    }

    pub fn get_entries(&self) -> &[RgzEntry] {
        &self.entries
    }

    pub fn take_entries(self) -> Vec<RgzEntry> {
        self.entries
    }
}
