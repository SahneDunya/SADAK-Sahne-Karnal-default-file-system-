#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel Sahne64 tipleri ve modülleri
use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri ve Handle
// Sahne64 resource modülü
#[cfg(not(feature = "std"))]
use crate::resource;
// Sahne64 fs modülü (fs::open, fs::read_at, fs::fstat için varsayım)
#[cfg(not(feature = "std"))]
use crate::fs;

// alloc crate for String (error messages)
use alloc::string::String;
use alloc::format;

// Helper function to map SahneError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
    // TODO: Implement a proper mapping based on SahneError variants
}


/// Sahne64 ortamında genel bir binary dosyayı temsil eder.
/// Offset tabanlı okuma ve dosya boyutu bilgisi sağlar (eğer Sahne64 API destekliyorsa).
pub struct BinFile {
    /// Sahne64 dosya kaynağının Handle'ı.
    handle: Handle,
    /// Dosyanın boyutu (bayt olarak).
    size: usize,
}

impl BinFile {
    /// Belirtilen dosya yolundaki (kaynak ID'si) binary dosyayı okuma modunda açar.
    ///
    /// # Arguments
    ///
    /// * `path` - Dosya yolu veya Sahne64 kaynak ID'si.
    ///
    /// # Returns
    ///
    /// Başarılı olursa bir `BinFile` örneği veya bir `FileSystemError`.
    #[cfg(not(feature = "std"))] // Only for no_std Sahne64
    pub fn open<P: AsRef<str>>(path: P) -> Result<BinFile, FileSystemError> { // FileSystemError döner
        let path_str = path.as_ref();
        // Kaynağı edin
        let handle = resource::acquire(path_str, resource::MODE_READ)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        // Dosyanın boyutunu almak için fs::fstat syscall'ını kullanalım (varsayım)
        // fs::fstat(handle) Result<FileStat, SahneError> döndürür ve FileStat size alanı içerir.
        let file_stat = fs::fstat(handle)
            .map_err(|e| {
                 let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
                 map_sahne_error_to_fs_error(e)
             })?; // SahneError -> FileSystemError

        let size = file_stat.size as usize; // Assuming size is u64 or usize compatible

        Ok(BinFile { handle, size })
    }

    /// Dosyanın boyutunu (bayt olarak) döndürür.
    pub fn len(&self) -> usize {
        self.size // open fonksiyonunda alınan gerçek boyut döner
    }

    /// Dosyanın boş olup olmadığını kontrol eder.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Dosyanın tamamını bellek içi bir slice olarak döndürmek şu anki Sahne64 API varsayımlarıyla mümkün değil.
    /// Dosyanın tamamını okuyup Vec'e yüklemek gerekir, bu da büyük dosyalar için verimsizdir.
    /// Bu fonksiyonun amacı veya implementasyonu Sahne64'ün bellek yönetimi ve dosya API'sına bağlıdır.
    /// Şimdilik unimplemented bırakalım.
    pub fn as_bytes(&self) -> &[u8] {
        // Eğer dosyanın tamamı `open` sırasında belleğe yüklenmişse (ki şu an değil), bu uygulanabilir olurdu.
        // Veya mmap benzeri bir mekanizma olmalı.
        unimplemented!("as_bytes fonksiyonu şu anki Sahne64 API varsayımlarıyla tam olarak desteklenmiyor.");
    }

    /// Dosyanın belirtilen ofsetindeki 1 baytı okur.
    ///
    /// # Arguments
    ///
    /// * `offset` - Okunacak baytın ofseti (dosya başından itibaren).
    ///
    /// # Returns
    ///
    /// Başarılı olursa okunan bayt (`Some(u8)`) veya ofset dosya boyutunun dışındaysa `None`.
    /// Herhangi bir Sahne64 I/O hatası durumunda bir `FileSystemError`.
    #[cfg(not(feature = "std"))] // Only for no_std Sahne64
    pub fn read_u8(&self, offset: usize) -> Result<Option<u8>, FileSystemError> { // FileSystemError döner
        if offset >= self.size {
            return Ok(None); // Ofset dosya boyutunun dışında
        }
        let mut buffer = [0u8; 1];
        // fs::read_at(handle, offset, buf) Result<usize, SahneError> döner (varsayım)
        let bytes_read = fs::read_at(self.handle, offset as u64, &mut buffer)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        if bytes_read == 1 {
            Ok(Some(buffer[0]))
        } else {
            // Eğer ofset geçerliyse ama 1 bayttan az okunduysa, bu beklenmedik bir durum.
            // Yine de Option::None dönmek API'ye uygun olabilir veya hata verilebilir.
            // Mevcut kod Option::None döndürüyor, bunu koruyalım ama bir uyarı ekleyelim.
             if bytes_read > 0 && bytes_read < 1 {
                  // Teorik olarak buraya gelmemeli (read 0 veya tam okuma yapar)
                  eprintln!("WARN: read_u8 at offset {} okunan bayt sayısı beklenmiyor: {}", offset, bytes_read); // no_std print
             }
            Ok(None) // Genellikle EOF anlamına gelir read_at için (offset geçerliyse)
        }
    }

    /// Dosyanın belirtilen ofsetindeki 2 baytı Little Endian olarak okur.
    ///
    /// # Arguments
    ///
    /// * `offset` - Okunacak verinin başlangıç ofseti.
    ///
    /// # Returns
    ///
    /// Başarılı olursa okunan u16 değeri (`Some(u16)`) veya ofset+2 dosya boyutunun dışındaysa `None`.
    /// Herhangi bir Sahne64 I/O hatası durumunda bir `FileSystemError`.
    #[cfg(not(feature = "std"))] // Only for no_std Sahne64
    pub fn read_u16_le(&self, offset: usize) -> Result<Option<u16>, FileSystemError> { // FileSystemError döner
        if offset.checked_add(size_of::<u16>()).map_or(true, |end| end > self.size) {
            return Ok(None); // Ofset + boyut dosya boyutunun dışında
        }
        let mut buffer = [0u8; size_of::<u16>()];
        let bytes_read = fs::read_at(self.handle, offset as u64, &mut buffer)
             .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        if bytes_read == size_of::<u16>() {
            Ok(Some(u16::from_le_bytes(buffer)))
        } else {
            // Eğer ofset geçerliyse ama tam okuma yapılamadıysa
             if bytes_read > 0 && bytes_read < size_of::<u16>() {
                 eprintln!("WARN: read_u16_le at offset {} okunan bayt sayısı beklenmiyor: {}", offset, bytes_read); // no_std print
             }
            Ok(None) // Genellikle EOF anlamına gelir read_at için
        }
    }

    /// Dosyanın belirtilen ofsetindeki 4 baytı Little Endian olarak okur.
    ///
    /// # Arguments
    ///
    /// * `offset` - Okunacak verinin başlangıç ofseti.
    ///
    /// # Returns
    ///
    /// Başarılı olursa okunan u32 değeri (`Some(u32)`) veya ofset+4 dosya boyutunun dışındaysa `None`.
    /// Herhangi bir Sahne64 I/O hatası durumunda bir `FileSystemError`.
    #[cfg(not(feature = "std"))] // Only for no_std Sahne64
    pub fn read_u32_le(&self, offset: usize) -> Result<Option<u32>, FileSystemError> { // FileSystemError döner
        if offset.checked_add(size_of::<u32>()).map_or(true, |end| end > self.size) {
            return Ok(None); // Ofset + boyut dosya boyutunun dışında
        }
        let mut buffer = [0u8; size_of::<u32>()];
        let bytes_read = fs::read_at(self.handle, offset as u64, &mut buffer)
             .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        if bytes_read == size_of::<u32>() {
            Ok(Some(u32::from_le_bytes(buffer)))
        } else {
             if bytes_read > 0 && bytes_read < size_of::<u32>() {
                 eprintln!("WARN: read_u32_le at offset {} okunan bayt sayısı beklenmiyor: {}", offset, bytes_read); // no_std print
             }
            Ok(None) // Genellikle EOF anlamına gelir read_at için
        }
    }

     // ... diğer okuma fonksiyonları (read_u64_le, read_i8, read_i16_le, read_exact_at, etc.)
     // read_exact_at(offset, buf) -> Result<(), FileSystemError> gibi fonksiyonlar da eklenebilir.

    /// Açık dosyayı kapatır ve Sahne64 Handle'ını serbest bırakır.
    ///
    /// # Returns
    ///
    /// Başarılı olursa Ok(()) veya bir `FileSystemError`.
    #[cfg(not(feature = "std"))] // Only for no_std Sahne64
    pub fn close(&mut self) -> Result<(), FileSystemError> { // FileSystemError döner
        resource::release(self.handle)
            .map_err(map_sahne_error_to_fs_error) // SahneError -> FileSystemError
    }

     // Destructor (Drop trait) Sahne64 ortamında kaynak yönetimi için kritik olabilir.
     // Handle'ın otomatik serbest bırakılması için Drop implementasyonu eklenebilir.
     // Ancak, Drop içinde Result dönemeyiz, bu yüzden hatalar sessizce yutulur veya loglanır.
     // Açıkça close() çağırmak genellikle tercih edilir.
      #[cfg(not(feature = "std"))]
      impl Drop for BinFile {
          fn drop(&mut self) {
              let _ = resource::release(self.handle); // Hata yutuldu
          }
      }
}

// TODO: Add a std implementation if needed.
 #[cfg(feature = "std")]
 impl BinFile {
     pub fn open<P: AsRef<Path>>(path: P) -> Result<BinFile, std::io::Error> { ... }
     pub fn len(&self) -> usize { ... }
     pub fn is_empty(&self) -> bool { ... }
     pub fn as_bytes(&self) -> Result<&[u8], std::io::Error> { // Memory mapping needed }
     pub fn read_u8(&self, offset: usize) -> Result<Option<u8>, std::io::Error> { ... }
     // ... other read methods
     pub fn close(&mut self) -> Result<(), std::io::Error> { ... }
 }


// Helper function to map SahneError to FileSystemError (defined earlier, copied for clarity)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e))
    // TODO: Implement a proper mapping based on SahneError variants
}


// Example main function (no_std)
#[cfg(feature = "example_bin")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("BinFile example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // Test with a hypothetical file/resource ID
     let file_path_or_resource_id = "sahne://files/config.bin";

     match BinFile::open(file_path_or_resource_id) {
         Ok(mut bin_file) => { // Need mut to call close()
             println!("'{}' dosyası açıldı. Boyut: {} bayt", file_path_or_resource_id, bin_file.len());

             // Belirli ofsetlerden değerleri okuma örneği
             let offset1 = 0;
             match bin_file.read_u8(offset1) {
                 Ok(Some(value)) => println!("Ofset {}: u8 değeri: {}", offset1, value),
                 Ok(None) => println!("Ofset {}: Dosya sonu veya okuma hatası.", offset1),
                 Err(e) => eprintln!("Ofset {}: Okuma hatası: {}", offset1, e),
             }

             let offset2 = 4;
             match bin_file.read_u32_le(offset2) {
                 Ok(Some(value)) => println!("Ofset {}: u32_le değeri: {}", offset2, value),
                 Ok(None) => println!("Ofset {}: Dosya sonu veya okuma hatası.", offset2),
                 Err(e) => eprintln!("Ofset {}: Okuma hatası: {}", offset2, e),
             }

             // Dosyayı kapat
             if let Err(e) = bin_file.close() {
                 eprintln!("Dosya kapatma hatası: {}", e);
             }

         }
         Err(e) => {
             eprintln!("'{}' dosyası açılamadı: {}", file_path_or_resource_id, e);
         }
     }

     eprintln!("BinFile example (no_std) finished.");

     Ok(())
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_bin")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), std::io::Error> { // Return std::io::Error for std example
     eprintln!("BinFile example (std) starting...");
     // TODO: Implement a std version of BinFile and use it here.
     eprintln!("BinFile example (std) not fully implemented.");
     Ok(())
}


// Test module (primarily for no_std implementation using mocks or sim)
#[cfg(test)]
#[cfg(not(feature = "std"))] // Only compile tests for no_std
mod tests {
    use super::*;
    // Need a mocking or simulation framework for Sahne64 resource/fs syscalls
    // For example, a global mutable state representing the file system.
    // This is complex and beyond simple unit tests in isolation.

    // TODO: Implement tests for BinFile using a mock Sahne64 environment.
    // This would involve creating a mock fs::read_at, fs::fstat, resource::acquire/release
    // that operate on in-memory data or a simulated file structure.
    // Example mock structure:
     mod mock_sahne_fs {
        use super::*; // Bring in Handle, SahneError, FileStat, etc.
        use core::collections::BTreeMap; // Requires alloc and a map implementation
        use spin::Mutex; // Requires a Mutex implementation for no_std
    //
    //    // Mock file system state (resource ID -> content, size, etc.)
        lazy_static! { // Requires lazy_static crate
            static ref MOCK_FILES: Mutex<BTreeMap<String, (alloc::vec::Vec<u8>, u64)>> = Mutex::new(BTreeMap::new());
            static ref NEXT_HANDLE_ID: Mutex<u64> = Mutex::new(1);
            static ref OPEN_HANDLES: Mutex<BTreeMap<Handle, String>> = Mutex::new(BTreeMap::new()); // Handle -> resource_id
        }
    //
        pub fn add_mock_file(resource_id: &str, content: alloc::vec::Vec<u8>) {
            MOCK_FILES.lock().insert(resource_id.into(), (content.clone(), content.len() as u64));
        }
    //
        pub fn acquire(resource_id: &str, mode: u64) -> Result<Handle, SahneError> {
            let files = MOCK_FILES.lock();
            if files.contains_key(resource_id) {
                let mut next_id = NEXT_HANDLE_ID.lock();
                let handle = Handle(*next_id);
                *next_id += 1;
                OPEN_HANDLES.lock().insert(handle, resource_id.into());
                Ok(handle)
            } else {
                Err(SahneError::ResourceNotFound)
            }
        }
    
        pub fn release(handle: Handle) -> Result<(), SahneError> {
            if OPEN_HANDLES.lock().remove(&handle).is_some() {
                Ok(())
            } else {
                Err(SahneError::InvalidHandle)
            }
        }
    
        pub fn read_at(handle: Handle, offset: u64, buf: &mut [u8]) -> Result<usize, SahneError> {
            let handles = OPEN_HANDLES.lock();
            let resource_id = handles.get(&handle).ok_or(SahneError::InvalidHandle)?;
            let files = MOCK_FILES.lock();
            let (content, file_size) = files.get(resource_id).ok_or(SahneError::ResourceNotFound)?; // Should not happen if handle is valid
    //
            if offset >= *file_size {
                return Ok(0); // EOF
            }
    //
            let bytes_available = (*file_size - offset) as usize;
            let bytes_to_read = core::cmp::min(buf.len(), bytes_available);
    //
            buf[..bytes_to_read].copy_from_slice(&content[offset as usize..offset as usize + bytes_to_read]);
    //
            Ok(bytes_to_read)
        }
    //
        pub struct FileStat {
            pub size: u64,
    //        // Add other fields if needed
        }
    //
        pub fn fstat(handle: Handle) -> Result<FileStat, SahneError> {
            let handles = OPEN_HANDLES.lock();
            let resource_id = handles.get(&handle).ok_or(SahneError::InvalidHandle)?;
            let files = MOCK_FILES.lock();
            let (_, file_size) = files.get(resource_id).ok_or(SahneError::ResourceNotFound)?;
    //
            Ok(FileStat { size: *file_size })
        }
     }
    //
    // // Override the real syscalls/api calls with mocks for tests
     #[allow(unused_imports)] // For mocking
     use mock_sahne_fs as fs;
     #[allow(unused_imports)] // For mocking
     use mock_sahne_fs as resource; // Assuming acquire/release are in resource

     #[test]
     fn test_binfile_open_len_close() {
        mock_sahne_fs::add_mock_file("test_file.bin", vec![1, 2, 3, 4, 5]);
        let bin_file_res = BinFile::open("test_file.bin");
        assert!(bin_file_res.is_ok(), "BinFile::open failed: {:?}", bin_file_res.err());
        let mut bin_file = bin_file_res.unwrap();
        assert_eq!(bin_file.len(), 5);
        assert!(!bin_file.is_empty());
        let close_res = bin_file.close();
        assert!(close_res.is_ok(), "BinFile::close failed: {:?}", close_res.err());
     }
    //
     #[test]
     fn test_binfile_read_u8() {
         mock_sahne_fs::add_mock_file("test_read_u8.bin", vec![10, 20, 30]);
         let mut bin_file = BinFile::open("test_read_u8.bin").unwrap();
    //
         assert_eq!(bin_file.read_u8(0).unwrap(), Some(10));
         assert_eq!(bin_file.read_u8(1).unwrap(), Some(20));
         assert_eq!(bin_file.read_u8(2).unwrap(), Some(30));
         assert_eq!(bin_file.read_u8(3).unwrap(), None); // EOF
         assert_eq!(bin_file.read_u8(10).unwrap(), None); // Offset beyond size
     }
    //
     #[test]
     fn test_binfile_read_u16_le() {
         mock_sahne_fs::add_mock_file("test_read_u16.bin", vec![0xAA, 0xBB, 0xCC, 0xDD]); // BBAA, DDCC
         let mut bin_file = BinFile::open("test_read_u16.bin").unwrap();
    //
         assert_eq!(bin_file.read_u16_le(0).unwrap(), Some(0xBBAA));
         assert_eq!(bin_file.read_u16_le(2).unwrap(), Some(0xDDCC));
         assert_eq!(bin_file.read_u16_le(3).unwrap(), None); // Not enough bytes
         assert_eq!(bin_file.read_u16_le(4).unwrap(), None); // EOF
    }
    //
     #[test]
     fn test_binfile_read_u32_le() {
         mock_sahne_fs::add_mock_file("test_read_u32.bin", vec![0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44]); // DDCCBBAA, 44332211
         let mut bin_file = BinFile::open("test_read_u32.bin").unwrap();
    //
         assert_eq!(bin_file.read_u32_le(0).unwrap(), Some(0xDDCCBBAA));
         assert_eq!(bin_file.read_u32_le(4).unwrap(), Some(0x44332211));
         assert_eq!(bin_file.read_u32_le(5).unwrap(), None); // Not enough bytes
         assert_eq!(bin_file.read_u32_le(8).unwrap(), None); // EOF
     }

}
