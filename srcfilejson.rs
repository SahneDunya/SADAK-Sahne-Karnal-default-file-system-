#![no_std]
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


// alloc crate for String, Vec, format!
use alloc::string::String;
use alloc::vec::Vec; // Used in read_all buffer or String's internal allocation
use alloc::format;

// core::result, core::option, core::str, core::fmt
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


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilegif.rs'den kopyalandı)
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


/// Represents a JSON file by storing its entire content as a String.
/// Provides a basic, limited method for extracting string values by key.
/// Note: This is NOT a full or robust JSON parser.
#[derive(Debug)] // Add Debug trait
pub struct JsonFile {
    pub content: String, // JSON içeriğini bir String olarak saklayacağız
}

impl JsonFile {
    /// Creates a new `JsonFile` instance by reading the entire content
    /// of the specified file (std) or resource (no_std) into a String.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the JSON file (std) or Sahne64 resource ID (no_std).
    ///
    /// # Returns
    ///
    /// A Result containing the `JsonFile` or a FileSystemError.
    #[cfg(feature = "std")]
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, FileSystemError> { // Return FileSystemError
        let file = File::open(path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
        let mut reader = BufReader::new(file); // BufReader implements StdRead

        let mut content = String::new(); // Requires alloc
        reader.read_to_string(&mut content).map_err(map_std_io_error_to_fs_error)?; // Read entire content to String

        Ok(JsonFile { content })
    }

    #[cfg(not(feature = "std"))]
    pub fn new(path: &str) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Acquire the resource
        let handle = resource::acquire(path, resource::MODE_READ)
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

        // Read the entire content into a String
        let mut content = String::new(); // Requires alloc
        // Use a buffer loop to read and append, handling UTF8 conversion
        const BUFFER_SIZE: usize = 4096; // Chunk size
        let mut buffer = vec![0u8; BUFFER_SIZE]; // Use alloc::vec::Vec

        loop {
             // Use read from core::io::Read
             let bytes_read = reader.read(&mut buffer).map_err(map_core_io_error_to_fs_error)?; // core::io::Error -> FileSystemError
             if bytes_read == 0 {
                  break; // EOF
             }
             // Attempt UTF8 conversion for the chunk and append
             let chunk_str = from_utf8(&buffer[..bytes_read])
                 .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?; // UTF8 error -> FileSystemError::InvalidData
             content.push_str(chunk_str); // Requires alloc
        }

        // Release the resource
        let _ = resource::release(handle).map_err(|e| {
             eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
             map_sahne_error_to_fs_error(e) // Return this error if crucial, or just log
         });


        Ok(JsonFile { content })
    }

    /// Attempts to find a key in the JSON content and return its associated value as a string slice.
    /// This method is very basic and has limitations. It does NOT parse the JSON structure correctly.
    /// It only performs string searching and manipulation.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// An Option containing a string slice of the value if found, otherwise None.
    pub fn get_value(&self, key: &str) -> Option<&str> {
        // This implementation is kept as is, acknowledging its limitations.
        // A proper JSON parser (like `serde-json-core` if available/ported)
        // would be needed for robust JSON parsing.

        let key_with_quotes = format!("\"{}\"", key); // Requires alloc and format!
        let search_string = format!("{}:", key_with_quotes); // Requires alloc and format!

        if let Some(start) = self.content.find(&search_string) {
            let value_start_index = start + search_string.len();

            // Find the end of the value. This logic is highly simplified and error-prone.
            // It does not handle nested structures, escaped characters correctly.
            // A real JSON parser is required for this.
             let end_index_candidate = self.content[value_start_index..].find(',').unwrap_or(self.content.len() - value_start_index); // Find comma or end
             let end_index = value_start_index + end_index_candidate;

             let value_slice = &self.content[value_start_index..end_index].trim();

             // Basic handling for quoted strings
             if value_slice.starts_with('"') && value_slice.ends_with('"') && value_slice.len() > 1 {
                  // Note: This doesn't handle escaped quotes inside the string.
                  return Some(&value_slice[1..value_slice.len() - 1]);
             }

             // Return value as is (for numbers, booleans, null, unquoted strings - though unquoted strings are not standard JSON values)
             Some(value_slice)

        } else {
            None // Key not found
        }
    }

    /// Add methods for getting other data types (int, bool, array, object) if a proper parser were implemented.
     pub fn get_int(&self, key: &str) -> Option<i64> { ... }
     pub fn get_bool(&self, key: &str) -> Option<bool> { ... }
     pub fn get_array(&self, key: &str) -> Option<JsonArray> { ... } // Need JsonArray struct
     pub fn get_object(&self, key: &str) -> Option<JsonObject> { ... } // Need JsonObject struct
}

// Example main function (no_std)
#[cfg(feature = "example_json")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("JSON file example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy JSON file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/config.json" exists
     with content like {"name": "SahneOS", "version": 1}.
      let json_file_res = JsonFile::new("sahne://files/config.json");
      match json_file_res {
          Ok(json_file) => {
              crate::println!("JSON content loaded (first 100 chars): {}", &json_file.content.chars().take(100).collect::<String>());
              if let Some(name) = json_file.get_value("name") {
                  crate::println!("Value for key 'name': {}", name);
              } else {
                  crate::println!("Key 'name' not found.");
              }
              if let Some(version) = json_file.get_value("version") {
                  crate::println!("Value for key 'version': {}", version);
              } else {
                  crate::println!("Key 'version' not found.");
              }
          },
          Err(e) => crate::eprintln!("Error opening or reading JSON file: {:?}", e),
      }

     eprintln!("JSON file example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_json")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("JSON file example (std) starting...");
     eprintln!("JSON file example (std) not fully implemented, using simplified parser.");

     // This example needs a dummy JSON file.
     use std::fs::remove_file;
     use std::io::Write;

     let json_path = Path::new("example.json");

     // Create a dummy JSON file
     let dummy_json_content = r#"{
         "os_name": "MySahneOS",
         "version": 1.0,
         "is_active": true,
         "settings": {"theme": "dark"},
         "features": ["fs", "networking"]
     }"#;

     match File::create(json_path) {
          Ok(mut file) => {
               if let Err(e) = file.write_all(dummy_json_content.as_bytes()) {
                    eprintln!("Error writing dummy JSON file: {}", e);
                    return Err(map_std_io_error_to_fs_error(e));
               }
          },
          Err(e) => {
               eprintln!("Error creating dummy JSON file: {}", e);
               return Err(map_std_io_error_to_fs_error(e));
          }
     }


     match JsonFile::new(json_path) { // Call the function that opens and reads
         Ok(json_file) => {
             println!("JSON content loaded (first 100 chars): {}", &json_file.content.chars().take(100).collect::<String>()); // Use std String methods
             if let Some(name) = json_file.get_value("os_name") {
                 println!("Value for key 'os_name': {}", name);
             } else {
                 println!("Key 'os_name' not found.");
             }
             if let Some(version) = json_file.get_value("version") {
                 println!("Value for key 'version': {}", version);
             } else {
                 println!("Key 'version' not found.");
             }
              if let Some(is_active) = json_file.get_value("is_active") {
                 println!("Value for key 'is_active': {}", is_active);
             } else {
                 println!("Key 'is_active' not found.");
             }
              if let Some(settings) = json_file.get_value("settings") {
                 println!("Value for key 'settings': {}", settings); // Will output "{"theme":"dark"}" - basic string extraction
             } else {
                 println!("Key 'settings' not found.");
             }
              if let Some(features) = json_file.get_value("features") {
                 println!("Value for key 'features': {}", features); // Will output "["fs", "networking"]" - basic string extraction
             } else {
                 println!("Key 'features' not found.");
             }
               if let Some(non_existent_key) = json_file.get_value("non_existent_key") {
                 println!("Value for key 'non_existent_key': {}", non_existent_key);
             } else {
                 println!("Key 'non_existent_key' not found.");
             }
         }
         Err(e) => {
              eprintln!("Error opening or reading JSON file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(json_path) {
          eprintln!("Error removing dummy JSON file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("JSON file example (std) finished.");

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
      use serde_json::json; // For creating complex JSON data easily in std tests


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests

     // Helper function to create a dummy JSON file string
      #[cfg(feature = "std")] // This helper is simple string, could be used in no_std tests with mocks
     fn create_dummy_json_string(content: &str) -> String {
          String::from(content) // Requires alloc
     }

     // Helper function to create a JsonFile instance from a string (for testing get_value)
     fn create_json_file_from_string(content: &str) -> JsonFile {
          JsonFile { content: create_dummy_json_string(content) }
     }


     // Test for get_value method
     #[test]
     fn test_get_value() {
          let json_string = r#"{
              "name": "TestItem",
              "id": 12345,
              "active": true,
              "price": 99.99,
              "tags": ["a", "b", "c"],
              "details": {"color": "red"},
              "description": "a \"quoted\" string, with comma, in it"
          }"#;
          let json_file = create_json_file_from_string(json_string);

          // Test extracting different types (as strings)
          assert_eq!(json_file.get_value("name"), Some("TestItem"));
          assert_eq!(json_file.get_value("id"), Some("12345"));
          assert_eq!(json_file.get_value("active"), Some("true"));
          assert_eq!(json_file.get_value("price"), Some("99.99"));
          assert_eq!(json_file.get_value("tags"), Some("[\"a\", \"b\", \"c\"]")); // Note: returns the raw string including brackets and quotes
          assert_eq!(json_file.get_value("details"), Some("{\"color\": \"red\"}")); // Note: returns the raw string including braces
           assert_eq!(json_file.get_value("description"), Some("a \"quoted\" string, with comma, in it")); // Note: simplified parsing might fail here in complex cases

          // Test non-existent key
          assert_eq!(json_file.get_value("non_existent"), None);

          // Test with different formatting (spaces, newlines)
           let json_string_formatted = r#"{
               "key1" : "value1"
               , "key2" : 123
           }"#;
           let json_file_formatted = create_json_file_from_string(json_string_formatted);
           assert_eq!(json_file_formatted.get_value("key1"), Some("value1"));
           assert_eq!(json_file_formatted.get_value("key2"), Some("123"));

          // Test with leading/trailing whitespace around value
           let json_string_whitespace = r#"{ "key" :   " value with spaces "   }"#; // Includes non-breaking space
           let json_file_whitespace = create_json_file_from_string(json_string_whitespace);
           // Note: trim() only removes standard whitespace. Non-breaking space might remain.
           // The current trim() uses str::trim() which should handle Unicode whitespace.
           assert_eq!(json_file_whitespace.get_value("key"), Some(" value with spaces ")); // Value inside quotes
     }


     // Test for JsonFile::new in std environment
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_json_file_new_std() -> Result<(), FileSystemError> { // Return FileSystemError
          let dir = tempdir().map_err(|e| FileSystemError::IOError(format!("Tempdir error: {}", e)))?;
          let file_path = dir.path().join("test_std.json");

          // Create a dummy JSON file using std FS
          let dummy_json_content = r#"{"name": "StdTest", "value": 42}"#;
          let mut file = File::create(&file_path).map_err(|e| map_std_io_error_to_fs_error(e))?;
          file.write_all(dummy_json_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e))?;

          // Call JsonFile::new with the file path
          let json_file = JsonFile::new(&file_path).map_err(|e| {
               // Clean up the file on error before returning
               let _ = remove_file(&file_path);
               e
          })?;

          // Assert the content was read correctly
          assert_eq!(json_file.content, dummy_json_content);

          // Clean up the dummy file
          let _ = remove_file(&file_path); // Ignore result, best effort cleanup

          Ok(())
      }

     // TODO: Add tests for JsonFile::new in no_std environment using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include reading valid JSON content and handling file not found or IO errors.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_json", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
