#![no_std]
#![allow(dead_code)]

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

pub mod fs {
    use super::{SahneError, arch, syscall};

    pub const O_RDONLY: u32 = 0;
    // ... diğer dosya açma modları ...

    pub fn open(path: &str, flags: u32) -> Result<u64, SahneError> { /* ... */ Ok(0) }
    pub fn read(fd: u64, buffer: &mut [u8]) -> Result<usize, SahneError> { /* ... */ Ok(0) }
    pub fn close(fd: u64) -> Result<(), SahneError> { /* ... */ Ok(()) }
}

pub struct JsonFile {
    pub content: String, // JSON içeriğini bir String olarak saklayacağız (basitleştirilmiş)
}

impl JsonFile {
    pub fn new(path: &str) -> Result<Self, SahneError> {
        match fs::open(path, fs::O_RDONLY) {
            Ok(fd) => {
                const BUFFER_SIZE: usize = 1024; // Uygun bir tampon boyutu seçin
                let mut buffer = [0u8; BUFFER_SIZE];
                let mut content = String::new();
                loop {
                    match fs::read(fd, &mut buffer) {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                break; // Dosyanın sonuna gelindi
                            }
                            match core::str::from_utf8(&buffer[..bytes_read]) {
                                Ok(s) => content.push_str(s),
                                Err(_) => {
                                    fs::close(fd)?;
                                    return Err(SahneError::InvalidParameter); // Veya daha uygun bir hata türü
                                }
                            }
                        }
                        Err(e) => {
                            fs::close(fd)?;
                            return Err(e);
                        }
                    }
                }
                fs::close(fd)?;
                Ok(JsonFile { content })
            }
            Err(e) => Err(e),
        }
    }

    // JSON verilerini ayrıştırmak için basit bir yardımcı fonksiyon (çok temel bir örnek)
    pub fn get_value(&self, key: &str) -> Option<&str> {
        let key_with_quotes = format!("\"{}\"", key);
        if let Some(start) = self.content.find(&key_with_quotes) {
            let value_start = self.content[start + key_with_quotes.len()..].find(':')?;
            let start_index = start + key_with_quotes.len() + value_start + 1;
            let mut end_index = start_index;
            let mut in_string = false;
            let mut bracket_level = 0;
            let mut brace_level = 0;

            while end_index < self.content.len() {
                let char = self.content.chars().nth(end_index)?;
                match char {
                    '"' => in_string = !in_string,
                    '[' if !in_string => bracket_level += 1,
                    ']' if !in_string => bracket_level -= 1,
                    '{' if !in_string => brace_level += 1,
                    '}' if !in_string => brace_level -= 1,
                    ',' if !in_string && bracket_level == 0 && brace_level == 0 => break,
                    ' ' | '\t' | '\n' | '\r' if !in_string && bracket_level == 0 && brace_level == 0 && end_index == start_index => start_index += 1, // Başlangıçtaki boşlukları atla
                    _ => {}
                }
                if !in_string && bracket_level == 0 && brace_level == 0 && char == ',' && end_index > start_index {
                    break;
                }
                if !in_string && bracket_level == 0 && brace_level == 0 && char == '}' && end_index > start_index {
                    break;
                }
                if !in_string && bracket_level == 0 && brace_level == 0 && char == ']' && end_index > start_index {
                    break;
                }
                end_index += 1;
            }
            let value = self.content[start_index..end_index].trim();
            if value.starts_with('"') && value.ends_with('"') {
                return Some(&value[1..value.len() - 1]);
            }
            return Some(value);
        }
        None
    }

    // ... Diğer veri tipleri için benzer (ve daha karmaşık) get fonksiyonları yazılabilir.
    // Bu örnek sadece temel bir String alma işlemini gösteriyor.
}

// Test modülü (standart kütüphaneye bağımlı kısımlar çıkarıldı)
#[cfg(feature = "std")] // Sadece standart kütüphane varsa derlenecek
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use serde_json::json;

    #[test]
    fn test_json_file_sahne64() {
        // Geçici bir dizin ve JSON dosyası oluştur (standart kütüphane kullanılarak)
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_sahne.json");
        let mut file = File::create(&file_path).unwrap();
        let json_data = json!({
            "name": "SahneRust",
            "version": 64,
            "enabled": true
        });
        write!(file, "{}", json_data.to_string()).unwrap();

        // JsonFile örneğini oluştur ve verileri kontrol et
        let file_path_str = file_path.to_str().unwrap();
        let json_file = JsonFile::new(file_path_str).unwrap();
        assert_eq!(json_file.get_value("name"), Some("SahneRust"));
        assert_eq!(json_file.get_value("version"), Some("64"));
        assert_eq!(json_file.get_value("enabled"), Some("true"));
    }
}

// Standart kütüphane olmayan ortam için panic handler
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}