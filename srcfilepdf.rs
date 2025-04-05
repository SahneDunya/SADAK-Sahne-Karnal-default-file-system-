#![no_std]
#![allow(dead_code)]

// Gerekli modülleri ve sabitleri içe aktar
pub mod arch {
    pub const SYSCALL_FILE_OPEN: u64 = 5;
    pub const SYSCALL_FILE_READ: u64 = 6;
    pub const SYSCALL_FILE_CLOSE: u64 = 8;
}

pub mod fs {
    use super::{SahneError, arch, syscall};

    pub const O_RDONLY: u32 = 0;

    extern "sysv64" {
        fn syscall(number: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64;
    }

    pub fn open(path: &str, flags: u32) -> Result<u64, SahneError> {
        let path_ptr = path.as_ptr() as u64;
        let path_len = path.len() as u64;
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_OPEN, path_ptr, path_len, flags as u64, 0, 0)
        };
        if result < 0 {
            match result {
                -2 => Err(SahneError::FileNotFound),
                -13 => Err(SahneError::PermissionDenied),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(result as u64)
        }
    }

    pub fn read(fd: u64, buffer: &mut [u8]) -> Result<usize, SahneError> {
        let buffer_ptr = buffer.as_mut_ptr() as u64;
        let buffer_len = buffer.len() as u64;
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_READ, fd, buffer_ptr, buffer_len, 0, 0)
        };
        if result < 0 {
            match result {
                -9 => Err(SahneError::InvalidFileDescriptor),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(result as usize)
        }
    }

    pub fn close(fd: u64) -> Result<(), SahneError> {
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_CLOSE, fd, 0, 0, 0, 0)
        };
        if result < 0 {
            match result {
                -9 => Err(SahneError::InvalidFileDescriptor),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(())
        }
    }
}

pub mod memory {
    // Sahne64 bellek yönetimi fonksiyonları (gerekirse buraya eklenebilir)
}

pub mod process {
    // Sahne64 süreç yönetimi fonksiyonları (gerekirse buraya eklenebilir)
}

pub mod sync {
    // Sahne64 senkronizasyon fonksiyonları (gerekirse buraya eklenebilir)
}

pub mod ipc {
    // Sahne64 IPC fonksiyonları (gerekirse buraya eklenebilir)
}

pub mod kernel {
    // Sahne64 çekirdek fonksiyonları (gerekirse buraya eklenebilir)
}

#[derive(Debug)]
pub enum SahneError {
    OutOfMemory,
    InvalidAddress,
    InvalidParameter,
    FileNotFound,
    PermissionDenied,
    FileAlreadyExists,
    InvalidFileDescriptor,
    ResourceBusy,
    Interrupted,
    NoMessage,
    InvalidOperation,
    NotSupported,
    UnknownSystemCall,
    ProcessCreationFailed,
}

use lopdf::Document;
use lopdf::Error as LopdfError;

pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub page_count: u32,
}

fn get_metadata_string(document: &Document, key: &[u8]) -> Option<String> {
    document
        .get_trailer()
        .get(b"Info")
        .ok_or(LopdfError::DictionaryNotFound)
        .and_then(|info_obj| document.get_dictionary(info_obj))
        .ok()
        .and_then(|dict| dict.get(key))
        .and_then(|object| object.as_string().ok())
        .map(|s| s.to_string())
}

pub fn read_pdf_metadata_sahne(file_path: &str) -> Result<PdfMetadata, LopdfError> {
    let fd_result = fs::open(file_path, fs::O_RDONLY);
    let fd = match fd_result {
        Ok(file_descriptor) => file_descriptor,
        Err(e) => {
            match e {
                SahneError::FileNotFound => return Err(LopdfError::IOError(std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"))),
                SahneError::PermissionDenied => return Err(LopdfError::IOError(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied"))),
                _ => return Err(LopdfError::IOError(std::io::Error::new(std::io::ErrorKind::Other, format!("SahneError: {:?}", e)))),
            }
        }
    };

    let mut contents = Vec::new();
    let mut buffer = [0u8; 4096]; // Okuma için bir tampon oluşturuyoruz
    loop {
        let read_result = fs::read(fd, &mut buffer);
        match read_result {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break; // Dosyanın sonuna ulaşıldı
                }
                contents.extend_from_slice(&buffer[..bytes_read]);
            }
            Err(e) => {
                fs::close(fd).unwrap_or_default(); // Hata durumunda dosyayı kapat
                return Err(LopdfError::IOError(std::io::Error::new(std::io::ErrorKind::Other, format!("SahneError during read: {:?}", e))));
            }
        }
    }

    let close_result = fs::close(fd);
    if let Err(e) = close_result {
        eprintln!("Dosya kapatılırken hata oluştu: {:?}", e);
        // Kapatma hatası kritik olmasa da loglayabiliriz.
    }

    let document = Document::load_mem(&contents)?;

    let title = get_metadata_string(&document, b"Title");
    let author = get_metadata_string(&document, b"Author");
    let creator = get_metadata_string(&document, b"Creator");
    let producer = get_metadata_string(&document, b"Producer");

    let page_count = document.get_pages().len() as u32;

    Ok(PdfMetadata {
        title,
        author,
        creator,
        producer,
        page_count,
    })
}

#[cfg(feature = "std")] // Testler için standart kütüphaneyi kullanmaya devam edebiliriz
mod tests {
    use super::*;
    use std::io::Write;
    use std::fs::File;

    #[test]
    fn test_read_pdf_metadata_sahne() {
        // Minimal bir PDF içeriği (gerçek bir dosya sistemimiz olmadığı için)
        let pdf_content = include_bytes!("../assets/minimal.pdf");
        let file_path = "test_sahne.pdf"; // Farklı bir isim kullanabiliriz

        // Standart kütüphaneyi kullanarak bir test dosyası oluşturalım (şimdilik)
        let mut test_file = File::create(file_path).expect("Test dosyası oluşturulamadı");
        test_file.write_all(pdf_content).expect("PDF içeriği yazılamadı");

        match read_pdf_metadata_sahne(file_path) {
            Ok(metadata) => {
                println!("Sahne64 PDF Metadata:");
                println!("Title: {:?}", metadata.title);
                println!("Author: {:?}", metadata.author);
                println!("Creator: {:?}", metadata.creator);
                println!("Producer: {:?}", metadata.producer);
                println!("Page Count: {}", metadata.page_count);
                assert_eq!(metadata.page_count, 1, "Sayfa sayısı 1 olmalı (minimal.pdf)");
            }
            Err(e) => {
                eprintln!("PDF meta verileri okunurken hata oluştu (Sahne64): {}", e);
                panic!("Test başarısız oldu (Sahne64): {}", e);
            }
        }

        // Test dosyasını temizle (isteğe bağlı)
        std::fs::remove_file(file_path).expect("Test dosyası silinemedi");
    }
}

// Standart kütüphane olmayan ortam için gerekli panic handler
#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}