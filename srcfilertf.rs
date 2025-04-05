#![no_std] // Eğer bu dosya Sahne64 içinde kullanılacaksa bu satır kalmalı

// Gerekli Sahne64 modüllerini içeri aktar
#[cfg(not(feature = "std"))]
use crate::{
    fs,
    memory,
    process,
    sync,
    kernel,
    SahneError,
    arch,
};

use super::fs;
use super::SahneError;

pub struct RtfFile {
    pub content: String,
}

impl RtfFile {
    pub fn new(path: &str) -> Result<Self, SahneError> {
        match fs::open(path, fs::O_RDONLY) {
            Ok(fd) => {
                let mut content = String::new();
                let mut buffer = [0u8; 128]; // Okuma arabelleği
                loop {
                    match fs::read(fd, &mut buffer) {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                break; // Dosyanın sonuna gelindi
                            }
                            content.push_str(&Self::parse_rtf(&buffer[..bytes_read]));
                        }
                        Err(e) => {
                            fs::close(fd).unwrap_or_else(|_| {}); // Hata durumunda dosyayı kapat
                            return Err(e);
                        }
                    }
                }
                fs::close(fd)?;
                Ok(Self { content })
            }
            Err(e) => Err(e),
        }
    }

    fn parse_rtf(buffer: &[u8]) -> String {
        let mut content = String::new();
        let mut i = 0;

        while i < buffer.len() {
            if buffer[i] == b'\\' {
                i += 1;
                if i < buffer.len() {
                    match buffer[i] {
                        b'{' | b'}' | b'\\' => content.push(buffer[i] as char),
                        _ => {
                            // Basit kontrol kelimelerini atla
                            while i < buffer.len() && buffer[i].is_ascii_alphabetic() {
                                i += 1;
                            }
                            if i < buffer.len() && buffer[i] == b';' {
                                i += 1;
                            }
                            continue;
                        }
                    }
                }
            } else if buffer[i] == b'{' || buffer[i] == b'}' {
                // Grupları şimdilik yoksay
            } else {
                content.push(buffer[i] as char);
            }
            i += 1;
        }

        content
    }
}