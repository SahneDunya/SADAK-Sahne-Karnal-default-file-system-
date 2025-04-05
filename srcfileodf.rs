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

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64", target_arch = "sparc64", target_arch = "openrisc", target_arch = "powerpc64", target_arch = "loongarch64", target_arch = "elbrus", target_arch = "mips64"))]
pub mod arch {
    pub const SYSCALL_FILE_OPEN: u64 = 5;
    pub const SYSCALL_FILE_READ: u64 = 6;
    pub const SYSCALL_FILE_CLOSE: u64 = 8;
}

#[derive(Debug)]
pub enum SahneError {
    FileNotFound,
    PermissionDenied,
    InvalidFileDescriptor,
    UnknownSystemCall,
    OutOfMemory, // Gerekli olabilir
    InvalidParameter, // Gerekli olabilir
    // ... diğer hatalar
}

extern "sysv64" {
    fn syscall(number: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64;
}

pub mod fs {
    use super::{SahneError, arch, syscall};

    pub const O_RDONLY: u32 = 0;

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

pub struct OdfDocument {
    pub content: String,
    // Diğer ODF meta verileri ve içerik alanları burada eklenebilir.
}

impl OdfDocument {
    pub fn new(file_path: &str) -> Result<OdfDocument, SahneError> {
        let fd = fs::open(file_path, fs::O_RDONLY)?;
        const BUFFER_SIZE: usize = 1024;
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut content = String::new();

        loop {
            match fs::read(fd, &mut buffer) {
                Ok(0) => break, // Dosyanın sonuna gelindi
                Ok(bytes_read) => {
                    // Burada basitçe okunan byte'ları String'e dönüştürüyoruz.
                    // Gerçek bir XML ayrıştırması daha karmaşık olacaktır.
                    if let Ok(s) = core::str::from_utf8(&buffer[..bytes_read]) {
                        content.push_str(s);
                    } else {
                        // UTF-8 dönüşümü başarısız olursa bir hata döndürebiliriz.
                        fs::close(fd)?;
                        return Err(SahneError::InvalidParameter); // Veya daha uygun bir hata türü
                    }
                }
                Err(e) => {
                    fs::close(fd)?;
                    return Err(e);
                }
            }
        }

        fs::close(fd)?;

        Ok(OdfDocument {
            content,
        })
    }
}

#[cfg(feature = "std")] // Testler için standart kütüphaneyi kullanmaya devam edebiliriz
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_odf_parsing_sahne64() {
        // Test için basit bir content.xml dosyası oluştur
        let mut test_file = fs::File::create("test_content.xml").unwrap();
        test_file.write_all(b"<office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\">\
                               <office:body>\
                                 <office:text>\
                                   <text:p xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">Sahne64 Test Metni.</text:p>\
                                 </office:text>\
                               </office:body>\
                             </office:document-content>").unwrap();

        match OdfDocument::new("test_content.xml") {
            Ok(odf_document) => {
                assert_eq!(odf_document.content.contains("Sahne64 Test Metni."), true);
            }
            Err(e) => panic!("Dosya okuma hatası: {:?}", e),
        }

        // Test dosyasını sil
        fs::remove_file("test_content.xml").unwrap();
    }
}

// Standart kütüphanenin bazı temel fonksiyonlarının (örneğin println!) kendi implementasyonunuz
// veya harici bir crate (örneğin core::fmt) kullanılarak sağlanması gerekebilir.
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Burada gerçek çıktı mekanizmasına (örneğin, bir UART sürücüsüne) erişim olmalı.
            // Bu örnekte, çıktı kaybolacaktır çünkü gerçek bir çıktı yok.
            // Gerçek bir işletim sisteminde, bu kısım donanıma özel olacaktır.
            Ok(())
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => ({
            let mut stdout = $crate::print::Stdout;
            core::fmt::write(&mut stdout, core::format_args!($($arg)*)).unwrap();
        });
    }

    #[macro_export]
    macro_rules! println {
        () => ($crate::print!("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
    }
}