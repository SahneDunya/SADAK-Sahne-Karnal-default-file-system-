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


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs::{self, O_RDONLY}, resource, SahneError, FileSystemError, Handle}; // fs, O_RDONLY, resource, SahneError, FileSystemError, Handle

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


/// Custom error type for RTF parsing issues.
#[derive(Debug)]
pub enum RtfError {
    UnexpectedEof(String), // During reading
    InvalidRtfSignature, // Expected "{\rtf"
    ParsingError(String), // Generic parsing error (due to basic parser limitations)
    SeekError(u64), // Failed to seek
    InvalidUtf8, // Error converting bytes to UTF-8 string
    // Add other RTF specific parsing errors here
}

// Implement Display for RtfError
impl fmt::Display for RtfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RtfError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            RtfError::InvalidRtfSignature => write!(f, "Geçersiz RTF imzası"),
            RtfError::ParsingError(msg) => write!(f, "RTF ayrıştırma hatası: {}", msg),
            RtfError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
            RtfError::InvalidUtf8 => write!(f, "Geçersiz UTF-8 verisi"),
        }
    }
}

// Helper function to map RtfError to FileSystemError
fn map_rtf_error_to_fs_error(e: RtfError) -> FileSystemError {
    match e {
        RtfError::UnexpectedEof(_) | RtfError::SeekError(_) => FileSystemError::IOError(format!("RTF IO hatası: {}", e)), // Map IO related errors
        RtfError::InvalidUtf8 => FileSystemError::InvalidData(format!("RTF UTF-8 hatası: {}", e)),
        _ => FileSystemError::InvalidData(format!("RTF ayrıştırma hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilerar.rs'den kopyalandı)
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
// Removed redundant print module boilerplate.


/// Basic RTF parser. Reads and provides a method to extract plain text.
pub struct RtfFile<R: Read> { // Only Read is needed if we read sequentially, but Seek is needed for file size and other potential operations
    reader: R, // Reader implementing Read
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size (Seek is needed for this)
}

impl<R: Read + Seek> RtfFile<R> { // Add Seek bound back for file_size and potential future use
    /// Creates a new `RtfFile` instance from a reader.
    /// Reads the RTF signature to validate the file type.
    /// This is used internally after opening the file/resource.
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Read the first 5 bytes to check for RTF signature "{\rtf"
        let mut signature_buffer = [0u8; 5];
         reader.read_exact(&mut signature_buffer).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_rtf_error_to_fs_error(RtfError::UnexpectedEof(String::from("RTF signature"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
         })?;


        if &signature_buffer != b"{\\rtf" {
             return Err(map_rtf_error_to_fs_error(RtfError::InvalidRtfSignature));
        }

        // Seek back to the start after reading the signature, so extraction starts from beginning
        reader.seek(SeekFrom::Start(0)).map_err(|e| map_core_io_error_to_fs_error(e))?;


        Ok(RtfFile {
            reader, // Store the reader
            handle,
            file_size,
        })
    }

    /// Performs a very basic parsing of RTF content to extract plain text.
    /// This parser is rudimentary and will not correctly handle complex RTF features.
    /// Reads the entire file content using the internal reader.
    ///
    /// # Returns
    ///
    /// A Result containing the extracted plain text as String or FileSystemError.
    pub fn extract_plain_text(&mut self) -> Result<String, FileSystemError> { // Return FileSystemError
        let mut plain_text = String::new(); // Requires alloc
        let mut buffer = [0u8; 512]; // Read in chunks


        loop {
            // Read a chunk from the file
            let bytes_read = self.reader.read(&mut buffer).map_err(|e| map_core_io_error_to_fs_error(e))?;

            if bytes_read == 0 {
                break; // End of file
            }

            // Pass the chunk to the basic RTF parser and append the result
            // Note: This basic parser might produce incorrect results for complex RTF.
            let chunk_plain_text = Self::parse_rtf_chunk(&buffer[..bytes_read])?; // Handle parsing errors

            plain_text.push_str(&chunk_plain_text); // Requires alloc
        }


        Ok(plain_text) // Return the accumulated plain text
    }


    /// Performs a very basic parsing of an RTF chunk to extract plain text.
    /// This is a helper function used by `extract_plain_text`.
    ///
    /// # Arguments
    ///
    /// * `buffer` - A byte slice containing an RTF chunk.
    ///
    /// # Returns
    ///
    /// A Result containing the extracted plain text for the chunk as String or RtfError.
    fn parse_rtf_chunk(buffer: &[u8]) -> Result<String, RtfError> { // Return RtfError for parsing issues
        let mut content = String::new(); // Requires alloc
        let mut i = 0;

        while i < buffer.len() {
            if buffer[i] == b'\\' {
                i += 1;
                if i < buffer.len() {
                    match buffer[i] {
                        // Escaped characters
                        b'{' | b'}' | b'\\' => {
                            content.push(buffer[i] as char);
                            i += 1;
                        },
                        // Control words
                        _ => {
                            // Skip alphabetic characters (control word name)
                            while i < buffer.len() && buffer[i].is_ascii_alphabetic() {
                                i += 1;
                            }
                            // Skip optional parameter (digits and sign) - basic handling
                             while i < buffer.len() && (buffer[i].is_ascii_digit() || buffer[i] == b'-') {
                                 i += 1;
                             }

                            // Skip delimiter (space or newline or group start/end, or other token boundary)
                            // The original code checked for ';', but RTF delimiters are more complex.
                            // A simple approach is to skip the next character if it's a common delimiter.
                            // However, relying on a simple character check is fragile.
                            // A more robust parser needs to understand RTF tokens.
                            // For this basic stub, let's just skip space after control word name/parameter.
                            if i < buffer.len() && buffer[i] == b' ' {
                                i += 1;
                            }

                            // If it's not a known escape and not a simple control word pattern,
                            // it might be an invalid sequence or a different type of control.
                            // The original code just continued. A proper parser would handle errors.
                            // For this refactor, let's just skip the unrecognized control sequence.
                            // Note: This skipping logic is still very basic.
                            continue; // Move to the next character after the potential control word sequence
                        }
                    }
                } else {
                     // Backslash at end of buffer chunk
                     // This indicates an incomplete control sequence at the chunk boundary.
                     // A stateful parser is needed to handle this correctly across chunks.
                     // For this simple chunk parser, just stop processing this chunk.
                     // Or indicate an error if an incomplete sequence is considered invalid.
                     // Let's return an error for incomplete escape sequences at chunk end.
                     return Err(RtfError::ParsingError(String::from("Incomplete RTF escape sequence at chunk end"))); // Requires alloc
                }
            } else if buffer[i] == b'{' || buffer[i] == b'}' {
                // Ignore group delimiters in this basic plain text extraction
                i += 1;
            } else if buffer[i] == b'\n' || buffer[i] == b'\r' {
                 // Handle newlines if desired in plain text output
                 content.push(buffer[i] as char); // Append newline/carriage return
                 i += 1;
            }
            else {
                 // Append other characters as plain text (assuming they are valid characters)
                 // Need to handle character sets/code pages for non-ASCII.
                 // Assuming simple single-byte characters for this basic parser.
                 content.push(buffer[i] as char);
                 i += 1;
            }
        }

        Ok(content) // Return extracted plain text for this chunk
    }

    // Add other methods here for accessing parsed RTF structure or data if a more advanced parser is used.
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for RtfFile<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the RtfFile is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: RtfFile drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens an RTF file from the given path (std) or resource ID (no_std)
/// and creates an RtfFile instance after validating the signature.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the RtfFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_rtf_file<P: AsRef<Path>>(file_path: P) -> Result<RtfFile<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    // Seek to end to get size, then seek back to start
    let mut temp_reader = BufReader::new(File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?); // Need a temporary reader to get size without moving the main one
    let file_size = temp_reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    // No need to seek temp_reader back, it will be dropped.

    // Create an RtfFile by validating the signature from the reader
    RtfFile::from_reader(reader, None, file_size) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_rtf_file(file_path: &str) -> Result<RtfFile<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    // Create an RtfFile by validating the signature from the reader
    RtfFile::from_reader(reader, Some(handle), file_size) // Pass the handle to the RtfFile struct
}


// Example main function (no_std)
#[cfg(feature = "example_rtf")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("RTF parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy RTF file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Create dummy RTF data bytes for the mock filesystem
     let dummy_rtf_data: Vec<u8> = vec![
         0x7b, 0x5c, 0x72, 0x74, 0x66, // Signature "{\rtf"
         // Add dummy RTF content for testing the basic parser
          0x5c, 0x70, 0x61, 0x72, 0x20, // \par
          0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x5c, 0x62, 0x20, // Hello, \b
          0x57, 0x6f, 0x72, 0x6c, 0x64, 0x21, // World!
          0x7d // } (end group)
     ];
      // Assuming the mock filesystem is set up to provide this data for "sahne://files/document.rtf"

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/document.rtf" exists with the dummy data.
      let rtf_res = open_rtf_file("sahne://files/document.rtf");
      match rtf_res {
          Ok(mut rtf_file) => { // Need mut to extract plain text
              crate::println!("RTF file loaded (signature checked).");
     //
     //         // Extract plain text using the basic parser
              match rtf_file.extract_plain_text() {
                  Ok(plain_text) => {
                      crate::println!("Extracted Plain Text: {}", plain_text); // Requires String Display
     //                 // Expected output from dummy data: "Hello, World!" (assuming basic parser skips \par and \b)
     //                 // A more accurate basic parser might output "Hello, World!" without the control words.
     //                 // The current basic parser should output "Hello, World!"
                  },
                  Err(e) => crate::eprintln!("Error extracting plain text: {:?}", e),
              }
     //
     //         // File is automatically closed when rtf_file goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening RTF file: {:?}", e),
      }

     eprintln!("RTF parser example (no_std) needs Sahne64 mocks to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The dummy RTF data to be available at the specified path.

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_rtf")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("RTF parser example (std) starting...");
     eprintln!("RTF parser example (std) using basic RTF parsing.");

     // Create dummy RTF data bytes
     let dummy_rtf_data: Vec<u8> = vec![
         0x7b, 0x5c, 0x72, 0x74, 0x66, // Signature "{\rtf"
         // Add dummy RTF content for testing the basic parser
          0x5c, 0x70, 0x61, 0x72, 0x20, // \par (control word)
          0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, // Hello,
          0x7b, 0x5c, 0x62, 0x20, 0x57, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x7d, // {\b World!} (group with control word)
          0x5c, 0x7b, // \{ (escaped brace)
          0x5c, 0x7d, // \} (escaped brace)
          0x5c, 0x5c, // \\ (escaped backslash)
          0x0a, 0x0d, // newline, carriage return
          0x78, 0x79, 0x7a // xyz (plain text)
     ];


     let file_path = Path::new("example.rtf");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_rtf_data).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy RTF file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy RTF file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_rtf_file(file_path) { // Call the function that opens and validates signature
         Ok(mut rtf_file) => { // Need mut to extract plain text
             println!("RTF file loaded (signature checked).");

             // Extract plain text using the basic parser
             match rtf_file.extract_plain_text() {
                 Ok(plain_text) => {
                     println!("Extracted Plain Text: {}", plain_text); // Requires String Display
                     // Expected output based on the basic parser and dummy data:
                     // "Hello, World!{}\"
                     // The basic parser ignores control words and groups, but keeps escaped chars.
                     // It also keeps newlines/carriage returns and other plain text.
                     // Expected output should be: "Hello, World!{}\xyz" (newlines/returns ignored by simple push(char))
                     // Let's adjust expected output based on the updated parse_rtf_chunk logic (keeps newlines/returns):
                     // "Hello, \b World!{}\\\n\rxyz" (basic parser skips \par, but keeps \b as control word)
                     // With updated parse_rtf_chunk, \par is skipped, \b is skipped, escaped chars \ { } \ are kept, groups { } are ignored.
                     // Newlines/returns are kept.
                     // Input: {\rtf\par Hello, {\b World!}{\ \{ \} \\ \n \r xyz
                     // Expected: " Hello,  World!{}\\\n\rxyz" (spaces after control words kept by parser logic)
                      assert_eq!(plain_text, " Hello,  World!{}\\\n\rxyz");

                 },
                 Err(e) => {
                     eprintln!("Error extracting plain text: {}", e); // std error display
                      // Don't return error, let cleanup run
                 }
             }

             // File is automatically closed when rtf_file goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening RTF file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy RTF file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("RTF parser example (std) finished.");

     Ok(())
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


     // Helper function to create dummy RTF bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor and Write
       fn create_dummy_rtf_bytes(content: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(Vec::new());
           // Signature "{\rtf"
           buffer.write_all(b"{\\rtf")?;
           // Content
           buffer.write_all(content)?;

           Ok(buffer.into_inner())
       }


     // Test basic parsing of RTF content chunk
      #[test]
      fn test_parse_rtf_chunk_basic() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error> for std test error handling
           // RTF chunk with various elements for basic parsing test
           let rtf_chunk = b"\\par Hello, {\\b World!}{\\{ \\} \\\\ \n \r xyz";

           let plain_text_result = RtfFile::parse_rtf_chunk(rtf_chunk).map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Parsing error: {}", e))))?; // Map RtfError to std Error

           // Expected output based on the basic parser logic:
           // Skips \par, skips \b, keeps {, keeps }, keeps \, keeps \n, keeps \r, keeps xyz.
           // Spaces after control words are kept by the parser logic.
           assert_eq!(plain_text_result, " Hello,  World!{}\\\n\rxyz");

           Ok(())
      }

      // Test handling of incomplete escape sequence at chunk end
       #[test]
       fn test_parse_rtf_chunk_incomplete_escape() {
            // RTF chunk ending with a backslash
            let rtf_chunk = b"Hello\\";

            // Attempt to parse the chunk, expect an error
            let result = RtfFile::parse_rtf_chunk(rtf_chunk);

            assert!(result.is_err());
            match result.unwrap_err() {
                RtfError::ParsingError(msg) => { // Mapped from RtfError::ParsingError
                    assert!(msg.contains("Incomplete RTF escape sequence at chunk end"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }


     // Test opening and extracting plain text from a valid RTF file in memory
      #[test]
      fn test_extract_plain_text_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError

           // Create dummy RTF bytes
           let rtf_content_bytes = b"\\par Hello, {\\b World!} This is a test.";
           let dummy_rtf_bytes = create_dummy_rtf_bytes(rtf_content_bytes).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


           // Use Cursor as a Read + Seek reader
           let file_size = dummy_rtf_bytes.len() as u64;
           let cursor = Cursor::new(dummy_rtf_bytes.clone());

           // Create an RtfFile instance from the cursor reader
           let mut rtf_file = RtfFile::from_reader(cursor, None, file_size)?; // Signature check happens here

           // Extract plain text
           let plain_text = rtf_file.extract_plain_text()?;

           // Expected plain text based on the basic parser: " Hello,  World! This is a test."
           assert_eq!(plain_text, " Hello,  World! This is a test.");


           Ok(())
      }


     // Test handling of invalid RTF signature during opening
      #[test]
      fn test_open_rtf_file_invalid_signature() {
           // Create dummy bytes that do not start with "{\rtf"
           let dummy_bytes = b"NOT{\\rtf Test content.";

           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes.to_vec());

           // Attempt to open the file, expect an error during signature validation
           let result = RtfFile::from_reader(cursor, None, file_size);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from RtfError::InvalidRtfSignature
                   assert!(msg.contains("Geçersiz RTF imzası"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       // Test handling of unexpected EOF during signature reading
        #[test]
        fn test_open_rtf_file_truncated_signature() {
             // Truncated data (only first 3 bytes of signature)
             let dummy_bytes = b"{\\r"; // "{ \ r"

             let file_size = dummy_bytes.len() as u64;
             let cursor = Cursor::new(dummy_bytes.to_vec());

             // Attempt to open the file, expect an error during signature reading (UnexpectedEof)
             let result = RtfFile::from_reader(cursor, None, file_size);
             assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from RtfError::UnexpectedEof (via read_exact)
                     assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                     assert!(msg.contains("RTF signature"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }
        }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during reading,
     // and verifying the basic plain text extraction from mock data.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_rtf", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
