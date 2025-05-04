// srcfilemd.rs
// Markdown (.md) file reader for Sahne64

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


// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::str, core::fmt, core::cmp
use core::result::Result;
use core::option::Option;
use core::str::from_utf8; // For UTF8 conversion
use core::fmt;
use core::cmp; // For min


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


/// Represents a Markdown file by storing its content as a vector of Strings (lines).
#[derive(Debug)] // Add Debug trait
pub struct MdFile {
    pub path: String, // File path or resource ID
    pub content: Vec<String>, // File content as a vector of lines
}

impl MdFile {
    /// Creates a new `MdFile` instance by reading the content of the
    /// specified file (std) or resource (no_std) line by line.
    /// Handles UTF8 decoding and line splitting.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the Markdown file (std) or Sahne64 resource ID (no_std).
    ///
    /// # Returns
    ///
    /// A Result containing the `MdFile` or a FileSystemError.
    #[cfg(feature = "std")]
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, FileSystemError> { // Return FileSystemError
        let file = File::open(path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
        let reader = BufReader::new(file); // BufReader implements StdRead

        let mut content = Vec::new(); // Use alloc::vec::Vec
        let mut current_line = String::new(); // Requires alloc

        // Read the file in chunks and process lines
        const BUFFER_SIZE: usize = 1024; // Chunk size
        let mut buffer = vec![0u8; BUFFER_SIZE]; // Use alloc::vec::Vec

        // Use core::io::Read trait on BufReader
        let mut reader_trait: &mut dyn core::io::Read = &mut BufReader::new(reader); // Use BufReader over the File and treat as Read trait

        loop {
             let bytes_read = reader_trait.read(&mut buffer).map_err(map_core_io_error_to_fs_error)?; // core::io::Error -> FileSystemError
             if bytes_read == 0 {
                  // EOF reached. Process any remaining data in current_line.
                  if !current_line.is_empty() {
                       content.push(current_line); // Add the last line
                  }
                  break;
             }

             // Attempt UTF8 conversion for the chunk
             let chunk_str = from_utf8(&buffer[..bytes_read])
                 .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?; // UTF8 error -> FileSystemError::InvalidData

             // Process the chunk, splitting by newline and handling partial lines
             let mut chars = chunk_str.chars();
             while let Some(ch) = chars.next() {
                 if ch == '\n' {
                      content.push(current_line); // Push completed line
                      current_line = String::new(); // Start a new line
                 } else {
                      current_line.push(ch); // Append character to current line
                 }
             }
             // Any remaining characters in the chunk are part of an incomplete line,
             // they are left in `current_line`.
        }


        Ok(MdFile {
            path: path.as_ref().to_string_lossy().into_owned(), // Convert Path to String
            content,
        })
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
        let mut reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read

        let mut content = Vec::new(); // Use alloc::vec::Vec
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
                       content.push(current_line); // Add the last line
                  }
                  break;
             }

             // Attempt UTF8 conversion for the chunk
             let chunk_str = from_utf8(&buffer[..bytes_read])
                 .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?; // UTF8 error -> FileSystemError::InvalidData

             // Process the chunk, splitting by newline and handling partial lines
             let mut chars = chunk_str.chars();
             while let Some(ch) = chars.next() {
                 if ch == '\n' {
                      content.push(current_line); // Push completed line
                      current_line = String::new(); // Start a new line
                 } else {
                      current_line.push(ch); // Append character to current line
                 }
             }
             // Any remaining characters in the chunk are part of an incomplete line,
             // they are left in `current_line`.
        }


        // Release the resource
        let _ = resource::release(handle).map_err(|e| {
             eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
             map_sahne_error_to_fs_error(e) // Return this error if crucial, or just log
         });


        Ok(MdFile {
            path: path.into(), // Convert &str to String
            content,
        })
    }

    /// Prints the content of the Markdown file line by line.
    #[cfg(feature = "std")] // Use std print
    pub fn print_content(&self) {
         for line in &self.content {
             println!("{}", line);
         }
    }

     /// Prints the content of the Markdown file line by line (no_std version).
     #[cfg(not(feature = "std"))] // Use no_std print
     pub fn print_content(&self) {
          for line in &self.content {
              crate::println!("{}", line);
          }
     }


    /// Counts the total number of words in the Markdown file content.
    /// Words are separated by whitespace.
    pub fn word_count(&self) -> usize {
        self.content
            .iter()
            .map(|line| line.split_whitespace().count())
            .sum()
    }
}


// Example main function (no_std)
#[cfg(feature = "example_md")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("Markdown file example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy Markdown file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/README.md" exists
     // // with content like "# README\n\nThis is a test.".
     // let md_file_res = MdFile::new("sahne://files/README.md");
     // match md_file_res {
     //     Ok(md_file) => {
     //         crate::println!("Markdown file loaded:");
     //         md_file.print_content(); // Use the no_std print
     //         crate::println!("Word count: {}", md_file.word_count());
     //     },
     //     Err(e) => crate::eprintln!("Error opening or reading Markdown file: {:?}", e),
     // }

     eprintln!("Markdown file example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_md")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("Markdown file example (std) starting...");
     eprintln!("Markdown file example (std) using simple line reader.");

     // This example needs a dummy Markdown file.
     use std::fs::remove_file;
     use std::io::Write;

     let md_path = Path::new("example.md");

     // Create a dummy Markdown file
     let dummy_content = r#"
# My Document

This is the first paragraph.

This is the second paragraph. It has multiple words.
"#;

     match File::create(md_path) {
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


     match MdFile::new(md_path) { // Call the function that opens and reads
         Ok(md_file) => {
             println!("Markdown file loaded ({} lines):", md_file.content.len());
             md_file.print_content(); // Use std print
             println!("Word count: {}", md_file.word_count());
         }
         Err(e) => {
              eprintln!("Error opening or reading file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(md_path) {
          eprintln!("Error removing dummy file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("Markdown file example (std) finished.");

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


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests


     // Helper function to create MdFile from a string (simulates reading from file)
      fn create_md_file_from_string(content: &str) -> Result<MdFile, FileSystemError> {
          // Simulate reading from a buffer in memory.
          // We need a Read + Seek implementation over the string bytes.
          // In std, use std::io::Cursor. In no_std tests, use a mock reader or a custom Cursor.
          // Since this test helper is only used in #[cfg(feature = "std")] tests below,
          // we can use std::io::Cursor.

          #[cfg(feature = "std")]
          {
               let content_bytes = content.as_bytes().to_vec(); // Convert string slice to Vec<u8>
               let mut cursor = Cursor::new(content_bytes); // Create an in-memory reader/seeker

               let mut content_lines = Vec::new();
               let mut current_line = String::new();

               const BUFFER_SIZE: usize = 1024;
               let mut buffer = vec![0u8; BUFFER_SIZE];

               // Use core::io::Read trait on Cursor
               let mut reader_trait: &mut dyn core::io::Read = &mut cursor;


               loop {
                   let bytes_read = reader_trait.read(&mut buffer).map_err(|e| FileSystemError::IOError(format!("Cursor read error: {}", e)))?; // Map std::io::Error
                    if bytes_read == 0 {
                        if !current_line.is_empty() {
                            content_lines.push(current_line); // Add the last line
                        }
                       break;
                   }

                   let chunk_str = from_utf8(&buffer[..bytes_read])
                       .map_err(|e| FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)))?;

                   let mut chars = chunk_str.chars();
                   while let Some(ch) = chars.next() {
                       if ch == '\n' {
                            content_lines.push(current_line);
                            current_line = String::new();
                       } else {
                            current_line.push(ch);
                       }
                   }
               }
               Ok(MdFile { path: String::from("in_memory_test.md"), content: content_lines })
          }

          #[cfg(not(feature = "std"))]
          {
              // In no_std tests, this helper would need a mock SahneResourceReader over the string data.
              // This is more complex and deferred to the dedicated no_std test section.
              unimplemented!("create_md_file_from_string not implemented for no_std tests directly");
          }

      }


     // Test the line parsing logic with various inputs (using the helper)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_md_parsing_lines() -> Result<(), FileSystemError> { // Return FileSystemError
          let simple_content = "Line 1\nLine 2\nLine 3"; // Last line no newline
          let md_file = create_md_file_from_string(simple_content)?;
          assert_eq!(md_file.content.len(), 3);
          assert_eq!(md_file.content[0], "Line 1");
          assert_eq!(md_file.content[1], "Line 2");
          assert_eq!(md_file.content[2], "Line 3");

          let content_with_empty_lines = "Line 1\n\nLine 3\n"; // Empty line, trailing newline
          let md_file_empty = create_md_file_from_string(content_with_empty_lines)?;
          assert_eq!(md_file_empty.content.len(), 4); // Note: The empty line is captured, trailing newline results in an empty string at the end
          assert_eq!(md_file_empty.content[0], "Line 1");
          assert_eq!(md_file_empty.content[1], "");
          assert_eq!(md_file_empty.content[2], "Line 3");
           assert_eq!(md_file_empty.content[3], ""); // Trailing newline creates an empty line

           let content_with_crlf = "Line 1\r\nLine 2\r\n"; // CRLF newlines
            // The current logic only splits on '\n'. '\r' characters will be included in the line.
            // A more robust parser would handle \r\n. Let's confirm the current behavior.
           let md_file_crlf = create_md_file_from_string(content_with_crlf)?;
           assert_eq!(md_file_crlf.content.len(), 2);
           assert_eq!(md_file_crlf.content[0], "Line 1\r");
           assert_eq!(md_file_crlf.content[1], "Line 2\r");


          Ok(())
     }

     // Test the word_count method (independent of file I/O)
     #[test]
     fn test_word_count() {
          let md_file = MdFile {
               path: String::from("dummy.md"),
               content: vec![
                   String::from("This is a line with 6 words."),
                   String::from("Another line."),
                   String::from(" Single."),
                   String::from(""), // Empty line
                   String::from(" Multiple   spaces "), // Multiple spaces
               ]
          };
          assert_eq!(md_file.word_count(), 6 + 2 + 1 + 0 + 3); // 12
     }


     // Test for MdFile::new in std environment (uses actual file I/O)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_md_file_new_std() -> Result<(), FileSystemError> { // Return FileSystemError
          let dir = tempdir().map_err(|e| FileSystemError::IOError(format!("Tempdir error: {}", e)))?;
          let file_path = dir.path().join("test_file_std.md");

          // Create a dummy file using std FS
           let dummy_content = r#"
# Header

First paragraph.
Second paragraph.
"#;
          let mut file = File::create(&file_path).map_err(|e| map_std_io_error_to_fs_error(e))?;
          file.write_all(dummy_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e))?;

          // Call MdFile::new with the file path
          let md_file = MdFile::new(&file_path).map_err(|e| {
               // Clean up the file on error before returning
               let _ = remove_file(&file_path);
               e
          })?;

          // Assert the content was read and split correctly
           // Note: The initial empty line due to leading newline in dummy_content will be captured.
           assert_eq!(md_file.content.len(), 4); // Expected lines: "", "# Header", "First paragraph.", "Second paragraph."
           assert_eq!(md_file.content[0], "");
           assert_eq!(md_file.content[1], "# Header");
           assert_eq!(md_file.content[2], "First paragraph.");
           assert_eq!(md_file.content[3], "Second paragraph.");

          // Clean up the dummy file
          let _ = remove_file(&file_path); // Ignore result, best effort cleanup

          Ok(())
      }

     // TODO: Add tests for MdFile::new in no_std environment using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include reading valid content, handling file not found, IO errors, UTF8 errors,
     // and different line endings (\n, \r\n).
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_md", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
