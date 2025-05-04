#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

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


// toml crate (no_std compatible with features)
// Assumes toml::Value and toml::from_str are available with the configured features.
// toml-rs typically requires features like `std` or `alloc` for certain functionalities.
// `no_std` usage usually involves parsing into a struct that derives Deserialize.
// Using `toml::Value` directly might require features that aren't purely no_std,
// or specific configurations. Let's assume `toml::Value` and `toml::from_str`
// are usable in a no_std environment with alloc.
use toml::Value;
use toml::de::Error as TomlDeError;


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

/// Helper function to map TomlDeError to FileSystemError.
fn map_toml_de_error_to_fs_error(e: TomlDeError) -> FileSystemError {
     FileSystemError::InvalidData(format!("TOML deserialization error: {:?}", e)) // Map to InvalidData
     // TODO: Possibly map specific TomlDeError kinds to more specific FileSystemError variants if available
}


/// Custom error type for TOML parsing issues.
#[derive(Debug)]
pub enum TomlError {
    UnexpectedEof(String), // During reading
    InvalidUtf8, // Error converting bytes to UTF-8 string
    TomlParseError(String), // Error from the toml deserializer
    SeekError(u64), // Failed to seek
    // Add other TOML specific parsing errors here
}

// Implement Display for TomlError
impl fmt::Display for TomlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TomlError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            TomlError::InvalidUtf8 => write!(f, "Geçersiz UTF-8 verisi"),
            TomlError::TomlParseError(msg) => write!(f, "TOML ayrıştırma hatası: {}", msg),
            TomlError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map TomlError to FileSystemError
fn map_toml_error_to_fs_error(e: TomlError) -> FileSystemError {
    match e {
        TomlError::UnexpectedEof(_) | TomlError::SeekError(_) | TomlError::InvalidUtf8 => FileSystemError::IOError(format!("TOML IO/Encoding hatası: {}", e)), // Map IO/Encoding related errors
        TomlError::TomlParseError(_) => FileSystemError::InvalidData(format!("TOML ayrıştırma hatası: {}", e)), // Map parsing errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilesvg.rs'den kopyalandı)
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

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReader {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReader is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReader drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant fs and SahneError definitions.
// Removed redundant print module and panic handler boilerplate.


/// Represents a parsed TOML file.
#[derive(Debug, PartialEq)] // Add PartialEq for tests
pub struct TomlFile {
    pub data: Value, // Requires the 'toml' crate and Deserialize trait
}

impl TomlFile {
    // from_file is replaced by open_toml_file and parse_toml
    // new is removed

    /// Parses TOML content from a reader.
    /// Reads the entire content from the reader into memory and then parses it.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed toml::Value or a FileSystemError.
    pub fn parse_toml<R: Read>(mut reader: R) -> Result<Value, FileSystemError> { // Return FileSystemError
        // Read the entire content into a Vec<u8>
        let mut buffer = Vec::new(); // Requires alloc
         reader.read_to_end(&mut buffer).map_err(|e| map_core_io_error_to_fs_error(e))?;


        // Convert bytes to String (UTF-8)
         let contents = String::from_utf8(buffer).map_err(|_| {
             map_toml_error_to_fs_error(TomlError::InvalidUtf8) // Map UTF-8 error
         })?;


        // Parse the TOML content from the string
        let data: Result<Value, TomlDeError> = toml::from_str(&contents);
        match data {
            Ok(parsed_data) => Ok(parsed_data),
            Err(e) => Err(map_toml_de_error_to_fs_error(e)), // Map TOML parsing error
        }
    }

    /// Gets a string value from the parsed TOML data by key.
    /// Returns None if the key is not found or the value is not a string.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(Value::as_str)
    }

    /// Gets an integer value from the parsed TOML data by key.
    /// Returns None if the key is not found or the value is not an integer.
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        self.data.get(key).and_then(Value::as_integer)
    }

    // Add other similar get methods for other TOML data types if needed.
     pub fn get_boolean(&self, key: &str) -> Option<bool> { ... }
     pub fn get_float(&self, key: &str) -> Option<f64> { ... }
     pub fn get_array(&self, key: &str) -> Option<&toml::value::Array> { ... }
     pub fn get_table(&self, key: &str) -> Option<&toml::value::Table> { ... }
}


/// Opens a TOML file from the given path (std) or resource ID (no_std)
/// and parses its content.
/// Requires the 'toml' crate to be available.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the parsed TomlFile data or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_toml_file<P: AsRef<Path>>(file_path: P) -> Result<TomlFile, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (optional for parse_toml, but good practice)
    // Seek to end to get size, then seek back to start
     let mut temp_file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
     let file_size = temp_file.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    // No need to seek temp_file back, it will be dropped.


    // Parse the TOML data from the reader
    let parsed_data = TomlFile::parse_toml(reader)?; // Call the parse_toml function

    // The reader/file is closed when it goes out of scope due to Drop.

    Ok(TomlFile { data: parsed_data }) // Return the TomlFile struct
}

#[cfg(not(feature = "std"))]
pub fn open_toml_file(file_path: &str) -> Result<TomlFile, FileSystemError> { // Return FileSystemError
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutını al (needed for SahneResourceReader and potential validation)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReader oluştur
    let reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek


    // Parse the TOML data from the reader
    let parsed_data = TomlFile::parse_toml(reader)?; // Call the parse_toml function


    // File handle is released when 'reader' goes out of scope (due to Drop on SahneResourceReader).

    Ok(TomlFile { data: parsed_data }) // Return the TomlFile struct
}


// Example main function (std)
#[cfg(feature = "example_toml")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("TOML parser example (std) starting...");
     eprintln!("TOML parser example (std) using toml crate.");

     // Example TOML file content
     let toml_content = r#"
         title = "Sahne64 Example"
         version = 1
         [database]
         server = "192.168.1.1"
         ports = [ 8000, 8001, 8002 ]
         enabled = true
     "#;


     let file_path = Path::new("example.toml");

      // Write example content to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(toml_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy TOML file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy TOML file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_toml_file(file_path) { // Call the function that opens and parses
         Ok(toml_file) => {
             println!("Parsed TOML Data:");

             // Access and print values
             if let Some(title) = toml_file.get_string("title") {
                 println!(" title: {}", title);
                  assert_eq!(title, "Sahne64 Example");
             } else {
                  eprintln!("Error: 'title' key not found or not a string.");
             }

              if let Some(version) = toml_file.get_integer("version") {
                  println!(" version: {}", version);
                   assert_eq!(version, 1);
              } else {
                   eprintln!("Error: 'version' key not found or not an integer.");
              }

              // Access nested data (requires more sophisticated get methods or direct Value access)
              if let Some(database_table) = toml_file.data.get("database").and_then(Value::as_table) {
                  if let Some(server) = database_table.get("server").and_then(Value::as_str) {
                       println!(" database.server: {}", server);
                       assert_eq!(server, "192.168.1.1");
                  }
                   if let Some(ports) = database_table.get("ports").and_then(Value::as_array) {
                       println!(" database.ports: {:?}", ports);
                        // Add assertions for ports array elements
                   }
                    if let Some(enabled) = database_table.get("enabled").and_then(Value::as_bool) {
                        println!(" database.enabled: {}", enabled);
                         assert_eq!(enabled, true);
                    }
              } else {
                   eprintln!("Error: 'database' key not found or not a table.");
              }


             // File is automatically closed when the underlying reader/handle goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening/parsing TOML file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy TOML file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("TOML parser example (std) finished.");

     Ok(())
}

// Example main function (no_std)
#[cfg(feature = "example_toml")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("TOML parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy TOML file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Create dummy TOML content bytes for the mock filesystem
     let toml_content = r#"
         app = "Sahne64 App"
         count = 42
     "#;
     let dummy_toml_data: Vec<u8> = toml_content.as_bytes().to_vec(); // Requires alloc


      // Assuming the mock filesystem is set up to provide this data for "sahne://files/config.toml"

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/config.toml" exists with the dummy data.
      let toml_res = open_toml_file("sahne://files/config.toml");
      match toml_res {
          Ok(toml_file) => {
              crate::println!("Parsed TOML Data:");
     //
     //         // Access and print values
              if let Some(app_name) = toml_file.get_string("app") {
                  crate::println!(" app: {}", app_name);
                   assert_eq!(app_name, "Sahne64 App");
              } else {
                  crate::eprintln!("Error: 'app' key not found or not a string.");
              }
     //
               if let Some(count) = toml_file.get_integer("count") {
                   crate::println!(" count: {}", count);
                    assert_eq!(count, 42);
               } else {
                    crate::eprintln!("Error: 'count' key not found or not an integer.");
               }
     //
     //         // File is automatically closed when the underlying reader/handle goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening/parsing TOML file: {:?}", e),
      }

     eprintln!("TOML parser example (no_std) needs Sahne64 mocks and toml crate with no_std+alloc features to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The dummy TOML data to be available at the specified path.
     // 3. The toml crate compiled with no_std and alloc features.

     Ok(()) // Dummy return
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
#[cfg(feature = "std")] // Only run tests with std feature enabled
mod tests {
     // Needs std::io::Cursor for testing Read+Seek on dummy data
     use std::io::Cursor;
     use std::io::{Read, Seek, SeekFrom};
     use std::fs::remove_file; // For cleanup
     use std::path::Path;
     use std::io::Write; // For creating dummy files


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec::Vec; // For Vec
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests
     use std::error::Error; // For Box<dyn Error>


     // Helper function to create dummy TOML bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor
       fn create_dummy_toml_bytes(content: &str) -> Result<Vec<u8>, Box<dyn Error>> {
           Ok(content.as_bytes().to_vec())
       }


     // Test parsing a valid TOML string using parse_toml with Cursor
      #[test]
      fn test_parse_toml_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
           let toml_content = r#"
               name = "test"
               value = 123
           "#;

           let dummy_toml_bytes = create_dummy_toml_bytes(toml_content).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


           // Use Cursor as a Read + Seek reader
           let cursor = Cursor::new(dummy_toml_bytes.clone());

           // Parse the TOML data from the reader
           let parsed_value = TomlFile::parse_toml(cursor)?;

           // Assert parsed TOML data (as Value)
           assert!(parsed_value.is_table());
           let table = parsed_value.as_table().unwrap();
           assert_eq!(table.get("name").and_then(Value::as_str), Some("test"));
           assert_eq!(table.get("value").and_then(Value::as_integer), Some(123));


           Ok(())
      }

     // Test handling of invalid TOML data
      #[test]
      fn test_parse_toml_invalid_syntax_cursor() {
           let invalid_toml_content = r#"
               name = "test"
               value = 123 # This is missing a closing quote for the next key
               another key = "abc"
           "#; // Invalid syntax

           let dummy_toml_bytes = create_dummy_toml_bytes(invalid_toml_content).unwrap();

           let cursor = Cursor::new(dummy_toml_bytes);
           // Attempt to parse from the reader, expect an error
           let result = TomlFile::parse_toml(cursor);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from TomlError::TomlParseError (via map_toml_de_error_to_fs_error)
                   assert!(msg.contains("TOML ayrıştırma hatası"));
                    // Check if the underlying TOML error message is included (might vary)
                    #[cfg(feature = "std")] // std toml-rs error message check
                    assert!(msg.contains("expected newline, comment, or end of file after a key"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       // Test handling of invalid UTF-8 data
        #[test]
        fn test_parse_toml_invalid_utf8_cursor() {
             // Bytes that are not valid UTF-8
             let dummy_bytes = vec![0x00, 0x80, 0xc3, 0x28]; // Starts with valid UTF-8, but has invalid sequence later

             let cursor = Cursor::new(dummy_bytes);
             // Attempt to parse from the reader, expect an error during UTF-8 conversion
             let result = TomlFile::parse_toml(cursor);
             assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from TomlError::InvalidUtf8
                     assert!(msg.contains("TOML IO/Encoding hatası"));
                     assert!(msg.contains("Geçersiz UTF-8 verisi"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }
        }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during reading,
     // and correctly parsing TOML data from mock data using the no_std toml crate.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_toml", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
