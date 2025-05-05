 #![no_std] // Keep no_std if serde_yaml can be made no_std, otherwise #[cfg] std
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

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


// serde and serde_yaml crates (serde_yaml likely requires std by default)
// Assuming serde_yaml can be made no_std compatible with features like "alloc"
use serde::Deserialize; // Deserialize trait
use serde_yaml::{Value, Error as SerdeYamlError}; // Value type, SerdeYamlError

// alloc crate for String, Vec, format!
use alloc::string::{String, ToString, FromUtf8Error}; // Import String, Vec, format!, ToString, FromUtf8Error
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io, core::str
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt}; // core::io
use core::str; // For core::str::FromUtf8Error (though String::from_utf8 handles it)


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


/// Helper function to map serde_yaml::Error to FileSystemError.
fn map_serde_yaml_error_to_fs_error(e: SerdeYamlError) -> FileSystemError {
     #[cfg(feature = "std")] // serde_yaml error might contain std::io::Error in std builds
     {
         if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<StdIOError>()) {
              return map_std_io_error_to_fs_error(io_err.clone()); // Clone is needed if source returns reference
         }
     }

    // Map serde_yaml error variants to FileSystemError
    // SerdeYamlError is opaque, relying on Display/Debug
    FileSystemError::InvalidData(format!("YAML parsing error: {}", e)) // Generic mapping based on Display
    // TODO: If serde_yaml error variants are inspectable in no_std, provide more specific mapping
}

/// Helper function to map alloc::string::FromUtf8Error to FileSystemError.
fn map_from_utf8_error_to_fs_error(e: FromUtf8Error) -> FileSystemError {
    FileSystemError::InvalidData(format!("UTF-8 decoding error: {}", e)) // Map UTF-8 errors to InvalidData
}


// Sahne64 Handle'ı için core::io::Read implementasyonu (copied from srcfilexml.rs)
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


/// Represents a parsed YAML file.
/// Stores the parsed data as a serde_yaml::Value.
#[derive(Debug, Deserialize)] // Keep Debug, Deserialize
pub struct YamlFile {
    pub data: Value, // Parsed YAML data (Requires alloc, serde_yaml::Value)
}

impl YamlFile {
    /// Loads and parses a YAML file from the given path.
    /// Reads the entire file content, converts to UTF-8, and deserializes using serde_yaml.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to the YAML file.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed YamlFile or a FileSystemError.
    pub fn load_from_file(path: &str) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Open the file using the standardized function
        let mut reader = open_yaml_reader(path)?; // open_yaml_reader returns a reader implementing Read+Seek+Drop


        // Read the entire file content into a Vec<u8>
        let mut contents_bytes = Vec::new(); // Requires alloc
         reader.read_to_end(&mut contents_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // Read entire content (Requires alloc)

        // The underlying reader/handle is automatically dropped here.


        // Convert the bytes to a String, handling UTF-8 errors
         let contents_string = String::from_utf8(contents_bytes).map_err(|e| map_from_utf8_error_to_fs_error(e))?; // Map FromUtf8Error


        // Parse the YAML content using serde_yaml
        let data = serde_yaml::from_str(&contents_string).map_err(|e| map_serde_yaml_error_to_fs_error(e))?; // Map SerdeYamlError


        Ok(YamlFile { data }) // Return the parsed YamlFile
    }

    /// Gets a value from the parsed YAML data, deserializing it to a specific type.
    ///
    /// # Arguments
    ///
    /// * `key`: The key to access within the YAML structure.
    /// * `T`: The target type to deserialize the value into.
    ///
    /// # Returns
    ///
    /// An Option containing the deserialized value, or None if the key is not found
    /// or deserialization fails.
    pub fn get_value<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> { // Return Option<T>
        // Use serde_yaml::Value's get method and then attempt to deserialize
        self.data.get(key).and_then(|value| {
            // Map the Result<T, SerdeYamlError> from from_value to Option<T>
            serde_yaml::from_value::<T>(value).ok()
        })
    }
}


/// Opens a YAML file from the given path (std) or resource ID (no_std)
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
pub fn open_yaml_reader<P: AsRef<Path>>(file_path: P) -> Result<File, FileSystemError> { // Return std::fs::File (implements Read+Seek+Drop)
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    Ok(file)
}

#[cfg(not(feature = "std"))]
pub fn open_yaml_reader(file_path: &str) -> Result<SahneResourceReadSeek, FileSystemError> { // Return SahneResourceReadSeek (implements Read+Seek+Drop)
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


// Example main function (std)
#[cfg(feature = "example_yaml")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> io::Result<()> { // Return std::io::Result for std example
     eprintln!("YAML parser example (std) starting...");
     eprintln!("YAML parser example (std) using serde_yaml.");

     // Example YAML content
     let yaml_content = r#"
         database:
             host: localhost
             port: 5432
             enabled: true
         users:
             - name: Alice
               id: 1
             - name: Bob
               id: 2
     "#;

     let file_path = Path::new("example.yaml");

      // Write example content to a temporary file for std example
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(yaml_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy YAML file: {}", e);
                       // Map FileSystemError back to std::io::Error for std main
                      match e {
                          FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                          _ => return Err(io::Error::new(io::ErrorKind::Other, format!("Mapped FS error: {:?}", e))), // Generic map for others
                      }
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy YAML file: {}", e);
                  return Err(e); // Return std::io::Error
             }
        }
        println!("Dummy YAML file created: {}", file_path.display());


     // Load and parse the YAML file
     match YamlFile::load_from_file(file_path.to_string_lossy().into_owned().as_str()) { // Pass as &str after converting PathBuf to String
         Ok(yaml_file) => {
             println!("YAML file loaded and parsed successfully.");
             println!("Parsed data as serde_yaml::Value: {:?}", yaml_file.data);

              // Example of accessing values using get_value
              if let Some(db_host) = yaml_file.get_value::<String>("database.host") { // Access nested key (if Value::get supports this syntax)
                   println!("Database Host: {}", db_host);
              } else {
                   println!("Database host not found or could not be deserialized.");
              }

               if let Some(first_user_name) = yaml_file.data.get("users").and_then(|users| users.as_sequence()).and_then(|seq| seq.first()).and_then(|user| user.get("name")).and_then(|name| name.as_str()) {
                  // Accessing nested values manually via Value methods
                   println!("First User Name (manual access): {}", first_user_name);
               } else {
                    println!("First user name not found (manual access).");
               }

                // Using get_value with a struct
                #[derive(Debug, Deserialize, PartialEq)] // Add PartialEq for assertion
                struct DatabaseConfig {
                     host: String,
                     port: u16,
                     enabled: bool,
                }
                 if let Some(db_config) = yaml_file.get_value::<DatabaseConfig>("database") {
                      println!("Deserialized Database Config: {:?}", db_config);
                      assert_eq!(db_config, DatabaseConfig { host: "localhost".to_string(), port: 5432, enabled: true });
                 } else {
                      println!("Database config not found or could not be deserialized.");
                 }


         }
         Err(e) => {
             eprintln!("Error loading or parsing YAML file: {}", e); // std error display
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
               eprintln!("Error removing dummy YAML file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("YAML parser example (std) finished.");

     Ok(()) // Return Ok from std main
}

// Example main function (no_std)
#[cfg(feature = "example_yaml")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for no_std example
     eprintln!("YAML parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // and simulate fs syscalls.
     // This is complex and requires a testing framework or simulation.
     // It also requires serde_yaml compiled with no_std + alloc features.

     eprintln!("YAML parser example (no_std) needs Sahne64 mocks and serde_yaml with no_std features to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real) and dummy data.
     // 2. The serde_yaml crate compiled with no_std and alloc features.

      // Hypothetical usage with Sahne64 mocks:
      // // Assume a mock filesystem has a file at "sahne://files/config.yaml" with dummy YAML data.
      // // We would need to simulate writing this data to the mock file first if it doesn't exist.
      //
      // // Load and parse the YAML file
       match YamlFile::load_from_file("sahne://files/config.yaml") {
           Ok(yaml_file) => {
               crate::println!("YAML file loaded and parsed successfully.");
                crate::println!("Parsed data as serde_yaml::Value: {:?}", yaml_file.data); // Might need no_std Debug for Value
      //
               // Example of accessing values using get_value
                if let Some(db_host) = yaml_file.get_value::<String>("database.host") { // Access nested key (if Value::get supports this syntax)
                     crate::println!("Database Host: {}", db_host);
                } else {
                     crate::println!("Database host not found or could not be deserialized.");
                }
      //
      //          // Using get_value with a struct (if DatabaseConfig struct is defined in no_std)
                 #[derive(Debug, Deserialize, PartialEq)] struct DatabaseConfig { ... }
                 if let Some(db_config) = yaml_file.get_value::<DatabaseConfig>("database") {
      //          //      crate::println!("Deserialized Database Config: {:?}", db_config);
                 } else {
                      crate::println!("Database config not found or could not be deserialized.");
                 }
      
           }
           Err(e) => {
               crate::eprintln!("Error loading or parsing YAML file: {:?}", e); // no_std print
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

    // Helper to create dummy YAML bytes in memory
    fn create_dummy_yaml_bytes(content: &str) -> Vec<u8> {
        content.as_bytes().to_vec() // Requires alloc and String
    }


    // Mock open_yaml_reader for std tests using Cursor
    struct MockOpenYamlReader {
        cursor: Option<StdCursor<Vec<u8>>>, // Use Option to allow taking the cursor
    }
    impl MockOpenYamlReader {
        fn new(data: Vec<u8>) -> Self { MockOpenYamlReader { cursor: Some(StdCursor::new(data)) } }
    }
    #[cfg(feature = "std")] // Implement core::io traits for Mock
    impl Read for MockOpenYamlReader { fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> { self.cursor.as_mut().unwrap().read(buf) } }
    #[cfg(feature = "std")]
    impl Seek for MockOpenYamlReader { fn seek(&mut self, pos: SeekFrom) -> Result<u64, core::io::Error> { self.cursor.as_mut().unwrap().seek(pos) } }
    #[cfg(feature = "std")]
    impl Drop for MockOpenYamlReader { fn drop(&mut self) { println!("MockOpenYamlReader dropped"); } } // For testing Drop


    #[test]
    fn test_load_from_file_valid_yaml_cursor() -> Result<(), FileSystemError> { // Return FileSystemError for std test
        let yaml_content = r#"
             name: Test
             version: 1.0
             features:
                 - feature1
                 - feature2
             config:
                 timeout: 30
                 retries: 3
         "#;

         // In-memory reader using Cursor
         let raw_bytes = create_dummy_yaml_bytes(yaml_content);
         let mock_reader = MockOpenYamlReader::new(raw_bytes);

         // Call the parsing logic directly with the mock reader
         // Refactor YamlFile::load_from_file to use a private helper that takes Read.
         // Or, since open_yaml_reader is already mocked, just use load_from_file with a dummy path.

         // Let's test by providing a mock open_yaml_reader to the load_from_file logic.
         // This requires modifying load_from_file to accept a reader or a reader factory.
         // A simpler test approach is to test the core parsing function that takes a reader.
         // Let's create a helper parse_from_reader.

         impl YamlFile { // Add parse_from_reader helper to YamlFile impl
             fn parse_from_reader<R: Read>(mut reader: R) -> Result<Self, FileSystemError> { // Takes Read, not Read+Seek needed for seek
                 let mut contents_bytes = Vec::new();
                 reader.read_to_end(&mut contents_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?;
                 let contents_string = String::from_utf8(contents_bytes).map_err(|e| map_from_utf8_error_to_fs_error(e))?;
                 let data = serde_yaml::from_str(&contents_string).map_err(|e| map_serde_yaml_error_to_fs_error(e))?;
                 Ok(YamlFile { data })
             }
         }

         // Use the parse_from_reader helper directly with the cursor
         let mut cursor_reader = StdCursor::new(create_dummy_yaml_bytes(yaml_content));
         let parsed_yaml_file = YamlFile::parse_from_reader(&mut cursor_reader)?; // Pass mutable reference to cursor


        // Assert the structure of the parsed YAML data (using serde_yaml::Value methods)
        assert!(parsed_yaml_file.data.is_mapping()); // It should be a top-level map
        let root_map = parsed_yaml_file.data.as_mapping().unwrap();

        assert!(root_map.get(&Value::from("name")).unwrap().is_string());
        assert_eq!(root_map.get(&Value::from("name")).unwrap().as_str(), Some("Test"));

        assert!(root_map.get(&Value::from("version")).unwrap().is_f64()); // YAML parses 1.0 as float
        assert_eq!(root_map.get(&Value::from("version")).unwrap().as_f64(), Some(1.0));

        assert!(root_map.get(&Value::from("features")).unwrap().is_sequence());
        let features = root_map.get(&Value::from("features")).unwrap().as_sequence().unwrap();
        assert_eq!(features.len(), 2);
        assert_eq!(features[0].as_str(), Some("feature1"));
        assert_eq!(features[1].as_str(), Some("feature2"));


         // Test get_value
          if let Some(name_str) = parsed_yaml_file.get_value::<String>("name") {
               assert_eq!(name_str, "Test");
          } else { panic!("Failed to get 'name'"); }

          if let Some(timeout_int) = parsed_yaml_file.get_value::<u32>("config.timeout") { // Value::get might support dot notation
             // Note: serde_yaml::Value::get might not directly support "config.timeout" notation.
             // We might need to traverse the Value manually or use a helper.
             // Let's test with manual traversal for robustness.
               let config_map = root_map.get(&Value::from("config")).unwrap().as_mapping().unwrap();
               let timeout_value = config_map.get(&Value::from("timeout")).unwrap();
               let deserialized_timeout: u32 = serde_yaml::from_value(timeout_value.clone()).unwrap(); // Deserialize from Value slice
               assert_eq!(deserialized_timeout, 30);
                // Re-test get_value with manual traversal logic
                if let Some(config_val) = parsed_yaml_file.data.get("config") {
                     if let Some(timeout_val_nested) = config_val.get("timeout") {
                         if let Some(timeout_int_nested) = serde_yaml::from_value::<u32>(timeout_val_nested.clone()).ok() {
                              assert_eq!(timeout_int_nested, 30);
                         } else { panic!("Failed to deserialize nested timeout"); }
                     } else { panic!("Failed to get nested timeout value"); }
                } else { panic!("Failed to get config map"); }
          }


        Ok(()) // Return Ok from test function
    }

     #[test]
      fn test_load_from_file_invalid_yaml_cursor() {
           let invalid_yaml_content = r#"
             key: value
             list:
               - item 1
             invalid: [
           "#; // Incomplete list


           let raw_bytes = create_dummy_yaml_bytes(invalid_yaml_content);
           let mut cursor = StdCursor::new(raw_bytes);

            // Use the parse_from_reader helper directly with the cursor
           let result = YamlFile::parse_from_reader(&mut cursor);


           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from serde_yaml::Error
                   assert!(msg.contains("YAML parsing error"));
                   // Specific error message content might vary based on serde_yaml version and exact malformation
                    #[cfg(feature = "std")] // Check std error message details if possible
                    assert!(msg.contains("while parsing a block collection"));
               },
                FileSystemError::IOError(msg) => { // Could also be an IO error if reading fails
                    assert!(msg.contains("XML IO hatası") || msg.contains("CoreIOError")); // Error message might be from underlying reader
                }
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       #[test]
        fn test_load_from_file_invalid_utf8_cursor() {
             let raw_bytes_invalid_utf8 = vec![0x41, 0x42, 0xFF, 0x43, b':', b' ', b'v']; // A, B, invalid byte, C : v

             let mut cursor = StdCursor::new(raw_bytes_invalid_utf8);

              // Use the parse_from_reader helper directly with the cursor
             let result = YamlFile::parse_from_reader(&mut cursor);


             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from FromUtf8Error
                     assert!(msg.contains("UTF-8 decoding error"));
                     assert!(msg.contains("invalid utf-8 sequence"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
    // Test cases should include opening valid/invalid files, handling IO errors during reading,
    // and verifying the parsed Value structure or error results with mock data.
    // This requires a no_std compatible serde_yaml and a mock Sahne64 filesystem.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_yaml", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
