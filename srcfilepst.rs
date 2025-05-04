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
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec, format!
use alloc::string::String;
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


/// Custom error type for PST file handling issues.
#[derive(Debug)]
pub enum PstError {
    UnexpectedEof(String), // During header or node data reading
    InvalidHeaderSignature([u8; 4]), // Expected "!BDN" or similar
    InvalidNodeDataSize(usize, u32), // Read incorrect number of bytes for node data
    SeekError(u64), // Failed to seek
    // Add other PST specific parsing errors here (e.g., invalid node structure)
}

// Implement Display for PstError
impl fmt::Display for PstError {
    fn fmt(&mut self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PstError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            PstError::InvalidHeaderSignature(signature) => write!(f, "Geçersiz PST başlık imzası: {:x?}", signature),
            PstError::InvalidNodeDataSize(read, expected) => write!(f, "Geçersiz düğüm veri boyutu: {} okundu, {} bekleniyordu", read, expected),
            PstError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map PstError to FileSystemError
fn map_pst_error_to_fs_error(e: PstError) -> FileSystemError {
    match e {
        PstError::UnexpectedEof(_) | PstError::SeekError(_) => FileSystemError::IOError(format!("PST IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("PST ayrıştırma/veri hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilepsd.rs'den kopyalandı)
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


/// PST file handler. Provides methods to read basic file header and node data.
/// This is a simplified handler and does not implement full PST parsing.
pub struct PstFile<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size
}

impl<R: Read + Seek> PstFile<R> {
    /// Creates a new `PstFile` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        PstFile {
            reader, // Store the reader
            handle,
            file_size,
        }
    }

    /// Reads the PST file header (first 4 bytes).
    /// The reader is assumed to be positioned at the start of the file.
    ///
    /// # Returns
    ///
    /// A Result containing the 4-byte header signature or a FileSystemError.
    pub fn read_header(&mut self) -> Result<[u8; 4], FileSystemError> { // Return FileSystemError
        let mut header = [0u8; 4];
        // Use read_exact to ensure exactly 4 bytes are read
        self.reader.read_exact(&mut header).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_pst_error_to_fs_error(PstError::UnexpectedEof(String::from("header signature"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Basic validation (optional, PST header can be '!BDN' or other values depending on version)
         if &header != b"!BDN" { /* You might add checks for other valid signatures here */ }

        Ok(header)
    }

    /// Reads the data for a specific "node" at the given offset and size.
    ///
    /// # Arguments
    ///
    /// * `offset` - The starting position of the node data in the file.
    /// * `size` - The size of the node data in bytes.
    ///
    /// # Returns
    ///
    /// A Result containing the node data as Vec<u8> or FileSystemError.
    pub fn read_node(&mut self, offset: u64, size: u32) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
        // Seek to the specified offset. Map core::io::Error to FileSystemError.
        self.reader.seek(SeekFrom::Start(offset)).map_err(|e| map_core_io_error_to_fs_error(e))?;

        // Create a Vec to hold the node data. Requires alloc.
        let mut buffer = Vec::with_capacity(size as usize);
        // Resize the vector to the expected size for read_exact
        buffer.resize(size as usize, 0);


        // Read exactly `size` bytes into the buffer. Map core::io::Error to FileSystemError.
         let bytes_read = self.reader.read(&mut buffer).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use read to get bytes_read

        // Check if the correct number of bytes was read
         if bytes_read != size as usize {
             return Err(map_pst_error_to_fs_error(PstError::InvalidNodeDataSize(bytes_read, size)));
         }


        Ok(buffer)
    }

    // Add other PST parsing and reading functionalities here (e.g., parsing node structure, reading properties).
     pub fn parse_node(&self, node_data: &[u8]) -> Result<PstNode, FileSystemError> { ... }
     pub fn find_node_by_id(&mut self, node_id: u32) -> Result<Option<(u64, u32)>, FileSystemError> { ... }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for PstFile<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the PstFile is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: PstFile drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a PST file from the given path (std) or resource ID (no_std)
/// and creates a PstFile instance.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the PstFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_pst_file<P: AsRef<Path>>(file_path: P) -> Result<PstFile<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    // Seek to end to get size, then seek back to start
    let mut temp_reader = BufReader::new(File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?); // Need a temporary reader to get size without moving the main one
    let file_size = temp_reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    // No need to seek temp_reader back, it will be dropped.

    // Create a PstFile instance with the reader
    Ok(PstFile::from_reader(reader, None, file_size)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_pst_file(file_path: &str) -> Result<PstFile<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    // Create a PstFile instance with the reader
    Ok(PstFile::from_reader(reader, Some(handle), file_size)) // Pass the handle to the PstFile struct
}


// Example main function (no_std)
#[cfg(feature = "example_pst")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("PST file handler example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy PST file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Create dummy PST data bytes for the mock filesystem
     // Minimal PST header (4 bytes), then some dummy node data
     let dummy_pst_data: Vec<u8> = vec![
         0x21, 0x42, 0x44, 0x4e, // Minimal header signature "!BDN"
         // Add dummy node data (e.g., at offset 1024, size 512)
         // Need to pad up to offset 1024 if needed
         // For simplicity, let's just have a header and a small dummy node right after.
         // A real PST has complex structures after the header.
         // Header is at offset 0. Let's put dummy node data at offset 4.
         // Node data size 16.
          0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01, 0x02,
          0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A // 16 bytes dummy node data
     ];
      // Assuming the mock filesystem is set up to provide this data for "sahne://files/example.pst"

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/example.pst" exists with the dummy data.
      let pst_res = open_pst_file("sahne://files/example.pst");
      match pst_res {
          Ok(mut pst_file) => { // Need mut to read header/nodes
              crate::println!("PST file loaded.");
     //
     //         // Read the header
              match pst_file.read_header() {
                  Ok(header_sig) => crate::println!("PST Header Signature: {:x?}", header_sig),
                  Err(e) => crate::eprintln!("Error reading PST header: {:?}", e),
              }
     //
     //         // Example: Read dummy node data (at offset 4, size 16)
              let node_offset = 4;
              let node_size = 16;
              match pst_file.read_node(node_offset, node_size) {
                  Ok(node_data) => {
                      crate::println!("Read {} bytes for node at offset {}: {:x?}", node_data.len(), node_offset, node_data);
                  },
                  Err(e) => crate::eprintln!("Error reading node data: {:?}", e),
              }
     //
     //         // File is automatically closed when pst_file goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening PST file: {:?}", e),
      }

     eprintln!("PST file handler example (no_std) needs Sahne64 mocks to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The dummy PST data to be available at the specified path.

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_pst")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("PST file handler example (std) starting...");
     eprintln!("PST file handler example (std) using core::io.");

     // Create dummy example.pst data bytes
     let dummy_pst_data: Vec<u8> = vec![
         0x21, 0x42, 0x44, 0x4e, // Minimal header signature "!BDN"
         // Add dummy node data (e.g., at offset 1024, size 512)
         // Need to pad up to offset 1024 if needed
         // Let's add some padding and then a dummy node.
         // Header is at offset 0 (4 bytes). Let's put a dummy node at offset 20.
         // Need 16 bytes of padding (20 - 4 = 16).
          // Padding (16 bytes)
          0x00; 16
          // Dummy node data (16 bytes)
          0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01, 0x02,
          0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A // 16 bytes dummy node data
     ];


     let file_path = Path::new("example.pst");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_pst_data).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy PST file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy PST file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_pst_file(file_path) { // Call the function that opens and creates the handler
         Ok(mut pst_file) => { // Need mut to read header/nodes
             println!("PST file loaded.");

             // Read the header
             match pst_file.read_header() {
                 Ok(header_sig) => {
                     println!("PST Header Signature: {:x?}", header_sig);
                     // Assert basic header signature
                     assert_eq!(header_sig, [0x21, 0x42, 0x44, 0x4e]); // "!BDN"
                 },
                 Err(e) => {
                     eprintln!("Error reading PST header: {}", e); // std error display
                     // Don't return error, let cleanup run
                 }
             }

             // Example: Read dummy node data (at offset 20, size 16)
             let node_offset = 20;
             let node_size = 16;
             match pst_file.read_node(node_offset, node_size) {
                 Ok(node_data) => {
                     println!("Read {} bytes for node at offset {}: {:x?}", node_data.len(), node_offset, node_data);
                     // Assert node data
                      assert_eq!(node_data, vec![
                          0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01, 0x02,
                          0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A
                      ]);
                 },
                 Err(e) => {
                     eprintln!("Error reading node data: {}", e); // std error display
                      // Don't return error, let cleanup run
                 }
             }

             // File is automatically closed when pst_file goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening PST file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy PST file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("PST file handler example (std) finished.");

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
     use byteorder::WriteBytesExt as StdWriteBytesExt; // For writing integers in BigEndian


     // Helper function to create dummy PST data in memory
      #[cfg(feature = "std")] // Uses std::io::Cursor
       fn create_dummy_pst_bytes(header_sig: [u8; 4], node_offset: u64, node_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(Vec::new());
           // Write header signature
           buffer.write_all(&header_sig)?;

           // Write padding up to node_offset
           let header_len = header_sig.len() as u64;
           if node_offset > header_len {
               let padding_size = node_offset - header_len;
               buffer.write_all(&vec![0u8; padding_size as usize])?;
           } else if node_offset < header_len {
                // Error: node_offset is within header
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Node offset is within header")));
           }


           // Write node data
           buffer.write_all(node_data)?;


           Ok(buffer.into_inner())
       }


     // Test parsing header from a valid minimal PST header in memory
     #[test]
     fn test_read_pst_header_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
          // Create dummy PST bytes with a valid header signature
          let header_sig = *b"!BDN"; // Example valid signature
          // Add some minimal data afterwards so read_exact doesn't fail immediately
          let dummy_pst_bytes = create_dummy_pst_bytes(header_sig, 4, &[0u8; 10]).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?; // Node at offset 4, size 10


          // Use Cursor as a Read + Seek reader
          let file_size = dummy_pst_bytes.len() as u64;
          let cursor = Cursor::new(dummy_pst_bytes.clone()); // Clone for potential re-reads in test

          // Create a PstFile instance from the cursor reader
          let mut pst_file = PstFile::from_reader(cursor, None, file_size); // Pass None for handle

          // Read the header
          let read_header_sig = pst_file.read_header()?;

          // Assert header signature is correct
          assert_eq!(read_header_sig, header_sig);

          // Verify the reader is positioned after the header (4 bytes)
          assert_eq!(pst_file.reader.stream_position().unwrap(), 4);


          Ok(())
     }

      // Test handling of invalid header signature
       #[test]
       fn test_read_pst_header_invalid_signature_cursor() {
            // Create dummy bytes with invalid signature
            let invalid_sig = [0xAA, 0xBB, 0xCC, 0xDD];
            // Add some minimal data afterwards
             let dummy_pst_bytes = create_dummy_pst_bytes(invalid_sig, 4, &[0u8; 10]).unwrap();


            let file_size = dummy_pst_bytes.len() as u64;
            let cursor = Cursor::new(dummy_pst_bytes);
            let mut pst_file = PstFile::from_reader(cursor, None, file_size);

            // Attempt to read header, expect an error
            let result = pst_file.read_header();

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from PstError::InvalidHeaderSignature
                    assert!(msg.contains("Geçersiz PST başlık imzası"));
                     assert!(msg.contains("aabbccdd")); // Hex representation
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }

       // Test handling of unexpected EOF during header reading
        #[test]
        fn test_read_pst_header_truncated_cursor() {
             // Truncated header (only 2 bytes)
             let dummy_bytes = vec![0x21, 0x42]; // "!B"

             let file_size = dummy_bytes.len() as u64;
             let cursor = Cursor::new(dummy_bytes);
             let mut pst_file = PstFile::from_reader(cursor, None, file_size);

             // Attempt to read header, expect an error
             let result = pst_file.read_header();
             assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from PstError::UnexpectedEof (via read_exact)
                     assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                     assert!(msg.contains("header signature"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }
        }


     // Test reading node data from a valid data block in memory
      #[test]
      fn test_read_pst_node_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
           // Create dummy PST bytes with a header and dummy node data
           let header_sig = *b"!BDN";
           let node_offset = 20; // Node starts at offset 20
           let node_data_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]; // 10 bytes of node data
           let dummy_pst_bytes = create_dummy_pst_bytes(header_sig, node_offset, &node_data_bytes).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


           // Use Cursor as a Read + Seek reader
           let file_size = dummy_pst_bytes.len() as u64;
           let cursor = Cursor::new(dummy_pst_bytes.clone());

           // Create a PstFile instance from the cursor reader
           let mut pst_file = PstFile::from_reader(cursor, None, file_size);

           // Seek the reader past the header to the node offset manually for this test
           // In a real scenario, node offset would be determined by PST structure.
           pst_file.reader.seek(SeekFrom::Start(0)).map_err(|e| map_core_io_error_to_fs_error(e))?; // Start from beginning
            let _header = pst_file.read_header()?; // Read header to position reader after it
            // Now the reader is at offset 4. We need to seek to node_offset (20).
            // The read_node function handles the seek itself, so we just need the correct offset.


           // Read the node data using the read_node method
           let read_data = pst_file.read_node(node_offset, node_data_bytes.len() as u32)?;

           // Assert the read data is correct
           assert_eq!(read_data, node_data_bytes);

           // Verify the reader is positioned after the node data
           assert_eq!(pst_file.reader.stream_position().unwrap(), node_offset + node_data_bytes.len() as u64);


           Ok(())
      }

       // Test handling of unexpected EOF during node data reading
        #[test]
        fn test_read_pst_node_truncated_cursor() {
             // Create dummy PST bytes with header and truncated node data
             let header_sig = *b"!BDN";
             let node_offset = 20;
             let node_data_bytes = vec![1, 2, 3]; // Only 3 bytes of node data
             // Requesting 10 bytes of node data, but only 3 are available after offset.
             let dummy_pst_bytes = create_dummy_pst_bytes(header_sig, node_offset, &node_data_bytes).unwrap(); // Node at offset 20, size 3


            let file_size = dummy_pst_bytes.len() as u64;
            let cursor = Cursor::new(dummy_pst_bytes);
            let mut pst_file = PstFile::from_reader(cursor, None, file_size);

            // Attempt to read node data, expect an error (UnexpectedEof)
            let node_size_requested = 10;
            let result = pst_file.read_node(node_offset, node_size_requested);
            assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from PstError::InvalidNodeDataSize
                     assert!(msg.contains("Geçersiz düğüm veri boyutu"));
                     assert!(msg.contains(&format!("{} okundu, {} bekleniyordu", node_data_bytes.len(), node_size_requested)));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during header/node reading,
     // and correctly reading header and node data from mock data.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_pst", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
