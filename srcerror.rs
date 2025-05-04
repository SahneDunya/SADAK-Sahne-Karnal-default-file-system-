#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Sahne64 API'sından gelen düşük seviye hatalar için temel tip
// SADAK dosya sistemi hataları bu tipi sarabilir veya bu tipten gelen hataları
// kendi hata tiplerine dönüştürebilir.
use crate::SahneError;

// no_std ortamında formatlama için core::fmt kullanılır.
use core::fmt;

// std ortamında Error trait'ini implement etmek için
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

/// SADAK dosya sistemine özgü hata türleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemError {
    /// Aygıt katmanından gelen hatalar. Hatanın oluştuğu sürücü tipini içerir.
    DeviceError {
        drive_type: DriveType,
        message: String, // Aygıttan gelen hata mesajı veya açıklaması
        // Orijinal Sahne64::SahneError burada saklanabilir: source: SahneError,
    },
    /// Inode yönetimi ile ilgili hatalar.
    InodeError(String),
    /// Veri bloğu okuma/yazma veya yönetimi ile ilgili hatalar.
    DataBlockError(String),
    /// Dizin işlemleri (oluşturma, silme, arama, listeleme) ile ilgili hatalar.
    DirectoryError(String),
    /// Superblock okuma/yazma veya doğrulama ile ilgili hatalar.
    SuperblockError(String),
    /// Boş alan yönetimi (bitmap veya liste) ile ilgili hatalar.
    FreeSpaceError(String),
    /// Genel Giriş/Çıkış (I/O) işlemleri sırasında oluşan hatalar.
    /// Bu hatalar genellikle alttaki aygıttan veya Sahne64 API'sından gelir.
    IOError(String), // Orijinal Sahne64::SahneError burada string olarak saklanıyor.
    /// Tanımlanmamış veya beklenmeyen diğer hatalar.
    Other(String),
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileSystemError::DeviceError { drive_type, message } => {
                write!(f, "Device Error on {}: {}", drive_type, message)
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

// Örnek hata eşleştirme ve yazdırma
pub fn handle_error(error: FileSystemError) {
    match error {
        FileSystemError::DeviceError { .. } => {
            #[cfg(feature = "std")]
            println!("Error occurred: {}", error);
            #[cfg(not(feature = "std"))]
            crate::println!("Error occurred: {}", error); // no_std uyumlu println!
        }
        FileSystemError::IOError(msg) => {
            #[cfg(feature = "std")]
            println!("IO Error: {}", msg);
            #[cfg(not(feature = "std"))]
            crate::println!("IO Error: {}", msg); // no_std uyumlu println!
        }
        _ => {
            #[cfg(feature = "std")]
            println!("Unexpected error: {}", error);
            #[cfg(not(feature = "std"))]
            crate::println!("Unexpected error: {}", error); // no_std uyumlu println!
        }
    }
}

// Örnek main fonksiyonu (std ortamı için)
#[cfg(feature = "std")]
fn main() {
    use alloc::string::ToString;
    let error = create_device_error(DriveType::SSD, "Read error".to_string());
    handle_error(error);
}

// Örnek main fonksiyonu (no_std ortamı için)
#[cfg(not(feature = "std"))]
fn main() {
    // no_std ortamında test amaçlı main fonksiyonu.
    // Gerçek uygulamada entry point başka yerde olacaktır.
    // Konsol Handle'ı ayarlanmış olmalıdır (örn. crate::init_console ile).
     #[cfg(not(feature = "std"))]
     { // no_std println! makrosunun scope'u
          // Varsayımsal bir konsol handle'ı ayarlayalım.
          // crate::init_console(crate::Handle(3)); // init_console'ı çağırabilmek için Handle tipi ve init_console fonksiyonu pub olmalı.
          // Şimdilik çağrıyı yorum satırı yapalım, test amaçlı main'de dışarıdan init edilmesi gerekir.
     }

    use alloc::string::ToString;
    let error = create_device_error(DriveType::SSD, "Read error".to_string());
    handle_error(error);
}

// Tekrarlanan no_std print modülü ve panic handler kaldırıldı.
