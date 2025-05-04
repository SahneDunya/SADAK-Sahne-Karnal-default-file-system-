#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt}; // Include ReadExt for read_to_string
#[cfg(feature = "std")]
use std::path::Path; // For std file paths


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt}; // core::io, Include ReadExt for read_to_string


// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülünden import edildiği varsayılır


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


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilejpg.rs'den kopyalandı)
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
impl core::io::Read for SahneResourceReader { // Use core::io::Read trait
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> { // Return core::io::Error
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
            .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::read_at error: {:?}", e)))?; // Map SahneError to core::io::Error

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read that uses read
    // read_to_end has a default implementation in core::io::ReadExt that uses read
    // read_to_string has a default implementation in core::io::ReadExt that uses read and from_utf8
}

#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReader { // Use core::io::Seek trait
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, core::io::Error> { // Return core::io::Error
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize,
            SeekFrom::End(offset) => {
                file_size_isize.checked_add(offset)
                    .ok_or_else(|| core::io::Error::new(core::io::ErrorKind::InvalidInput, format!("Seek position out of bounds (from end)")))?
            },
            SeekFrom::Current(offset) => {
                (self.position as isize).checked_add(offset)
                     .ok_or_else(|| core::io::Error::new(core::io::ErrorKind::InvalidInput, format!("Seek position out of bounds (from current)")))?
            },
        };

        if new_pos < 0 {
            return Err(core::io::Error::new(core::io::ErrorKind::InvalidInput, format!("Invalid seek position (result is negative)")));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
    // stream_position has a default implementation in core::io::Seek that uses seek(Current(0))
}

// Removed redundant arch, SahneError, syscall, fs module definitions.
// Removed redundant print module and panic handler.


/// Represents a loaded ODF document (currently only stores content.xml as String).
/// NOTE: This is a highly simplified representation for demonstration.
/// A real ODF parser would handle ZIP archives and XML parsing.
pub struct OdfDocument {
    // Store relevant ODF metadata here later, not the full content.
    // For now, this struct is simplified or might become obsolete if we only provide a read_file_as_string function.
     // pub content: String, // Removed full content storage
}

impl OdfDocument {
    // This method is not directly used in the refactored approach,
    // but could be a placeholder for future ODF processing methods.
    // pub fn process(&self) { ... }
}


/// Reads the entire content of a file into a String, assuming UTF-8 encoding.
/// This function provides basic text file reading for Sahne64.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the file content as String or a FileSystemError.
#[cfg(feature = "std")]
pub fn read_file_as_string<P: AsRef<Path>>(file_path: P) -> Result<String, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    let mut content = String::new(); // Requires alloc
    // Use the standard read_to_string method
    reader.read_to_string(&mut content).map_err(|e| map_std_io_error_to_fs_error(e))?; // Maps IO errors and UTF8 errors

    // In std, file is closed when 'reader' (and thus 'file') goes out of scope.
    Ok(content)
}

#[cfg(not(feature = "std"))]
pub fn read_file_as_string(file_path: &str) -> Result<String, FileSystemError> { // Return FileSystemError
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
    let mut reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek

    let mut content = String::new(); // Requires alloc
    // Use the standard read_to_string method
    // read_to_string handles reading chunks and UTF8 conversion
    reader.read_to_string(&mut content).map_err(|e| map_core_io_error_to_fs_error(e))?; // Maps IO errors and UTF8 errors

    // The Handle is automatically released when 'reader' goes out of scope (due to Drop on SahneResourceReader if implemented)
    // or it needs explicit release if SahneResourceReader doesn't implement Drop for the handle.
    // Let's assume SahneResourceReader handles its handle resource release.
    // If not, we would need to wrap the reader in a struct that implements Drop for the handle.
    // Based on previous file refactors (MkvParser, Mp4Parser, ObjFile, OFile), the resource Handle
    // was explicitly stored in the parser struct and released in Drop. Let's maintain that pattern.
    // We need a struct to hold the reader and the handle. The function signature would return this struct.
    // Or, the function could release the handle after reading. Releasing after reading is simpler for read_file_as_string.
    // Let's modify SahneResourceReader to NOT implement Drop for the handle, and release the handle here.

    // Release the handle after reading (alternative to Drop in reader/wrapper struct)
     if let Err(e) = resource::release(handle) {
          eprintln!("WARN: read_file_as_string sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
          // This is a warning, don't return an error if the read was successful
     }


    Ok(content)
}


// Example main function (no_std)
#[cfg(feature = "example_odf")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("ODF content reader example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy text file (simulating content.xml) and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/odf_content.xml" exists.
     // let content_res = read_file_as_string("sahne://files/odf_content.xml");
     // match content_res {
     //     Ok(content) => {
     //         crate::println!("Read {} bytes from ODF content.xml.", content.len());
     //         // Basic check if content contains expected string (simulating simple ODF analysis)
     //         if content.contains("Sahne64 Test Metni.") { // Requires String::contains
     //             crate::println!("Content found: 'Sahne64 Test Metni.'");
     //         } else {
     //              crate::println!("Content not found: 'Sahne64 Test Metni.'");
     //         }
     //     },
     //     Err(e) => crate::eprintln!("Error reading ODF content.xml: {:?}", e),
     // }

     eprintln!("ODF content reader example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_odf")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("ODF content reader example (std) starting...");
     eprintln!("ODF content reader example (std) using text file reading.");

     // This example needs a dummy text file (simulating content.xml).
     use std::fs::remove_file;
     use std::io::Write;


     let file_path = Path::new("test_content.xml");

     // Create a dummy content.xml file
     let dummy_content = b"<office:document-content>\
                             <office:body>\
                              <office:text>\
                               <text:p>Sahne64 Test Metni.</text:p>\
                              </office:text>\
                             </office:body>\
                            </office:document-content>";

     match File::create(file_path) {
          Ok(mut file) => {
               if let Err(e) = file.write_all(dummy_content) {
                    eprintln!("Error writing dummy content.xml file: {}", e);
                    return Err(map_std_io_error_to_fs_error(e));
               }
          },
          Err(e) => {
               eprintln!("Error creating dummy content.xml file: {}", e);
               return Err(map_std_io_error_to_fs_error(e));
          }
     }

     match read_file_as_string(file_path) { // Call the function that reads the file content
         Ok(content) => {
             println!("Read {} bytes from file.", content.len());
              // Basic check if content contains expected string
              if content.contains("Sahne64 Test Metni.") { // Requires String::contains
                  println!("Content found: 'Sahne64 Test Metni.'");
              } else {
                   println!("Content not found: 'Sahne64 Test Metni.'");
              }
         }
         Err(e) => {
              eprintln!("Error reading file: {}", e); // std error display
              // Don't return error, let cleanup run
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(file_path) {
          eprintln!("Error removing dummy file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("ODF content reader example (std) finished.");

     Ok(())
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
mod tests {
     // Needs std::io::Cursor for testing Read on dummy data
     #[cfg(feature = "std")]
     use std::io::Cursor;
     #[cfg(feature = "std")]
     use std::io::Read;
      #[cfg(feature = "std")]
      use std::fs::remove_file; // For cleanup
      #[cfg(feature = "std")]
      use std::path::Path;
      #[cfg(feature = "std")]
      use std::io::Write; // For creating dummy files


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec::Vec; // For Vec
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests


     // Test reading a simple string using an in-memory cursor
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_read_file_as_string_cursor() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error> for std test error handling
         let dummy_data = b"This is a test string.";
         let cursor = Cursor::new(dummy_data.to_vec()); // Create a Cursor from bytes

         // We need to simulate the SahneResourceReader's behavior wrapped in the file opening logic.
         // The read_file_as_string function takes a path, not a reader.
         // For testing the read_to_string logic itself, we can test it on a mock reader.
         // However, the structure is built around the open_file -> get reader pattern.
         // Let's test the core read_to_string logic directly on a Cursor, assuming
         // open_file correctly creates a reader over the data.

         // Simulate the reader obtained from open_file
         let mut reader = cursor; // Cursor implements Read

         let mut content = String::new(); // Requires alloc
         reader.read_to_string(&mut content)?; // Test the read_to_string method


         // Assert content is correct
         assert_eq!(content, "This is a test string.");

         Ok(())
     }

     // Test open_file + read_file_as_string in std environment (uses actual file I/O)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_read_file_as_string_std() -> Result<(), FileSystemError> { // Return FileSystemError
          let dir = tempfile::tempdir().map_err(|e| FileSystemError::IOError(format!("Tempdir error: {}", e)))?;
          let file_path = dir.path().join("test_std_text.txt");

          // Create a dummy text file using std FS
           let dummy_content = b"Standard library text file test content.\nSecond line.";
          let mut file = File::create(&file_path).map_err(|e| map_std_io_error_to_fs_error(e))?;
          file.write_all(dummy_content).map_err(|e| map_std_io_error_to_fs_error(e))?;

          // Call read_file_as_string with the file path
          let content = read_file_as_string(&file_path).map_err(|e| {
               // Clean up the file on error before returning
               let _ = remove_file(&file_path);
               e
          })?;

          // Assert content is correct
           assert_eq!(content, "Standard library text file test content.\nSecond line.");


          // Clean up the dummy file
          let _ = remove_file(&file_path); // Ignore result, best effort cleanup

          Ok(())
      }

      // Test reading a file with invalid UTF-8 bytes
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_read_file_as_string_invalid_utf8() {
            let dir = tempfile::tempdir().unwrap();
            let file_path = dir.path().join("invalid_utf8.txt");

            // Create a file with some valid UTF-8 and some invalid bytes
             let mut dummy_bytes = b"Valid text ".to_vec();
             dummy_bytes.extend_from_slice(&[0xFF, 0xFE, 0xFD]); // Invalid UTF-8 sequence
             dummy_bytes.extend_from_slice(b" more text.");

            let mut file = File::create(&file_path).unwrap();
            file.write_all(&dummy_bytes).unwrap();


            // Call read_file_as_string, expect an error
            let result = read_file_as_string(&file_path);

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from std::io::Error::from(std::str::Utf8Error) by read_to_string
                    // The exact error message might vary depending on std version and platform,
                    // but it should indicate a UTF-8 decoding failure.
                     assert!(msg.contains("invalid utf-8") || msg.contains("InvalidData") || msg.contains("UTF-8 hatası"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }

            // Clean up the dummy file
             let _ = remove_file(&file_path);
       }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include opening valid/invalid files, handling IO errors,
     // and correctly reading content from mock data (including testing EOF and errors during read).
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_odf", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
