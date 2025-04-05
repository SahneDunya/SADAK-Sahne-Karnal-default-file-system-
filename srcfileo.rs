#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz (testler hariç)
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

use super::{fs, SahneError};

pub struct OFile {
    fd: u64, // Sahne64 dosya tanımlayıcısı
}

impl OFile {
    pub fn open(path: &str) -> Result<OFile, SahneError> {
        match fs::open(path, fs::O_RDONLY) {
            Ok(fd) => Ok(OFile { fd }),
            Err(e) => Err(e),
        }
    }

    pub fn read_all(&mut self) -> Result<Vec<u8>, SahneError> {
        let mut buffer = Vec::new();
        let mut temp_buffer = [0u8; 1024]; // Okuma için geçici bir tampon

        loop {
            match fs::read(self.fd, &mut temp_buffer) {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        break; // Dosyanın sonuna ulaşıldı
                    }
                    buffer.extend_from_slice(&temp_buffer[..bytes_read]);
                }
                Err(e) => return Err(e),
            }
        }

        Ok(buffer)
    }

    pub fn close(self) -> Result<(), SahneError> {
        fs::close(self.fd)
    }

    // İhtiyaca göre başka okuma yöntemleri eklenebilir.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self as std_fs, File}; // std::fs'yi farklı bir isimle içe aktar
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    // Test ortamında Sahne64 fs modülünü taklit etmek mümkün olmadığından,
    // bu testler hala standart kütüphaneyi kullanmaktadır.
    // Gerçek bir Sahne64 ortamında bu testlerin çekirdek üzerinde çalışması gerekecektir.

    #[test]
    fn test_open_and_read() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.sadak"); // SADAK uzantısını kullanalım
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Hello, Sahne64!").unwrap();
        let path_str = file_path.to_str().unwrap();

        let mut o_file = OFile::open(path_str).unwrap();
        let contents = o_file.read_all().unwrap();
        assert_eq!(contents, b"Hello, Sahne64!");
        o_file.close().unwrap();
    }

    #[test]
    fn test_open_nonexistent_file() {
        let path = "nonexistent.sadak";
        let result = OFile::open(path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SahneError::FileNotFound => (), // Doğru hata türünü kontrol ediyoruz
            _ => panic!("Yanlış hata türü döndü"),
        }
    }
}