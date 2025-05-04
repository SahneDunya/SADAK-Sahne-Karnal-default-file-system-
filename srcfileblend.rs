// srcfileblend.rs
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
// Sahne64 fs modülü (fs::read_at, fs::fstat için varsayım - used by SahneResourceReader)
#[cfg(not(feature = "std"))]
use crate::fs;


// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::{File, self};
#[cfg(feature = "std")]
use std::io::{BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, Write as StdWrite};
#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::vec::Vec as StdVec; // std::vec::Vec


// alloc crate for String, Vec, format!
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

// core::io traits and types
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io
use core::cmp; // core::cmp
use core::mem::size_of; // core::mem::size_of

// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülden import edildiği varsayılır


// Helper function to map SahneError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
    // TODO: Implement a proper mapping based on SahneError variants
}

// Helper function to map std::io::Error to FileSystemError (copied from other files)
#[cfg(feature = "std")]
fn map_std_io_error_to_fs_error(e: StdIOError) -> FileSystemError {
    FileSystemError::IOError(format!("IO Error: {}", e))
}

// Helper function to map CoreIOError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_core_io_error_to_fs_error(e: CoreIOError) -> FileSystemError {
     FileSystemError::IOError(format!("CoreIOError: {:?}", e))
     // TODO: Implement a proper mapping based on CoreIOErrorKind
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilebin.rs'den kopyalandı)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
// fstat ile dosya boyutunu alarak seek(End) desteği sağlar.
// Sahne64 API'sının bu syscall'ları Handle üzerinde sağladığı varsayılır.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu
}

#[cfg(not(feature = "std"))]
impl SahneResourceReader {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReader { handle, position: 0, file_size }
    }

    // Note: No explicit `write` method here, focused on reading.
    // A separate SahneResourceWriter might be needed or use resource::write directly.
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        // Ensure we don't read past the end of the file based on recorded size
        if self.position >= self.file_size {
            return Ok(0); // EOF
        }
        let bytes_available = (self.file_size - self.position) as usize;
        let bytes_to_read = cmp::min(buf.len(), bytes_available);

        if bytes_to_read == 0 {
             return Ok(0); // No bytes to read
        }

        // Assuming fs::read_at(handle, offset, buf) Result<usize, SahneError>
        let bytes_read = fs::read_at(self.handle, self.position, &mut buf[..bytes_to_read])
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::read_at error: {:?}", e)))?; // Map SahneError to CoreIOError

        self.position += bytes_read as u64; // Pozisyonu güncelle
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        // Use the stored file_size for SeekFrom::End
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize, // isize for calculations
            SeekFrom::End(offset) => {
                file_size_isize.checked_add(offset)
                    .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from end)")))?
            },
            SeekFrom::Current(offset) => {
                (self.position as isize).checked_add(offset)
                     .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from current)")))?
            },
        };

        if new_pos < 0 {
            return Err(CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Invalid seek position (result is negative)")));
        }

        self.position = new_pos as u64; // Update user-space position
        Ok(self.position) // Return new position
    }
    // stream_position has a default implementation in core::io::Seek
}


// Blend dosya formatı yapıları
#[derive(Debug)]
pub struct BlendFile {
    pub header: BlendHeader,
    pub data: Vec<u8>, // Use alloc::vec::Vec
}

// Blend dosya başlığı örneği
#[derive(Debug)]
pub struct BlendHeader {
    pub magic: [u8; 4], // "BLEN"
    pub version: u32,
    pub data_offset: u32, // Offset from start of file to the data section
}

/// Blend dosyasını okur ve ayrıştırır.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - Dosya yolu (std) veya Sahne64 kaynak ID'si (no_std).
///
/// # Returns
///
/// Başarılı olursa `BlendFile` yapısı veya bir `FileSystemError`.
#[cfg(feature = "std")]
pub fn read_from_file(path: &Path) -> Result<BlendFile, FileSystemError> { // FileSystemError döner
    let file = File::open(path).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Başlığı oku
    let mut header_bytes = [0; size_of::<BlendHeader>()]; // Header boyutu 12
    reader.read_exact(&mut header_bytes).map_err(map_std_io_error_to_fs_error)?;

    let header = BlendHeader {
        magic: [header_bytes[0], header_bytes[1], header_bytes[2], header_bytes[3]],
        version: u32::from_le_bytes(header_bytes[4..8].try_into().unwrap()), // try_into().unwrap() retained from original, consider safer map_err
        data_offset: u32::from_le_bytes(header_bytes[8..12].try_into().unwrap()), // try_into().unwrap() retained from original
    };

    // Sihirli sayıyı kontrol et
    if &header.magic != b"BLEN" {
        return Err(FileSystemError::InvalidData(format!("Geçersiz sihirli sayı: {:?}", header.magic))); // FileSystemError::InvalidData
    }

    // Verileri oku - daha verimli seek kullanımı
    let mut data = Vec::new(); // Use alloc::vec::Vec
    reader.seek(SeekFrom::Start(header.data_offset as u64)).map_err(map_std_io_error_to_fs_error)?; // SeekFrom::Start ile daha net
    reader.read_to_end(&mut data).map_err(map_std_io_error_to_fs_error)?; // read_to_end okunan bayt sayısını döner, tüm veriyi okur

    Ok(BlendFile { header, data })
}

#[cfg(not(feature = "std"))]
pub fn read_from_file(path: &str) -> Result<BlendFile, FileSystemError> { // FileSystemError döner
    // Kaynağı edin
    let handle = resource::acquire(path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutunu al (SahneResourceReader için gerekli)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle);
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;


    // Sahne64 Handle'ı için core::io::Read + Seek implementasyonu sağlayan Reader struct'ı oluştur
    let mut reader = SahneResourceReader::new(handle, file_size);

    // Başlığı oku
    let mut header_bytes = [0; size_of::<BlendHeader>()]; // Header boyutu 12
    reader.read_exact(&mut header_bytes).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError

    let header = BlendHeader {
        magic: [header_bytes[0], header_bytes[1], header_bytes[2], header_bytes[3]],
        // try_into().unwrap() yerine safer from_le_bytes ve hata kontrolü kullanılabilir
        version: u32::from_le_bytes(header_bytes[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("Versiyon baytları geçersiz")))?),
        data_offset: u32::from_le_bytes(header_bytes[8..12].try_into().map_err(|_| FileSystemError::InvalidData(format!("Veri ofset baytları geçersiz")))?),
    };

    // Sihirli sayıyı kontrol et
    if &header.magic != b"BLEN" {
        // Kaynağı serbest bırakmadan önce hata dön
         let _ = resource::release(handle).map_err(|e| {
              eprintln!("WARN: Kaynak serbest bırakma hatası: {:?}", e);
              map_sahne_error_to_fs_error(e)
          });
        return Err(FileSystemError::InvalidData(format!("Geçersiz sihirli sayı: {:?}", header.magic)));
    }

    // Verileri oku - seek kullanımı
    let mut data = Vec::new(); // Use alloc::vec::Vec
    // Seek to the data offset
    reader.seek(SeekFrom::Start(header.data_offset as u64)).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError

    // Read remaining data into the Vec
    // Need to read from the current position until the end of the file.
    // SahneResourceReader::read handles EOF by returning 0.
    // Loop to read all remaining data.
    let mut temp_buffer = [0u8; 1024]; // Chunk buffer
    loop {
        let bytes_read = reader.read(&mut temp_buffer).map_err(map_core_io_error_to_fs_error)?;
        if bytes_read == 0 {
            break; // EOF
        }
        // Extend Vec with the read bytes. This requires alloc.
        data.extend_from_slice(&temp_buffer[..bytes_read]);
    }


    // Kaynağı serbest bırak
    let _ = resource::release(handle).map_err(|e| {
         eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e);
         map_sahne_error_to_fs_error(e)
     });

    Ok(BlendFile { header, data })
}

/// BlendFile'ı dosyaya yazar.
///
/// # Arguments
///
/// * `path` - Dosya yolu (std) veya Sahne64 kaynak ID'si (no_std).
///
/// # Returns
///
/// Başarılı olursa Ok(()) veya bir `FileSystemError`.
#[cfg(feature = "std")]
pub fn write_to_file(&self, path: &Path) -> Result<(), FileSystemError> { // FileSystemError döner
    let mut file = File::create(path).map_err(map_std_io_error_to_fs_error)?;

    // Başlığı yaz
    file.write_all(&self.header.magic).map_err(map_std_io_error_to_fs_error)?;
    file.write_all(&self.header.version.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;
    file.write_all(&self.header.data_offset.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;

    // Veriyi yaz
     // Eğer data_offset header boyutundan (12) büyükse, aradaki boşluğa 0 yazılmalı.
     let header_size = size_of::<BlendHeader>() as u32; // Should be 12
     if self.header.data_offset > header_size {
          let padding_size = (self.header.data_offset - header_size) as usize;
          let padding = vec![0u8; padding_size]; // Requires alloc
          file.write_all(&padding).map_err(map_std_io_error_to_fs_error)?;
     } else if self.header.data_offset < header_size {
          // Geçersiz data_offset değeri
           eprintln!("WARN: Geçersiz BlendHeader data_offset değeri: {}", self.header.data_offset); // std print
           // Hata verilebilir veya sadece loglanabilir
           return Err(FileSystemError::InvalidData(format!("BlendHeader data_offset header boyutundan ({}) küçük.", header_size)));
     }

     file.write_all(&self.data).map_err(map_std_io_error_to_fs_error)?;


    Ok(())
}

#[cfg(not(feature = "std"))]
pub fn write_to_file(&self, path: &str) -> Result<(), FileSystemError> { // FileSystemError döner
    // Kaynağı yazma modunda edin (O_CREAT | O_WRONLY Sahne64 karşılığı varsayım)
    let handle = resource::acquire(path, resource::MODE_WRITE | resource::FLAG_CREATE)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Başlığı yaz
    // resource::write(handle, data) Result<usize, SahneError> döner (varsayım)
    resource::write(handle, &self.header.magic)
        .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
    resource::write(handle, &self.header.version.to_le_bytes())
        .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
    resource::write(handle, &self.header.data_offset.to_le_bytes())
        .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;

    // Veriyi yaz
     // Eğer data_offset header boyutundan (12) büyükse, aradaki boşluğa 0 yazılmalı.
     let header_size = size_of::<BlendHeader>() as u32; // Should be 12
     if self.header.data_offset > header_size {
          let padding_size = (self.header.data_offset - header_size) as usize;
          let padding = vec![0u8; padding_size]; // Requires alloc
          resource::write(handle, &padding)
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
     } else if self.header.data_offset < header_size {
           eprintln!("WARN: Geçersiz BlendHeader data_offset değeri: {}", self.header.data_offset); // no_std print
           // Hata verilebilir veya sadece loglanabilir
     }

     resource::write(handle, &self.data)
        .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;


    // Kaynağı serbest bırak
    resource::release(handle)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    Ok(())
}


/// BlendFile verilerini ayrıştırır (örnek olarak veri uzunluğunu yazdırır).
/// Gerçek ayrıştırma mantığı Blend dosya formatı spesifikasyonuna göre eklenebilir.
pub fn parse_data(&self) -> Result<(), FileSystemError> { // FileSystemError döner
    #[cfg(not(feature = "std"))]
    crate::println!("Veri uzunluğu: {} bayt", self.data.len()); // no_std print
    #[cfg(feature = "std")]
    println!("Veri uzunluğu: {} bayt", self.data.len()); // std print

    // Gerçek ayrıştırma mantığı buraya gelecek...
    // Örneğin, self.data üzerindeki verilere erişilerek yapıları ayrıştırılabilir.
    // Bu kısım Blend formatı spesifikasyonuna bağlıdır.
    // Örnek:
    // if self.data.len() > 4 {
    //     let first_four_bytes = u32::from_le_bytes(self.data[0..4].try_into().unwrap());
    //     #[cfg(not(feature = "std"))] crate::println!("Verinin ilk u32 değeri: {}", first_four_bytes);
    //     #[cfg(feature = "std")] println!("Verinin ilk u32 değeri: {}", first_four_bytes);
    // }

    Ok(())
}


// Example main functions
#[cfg(feature = "example_blend")] // Different feature flag
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     #[cfg(not(feature = "std"))]
     {
          eprintln!("Blend file example (no_std) starting...");
          // TODO: Call init_console(crate::Handle(3)); if needed
     }
     #[cfg(feature = "std")]
     {
          eprintln!("Blend file example (std) starting...");
     }

     // Test with a hypothetical file/resource ID
     let file_path_or_resource_id = "sahne://files/my_scene.blend";

     // Örnek başlık verisi
     let header = BlendHeader {
         magic: *b"BLEN",
         version: 280, // Örnek versiyon
         data_offset: 12, // Basit örnek: data hemen başlığın arkasından başlar
     };

     // Örnek veri
     let data_content = b"This is some sample blend file data."; // Example data bytes
     let blend_file_to_write = BlendFile {
         header,
         data: data_content.to_vec(), // Requires alloc
     };


     // Dosyaya yazma
     #[cfg(feature = "std")]
     let write_path = Path::new(file_path_or_resource_id);
     #[cfg(not(feature = "std"))]
     let write_path = file_path_or_resource_id;

     match write_to_file(&blend_file_to_write, write_path) {
         Ok(_) => println!("Blend dosyası başarıyla yazıldı: {}", file_path_or_resource_id),
         Err(e) => eprintln!("Blend dosyası yazılırken hata oluştu: {}", e),
     }


     // Dosyadan okuma
     #[cfg(feature = "std")]
     let read_path = Path::new(file_path_or_resource_id);
     #[cfg(not(feature = "std"))]
     let read_path = file_path_or_resource_id;

     match read_from_file(read_path) {
         Ok(loaded_blend_file) => {
             println!("Blend dosyası başarıyla okundu.");
             println!("Başlık: {:?}", loaded_blend_file.header);
             loaded_blend_file.parse_data()?; // Veri uzunluğunu yazdır
             // Optionally print some of the data content
              #[cfg(feature = "std")] // Only print data content in std for simplicity
              println!("Veri içeriğinin ilk 30 baytı: {:?}", &loaded_blend_file.data[..cmp::min(30, loaded_blend_file.data.len())]);
         }
         Err(e) => eprintln!("Blend dosyası okunurken hata oluştu: {}", e),
     }


     // Dosyayı temizle (std ortamında)
     #[cfg(feature = "std")]
     if let Err(e) = fs::remove_file(Path::new(file_path_or_resource_id)) {
          eprintln!("Test dosyası silinirken hata oluştu: {}", e);
     }
     // TODO: Sahne64 ortamında dosya silme API'sını kullan (fs::remove veya resource::control?)


     #[cfg(not(feature = "std"))]
     eprintln!("Blend file example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("Blend file example (std) finished.");

     Ok(())
}


// Test module (primarily for std implementation using mocks or sim)
#[cfg(test)]
#[cfg(feature = "std")] // Only compile tests for std
mod tests {
    use super::*;
    use std::io::Cursor; // In-memory reader/seeker
    use std::io::{Write, Read, Seek, SeekFrom}; // For Cursor traits
    use std::error::Error as StdError; // For error handling
    use alloc::vec; // vec! macro
    use alloc::string::ToString; // to_string() for error messages

    // Helper to create a basic Blend file in memory
    fn create_test_blend_file(version: u32, data_offset: u32, data_content: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::new();
        // Header
        buffer.extend_from_slice(b"BLEN");
        buffer.extend_from_slice(&version.to_le_bytes());
        buffer.extend_from_slice(&data_offset.to_le_bytes());
        // Padding if data_offset > header size (12)
        let header_size = size_of::<BlendHeader>() as u32;
        if data_offset > header_size {
            let padding_size = (data_offset - header_size) as usize;
            buffer.extend(vec![0u8; padding_size]); // Requires alloc
        }
        // Data
        buffer.extend_from_slice(data_content);
        buffer
    }

    #[test]
    fn test_read_from_file_basic() -> Result<(), FileSystemError> { // Return FileSystemError
        let version = 1;
        let data_offset = 12; // Data immediately after header
        let data_content = b"Test data.";
        let blend_data = create_test_blend_file(version, data_offset, data_content);

        // Use Cursor as a reader for the in-memory data
        let mut cursor = Cursor::new(blend_data);

        // Call the read function (adapted to work with Cursor)
        // Need to create a reader that takes a Cursor
        // The original read_from_file takes &Path, let's simulate that by writing to a temp file
        // Or, refactor read_from_file to take a generic reader, which was the path taken in other files.
        // Let's adapt read_from_file to take a generic reader for testing ease.

        // Refactor `read_from_file` to `read_from_reader` taking `impl Read + Seek`.

         // For this test, let's manually read using Cursor and the internal logic.
         // This requires copying the internal logic or making it public/generic.
         // Let's create a helper function that works on a generic reader.

        fn read_blend_from_reader<R: Read + Seek>(reader: &mut R) -> Result<BlendFile, FileSystemError> {
             // Copy of the parsing logic from the main read_from_file, but returning FileSystemError
             let mut header_bytes = [0; size_of::<BlendHeader>()]; // Header boyutu 12
             reader.read_exact(&mut header_bytes).map_err(|e| FileSystemError::IOError(format!("Header read error: {:?}", e)))?; // Map core::io::Error to FileSystemError

             let header = BlendHeader {
                 magic: [header_bytes[0], header_bytes[1], header_bytes[2], header_bytes[3]],
                 version: u32::from_le_bytes(header_bytes[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("Versiyon baytları geçersiz")))?),
                 data_offset: u32::from_le_bytes(header_bytes[8..12].try_into().map_err(|_| FileSystemError::InvalidData(format!("Veri ofset baytları geçersiz")))?),
             };

             if &header.magic != b"BLEN" {
                 return Err(FileSystemError::InvalidData(format!("Geçersiz sihirli sayı: {:?}", header.magic)));
             }

             let mut data = Vec::new(); // Use alloc::vec::Vec
             reader.seek(SeekFrom::Start(header.data_offset as u64)).map_err(|e| FileSystemError::IOError(format!("Seek error: {:?}", e)))?; // Map core::io::Error to FileSystemError

             let mut temp_buffer = [0u8; 1024]; // Chunk buffer
             loop {
                 let bytes_read = reader.read(&mut temp_buffer).map_err(|e| FileSystemError::IOError(format!("Data read error: {:?}", e)))?;
                 if bytes_read == 0 {
                     break; // EOF
                 }
                 data.extend_from_slice(&temp_buffer[..bytes_read]);
             }

             Ok(BlendFile { header, data })
        }

         let loaded_blend_file = read_blend_from_reader(&mut cursor)?;

        // Assert the loaded data
        assert_eq!(loaded_blend_file.header.magic, *b"BLEN");
        assert_eq!(loaded_blend_file.header.version, version);
        assert_eq!(loaded_blend_file.header.data_offset, data_offset);
        assert_eq!(loaded_blend_file.data, data_content);

        Ok(())
    }

     #[test]
     fn test_read_from_file_with_padding() -> Result<(), FileSystemError> {
          let version = 2;
          let data_offset = 30; // Data starts after padding
          let data_content = b"Padded data test.";
          let blend_data = create_test_blend_file(version, data_offset, data_content);

          let mut cursor = Cursor::new(blend_data);
           fn read_blend_from_reader<R: Read + Seek>(reader: &mut R) -> Result<BlendFile, FileSystemError> {
                let mut header_bytes = [0; size_of::<BlendHeader>()];
                reader.read_exact(&mut header_bytes).map_err(|e| FileSystemError::IOError(format!("Header read error: {:?}", e)))?;

                let header = BlendHeader {
                    magic: [header_bytes[0], header_bytes[1], header_bytes[2], header_bytes[3]],
                    version: u32::from_le_bytes(header_bytes[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("Versiyon baytları geçersiz")))?),
                    data_offset: u32::from_le_bytes(header_bytes[8..12].try_into().map_err(|_| FileSystemError::InvalidData(format!("Veri ofset baytları geçersiz")))?),
                };

                if &header.magic != b"BLEN" {
                    return Err(FileSystemError::InvalidData(format!("Geçersiz sihirli sayı: {:?}", header.magic)));
                }

                let mut data = Vec::new();
                reader.seek(SeekFrom::Start(header.data_offset as u64)).map_err(|e| FileSystemError::IOError(format!("Seek error: {:?}", e)))?;

                let mut temp_buffer = [0u8; 1024];
                loop {
                    let bytes_read = reader.read(&mut temp_buffer).map_err(|e| FileSystemError::IOError(format!("Data read error: {:?}", e)))?;
                    if bytes_read == 0 {
                        break;
                    }
                    data.extend_from_slice(&temp_buffer[..bytes_read]);
                }

                Ok(BlendFile { header, data })
           }

          let loaded_blend_file = read_blend_from_reader(&mut cursor)?;

          assert_eq!(loaded_blend_file.header.magic, *b"BLEN");
          assert_eq!(loaded_blend_file.header.version, version);
          assert_eq!(loaded_blend_file.header.data_offset, data_offset);
          assert_eq!(loaded_blend_file.data, data_content);

          Ok(())
     }

     #[test]
     fn test_write_to_file_basic() -> Result<(), FileSystemError> {
          let version = 3;
          let data_offset = 12;
          let data_content = b"Write test data.";
          let blend_file = BlendFile {
              header: BlendHeader {
                  magic: *b"BLEN",
                  version,
                  data_offset,
              },
              data: data_content.to_vec(), // Requires alloc
          };

         // Use Cursor as a writer for the in-memory data
         let mut cursor = Cursor::new(Vec::new());

         // Call the write function (adapted to work with Cursor)
          fn write_blend_to_writer<W: Write + Seek>(writer: &mut W, blend_file: &BlendFile) -> Result<(), FileSystemError> {
               writer.write_all(&blend_file.header.magic).map_err(|e| FileSystemError::IOError(format!("Magic write error: {:?}", e)))?;
               writer.write_all(&blend_file.header.version.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Version write error: {:?}", e)))?;
               writer.write_all(&blend_file.header.data_offset.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Offset write error: {:?}", e)))?;

               let header_size = size_of::<BlendHeader>() as u32;
               if blend_file.header.data_offset > header_size {
                   let padding_size = (blend_file.header.data_offset - header_size) as usize;
                   let padding = vec![0u8; padding_size];
                   writer.write_all(&padding).map_err(|e| FileSystemError::IOError(format!("Padding write error: {:?}", e)))?;
               }

               writer.write_all(&blend_file.data).map_err(|e| FileSystemError::IOError(format!("Data write error: {:?}", e)))?;
               Ok(())
          }

         write_blend_to_writer(&mut cursor, &blend_file)?;

         // Get the written data from the cursor
         let written_data = cursor.into_inner();

         // Manually construct the expected data
         let mut expected_data = vec![];
         expected_data.extend_from_slice(b"BLEN");
         expected_data.extend_from_slice(&version.to_le_bytes());
         expected_data.extend_from_slice(&data_offset.to_le_bytes());
         expected_data.extend_from_slice(data_content);

         // Compare the written data with the expected data
         assert_eq!(written_data, expected_data);

         Ok(())
     }

     #[test]
     fn test_write_to_file_with_padding() -> Result<(), FileSystemError> {
          let version = 4;
          let data_offset = 20; // Padding needed (20 - 12 = 8 bytes)
          let data_content = b"Padded write test data.";
          let blend_file = BlendFile {
              header: BlendHeader {
                  magic: *b"BLEN",
                  version,
                  data_offset,
              },
              data: data_content.to_vec(), // Requires alloc
          };

         let mut cursor = Cursor::new(Vec::new());
          fn write_blend_to_writer<W: Write + Seek>(writer: &mut W, blend_file: &BlendFile) -> Result<(), FileSystemError> {
               writer.write_all(&blend_file.header.magic).map_err(|e| FileSystemError::IOError(format!("Magic write error: {:?}", e)))?;
               writer.write_all(&blend_file.header.version.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Version write error: {:?}", e)))?;
               writer.write_all(&blend_file.header.data_offset.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Offset write error: {:?}", e)))?;

               let header_size = size_of::<BlendHeader>() as u32;
               if blend_file.header.data_offset > header_size {
                   let padding_size = (blend_file.header.data_offset - header_size) as usize;
                   let padding = vec![0u8; padding_size];
                   writer.write_all(&padding).map_err(|e| FileSystemError::IOError(format!("Padding write error: {:?}", e)))?;
               }

               writer.write_all(&blend_file.data).map_err(|e| FileSystemError::IOError(format!("Data write error: {:?}", e)))?;
               Ok(())
          }
         write_blend_to_writer(&mut cursor, &blend_file)?;

         let written_data = cursor.into_inner();

         let mut expected_data = vec![];
         expected_data.extend_from_slice(b"BLEN");
         expected_data.extend_from_slice(&version.to_le_bytes());
         expected_data.extend_from_slice(&data_offset.to_le_bytes());
         expected_data.extend(vec![0u8; (data_offset - size_of::<BlendHeader>() as u32) as usize]); // Padding
         expected_data.extend_from_slice(data_content);

         assert_eq!(written_data, expected_data);

         Ok(())
     }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
}


// Redundant no_std print module and panic handler removed.
