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
    blockdevice::BlockDevice,
};

#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(not(feature = "std"))]
use core::result::Result;

#[cfg(feature = "std")]
use crate::blockdevice::BlockDevice;
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(feature = "std")]
use std::io;
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(feature = "std")]
use std::fmt;

// Özel hata türü tanımlıyoruz
#[derive(Debug)]
pub enum BlockDeviceError {
    #[cfg(feature = "std")]
    IOError(io::Error),
    #[cfg(not(feature = "std"))]
    IOError(SahneError),
    BlockSizeError(String), // Örneğin, arabellek boyutu blok boyutuna uygun değilse
}

impl fmt::Display for BlockDeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockDeviceError::IOError(e) => write!(f, "Giriş/Çıkış Hatası: {}", e),
            BlockDeviceError::BlockSizeError(msg) => write!(f, "Blok Boyutu Hatası: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl Error for BlockDeviceError {}

#[cfg(feature = "std")]
impl From<io::Error> for BlockDeviceError {
    fn from(error: io::Error) -> Self {
        BlockDeviceError::IOError(error)
    }
}

pub struct HDD {
    #[cfg(feature = "std")]
    file: File,
    #[cfg(not(feature = "std"))]
    fd: u64,
    block_size: usize,
}

impl HDD {
    // Dosyayı hem okuma hem de yazma modunda açıyoruz ve hata türünü iyileştiriyoruz
    #[cfg(feature = "std")]
    pub fn new(path: &str, block_size: usize) -> Result<Self, BlockDeviceError> {
        if block_size == 0 {
            return Err(BlockDeviceError::BlockSizeError("Blok boyutu sıfır olamaz!".to_string()));
        }
        let file = File::options()
            .read(true)  // Okuma izni
            .write(true) // Yazma izni
            .create(true) // Dosya yoksa oluştur
            .open(path)?;
        Ok(HDD { file, block_size })
    }

    #[cfg(not(feature = "std"))]
    pub fn new(path: &str, block_size: usize) -> Result<Self, BlockDeviceError> {
        if block_size == 0 {
            return Err(BlockDeviceError::BlockSizeError("Blok boyutu sıfır olamaz!".to_string()));
        }
        let flags = fs::O_RDWR | fs::O_CREAT;
        let fd = fs::open(path, flags)?;
        Ok(HDD { fd, block_size })
    }
}

#[cfg(feature = "std")]
impl BlockDevice for HDD {
    // Daha spesifik hata türü kullanıyoruz ve arabellek boyutunu kontrol ediyoruz
    fn read_block(&mut self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockDeviceError> {
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Arabellek boyutu blok boyutuna ({}) eşit olmalı, fakat {} boyutunda.", self.block_size, buf.len())
            ));
        }
        let offset = block_id * self.block_size;
        self.file.seek(SeekFrom::Start(offset as u64))?;
        self.file.read_exact(buf)?;
        Ok(())
    }

    // Daha spesifik hata türü kullanıyoruz ve arabellek boyutunu kontrol ediyoruz
    fn write_block(&mut self, block_id: usize, buf: &[u8]) -> Result<(), BlockDeviceError> {
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Arabellek boyutu blok boyutuna ({}) eşit olmalı, fakat {} boyutunda.", self.block_size, buf.len())
            ));
        }
        let offset = block_id * self.block_size;
        self.file.seek(SeekFrom::Start(offset as u64))?;
        self.file.write_all(buf)?;
        Ok(())
    }

    fn block_size(&self) -> usize {
        self.block_size
    }
}

#[cfg(not(feature = "std"))]
impl BlockDevice for HDD {
    fn read_block(&mut self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockDeviceError> {
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Arabellek boyutu blok boyutuna ({}) eşit olmalı, fakat {} boyutunda.", self.block_size, buf.len())
            ));
        }
        let offset = (block_id * self.block_size) as u64;
        fs::seek(self.fd, fs::SeekFrom::Start(offset))?;
        fs::read(self.fd, buf)?;
        Ok(())
    }

    fn write_block(&mut self, block_id: usize, buf: &[u8]) -> Result<(), BlockDeviceError> {
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Arabellek boyutu blok boyutuna ({}) eşit olmalı, fakat {} boyutunda.", self.block_size, buf.len())
            ));
        }
        let offset = (block_id * self.block_size) as u64;
        fs::seek(self.fd, fs::SeekFrom::Start(offset))?;
        fs::write(self.fd, buf)?;
        Ok(())
    }

    fn block_size(&self) -> usize {
        self.block_size
    }
}

#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Gerçek çıktı mekanizmasına erişim olmalı (örneğin, UART).
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

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}