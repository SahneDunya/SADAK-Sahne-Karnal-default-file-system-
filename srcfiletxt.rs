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

pub struct TxtFile {
    pub path: String,
}

impl TxtFile {
    pub fn new(path: String) -> TxtFile {
        TxtFile { path }
    }

    pub fn read_lines(&self) -> Result<Vec<String>, super::SahneError> {
        match super::fs::open(&self.path, super::fs::O_RDONLY) {
            Ok(fd) => {
                let mut buffer = [0u8; 4096]; // Örnek bir buffer boyutu
                let mut lines = Vec::new();
                let mut current_pos = 0;

                loop {
                    match super::fs::read(fd, &mut buffer[current_pos..]) {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                break; // Dosyanın sonuna gelindi
                            }
                            current_pos += bytes_read;
                            // Şu ana kadar okunan veriyi satırlara ayır
                            if let Ok(content) = core::str::from_utf8(&buffer[..current_pos]) {
                                for line in content.split('\n') {
                                    if !line.is_empty() {
                                        lines.push(line.to_string());
                                    }
                                }
                            } else {
                                super::fs::close(fd).unwrap_or_else(|e| {
                                    eprintln!("Dosya kapatılırken hata: {:?}", e);
                                });
                                return Err(super::SahneError::InvalidParameter); // UTF-8 hatası
                            }
                            // Bir sonraki okuma için buffer'ı temizle (basit yaklaşım)
                            current_pos = 0;
                        }
                        Err(e) => {
                            super::fs::close(fd).unwrap_or_else(|e| {
                                eprintln!("Dosya kapatılırken hata: {:?}", e);
                            });
                            return Err(e);
                        }
                    }
                }
                super::fs::close(fd).unwrap_or_else(|e| {
                    eprintln!("Dosya kapatılırken hata: {:?}", e);
                });
                Ok(lines)
            }
            Err(e) => Err(e),
        }
    }

    pub fn write_lines(&self, lines: &[String]) -> Result<(), super::SahneError> {
        match super::fs::open(&self.path, super::fs::O_WRONLY | super::fs::O_CREAT | super::fs::O_TRUNC) {
            Ok(fd) => {
                for line in lines {
                    let line_with_newline = format!("{}\n", line);
                    let bytes = line_with_newline.as_bytes();
                    let mut written = 0;
                    while written < bytes.len() {
                        match super::fs::write(fd, &bytes[written..]) {
                            Ok(bytes_written) => {
                                written += bytes_written;
                            }
                            Err(e) => {
                                super::fs::close(fd).unwrap_or_else(|e| {
                                    eprintln!("Dosya kapatılırken hata: {:?}", e);
                                });
                                return Err(e);
                            }
                        }
                    }
                }
                super::fs::close(fd).unwrap_or_else(|e| {
                    eprintln!("Dosya kapatılırken hata: {:?}", e);
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn append_line(&self, line: &str) -> Result<(), super::SahneError> {
        // Append modu doğrudan olmayabilir, bu yüzden okuyup ekleyip tekrar yazabiliriz veya
        // eğer işletim sistemi destekliyorsa O_APPEND flag'i kullanılabilir.
        // Şimdilik basit bir yaklaşımla, dosyayı okuyup sonuna ekleyip tekrar yazalım.
        match self.read_lines() {
            Ok(mut existing_lines) => {
                existing_lines.push(line.to_string());
                self.write_lines(&existing_lines)
            }
            Err(super::SahneError::FileNotFound) => {
                // Dosya yoksa oluştur ve satırı yaz
                self.write_lines(&[line.to_string()])
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "std")] // Testler için std özelliğini kullanmaya devam edebiliriz
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_read_write() {
        let path = "test.txt".to_string();
        let file = TxtFile::new(path.clone());

        let lines_to_write = vec!["Sahne Line 1".to_string(), "Sahne Line 2".to_string()];
        file.write_lines(&lines_to_write).unwrap();

        let read_lines = file.read_lines().unwrap();
        assert_eq!(lines_to_write, read_lines);

        fs::remove_file(path).unwrap(); // Test dosyasını temizle
    }

    #[test]
    fn test_append() {
        let path = "test_append.txt".to_string();
        let file = TxtFile::new(path.clone());

        file.append_line("Sahne First line").unwrap();
        file.append_line("Sahne Second line").unwrap();

        let expected_lines = vec!["Sahne First line".to_string(), "Sahne Second line".to_string()];
        let read_lines = file.read_lines().unwrap();
        assert_eq!(expected_lines, read_lines);

        fs::remove_file(path).unwrap(); // Test dosyasını temizle
    }
}