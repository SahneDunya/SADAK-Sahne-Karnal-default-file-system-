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
    srcblockdevice::{BlockDevice, BlockDeviceError}, // BlockDeviceError'ı SahneError ile değiştireceğiz
};

#[cfg(not(feature = "std"))]
use core::result::Result;

#[cfg(not(feature = "std"))]
use core::option::Option;

#[cfg(feature = "std")]
use crate::srcblockdevice::{BlockDevice, BlockDeviceError};
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Read, Seek, SeekFrom, Write, ErrorKind};
#[cfg(feature = "std")]
use std::path::Path;

#[cfg(not(feature = "std"))]
pub struct SasDevice {
    fd: u64, // Sahne64 dosya tanımlayıcısı
    block_size: u64,
}

#[cfg(feature = "std")]
pub struct SasDevice {
    file: File,
    block_size: u64,
}

#[cfg(not(feature = "std"))]
impl SasDevice {
    pub fn new(path: &str, block_size: u64) -> Result<Self, SahneError> {
        let flags = fs::O_RDWR;
        let fd = fs::open(path, flags)?;
        Ok(SasDevice { fd, block_size })
    }
}

#[cfg(feature = "std")]
impl SasDevice {
    pub fn new(path: &str, block_size: u64) -> Result<Self, BlockDeviceError> {
        let filepath = Path::new(path);
        let file = File::open(filepath).map_err(|e| {
            match e.kind() {
                ErrorKind::NotFound => BlockDeviceError::OpenError,
                _ => BlockDeviceError::OpenError,
            }
        })?;
        Ok(SasDevice { file, block_size })
    }
}

#[cfg(not(feature = "std"))]
impl BlockDevice for SasDevice {
    fn read_block(&mut self, block_number: u64, buffer: &mut [u8]) -> Result<(), SahneError> {
        let offset = block_number * self.block_size;
        fs::lseek(self.fd, offset as i64, fs::SEEK_SET)?;
        fs::read(self.fd, buffer)?;
        Ok(())
    }

    fn write_block(&mut self, block_number: u64, buffer: &[u8]) -> Result<(), SahneError> {
        let offset = block_number * self.block_size;
        fs::lseek(self.fd, offset as i64, fs::SEEK_SET)?;
        fs::write(self.fd, buffer)?;
        Ok(())
    }

    fn block_size(&self) -> u64 {
        self.block_size
    }
}

#[cfg(feature = "std")]
impl BlockDevice for SasDevice {
    fn read_block(&mut self, block_number: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
        let offset = block_number * self.block_size;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| BlockDeviceError::SeekError)?;
        self.file
            .read_exact(buffer)
            .map_err(|_| BlockDeviceError::ReadError)?;
        Ok(())
    }

    fn write_block(&mut self, block_number: u64, buffer: &[u8]) -> Result<(), BlockDeviceError> {
        let offset = block_number * self.block_size;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| BlockDeviceError::SeekError)?;
        self.file
            .write_all(buffer)
            .map_err(|_| BlockDeviceError::WriteError)?;
        Ok(())
    }

    fn block_size(&self) -> u64 {
        self.block_size
    }
}

#[cfg(not(feature = "std"))]
pub mod print {
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