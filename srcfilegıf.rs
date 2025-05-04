#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel Sahne64 tipleri ve modülleri (assume these are defined elsewhere)
use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri, SahneError, Handle
// Sahne64 resource modülü (assume defined elsewhere)
#[cfg(not(feature = "std"))]
use crate::resource;
// Sahne64 fs modülü (for fstat and read_at, assume defined elsewhere)
#[cfg(not(feature = "std"))]
use crate::fs;


// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::{File, self};
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths


// alloc crate for String
use alloc::string::String;
use alloc::format;

// core::result, core::option, core::fmt
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::convert::TryInto; // For array slicing

// core::io traits and types needed for SahneResourceReader (if used)
#[cfg(not(feature = "std"))]
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io


// byteorder crate (no_std compatible)
use byteorder::{LittleEndian, ReadBytesExt, ByteOrder}; // LittleEndian, ReadBytesExt, ByteOrder trait/types


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


/// Custom error type for GIF parsing issues.
#[derive(Debug)]
pub enum GifError {
    InvalidSignature([u8; 3]),
    TruncatedHeader(usize), // Read size
    InvalidPackedFields(u8), // The packed byte value
    // Add other GIF specific parsing errors here (e.g., unexpected block type)
}

// Implement Display for GifError
impl fmt::Display for GifError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GifError::InvalidSignature(sig) => write!(f, "Geçersiz GIF imzası: {:x?}", sig),
            GifError::TruncatedHeader(bytes_read) => write!(f, "Başlık beklenenden kısa, {} bayt okundu.", bytes_read),
            GifError::InvalidPackedFields(packed) => write!(f, "Geçersiz paketlenmiş alanlar: {:x}", packed),
        }
    }
}

// Helper function to map GifError to FileSystemError
fn map_gif_error_to_fs_error(e: GifError) -> FileSystemError {
    match e {
        GifError::InvalidSignature(sig) => FileSystemError::InvalidData(format!("Geçersiz GIF imzası: {:x?}", sig)),
        GifError::TruncatedHeader(bytes_read) => FileSystemError::InvalidData(format!("GIF başlığı beklenenden kısa, {} bayt okundu.", bytes_read)),
        GifError::InvalidPackedFields(packed) => FileSystemError::InvalidData(format!("Geçersiz paketlenmiş alanlar: {:x}", packed)),
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilefbx.rs'den kopyalandı)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
// fstat ile dosya boyutını alarak seek(End) desteği sağlar.
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
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        if self.position >= self.file_size {
            return Ok(0); // EOF
        }
        let bytes_available = (self.file_size - self.position) as usize;
        let bytes_to_read = core::cmp::min(buf.len(), bytes_available);

        if bytes_to_read == 0 {
             return Ok(0);
        }

        // Assuming fs::read_at(handle, offset, buf) Result<usize, SahneError>
        let bytes_read = fs::read_at(self.handle, self.position, &mut buf[..bytes_to_read])
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::read_at error: {:?}", e)))?;

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read that uses read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize,
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

        self.position = new_pos as u64;
        Ok(self.position)
    }
    // stream_position has a default implementation in core::io::Seek that uses seek(Current(0))
}


// GIF Structures
#[derive(Debug)]
pub struct GifHeader {
    pub signature: [u8; 3], // "GIF"
    pub version: [u8; 3], // "87a" or "89a"
    pub logical_screen_width: u16,
    pub logical_screen_height: u16,
    pub global_color_table_flag: bool, // 1 bit
    pub color_resolution: u8, // 3 bits
    pub sort_flag: bool, // 1 bit
    pub global_color_table_size: u8, // 3 bits (actual size is 2^(value + 1))
    pub background_color_index: u8,
    pub pixel_aspect_ratio: u8,
}

/// Reads and parses the GIF header from the provided reader.
/// Assumes the reader is positioned at the start of the file (offset 0).
/// Uses Little Endian byte order for u16 values.
fn read_gif_header<R: Read + Seek>(reader: &mut R) -> Result<GifHeader, FileSystemError> { // FileSystemError döner
    let header_size = 13; // Size of the GIF Logical Screen Descriptor
    let mut buffer = [0u8; 13];

    reader.seek(SeekFrom::Start(0)).map_err(map_core_io_error_to_fs_error)?; // Ensure at start

    // Use read_exact from core::io::Read (implemented by SahneResourceReader/BufReader)
    reader.read_exact(&mut buffer).map_err(|e| match e.kind() {
         CoreIOErrorKind::UnexpectedEof => map_gif_error_to_fs_error(GifError::TruncatedHeader(e.bytes_initially_read().unwrap_or(0))), // Use bytes_initially_read if available
         _ => map_core_io_error_to_fs_error(e),
     })?;

    let signature: [u8; 3] = buffer[0..3].try_into().unwrap(); // OK to unwrap after read_exact of fixed size
    let version: [u8; 3] = buffer[3..6].try_into().unwrap(); // OK to unwrap after read_exact of fixed size

    // Check GIF signature ("GIF")
    if &signature != b"GIF" {
         return Err(map_gif_error_to_fs_error(GifError::InvalidSignature(signature))); // GifError -> FileSystemError
    }
    // Optional: Check version ("87a" or "89a") if needed

    // Use byteorder::ReadBytesExt for safe Little Endian reading
    let mut cursor = core::io::Cursor::new(&buffer[6..]); // Use Cursor to read bytes from buffer safely
    let logical_screen_width = cursor.read_u16::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;
    let logical_screen_height = cursor.read_u16::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;

    let packed_fields = buffer[10];
    let background_color_index = buffer[11];
    let pixel_aspect_ratio = buffer[12];

    let global_color_table_flag = (packed_fields & 0x80) != 0;
    let color_resolution = (packed_fields & 0x70) >> 4; // 3 bits
    let sort_flag = (packed_fields & 0x08) != 0; // 1 bit
    let global_color_table_size_encoded = packed_fields & 0x07; // 3 bits
    let global_color_table_size = 1 << (global_color_table_size_encoded + 1); // Actual size is 2^(value + 1)


    Ok(GifHeader {
        signature,
        version,
        logical_screen_width,
        logical_screen_height,
        global_color_table_flag,
        color_resolution,
        sort_flag,
        global_color_table_size: global_color_table_size as u8, // Store as u8 (max 256 fits)
        background_color_index,
        pixel_aspect_ratio,
    })
}


/// Reads and parses the GIF header from the given path (std) or resource ID (no_std).
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the parsed GifHeader or a FileSystemError.
#[cfg(feature = "std")]
pub fn read_gif_header_from_file<P: AsRef<Path>>(file_path: P) -> Result<GifHeader, FileSystemError> { // FileSystemError döner
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    read_gif_header(&mut reader) // Call the parsing function with the reader
}

#[cfg(not(feature = "std"))]
pub fn read_gif_header_from_file(file_path: &str) -> Result<GifHeader, FileSystemError> { // FileSystemError döner
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutunu al (SahneResourceReader için gerekli)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReader oluştur
    let mut reader = SahneResourceReader::new(handle, file_size);

    // Başlığı oku ve ayrıştır
    let header = read_gif_header(&mut reader) // Call the parsing function with the reader
        .map_err(|e| {
             // Log resource release error but return the parsing error
             let _ = resource::release(handle).map_err(|release_e| eprintln!("WARN: Kaynak serbest bırakma hatası after GIF header read error: {:?}", release_e));
             e // Pass the original parsing error
         })?;


    // Kaynağı serbest bırak (Sadece başlığı okuduk)
    let _ = resource::release(handle).map_err(|e| {
         eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
         map_sahne_error_to_fs_error(e) // Return this error if crucial, or just log
     });


    Ok(header)
}


// Example main functions
#[cfg(feature = "example_gif")] // Different feature flag
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     #[cfg(not(feature = "std"))]
     {
          eprintln!("GIF header example (no_std) starting...");
          // TODO: Call init_console(crate::Handle(3)); if needed
     }
     #[cfg(feature = "std")]
     {
          eprintln!("GIF header example (std) starting...");
     }

     // Test with a hypothetical file path (std) or resource ID (no_std)
     #[cfg(feature = "std")]
     let gif_path = Path::new("example.gif"); // This file needs to exist for the std example
     #[cfg(not(feature = "std"))]
     let gif_path = "sahne://files/example.gif"; // This resource needs to exist for the no_std example


     match read_gif_header_from_file(gif_path) { // Call the function that opens and reads
         Ok(header) => {
              #[cfg(not(feature = "std"))]
              crate::println!("GIF Başlığı: {:?}", header);
              #[cfg(feature = "std")]
              println!("GIF Başlığı: {:?}", header);
         }
         Err(e) => {
              #[cfg(not(feature = "std"))]
              crate::eprintln!("GIF başlığı okuma hatası: {:?}", e);
              #[cfg(feature = "std")]
              eprintln!("GIF başlığı okuma hatası: {}", e); // std error display
              return Err(e);
         }
     }

     #[cfg(not(feature = "std"))]
     eprintln!("GIF header example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("GIF header example (std) finished.");

     Ok(())
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
mod tests {
     // Needs byteorder for creating dummy data and std::io::Cursor for testing Read+Seek
     #[cfg(feature = "std")]
     use std::io::Cursor;
     #[cfg(feature = "std")]
     use byteorder::{LittleEndian as StdLittleEndian, WriteBytesExt as StdWriteBytesExt};
     #[cfg(feature = "std")]
     use std::io::{Read, Seek, SeekFrom, Write};
     #[cfg(feature = "std")]
     use std::fs::remove_file; // For cleanup
     #[cfg(feature = "std")]
     use std::path::Path;


     use super::*; // Import items from the parent module
     use alloc::vec; // vec! macro
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests


     // Helper function to create a dummy GIF header bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
      fn create_dummy_gif_header_bytes(width: u16, height: u16, packed: u8, bg_color: u8, aspect_ratio: u8) -> Vec<u8> {
          let mut buffer = Cursor::new(Vec::new());
          buffer.write_all(b"GIF").unwrap();
          buffer.write_all(b"89a").unwrap(); // Example version
          buffer.write_u16::<LittleEndian>(width).unwrap();
          buffer.write_u16::<LittleEndian>(height).unwrap();
          buffer.write_u8(packed).unwrap();
          buffer.write_u8(bg_color).unwrap();
          buffer.write_u8(aspect_ratio).unwrap();
          buffer.into_inner()
      }

     // Helper function to read GIF header from a generic reader (simulates read_gif_header)
     // This needs to be adapted to return FileSystemError
      fn read_gif_header_from_reader<R: Read + Seek>(reader: &mut R) -> Result<GifHeader, FileSystemError> {
          // Copy of the parsing logic from read_gif_header, but returning FileSystemError
           let header_size = 13;
           let mut buffer = [0u8; 13];

           reader.seek(SeekFrom::Start(0)).map_err(map_core_io_error_to_fs_error)?;

           reader.read_exact(&mut buffer).map_err(|e| match e.kind() {
                CoreIOErrorKind::UnexpectedEof => map_gif_error_to_fs_error(GifError::TruncatedHeader(e.bytes_initially_read().unwrap_or(0))),
                _ => map_core_io_error_to_fs_error(e),
           })?;

           let signature: [u8; 3] = buffer[0..3].try_into().unwrap();
           let version: [u8; 3] = buffer[3..6].try_into().unwrap();

           if &signature != b"GIF" {
                return Err(map_gif_error_to_fs_error(GifError::InvalidSignature(signature)));
           }

           let mut cursor = core::io::Cursor::new(&buffer[6..]);
           let logical_screen_width = cursor.read_u16::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;
           let logical_screen_height = cursor.read_u16::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;

           let packed_fields = buffer[10];
           let background_color_index = buffer[11];
           let pixel_aspect_ratio = buffer[12];

           let global_color_table_flag = (packed_fields & 0x80) != 0;
           let color_resolution = (packed_fields & 0x70) >> 4;
           let sort_flag = (packed_fields & 0x08) != 0;
           let global_color_table_size_encoded = packed_fields & 0x07;
           let global_color_table_size = 1 << (global_color_table_size_encoded + 1);


           Ok(GifHeader {
               signature,
               version,
               logical_screen_width,
               logical_screen_height,
               global_color_table_flag,
               color_resolution,
               sort_flag,
               global_color_table_size: global_color_table_size as u8,
               background_color_index,
               pixel_aspect_ratio,
           })
      }


     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_read_gif_header() -> Result<(), FileSystemError> { // Return FileSystemError
          // Create dummy GIF header bytes
          let width = 640;
          let height = 480;
          // Packed fields: GCT flag = 1 (present), Color Resolution = 7 (8 bpp), Sort Flag = 0, GCT Size = 7 (256 colors)
          let packed_fields = (1 << 7) | (7 << 4) | (0 << 3) | 7; // 128 | 112 | 0 | 7 = 247 (0xF7)
          let bg_color = 0;
          let aspect_ratio = 0;

          let dummy_header_bytes = create_dummy_gif_header_bytes(width, height, packed_fields, bg_color, aspect_ratio);

          // Use Cursor as a reader for the in-memory data
          let mut cursor = Cursor::new(dummy_header_bytes.clone());

          // Call the parsing function with the cursor
          let loaded_header = read_gif_header_from_reader(&mut cursor)?;

          // Assert the loaded header fields
          assert_eq!(loaded_header.signature, *b"GIF");
          assert_eq!(loaded_header.version, *b"89a");
          assert_eq!(loaded_header.logical_screen_width, width);
          assert_eq!(loaded_header.logical_screen_height, height);
          assert_eq!(loaded_header.global_color_table_flag, true);
          assert_eq!(loaded_header.color_resolution, 7);
          assert_eq!(loaded_header.sort_flag, false);
          assert_eq!(loaded_header.global_color_table_size, 256); // 2^(7+1)
          assert_eq!(loaded_header.background_color_index, bg_color);
          assert_eq!(loaded_header.pixel_aspect_ratio, aspect_ratio);

          Ok(())
     }

      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_read_gif_header_invalid_signature() {
           // Create dummy bytes with invalid signature
           let invalid_bytes = b"XXX89a\x00\x00\x00\x00\xf7\x00\x00".to_vec(); // 13 bytes total

           let mut cursor = Cursor::new(invalid_bytes);
           let result = read_gif_header_from_reader(&mut cursor);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => {
                   assert!(msg.contains("Geçersiz GIF imzası"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_read_gif_header_truncated() {
           // Create dummy bytes that are too short
           let truncated_bytes = b"GIF89a".to_vec(); // Only 6 bytes

           let mut cursor = Cursor::new(truncated_bytes);
           let result = read_gif_header_from_reader(&mut cursor);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof
                    assert!(msg.contains("Beklenenden erken dosya sonu"));
                    // The exact bytes_initially_read might vary based on Cursor/Read implementation details
                    // The original code mapped TruncatedHeader(bytes_read) which is also valid.
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This involves simulating resource acquire/release, fs::read_at, fs::fstat.
}

// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_gif", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
