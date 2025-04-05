use crate::fs::{FileSystem, VfsError, VfsNode};
use crate::fs; // fs modülünü içeri aktarıyoruz

pub struct DocxFile {
    fd: u64, // Dosya tanımlayıcısını tutuyoruz
    file_size: usize, // Dosya boyutunu saklayabiliriz (isteğe bağlı)
}

impl DocxFile {
    pub fn new(fd: u64, file_size: usize) -> Self {
        DocxFile { fd, file_size }
    }

    // İçeriği okuma fonksiyonunu basitleştiriyoruz.
    // Artık ZIP ve XML ayrıştırması yapmıyoruz.
    // Doğrudan dosyadan okuma yapacağız.
    pub fn read_all(&self) -> Result<Vec<u8>, VfsError> {
        let mut buffer = Vec::new();
        let mut offset = 0;
        let chunk_size = 4096; // Okuma parçası boyutu

        loop {
            let mut chunk = [0u8; chunk_size];
            let bytes_read = fs::read(self.fd, &mut chunk)
                .map_err(|e| match e {
                    super::SahneError::InvalidFileDescriptor => VfsError::InvalidDescriptor,
                    super::SahneError::FileNotFound => VfsError::NotFound,
                    super::SahneError::PermissionDenied => VfsError::PermissionDenied,
                    _ => VfsError::IOError, // Diğer SahneError türlerini genel bir IO hatasına eşleyebiliriz
                })?;

            if bytes_read == 0 {
                break; // Dosyanın sonuna ulaşıldı
            }

            buffer.extend_from_slice(&chunk[..bytes_read]);
            offset += bytes_read;
        }

        Ok(buffer)
    }
}

impl VfsNode for DocxFile {
    fn read(&self, buffer: &mut [u8], offset: usize) -> Result<usize, VfsError> {
        // Doğrudan fs::read'i kullanarak belirtilen offsetten okuma yapıyoruz.
        let bytes_read = fs::read_at(self.fd, buffer, offset as u64) // fs modülünde read_at benzeri bir fonksiyonun olduğunu varsayıyoruz
            .map_err(|e| match e {
                super::SahneError::InvalidFileDescriptor => VfsError::InvalidDescriptor,
                super::SahneError::FileNotFound => VfsError::NotFound,
                super::SahneError::PermissionDenied => VfsError::PermissionDenied,
                _ => VfsError::IOError,
            })?;
        Ok(bytes_read)
    }
}

// Yeni bir read_at benzeri sistem çağrısı ve fonksiyon tanımı eklememiz gerekebilir:

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64", target_arch = "sparc64", target_arch = "openrisc", target_arch = "powerpc64", target_arch = "loongarch64", target_arch = "elbrus", target_arch = "mips64"))]
pub mod arch {
    // ... mevcut sistem çağrı numaraları ...
    pub const SYSCALL_FILE_READ_AT: u64 = 22; // Yeni sistem çağrısı numarası
}

pub mod fs {
    use super::{SahneError, arch, syscall};
    // ... mevcut fs fonksiyonları ...

    /// Belirtilen dosya tanımlayıcısından (file descriptor) belirli bir konumdan (offset) veri okur.
    ///
    /// # Hatalar
    ///
    /// * `SahneError::InvalidFileDescriptor`: Geçersiz bir dosya tanımlayıcısı verilirse döner.
    pub fn read_at(fd: u64, buffer: &mut [u8], offset: u64) -> Result<usize, SahneError> {
        let buffer_ptr = buffer.as_mut_ptr() as u64;
        let buffer_len = buffer.len() as u64;
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_READ_AT, fd, buffer_ptr, buffer_len, offset, 0)
        };
        if result < 0 {
            if result == -9 { // Örnek hata kodu: EBADF (Kötü dosya numarası)
                Err(SahneError::InvalidFileDescriptor)
            } else {
                Err(SahneError::UnknownSystemCall)
            }
        } else {
            Ok(result as usize)
        }
    }
}

// VfsError tanımını da fs modülüne taşıyabiliriz veya crate seviyesinde tutabiliriz.
#[derive(Debug)]
pub enum VfsError {
    NotFound,
    PermissionDenied,
    InvalidDescriptor,
    IOError,
    InvalidData,
    NotSupported,
    // ... diğer VFS hataları
}

// FileSystem trait'ini de tanımlamamız gerekebilir (eğer henüz tanımlanmadıysa):
pub trait FileSystem {
    fn open(&self, path: &str, flags: u32) -> Result<Box<dyn VfsNode>, VfsError>;
    // ... diğer dosya sistemi operasyonları
}

pub trait VfsNode {
    fn read(&self, buffer: &mut [u8], offset: usize) -> Result<usize, VfsError>;
    // ... diğer VFS düğümü operasyonları (write, metadata vb.)
}