#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader as StdBufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt}; // Added BufReader
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::O_RDONLY; // Import necessary fs flags


// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ByteOrder, ReadBytesExt}; // BigEndian, ByteOrder, ReadBytesExt trait/types

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


/// Custom error type for WebM (EBML) handling issues.
#[derive(Debug)]
pub enum WebmError {
    UnexpectedEof(String), // During header reading
    InvalidEbmlHeaderId(u32), // EBML Header ID mismatch
    InvalidSegmentHeaderId(u32), // Segment Header ID mismatch
    SeekError(u64), // Failed to seek
    // Add other EBML/WebM specific parsing errors here
}

// Implement Display for WebmError
impl fmt::Display for WebmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebmError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            WebmError::InvalidEbmlHeaderId(id) => write!(f, "Geçersiz EBML başlık ID'si: Beklenen 0x1A45DFA3, bulunan 0x{:X}", id),
            WebmError::InvalidSegmentHeaderId(id) => write!(f, "Geçersiz Segment başlık ID'si: Beklenen 0x18538067, bulunan 0x{:X}", id),
            WebmError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map WebmError to FileSystemError
fn map_webm_error_to_fs_error(e: WebmError) -> FileSystemError {
    match e {
        WebmError::UnexpectedEof(_) | WebmError::SeekError(_) => FileSystemError::IOError(format!("WebM IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("WebM format/veri hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read implementasyonu (copied from srcfilewav.rs, simplified)
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


/// Represents a WebM file and provides basic parsing capabilities.
/// This struct holds the file path. Parsing operations open and close
/// the file as needed using standardized I/O.
pub struct WebM {
    pub file_path: String, // Store the file path
}

impl WebM {
    /// Creates a new WebM instance referring to the given path.
    pub fn new(file_path: String) -> Self {
        WebM { file_path }
    }

    /// Parses the basic structure (EBML and Segment headers) of the WebM file.
    /// Opens the file, reads the required header bytes, and validates them.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError.
    pub fn parse(&self) -> Result<(), FileSystemError> { // Return FileSystemError
        // Open the file using the standardized function
        let mut reader = open_webm_reader(&self.file_path)?; // open_webm_reader returns a reader implementing Read+Seek+Drop


        // Use a buffered reader for efficiency when reading small chunks
        #[cfg(feature = "std")]
        let mut buffered_reader = StdBufReader::new(&mut reader); // Wrap reference to reader
        #[cfg(not(feature = "std"))]
        // Assuming a custom no_std BufReader implementation exists and is in scope (e.g., crate::BufReader)
        let mut buffered_reader = crate::BufReader::new(&mut reader); // Wrap reference to reader


        // Read and parse EBML header
        let ebml_header = Self::read_ebml_header(&mut buffered_reader)?; // Pass the buffered reader
        println!("EBML Header: {:?}", ebml_header); // Use standardized print


        // Read and parse Segment header
        let segment_header = Self::read_segment_header(&mut buffered_reader)?; // Pass the buffered reader
        println!("Segment Header: {:?}", segment_header); // Use standardized print


        // The file is automatically closed when 'reader' goes out of scope (due to Drop on SahneResourceReadSeek or File).

        Ok(()) // Return success
    }

    /// Reads and validates the EBML header from a reader.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read.
    ///             The reader should be positioned at the start of the EBML header.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed EBMLHeader or a FileSystemError.
    fn read_ebml_header<R: Read>(reader: &mut R) -> Result<EBMLHeader, FileSystemError> { // Return FileSystemError
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer).map_err(|e| match e.kind() {
            core::io::ErrorKind::UnexpectedEof => map_webm_error_to_fs_error(WebmError::UnexpectedEof(String::from("EBML header"))), // Requires alloc
            _ => map_core_io_error_to_fs_error(e),
        })?;


        let id = BigEndian::read_u32(&buffer); // Use byteorder for Big Endian

        if id != 0x1A45DFA3 { // Expected EBML Header ID
            return Err(map_webm_error_to_fs_error(WebmError::InvalidEbmlHeaderId(id)));
        }

        Ok(EBMLHeader { id })
    }

    /// Reads and validates the Segment header from a reader.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read.
    ///             The reader should be positioned at the start of the Segment header.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed SegmentHeader or a FileSystemError.
    fn read_segment_header<R: Read>(reader: &mut R) -> Result<SegmentHeader, FileSystemError> { // Return FileSystemError
        // In a real EBML file, the Segment header follows the EBML header and its size information.
        // This basic parser assumes it's directly after the 4-byte EBML ID + Variable Length Integer size.
        // The actual Segment ID (0x18538067) is the first 4 bytes of the Segment element.
        // A proper parser would read the EBML header's size field and seek/read accordingly.
        // For this refactor, we'll keep the simplified assumption that the Segment ID is right after
        // a fixed number of bytes after the EBML ID (e.g., after the 4 byte EBML ID and a Variable Length Integer size).
        // The original code assumed it's 4 bytes after the EBML ID read position, which is incorrect.
        // Let's assume for this basic example, the Segment header is located at a fixed offset (e.g., 8 bytes) after the file start,
        // after reading the 4-byte EBML ID and a placeholder 4-byte VINT size (even though VINTs are variable length).

        // After reading the 4-byte EBML ID, a Variable Length Integer (VINT) follows, indicating the size of the EBML header element.
        // Let's assume a minimal VINT size representation (e.g., 4 bytes). A proper parser needs to read and decode the VINT.
        // We need to seek past the EBML header's size VINT before reading the Segment ID.
        // A typical minimal EBML header is 4 bytes (ID) + 4 bytes (size VINT) = 8 bytes.
        // The Segment element starts after the EBML header element.
        // The Segment element itself starts with the Segment ID (4 bytes) followed by its size VINT.

        // Let's read the VINT size after the EBML ID. This requires reading a VINT.
        // The original code did not read the VINT size, it just read the next 4 bytes.
        // This is a significant simplification.

        // For this refactor, let's *read* the next bytes after the EBML ID, assuming they contain the VINT size.
        // We won't fully parse the VINT size, but just read bytes to advance the reader.
        // A common minimal VINT size is 4 bytes.
        let mut vint_size_bytes = [0u8; 4];
        reader.read_exact(&mut vint_size_bytes).map_err(|e| match e.kind() {
            core::io::ErrorKind::UnexpectedEof => map_webm_error_to_fs_error(WebmError::UnexpectedEof(String::from("EBML header VINT size"))), // Requires alloc
            _ => map_core_io_error_to_fs_error(e),
        })?;
        // TODO: Implement proper VINT decoding. For now, just read the bytes.


        // Now, read the Segment ID (4 bytes)
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer).map_err(|e| match e.kind() {
            core::io::ErrorKind::UnexpectedEof => map_webm_error_to_fs_error(WebmError::UnexpectedEof(String::from("Segment header"))), // Requires alloc
            _ => map_core_io_error_to_fs_error(e),
        })?;


        let id = BigEndian::read_u32(&buffer); // Use byteorder for Big Endian

        if id != 0x18538067 { // Expected Segment Header ID
             // If not the Segment ID, it might be another EBML element after the header.
             // A full parser would handle this. For this basic parser, it's an error.
            return Err(map_webm_error_to_fs_error(WebmError::InvalidSegmentHeaderId(id)));
        }

        Ok(SegmentHeader { id })
    }
}

#[derive(Debug, PartialEq)] // Add PartialEq for tests
struct EBMLHeader {
    id: u32,
}

#[derive(Debug, PartialEq)] // Add PartialEq for tests
struct SegmentHeader {
    id: u32,
}

/// Opens a WebM file from the given path (std) or resource ID (no_std)
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
pub fn open_webm_reader<P: AsRef<Path>>(file_path: P) -> Result<File, FileSystemError> { // Return std::fs::File (implements Read+Seek+Drop)
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    Ok(file)
}

#[cfg(not(feature = "std"))]
pub fn open_webm_reader(file_path: &str) -> Result<SahneResourceReadSeek, FileSystemError> { // Return SahneResourceReadSeek (implements Read+Seek+Drop)
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
#[cfg(feature = "example_webm")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> io::Result<()> { // Return std::io::Result for std example
     eprintln!("WebM header parser example (std) starting...");
     eprintln!("WebM header parser example (std) using std::io and byteorder.");

     // Example WebM bytes (EBML Header + minimal EBML Header size VINT + Segment Header)
     // EBML Header ID: 1A 45 DF A3
     // EBML Header size VINT (minimal 4-byte): 80 00 00 00 (size 0 - not typical, but for minimal example)
     // Segment Header ID: 18 53 80 67
     let webm_bytes: Vec<u8> = vec![
         0x1A, 0x45, 0xDF, 0xA3, // EBML Header ID
         0x80, 0x00, 0x00, 0x00, // EBML Header size VINT (placeholder for 0)
         0x18, 0x53, 0x80, 0x67, // Segment Header ID
          // ... rest of the file ...
     ];


     let file_path = Path::new("example.webm");

      // Write example content to a temporary file for std example
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&webm_bytes).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy WebM file: {}", e);
                       // Map FileSystemError back to std::io::Error for std main
                      match e {
                          FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                          _ => return Err(io::Error::new(io::ErrorKind::Other, format!("Mapped FS error: {:?}", e))), // Generic map for others
                      }
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy WebM file: {}", e);
                  return Err(e); // Return std::io::Error
             }
        }
        println!("Dummy WebM file created: {}", file_path.display());


     // Create a WebM instance (holds the path)
     let webm_file = WebM::new(file_path.to_string_lossy().into_owned()); // Convert PathBuf to String


     // Parse the WebM file header
     match webm_file.parse() { // Call the parse method
         Ok(_) => {
             println!("WebM header parsed successfully.");
         }
         Err(e) => {
             eprintln!("Error parsing WebM header: {}", e); // std error display
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
               eprintln!("Error removing dummy WebM file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("WebM header parser example (std) finished.");

     Ok(()) // Return Ok from std main
}

// Example main function (no_std)
#[cfg(feature = "example_webm")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for no_std example
     eprintln!("WebM header parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // and simulate fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Example WebM bytes (EBML Header + minimal EBML Header size VINT + Segment Header)
     // EBML Header ID: 1A 45 DF A3
     // EBML Header size VINT (minimal 4-byte): 80 00 00 00 (size 0 - not typical, but for minimal example)
     // Segment Header ID: 18 53 80 67
     let webm_bytes: Vec<u8> = vec![
         0x1A, 0x45, 0xDF, 0xA3, // EBML Header ID
         0x80, 0x00, 0x00, 0x00, // EBML Header size VINT (placeholder for 0)
         0x18, 0x53, 0x80, 0x67, // Segment Header ID
          // ... rest of the file ...
     ]; // Requires alloc


     let filename = "sahne://files/example.webm";

      // Hypothetical usage with Sahne64 mocks:
      // // Assume a mock filesystem is set up and "sahne://files/example.webm" exists with the dummy data.
      // // We need to simulate writing this data to the mock file before reading.
      //
      // // Create a WebM instance (holds the path)
       let webm_file = WebM::new(String::from(filename)); // Requires alloc
      //
      // // Parse the WebM file header
       match webm_file.parse() { // Call the parse method
           Ok(_) => {
               crate::println!("WebM header parsed successfully.");
           }
           Err(e) => {
               crate::eprintln!("Error parsing WebM header: {:?}", e); // no_std print
           }
       }


     eprintln!("WebM header parser example (no_std) needs Sahne64 mocks and byteorder crate to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real) and dummy data.
     // 2. The byteorder crate compiled with no_std and alloc features.

     Ok(()) // Dummy return
}


// Test module (std feature active)
#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use std::io::Cursor as StdCursor; // For in-memory testing
    use byteorder::{BigEndian, WriteBytesExt}; // For byteorder traits on Cursor
    use std::error::Error; // For Box<dyn Error>


    // Helper to create dummy WebM bytes in memory
    fn create_dummy_webm_bytes(include_ebml_header: bool, include_segment_header: bool, invalid_ebml_id: Option<u32>, invalid_segment_id: Option<u32>) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buffer = StdCursor::new(Vec::new()); // Use std::io::Cursor for in-memory buffer

        // Write EBML Header (optional)
        if include_ebml_header {
            let ebml_id = invalid_ebml_id.unwrap_or(0x1A45DFA3);
            buffer.write_u32::<BigEndian>(ebml_id)?;
             // Write placeholder VINT size for EBML header (4 bytes for minimal example)
             // In a real scenario, this would be a variable-length integer encoding the size of the EBML header element itself.
            buffer.write_u32::<BigEndian>(0x80000000)?; // Placeholder for size 0 VINT
        }


        // Write Segment Header (optional)
        if include_segment_header {
            let segment_id = invalid_segment_id.unwrap_or(0x18538067);
            buffer.write_u32::<BigEndian>(segment_id)?;
             // Write placeholder VINT size for Segment (e.g., size of the rest of the file)
             // This would also be a variable-length integer. For test simplicity, just add a few bytes.
             buffer.write_all(&[0x80, 0x00, 0x00, 0x00])?; // Placeholder VINT size
        }

        // Add some extra bytes to simulate file content
        buffer.write_all(&[0xFF, 0xEE, 0xDD, 0xCC])?;


        Ok(buffer.into_inner()) // Return the underlying Vec<u8>
    }


    #[test]
    fn test_read_ebml_header_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError for std test
        // Create dummy bytes with a valid EBML header
        let webm_bytes = create_dummy_webm_bytes(true, false, None, None)
             .map_err(|e| FileSystemError::Other(format!("Test data creation error: {}", e)))?;

        // Create a cursor to read from the bytes
        let mut cursor = StdCursor::new(webm_bytes);

        // Use a buffered reader
        let mut buffered_reader = StdBufReader::new(&mut cursor);


        // Read and parse the EBML header
        let ebml_header = WebM::read_ebml_header(&mut buffered_reader)?; // Pass the buffered reader

        // Assert the parsed ID
        assert_eq!(ebml_header.id, 0x1A45DFA3);


        Ok(()) // Return Ok from test function
    }

    #[test]
    fn test_read_ebml_header_invalid_id_cursor() {
         // Create dummy bytes with an invalid EBML header ID
         let webm_bytes = create_dummy_webm_bytes(true, false, Some(0x11223344), None).unwrap(); // Use unwrap for test data


         let mut cursor = StdCursor::new(webm_bytes);
         let mut buffered_reader = StdBufReader::new(&mut cursor);


         // Attempt to read, expect an error
         let result = WebM::read_ebml_header(&mut buffered_reader);

         assert!(result.is_err());
         match result.unwrap_err() {
             FileSystemError::InvalidData(msg) => { // Mapped from WebmError::InvalidEbmlHeaderId
                 assert!(msg.contains("WebM format/veri hatası"));
                 assert!(msg.contains("Geçersiz EBML başlık ID'si"));
                 assert!(msg.contains("bulunan 0x11223344"));
             },
             _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
         }
    }

     #[test]
      fn test_read_ebml_header_truncated_cursor() {
           // Create dummy bytes that are too short for the EBML header
           let dummy_bytes = vec![0x1A, 0x45, 0xDF]; // Only 3 bytes

           let mut cursor = StdCursor::new(dummy_bytes);
           let mut buffered_reader = StdBufReader::new(&mut cursor);

           // Attempt to read, expect an error due to unexpected EOF
           let result = WebM::read_ebml_header(&mut buffered_reader);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from core::io::ErrorKind::UnexpectedEof
                   assert!(msg.contains("WebM IO hatası"));
                   assert!(msg.contains("Beklenmedik dosya sonu"));
                   assert!(msg.contains("EBML header"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }


     #[test]
    fn test_read_segment_header_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
        // Create dummy bytes with valid EBML and Segment headers
        let webm_bytes = create_dummy_webm_bytes(true, true, None, None)
             .map_err(|e| FileSystemError::Other(format!("Test data creation error: {}", e)))?;

        // Create a cursor to read from the bytes
        let mut cursor = StdCursor::new(webm_bytes);
         let mut buffered_reader = StdBufReader::new(&mut cursor);


        // Read past the EBML header and its VINT size to position for Segment header
         let mut ebml_header_id_buf = [0u8; 4];
          buffered_reader.read_exact(&mut ebml_header_id_buf).map_err(|e| map_core_io_error_to_fs_error(e))?; // Read EBML ID (4 bytes)
          let mut ebml_vint_size_buf = [0u8; 4];
          buffered_reader.read_exact(&mut ebml_vint_size_buf).map_err(|e| map_core_io_error_to_fs_error(e))?; // Read placeholder VINT size (4 bytes)


        // Read and parse the Segment header
        let segment_header = WebM::read_segment_header(&mut buffered_reader)?; // Pass the buffered reader

        // Assert the parsed ID
        assert_eq!(segment_header.id, 0x18538067);


        Ok(()) // Return Ok from test function
    }

     #[test]
      fn test_read_segment_header_invalid_id_cursor() {
           // Create dummy bytes with valid EBML header but invalid Segment header ID
           let webm_bytes = create_dummy_webm_bytes(true, true, None, Some(0x55667788)).unwrap(); // Use unwrap for test data


           let mut cursor = StdCursor::new(webm_bytes);
           let mut buffered_reader = StdBufReader::new(&mut cursor);


           // Read past the EBML header and its VINT size
            let mut ebml_header_id_buf = [0u8; 4];
             buffered_reader.read_exact(&mut ebml_header_id_buf).unwrap(); // Ignore error in test setup
             let mut ebml_vint_size_buf = [0u8; 4];
             buffered_reader.read_exact(&mut ebml_vint_size_buf).unwrap(); // Ignore error in test setup


           // Attempt to read Segment header, expect an error
           let result = WebM::read_segment_header(&mut buffered_reader);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from WebmError::InvalidSegmentHeaderId
                   assert!(msg.contains("WebM format/veri hatası"));
                   assert!(msg.contains("Geçersiz Segment başlık ID'si"));
                   assert!(msg.contains("bulunan 0x55667788"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }


    // TODO: Add tests for the parse method using the open_webm_reader helper and mock filesystem (no_std).
    // This requires simulating the filesystem operations (acquire, fstat, read_at, release).
    // Test cases should include opening valid/invalid files and verifying the parse result (Ok/Err).
    // Testing EBML VINT decoding would require extending the read_ebml_header/read_segment_header logic and testing that specifically.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_webm", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
