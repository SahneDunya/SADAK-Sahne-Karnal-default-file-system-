#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz (zip crate requires std by default, may need features)

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader as StdBufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt}; // Added BufReader, Error, ErrorKind
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::O_RDONLY; // Import necessary fs flags


// zip crate (requires std::io::Read + Seek by default, needs features for no_std)
// Assuming zip::ZipArchive, zip::result::ZipError are available
use zip::ZipArchive;
use zip::result::ZipError;


// alloc crate for String, Vec, format!
use alloc::string::{String, ToString}; // Import ToString trait for to_string()
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt}; // core::io


// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden or common module


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

/// Helper function to map zip::result::ZipError to FileSystemError.
fn map_zip_error_to_fs_error(e: ZipError) -> FileSystemError {
     #[cfg(feature = "std")] // zip error might contain std::io::Error in std builds
     {
         if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<StdIOError>()) {
              return map_std_io_error_to_fs_error(io_err.clone()); // Clone is needed if source returns reference
         }
     }

    // Map zip error variants to FileSystemError
    match e {
         ZipError::Io(io_err) => {
              #[cfg(not(feature = "std"))]
              // In no_std, zip::result::ZipError::Io might contain core::io::Error
              map_core_io_error_to_fs_error(io_err)
              #[cfg(feature = "std")] // Already handled above if source is std::io::Error
              map_core_io_error_to_fs_error(io_err) // Fallback mapping for core::io::Error
         },
        ZipError::InvalidArchive(msg) => FileSystemError::InvalidData(format!("Invalid Zip archive: {}", msg)), // Requires alloc
        ZipError::FileNotFound => FileSystemError::NotFound(String::from("File not found in Zip archive")), // Requires alloc
        ZipError::UnsupportedArchive(msg) => FileSystemError::NotSupported(format!("Unsupported Zip archive feature: {}", msg)), // Requires alloc
        ZipError::InvalidPassword => FileSystemError::PermissionDenied(String::from("Invalid password for Zip archive")), // Requires alloc
        ZipError::Other(msg) => FileSystemError::Other(format!("Zip error: {}", msg)), // Requires alloc
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (copied from srcfilexml.rs)
// This requires fs::read_at and fstat.
// Assuming these are part of the standardized Sahne64 FS API.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReadSeek { // Renamed to reflect Read+Seek capability
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu
}

#[cfg(not(feature = "std"))]
impl SahneResourceReadSeek {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReadSeek { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl core::io::Read for SahneResourceReadSeek { // Use core::io::Read trait
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
}

#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReadSeek { // Use core::io::Seek trait
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

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReadSeek {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReadSeek is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReadSeek drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant fs, SahneError definitions.
// Removed redundant panic handler and print module boilerplate.


/// Opens a ZIP file from the given path (std) or resource ID (no_std)
/// for reading and returns a reader wrapping the file handle.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing a reader (implementing Read + Seek + Drop) or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_zip_reader<P: AsRef<Path>>(file_path: P) -> Result<File, FileSystemError> { // Return std::fs::File (implements Read+Seek+Drop)
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    Ok(file)
}

#[cfg(not(feature = "std"))]
pub fn open_zip_reader(file_path: &str) -> Result<SahneResourceReadSeek, FileSystemError> { // Return SahneResourceReadSeek (implements Read+Seek+Drop)
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutını al (needed for SahneResourceReadSeek)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;


    // SahneResourceReadSeek oluştur
    let reader = SahneResourceReadSeek::new(handle, file_size); // Implements core::io::Read + Seek + Drop

    Ok(reader) // Return the reader
}


/// Lists the contents (file names and metadata) of a ZIP archive.
/// Uses the `zip` crate to parse the archive structure.
///
/// # Arguments
///
/// * `zip_path`: The path to the ZIP file.
///
/// # Returns
///
/// A Result indicating success or a FileSystemError.
pub fn list_zip_contents(zip_path: &str) -> Result<(), FileSystemError> { // Return FileSystemError
    // Open the ZIP file using the standardized function
    let reader = open_zip_reader(zip_path)?; // open_zip_reader returns a reader implementing Read+Seek+Drop
    // The underlying reader/handle is automatically dropped when 'reader' goes out of scope.


    // Create a ZipArchive from the reader
    // The zip crate's ZipArchive requires a Read + Seek reader.
    let mut archive = ZipArchive::new(reader).map_err(|e| map_zip_error_to_fs_error(e))?; // Map ZipError


    println!("ZIP file contents of '{}':", zip_path); // Use standardized print

    // Iterate through the files in the archive
    for i in 0..archive.len() {
        match archive.by_index(i) {
            Ok(file) => {
                println!("  File: {}", file.name()); // File name (Requires alloc)
                println!("    Size: {} bytes (compressed: {} bytes)", file.size(), file.compressed_size());
                println!("    Compression method: {:?}", file.compression());
                // Add other metadata as needed (e.g., last modified time, CRC32)
            },
            Err(e) => {
                eprintln!("  Error reading file index {}: {}", i, map_zip_error_to_fs_error(e)); // Log the error but continue listing other files
                // Mapping the error here ensures consistent error output
            }
        }
    }


    Ok(()) // Return success
}


// Example main function (std)
#[cfg(feature = "example_zip")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> io::Result<()> { // Return std::io::Result for std example
     eprintln!("ZIP lister example (std) starting...");
     eprintln!("ZIP lister example (std) using zip crate.");

     // Create a dummy ZIP file for the example
     let file_path = Path::new("example.zip");
      use std::fs::remove_file;
      use std::io::Write;
      use zip::write::{ZipWriter, FileOptions};
      use zip::CompressionMethod;

      // Create a dummy ZIP file with some content
       match File::create(file_path) {
           Ok(file) => {
               let mut zip = ZipWriter::new(file);
               let options = FileOptions::default().compressionMethod(CompressionMethod::Deflated); // Using Deflated compression

               if let Err(e) = zip.add_directory("dir1/", options).map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Zip add_directory error: {}", e))) { eprintln!("Error adding directory: {}", e); }
               if let Err(e) = zip.add_file("file1.txt", options).and_then(|_| zip.write_all(b"This is file 1.")).map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Zip add_file error: {}", e))) { eprintln!("Error adding file1.txt: {}", e); }
               if let Err(e) = zip.add_file("dir1/file2.txt", options).and_then(|_| zip.write_all(b"This is file 2 in dir1.")).map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Zip add_file error: {}", e))) { eprintln!("Error adding dir1/file2.txt: {}", e); }

               if let Err(e) = zip.finish().map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Zip finish error: {}", e))) { eprintln!("Error finishing zip: {}", e); }
           },
           Err(e) => {
               eprintln!("Error creating dummy ZIP file: {}", e);
               return Err(e); // Return std::io::Error
           }
       }
       println!("Dummy ZIP file created: {}", file_path.display());


     // List the contents of the dummy ZIP file
     match list_zip_contents(file_path.to_string_lossy().into_owned().as_str()) { // Pass as &str after converting PathBuf to String
         Ok(_) => {
             println!("ZIP contents listed successfully.");
         }
         Err(e) => {
             eprintln!("Error listing ZIP contents: {}", e); // std error display
              // Map FileSystemError back to std::io::Error for std main
             match e {
                 FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                 FileSystemError::InvalidData(msg) => return Err(io::Error::new(io::ErrorKind::InvalidData, msg)),
                 FileSystemError::NotFound(msg) => return Err(io::Error::new(io::ErrorKind::NotFound, msg)),
                 FileSystemError::PermissionDenied(msg) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, msg)),
                 FileSystemError::NotSupported(msg) => return Err(io::Error::new(io::ErrorKind::Unsupported, msg)),
                 FileSystemError::Other(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
             }
         }
     }


     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy ZIP file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("ZIP lister example (std) finished.");

     Ok(()) // Return Ok from std main
}

// Example main function (no_std)
#[cfg(feature = "example_zip")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for no_std example
     eprintln!("ZIP lister example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // and simulate fs syscalls.
     // This is complex and requires a testing framework or simulation.
     // It also requires the zip crate compiled with no_std + alloc features.

     eprintln!("ZIP lister example (no_std) needs Sahne64 mocks and zip crate with no_std features to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real) and a dummy ZIP file.
     // 2. The zip crate compiled with no_std and alloc features.


      // Hypothetical usage with Sahne64 mocks:
      // // Assume a mock filesystem has a ZIP file at "sahne://files/example.zip".
      //
      // // List the contents of the ZIP file
       match list_zip_contents("sahne://files/example.zip") {
           Ok(_) => {
               crate::println!("ZIP contents listed successfully.");
           }
           Err(e) => {
               crate::eprintln!("Error listing ZIP contents: {:?}", e); // no_std print
           }
       }


     Ok(()) // Dummy return
}


// Test module (std feature active)
#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use std::io::Cursor as StdCursor; // For in-memory testing
    use std::error::Error; // For Box<dyn Error>
    use zip::write::{ZipWriter, FileOptions};
    use zip::CompressionMethod;


    // Helper to create dummy ZIP bytes in memory
    fn create_dummy_zip_bytes() -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buffer = StdCursor::new(Vec::new()); // Use std::io::Cursor for in-memory buffer
        let mut zip = ZipWriter::new(&mut buffer);
        let options = FileOptions::default().compressionMethod(CompressionMethod::Deflated); // Using Deflated compression

        zip.add_directory("dir1/", options)?;
        zip.add_file("file1.txt", options).and_then(|_| zip.write_all(b"This is file 1."))?;
        zip.add_file("dir1/file2.txt", options).and_then(|_| zip.write_all(b"This is file 2 in dir1."))?;
        zip.add_file("empty.txt", options)?; // Add an empty file


        zip.finish()?; // Requires Seek on the underlying writer

        Ok(buffer.into_inner()) // Return the underlying Vec<u8>
    }


    // Mock open_zip_reader for std tests using Cursor
    struct MockOpenZipReader {
        cursor: Option<StdCursor<Vec<u8>>>, // Use Option to allow taking the cursor
    }
    impl MockOpenZipReader {
        fn new(data: Vec<u8>) -> Self { MockOpenZipReader { cursor: Some(StdCursor::new(data)) } }
    }
    #[cfg(feature = "std")] // Implement core::io traits for Mock
    impl Read for MockOpenZipReader { fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> { self.cursor.as_mut().unwrap().read(buf) } }
    #[cfg(feature = "std")]
    impl Seek for MockOpenZipReader { fn seek(&mut self, pos: SeekFrom) -> Result<u64, core::io::Error> { self.cursor.as_mut().unwrap().seek(pos) } }
    #[cfg(feature = "std")]
    impl Drop for MockOpenZipReader { fn drop(&mut self) { println!("MockOpenZipReader dropped"); } } // For testing Drop


    // Helper function to simulate list_zip_contents but taking a reader
     fn list_zip_contents_from_reader<R: Read + Seek>(mut reader: R, zip_path: &str) -> Result<(), FileSystemError> {
          // Create a ZipArchive from the reader
           let mut archive = ZipArchive::new(reader).map_err(|e| map_zip_error_to_fs_error(e))?; // Map ZipError

           println!("ZIP file contents of '{}':", zip_path); // Use standardized print

           // Iterate through the files in the archive
           for i in 0..archive.len() {
               match archive.by_index(i) {
                   Ok(file) => {
                       println!("  File: {}", file.name()); // File name (Requires alloc)
                       println!("    Size: {} bytes (compressed: {} bytes)", file.size(), file.compressed_size());
                       println!("    Compression method: {:?}", file.compression());
                   },
                   Err(e) => {
                       eprintln!("  Error reading file index {}: {}", i, map_zip_error_to_fs_error(e)); // Log the error
                   }
               }
           }
          Ok(())
     }


    #[test]
    fn test_list_zip_contents_valid_zip_cursor() -> Result<(), FileSystemError> { // Return FileSystemError for std test
        // Create dummy valid ZIP bytes in memory
        let zip_bytes = create_dummy_zip_bytes()
             .map_err(|e| FileSystemError::Other(format!("Test data creation error: {}", e)))?;

        // Create a mock reader with the ZIP data
        let mock_reader = MockOpenZipReader::new(zip_bytes);

        // Simulate calling list_zip_contents with the mock reader
        // The actual list_zip_contents takes a path and uses open_zip_reader internally.
        // We call the helper function list_zip_contents_from_reader for in-memory test.
        let zip_path = "mock_test.zip";
        list_zip_contents_from_reader(mock_reader, zip_path)?; // Use the helper


        // For automated testing, checking stdout is difficult.
        // A robust test would capture stdout or modify the function to return a list of file names/metadata.
        // For now, rely on the function not panicking and returning Ok.
        // Manual inspection of test output is needed.


        Ok(()) // Return Ok from test function
    }

    #[test]
     fn test_list_zip_contents_invalid_zip_cursor() {
          // Create dummy bytes that are NOT a valid ZIP archive
          let invalid_bytes = vec![0x00, 0x01, 0x02, 0x03, 0x04]; // Not a ZIP signature

          let mock_reader = MockOpenZipReader::new(invalid_bytes);

          let zip_path = "mock_invalid.zip";

          // Attempt to list contents, expect an error (InvalidArchive)
          let result = list_zip_contents_from_reader(mock_reader, zip_path); // Use the helper


          assert!(result.is_err());
          match result.unwrap_err() {
              FileSystemError::InvalidData(msg) => { // Mapped from ZipError::InvalidArchive
                  assert!(msg.contains("Invalid Zip archive"));
                  // Specific error message content might vary based on zip crate version
              },
               FileSystemError::IOError(msg) => { // Could also be an IO error if reading fails
                   assert!(msg.contains("XML IO hatası") || msg.contains("CoreIOError")); // Error message might be from underlying reader
               }
              _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
          }
     }

     #[test]
      fn test_list_zip_contents_empty_file_cursor() -> Result<(), FileSystemError> {
           // Create an empty byte vector
           let empty_bytes = vec![];

           let mock_reader = MockOpenZipReader::new(empty_bytes);

           let zip_path = "mock_empty.zip";

           // Attempt to list contents of an empty file (should result in an error, possibly UnexpectedEof or InvalidArchive)
           let result = list_zip_contents_from_reader(mock_reader, zip_path); // Use the helper

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from ZipError::InvalidArchive (expected for empty)
                   assert!(msg.contains("Invalid Zip archive"));
               },
               FileSystemError::IOError(msg) => { // Could be UnexpectedEof from underlying reader
                    assert!(msg.contains("XML IO hatası") || msg.contains("CoreIOError") || msg.contains("UnexpectedEof"));
               }
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }


           Ok(())
      }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
    // Test cases should include opening valid/invalid files, handling IO errors during reading,
    // and verifying the listed contents or error results with mock data.
    // This requires a no_std compatible zip crate and a mock Sahne64 filesystem.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_zip", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
