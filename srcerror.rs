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

#[cfg(not(feature = "std"))]
use core::fmt;

#[cfg(feature = "std")]
use std::fmt;

#[cfg(feature = "std")]
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveType {
    HDD,
    SSD,
    SATA,
    SAS,
    NVMe,
    UFS,
    eMMC,
    USB,
    Other(String), // Diğer sürücü türleri için
}

impl fmt::Display for DriveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriveType::HDD => write!(f, "HDD"),
            DriveType::SSD => write!(f, "SSD"),
            DriveType::SATA => write!(f, "SATA"),
            DriveType::SAS => write!(f, "SAS"),
            DriveType::NVMe => write!(f, "NVMe"),
            DriveType::UFS => write!(f, "UFS"),
            DriveType::eMMC => write!(f, "eMMC"),
            DriveType::USB => write!(f, "USB"),
            DriveType::Other(s) => write!(f, "Diğer({})", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemError {
    DeviceError {
        drive_type: DriveType,
        message: String,
    },
    InodeError(String),
    DataBlockError(String),
    DirectoryError(String),
    SuperblockError(String),
    FreeSpaceError(String),
    IOError(String),
    Other(String),
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileSystemError::DeviceError { drive_type, message } => {
                write!(f, "Device Error on {}: {}", drive_type, message) // Daha düzgün mesaj
            }
            FileSystemError::InodeError(msg) => write!(f, "Inode Error: {}", msg),
            FileSystemError::DataBlockError(msg) => write!(f, "Data Block Error: {}", msg),
            FileSystemError::DirectoryError(msg) => write!(f, "Directory Error: {}", msg),
            FileSystemError::SuperblockError(msg) => write!(f, "Superblock Error: {}", msg),
            FileSystemError::FreeSpaceError(msg) => write!(f, "Free Space Error: {}", msg),
            FileSystemError::IOError(msg) => write!(f, "IO Error: {}", msg),
            FileSystemError::Other(msg) => write!(f, "Other Error: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl Error for FileSystemError {}

// Örnek hata oluşturma fonksiyonu
pub fn create_device_error(drive_type: DriveType, message: String) -> FileSystemError {
    FileSystemError::DeviceError { drive_type, message }
}

// Örnek hata eşleştirme
pub fn handle_error(error: FileSystemError) {
    match error {
        FileSystemError::DeviceError { .. } => {
            #[cfg(feature = "std")]
            println!("Error occurred: {}", error);
            #[cfg(not(feature = "std"))]
            crate::println!("Error occurred: {}", error);
        }
        FileSystemError::IOError(msg) => {
            #[cfg(feature = "std")]
            println!("IO Error: {}", msg);
            #[cfg(not(feature = "std"))]
            crate::println!("IO Error: {}", msg);
        }
        _ => {
            #[cfg(feature = "std")]
            println!("Unexpected error: {}", error);
            #[cfg(not(feature = "std"))]
            crate::println!("Unexpected error: {}", error);
        }
    }
}

#[cfg(feature = "std")]
fn main() {
    let error = create_device_error(DriveType::SSD, "Read error".to_string());
    handle_error(error);
}

#[cfg(not(feature = "std"))]
fn main() {
    let error = create_device_error(DriveType::SSD, "Read error".to_string());
    handle_error(error);
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