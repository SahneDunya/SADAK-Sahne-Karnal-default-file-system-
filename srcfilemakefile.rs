#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

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
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::string::ToString as StdToString; // for to_string()
#[cfg(feature = "std")]
use std::vec::Vec as StdVec; // Use std::vec::Vec in std tests


// alloc crate for String, BTreeMap
use alloc::string::String;
use alloc::vec::Vec; // For read buffer
use alloc::collections::BTreeMap; // Use BTreeMap from alloc
use alloc::format;


// core::result, core::option, core::str, core::fmt, core::cmp
use core::result::Result;
use core::option::Option;
use core::str::from_utf8; // For UTF8 conversion
use core::fmt;
use core::cmp; // For min


// Need no_std println!/eprintln! macros (if used in example)
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


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilejson.rs'den kopyalandı)
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


/// Represents a simple key-value file (like .env or .ini) by storing data in a BTreeMap.
/// Parses lines of the form `key = value`. Ignores lines without an '='.
#[derive(Debug)] // Add Debug trait
pub struct MakelifeFile {
    pub data: BTreeMap<String, String>, // Use BTreeMap from alloc
}

impl MakelifeFile {
    /// Creates a new `MakelifeFile` instance by reading and parsing
    /// the specified file (std) or resource (no_std).
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path to the file (std) or Sahne64 resource ID (no_std).
    ///
    /// # Returns
    ///
    /// A Result containing the `MakelifeFile` or a FileSystemError.
    #[cfg(feature = "std")]
    pub fn new<P: AsRef<Path>>(file_path: P) -> Result<Self, FileSystemError> { // Return FileSystemError
        let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
        let mut reader = BufReader::new(file); // BufReader implements StdRead

        let mut data = BTreeMap::new(); // Use BTreeMap
        let mut current_line = String::new(); // Requires alloc

        // Read the file in chunks and process lines
        const BUFFER_SIZE: usize = 1024; // Chunk size
        let mut buffer = vec![0u8; BUFFER_SIZE]; // Use alloc::vec::Vec

        loop {
             // Use read from StdRead
             let bytes_read = reader.read(&mut buffer).map_err(map_std_io_error_to_fs_error)?; // std::io::Error -> FileSystemError
             if bytes_read == 0 {
                  // EOF reached. Process any remaining data in current_line.
                  if !current_line.is_empty() {
                       if let Some((key, value)) = current_line.trim().split_once('=') {
                            data.insert(key.to_string(), value.trim().to_string()); // Requires alloc::string::ToString
                       }
                  }
                  break;
             }

             // Attempt UTF8 conversion for the chunk and append
             let chunk_str = from_utf8(&buffer[..bytes_read])
                 .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?; // UTF8 error -> FileSystemError::InvalidData
             current_line.push_str(chunk_str); // Requires alloc

             // Process completed lines in current_line
             while let Some(newline_pos) = current_line.find('\n') {
                  let line = current_line.drain(..newline_pos + 1).collect::<String>(); // Requires alloc::string::String and collect
                  if let Some((key, value)) = line.trim().split_once('=') {
                       data.insert(key.to_string(), value.trim().to_string()); // Requires alloc::string::ToString
                  }
             }
        }

        Ok(MakelifeFile { data })
    }

    #[cfg(not(feature = "std"))]
    pub fn new(file_path: &str) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Acquire the resource
        let handle = resource::acquire(file_path, resource::MODE_READ)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        // Get file size to create SahneResourceReader
         let file_stat = fs::fstat(handle)
             .map_err(|e| {
                  let _ = resource::release(handle); // Release on error
                  map_sahne_error_to_fs_error(e)
              })?;
         let file_size = file_stat.size as u64;

        // Create a SahneResourceReader
        let mut reader = SahneResourceReader::new(handle, file_size);

        let mut data = BTreeMap::new(); // Use BTreeMap from alloc
        let mut current_line = String::new(); // Requires alloc

        // Read the file in chunks and process lines
        const BUFFER_SIZE: usize = 4096; // Chunk size
        let mut buffer = vec![0u8; BUFFER_SIZE]; // Use alloc::vec::Vec

        loop {
             // Use read from core::io::Read
             let bytes_read = reader.read(&mut buffer).map_err(map_core_io_error_to_fs_error)?; // core::io::Error -> FileSystemError
             if bytes_read == 0 {
                  // EOF reached. Process any remaining data in current_line.
                  if !current_line.is_empty() {
                       if let Some((key, value)) = current_line.trim().split_once('=') {
                            data.insert(key.to_string(), value.trim().to_string()); // Requires alloc::string::ToString
                       }
                  }
                  break;
             }

             // Attempt UTF8 conversion for the chunk and append
             let chunk_str = from_utf8(&buffer[..bytes_read])
                 .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?; // UTF8 error -> FileSystemError::InvalidData
             current_line.push_str(chunk_str); // Requires alloc

             // Process completed lines in current_line
             while let Some(newline_pos) = current_line.find('\n') {
                  let line = current_line.drain(..newline_pos + 1).collect::<String>(); // Requires alloc::string::String and collect
                  if let Some((key, value)) = line.trim().split_once('=') {
                       data.insert(key.to_string(), value.trim().to_string()); // Requires alloc::string::ToString
                  }
             }
        }


        // Release the resource
        let _ = resource::release(handle).map_err(|e| {
             eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
             map_sahne_error_to_fs_error(e) // Return this error if crucial, or just log
         });


        Ok(MakelifeFile { data })
    }


    /// Gets the value associated with the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up.
    ///
    /// # Returns
    ///
    /// An Option containing a reference to the value String if the key is found, otherwise None.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
}


// Example main function (no_std)
#[cfg(feature = "example_makelife")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("Makelife file example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy key-value file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/config.makelife" exists
     // // with content like "OS_NAME=SahneOS\nVERSION=1".
     // let makelife_file_res = MakelifeFile::new("sahne://files/config.makelife");
     // match makelife_file_res {
     //     Ok(makelife_file) => {
     //         if let Some(os_name) = makelife_file.get("OS_NAME") {
     //             crate::println!("OS Name: {}", os_name);
     //         } else {
     //             crate::println!("OS_NAME not found.");
     //         }
     //         if let Some(version) = makelife_file.get("VERSION") {
     //             crate::println!("Version: {}", version);
     //         } else {
     //             crate::println!("VERSION not found.");
     //         }
     //     },
     //     Err(e) => crate::eprintln!("Error opening or reading Makelife file: {:?}", e),
     // }

     eprintln!("Makelife file example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_makelife")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("Makelife file example (std) starting...");
     eprintln!("Makelife file example (std) using simplified parser.");

     // This example needs a dummy key-value file.
     use std::fs::remove_file;
     use std::io::Write;

     let file_path = Path::new("example.makelife");

     // Create a dummy key-value file
     let dummy_content = r#"
# This is a comment, should be ignored
Key1 = Value1
Another_Key = Another Value
NumberKey = 12345
BooleanKey = true
KeyWithSpaces = Value With Spaces = Even More Spaces # Everything after first = is value
EmptyValue =
"#;

     match File::create(file_path) {
          Ok(mut file) => {
               if let Err(e) = file.write_all(dummy_content.as_bytes()) {
                    eprintln!("Error writing dummy file: {}", e);
                    return Err(map_std_io_error_to_fs_error(e));
               }
          },
          Err(e) => {
               eprintln!("Error creating dummy file: {}", e);
               return Err(map_std_io_error_to_fs_error(e));
          }
     }

     match MakelifeFile::new(file_path) { // Call the function that opens and reads
         Ok(makelife_file) => {
             println!("Makelife content parsed:");
             if let Some(value) = makelife_file.get("Key1") {
                 println!("Key1: {}", value); // Should be "Value1"
             }
              if let Some(value) = makelife_file.get("Another_Key") {
                 println!("Another_Key: {}", value); // Should be "Another Value"
             }
              if let Some(value) = makelife_file.get("NumberKey") {
                 println!("NumberKey: {}", value); // Should be "12345"
             }
              if let Some(value) = makelife_file.get("BooleanKey") {
                 println!("BooleanKey: {}", value); // Should be "true"
             }
              if let Some(value) = makelife_file.get("KeyWithSpaces") {
                 println!("KeyWithSpaces: {}", value); // Should be "Value With Spaces = Even More Spaces"
             }
               if let Some(value) = makelife_file.get("EmptyValue") {
                 println!("EmptyValue: \"{}\"", value); // Should be ""
             } else {
                  println!("EmptyValue not found (incorrect parsing?).");
             }
              if let Some(_) = makelife_file.get("NonExistentKey") {
                 println!("NonExistentKey found unexpectedly.");
             } else {
                 println!("NonExistentKey: <not found>");
             }

              // Print all parsed data (optional)
              // println!("All parsed data: {:?}", makelife_file.data);

         }
         Err(e) => {
              eprintln!("Error opening or reading file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(file_path) {
          eprintln!("Error removing dummy file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("Makelife file example (std) finished.");

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
      #[cfg(feature = "std")]
      use tempfile::tempdir; // For creating temporary directories in std tests
      #[cfg(feature = "std")]
      use serde_json::json; // Using serde_json just for convenient JSON creation in std tests if needed, though not for Makelife format itself


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::collections::BTreeMap; // For BTreeMap assertion


     // Helper function to create a dummy key-value file string
     // #[cfg(feature = "std")] // This helper is simple string, could be used in no_std tests with mocks
     fn create_dummy_makelife_string(content: &str) -> String {
          String::from(content) // Requires alloc
     }

     // Helper function to create a MakelifeFile instance from a string (for testing parsing logic)
     // This simulates reading from a file without actual file I/O.
      fn parse_makelife_string(content: &str) -> Result<MakelifeFile, FileSystemError> {
          // Simulate reading from a buffer in memory.
          // We need a Read + Seek implementation over the string bytes.
          // In std, use std::io::Cursor. In no_std tests, use a mock reader or a custom Cursor.
          // Since this test helper is only used in #[cfg(feature = "std")] tests below,
          // we can use std::io::Cursor.

          #[cfg(feature = "std")]
          {
               let content_bytes = content.as_bytes().to_vec(); // Convert string slice to Vec<u8>
               let file_size = content_bytes.len() as u64;
               let mut cursor = Cursor::new(content_bytes); // Create an in-memory reader/seeker

               let mut data = BTreeMap::new();
               let mut current_line = String::new();

               const BUFFER_SIZE: usize = 1024;
               let mut buffer = vec![0u8; BUFFER_SIZE];

               loop {
                   let bytes_read = cursor.read(&mut buffer).map_err(|e| FileSystemError::IOError(format!("Cursor read error: {}", e)))?; // Map std::io::Error
                    if bytes_read == 0 {
                        if !current_line.is_empty() {
                            if let Some((key, value)) = current_line.trim().split_once('=') {
                                data.insert(key.to_string(), value.trim().to_string());
                            }
                        }
                       break;
                   }

                   let chunk_str = from_utf8(&buffer[..bytes_read])
                       .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?;
                   current_line.push_str(chunk_str);

                   while let Some(newline_pos) = current_line.find('\n') {
                       let line = current_line.drain(..newline_pos + 1).collect::<String>();
                       if let Some((key, value)) = line.trim().split_once('=') {
                           data.insert(key.to_string(), value.trim().to_string());
                       }
                   }
               }
              Ok(MakelifeFile { data })
          }

          #[cfg(not(feature = "std"))]
          {
              // In no_std tests, this helper would need a mock SahneResourceReader over the string data.
              // This is more complex and deferred to the dedicated no_std test section.
              unimplemented!("parse_makelife_string not implemented for no_std tests directly");
          }

      }


     // Test the parsing logic with various inputs (using the helper)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_makelife_parsing() -> Result<(), FileSystemError> { // Return FileSystemError
          let simple_content = "Key1=Value1\nKey2 = Value2\n";
          let makelife = parse_makelife_string(simple_content)?;
          assert_eq!(makelife.get("Key1"), Some(&String::from("Value1")));
          assert_eq!(makelife.get("Key2"), Some(&String::from("Value2")));
          assert_eq!(makelife.get("NonExistent"), None);

          let content_with_spaces_comments_empty = r#"
# This is a comment
   KeyWithLeadingSpace = ValueWithTrailingSpace   
Empty = 
Another = Value with = equals sign
LastKey = LastValue
"#;
          let makelife_complex = parse_makelife_string(content_with_spaces_comments_empty)?;

          // Lines starting with # or empty lines are ignored by split_once after trim()
          assert_eq!(makelife_complex.get("KeyWithLeadingSpace"), Some(&String::from("ValueWithTrailingSpace")));
          assert_eq!(makelife_complex.get("Empty"), Some(&String::from("")));
          assert_eq!(makelife_complex.get("Another"), Some(&String::from("Value with = equals sign")));
          assert_eq!(makelife_complex.get("LastKey"), Some(&String::from("LastValue")));
          assert_eq!(makelife_complex.get("# This is a comment"), None); // Comment line ignored
          assert_eq!(makelife_complex.get(""), None); // Empty line ignored

          Ok(())
     }


     // Test for MakelifeFile::new in std environment (uses actual file I/O)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_makelife_file_new_std() -> Result<(), FileSystemError> { // Return FileSystemError
          let dir = tempdir().map_err(|e| FileSystemError::IOError(format!("Tempdir error: {}", e)))?;
          let file_path = dir.path().join("test_file_std.makelife");

          // Create a dummy file using std FS
           let dummy_content = r#"
TestKey1=TestValue1
TestKey2 = TestValue2
"#;
          let mut file = File::create(&file_path).map_err(|e| map_std_io_error_to_fs_error(e))?;
          file.write_all(dummy_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e))?;

          // Call MakelifeFile::new with the file path
          let makelife_file = MakelifeFile::new(&file_path).map_err(|e| {
               // Clean up the file on error before returning
               let _ = remove_file(&file_path);
               e
          })?;

          // Assert the data was parsed correctly
          assert_eq!(makelife_file.get("TestKey1"), Some(&String::from("TestValue1")));
          assert_eq!(makelife_file.get("TestKey2"), Some(&String::from("TestValue2")));
          assert_eq!(makelife_file.get("NonExistent"), None);

          // Clean up the dummy file
          let _ = remove_file(&file_path); // Ignore result, best effort cleanup

          Ok(())
      }

     // TODO: Add tests for MakelifeFile::new in no_std environment using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include reading valid content, handling file not found, IO errors, UTF8 errors.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_makelife", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
