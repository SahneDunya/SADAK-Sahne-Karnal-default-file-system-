#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

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

use core::result::Result;
use core::collections::HashMap;
use core::str;

pub struct MakelifeFile {
    data: HashMap<String, String>,
}

impl MakelifeFile {
    pub fn new(file_path: &str) -> Result<MakelifeFile, super::SahneError> {
        let fd_result = super::fs::open(file_path, super::fs::O_RDONLY);
        let fd = match fd_result {
            Ok(file_descriptor) => file_descriptor,
            Err(e) => return Err(e),
        };

        let mut data = HashMap::new();
        const BUFFER_SIZE: usize = 128; // Okuma arabelleği boyutu
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut current_line = String::new();

        loop {
            let read_result = super::fs::read(fd, &mut buffer);
            match read_result {
                Ok(bytes_read) if bytes_read > 0 => {
                    if let Ok(s) = str::from_utf8(&buffer[..bytes_read]) {
                        current_line.push_str(s);
                        while let Some(newline_pos) = current_line.find('\n') {
                            let line = current_line.drain(..newline_pos + 1).collect::<String>();
                            if let Some((key, value)) = line.trim().split_once('=') {
                                data.insert(key.to_string(), value.trim().to_string());
                            }
                        }
                    } else {
                        let _ = super::fs::close(fd);
                        return Err(super::SahneError::InvalidParameter); // Dosya UTF-8 değil
                    }
                }
                Ok(_) => { // bytes_read == 0, dosyanın sonu
                    // Son satırı işle
                    if !current_line.is_empty() {
                        if let Some((key, value)) = current_line.trim().split_once('=') {
                            data.insert(key.to_string(), value.trim().to_string());
                        }
                    }
                    break;
                }
                Err(e) => {
                    let _ = super::fs::close(fd);
                    return Err(e);
                }
            }
        }

        let close_result = super::fs::close(fd);
        if let Err(e) = close_result {
            return Err(e);
        }

        Ok(MakelifeFile { data })
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
}

// #[cfg(test)] // Test bölümünü no_std ortamında doğrudan çalıştıramayız
// mod tests {
//     use super::*;
//     // ... Test kodu burada olurdu, ancak std::fs::write ve tempfile kullanamayız.
// }