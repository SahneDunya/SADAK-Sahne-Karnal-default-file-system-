#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths


// Gerekli Sahne64 modüllerini ve yapılarını içeri aktar (assume these are defined elsewhere)
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
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io


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


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilemp4.rs'den kopyalandı)
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


/// Simple wrapper around a Read + Seek reader for raw file access.
/// Manages the underlying file resource via a Handle.
pub struct OFile<R: Read + Seek> {
    pub reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    pub file_size: u64, // Store file size for checks
}

impl<R: Read + Seek> OFile<R> {
    /// Creates a new `OFile` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        Self { reader, handle, file_size }
    }

    /// Reads the entire content of the file into a Vec<u8>.
    ///
    /// # Returns
    ///
    /// A Result containing the file content as Vec<u8> or a FileSystemError.
    pub fn read_all(&mut self) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
        // Reset position to the beginning of the file
         self.reader.seek(SeekFrom::Start(0)).map_err(map_core_io_error_to_fs_error)?;

        let mut buffer = Vec::with_capacity(self.file_size as usize); // Allocate with capacity if size is known
        // Use the standard read_to_end method on the Read trait
        self.reader.read_to_end(&mut buffer).map_err(map_core_io_error_to_fs_error)?; // Map core::io::Error to FileSystemError
        Ok(buffer)
    }

    /// Explicitly closes the underlying file resource.
    /// The resource is also closed automatically when the OFile is dropped.
    pub fn close(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
        if let Some(handle) = self.handle.take() { // Use take() to prevent double-closing
             resource::release(handle).map_err(map_sahne_error_to_fs_error)?; // Map SahneError
        }
        Ok(())
    }

    // Add other reading/seeking methods if needed, using self.reader.
     pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, FileSystemError> {
         self.reader.read(buf).map_err(map_core_io_error_to_fs_error)
     }
     pub fn seek(&mut self, pos: SeekFrom) -> Result<u64, FileSystemError> {
         self.reader.seek(pos).map_err(map_core_io_error_to_fs_error)
     }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for OFile<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the OFile is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: OFile drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a file from the given path (std) or resource ID (no_std)
/// and creates an OFile instance.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the OFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_file<P: AsRef<Path>>(file_path: P) -> Result<OFile<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    Ok(OFile::from_reader(reader, None, file_size)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_file(file_path: &str) -> Result<OFile<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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
    let reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek

    Ok(OFile::from_reader(reader, Some(handle), file_size)) // Pass the handle to the OFile
}


// Example main function (no_std)
#[cfg(feature = "example_o")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("OFile example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/my_data.bin" exists.
      let file_res = open_file("sahne://files/my_data.bin");
      match file_res {
          Ok(mut o_file) => { // Need mut for read_all
              crate::println!("Attempting to read file content...");
              match o_file.read_all() {
                  Ok(content) => {
                      crate::println!("Read {} bytes.", content.len());
                      // Process content here if needed
                  },
                  Err(e) => {
                      crate::eprintln!("File read error: {:?}", e);
                      return Err(e);
                  }
              }
     //         // File is automatically closed when o_file goes out of scope (due to Drop)
     //         // Or you can call o_file.close()?; explicitly
          },
          Err(e) => crate::eprintln!("Error opening file: {:?}", e),
      }

     eprintln!("OFile example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_o")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("OFile example (std) starting...");
     eprintln!("OFile example (std) using simple reader.");

     // This example needs a dummy file.
     use std::fs::remove_file;
     use std::io::Write;

     let file_path = Path::new("example.bin");

     // Create a dummy file
     let dummy_content = b"This is some raw binary data.";

     match File::create(file_path) {
          Ok(mut file) => {
               if let Err(e) = file.write_all(dummy_content) {
                    eprintln!("Error writing dummy file: {}", e);
                    return Err(map_std_io_error_to_fs_error(e));
               }
          },
          Err(e) => {
               eprintln!("Error creating dummy file: {}", e);
               return Err(map_std_io_error_to_fs_error(e));
          }
     }

     match open_file(file_path) { // Call the function that opens and creates the OFile
         Ok(mut o_file) => { // Need mut for read_all
             println!("Attempting to read file content...");
             match o_file.read_all() {
                 Ok(content) => {
                     println!("Read {} bytes.", content.len());
                     println!("Content: {:?}", content); // Print raw bytes
                     // Process content here if needed
                 },
                 Err(e) => {
                     eprintln!("File read error: {}", e); // std error display
                     // Don't return error, let cleanup run
                 }
             }
              // File is automatically closed when o_file goes out of scope (due to Drop)
              // Or you can call o_file.close()?; explicitly
         }
         Err(e) => {
              eprintln!("Error opening file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(file_path) {
          eprintln!("Error removing dummy file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("OFile example (std) finished.");

     Ok(())
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
mod tests {
     // Needs std::io::Cursor for testing Read+Seek on dummy data
     #[cfg(feature = "std")]
     use std::io::Cursor;
     #[cfg(feature = "std")]
     use std::io::{Read, Seek, SeekFrom};
      #[cfg(feature = "std")]
      use std::fs::remove_file; // For cleanup
      #[cfg(feature = "std")]
      use std::path::Path;
      #[cfg(feature = "std")]
      use std::io::Write; // For creating dummy files


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests


     // Test read_all with in-memory cursor
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_read_all_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
         let dummy_data = b"This is some test data.";
         let file_size = dummy_data.len() as u64;
         let cursor = Cursor::new(dummy_data.to_vec()); // Create a Cursor from bytes

         // Create a dummy OFile with the cursor reader
         let mut o_file = OFile::from_reader(cursor, None, file_size); // Pass None for handle

         // Call read_all
         let content = o_file.read_all()?;

         // Assert content is correct
         assert_eq!(content, dummy_data);

         // Verify the reader position is at the end after read_all
         assert_eq!(o_file.reader.stream_position().unwrap(), file_size);


         Ok(())
     }

     // Test open_file in std environment (uses actual file I/O)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_open_file_std() -> Result<(), FileSystemError> { // Return FileSystemError
          let dir = tempfile::tempdir().map_err(|e| FileSystemError::IOError(format!("Tempdir error: {}", e)))?;
          let file_path = dir.path().join("test_std_o.bin");

          // Create a dummy file using std FS
           let dummy_content = b"Standard library file test.";
          let mut file = File::create(&file_path).map_err(|e| map_std_io_error_to_fs_error(e))?;
          file.write_all(dummy_content).map_err(|e| map_std_io_error_to_fs_error(e))?;

          // Call open_file with the file path
          let mut o_file = open_file(&file_path).map_err(|e| {
               // Clean up the file on error before returning
               let _ = remove_file(&file_path);
               e
          })?;

          // Assert file size is correct
           assert_eq!(o_file.file_size, dummy_content.len() as u64);

          // Read content and assert it's correct
           let content = o_file.read_all()?;
           assert_eq!(content, dummy_content);

           // Explicitly close the file
           o_file.close()?;


          // Clean up the dummy file
          let _ = remove_file(&file_path); // Ignore result, best effort cleanup

          Ok(())
      }


     // TODO: Add tests for open_file in no_std environment using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include opening valid/invalid files and handling IO errors.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_o", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
