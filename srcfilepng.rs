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

// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec, format!
use alloc::string::String;
use alloc::vec::Vec; // For temporary buffers
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


/// Custom error type for PNG parsing issues.
#[derive(Debug)]
pub enum PngError {
    UnexpectedEof(String), // During magic number or chunk header/data reading
    InvalidMagicNumber([u8; 8]),
    InvalidChunkType([u8; 4]), // Expected IHDR but got something else
    InvalidIhdrLength(u32), // IHDR length is not 13
    InvalidIhdrDataSize(usize), // Read incorrect number of bytes for IHDR data
    SeekError(u64), // Failed to seek
    // Add other PNG specific parsing errors here (e.g., invalid dimensions, invalid compression method)
}

// Implement Display for PngError
impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PngError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            PngError::InvalidMagicNumber(magic) => write!(f, "Geçersiz PNG sihirli sayısı: {:x?}", magic),
            PngError::InvalidChunkType(chunk_type) => write!(f, "Geçersiz PNG chunk tipi: {:x?}", chunk_type),
            PngError::InvalidIhdrLength(len) => write!(f, "Geçersiz IHDR chunk uzunluğu: {}", len),
            PngError::InvalidIhdrDataSize(size) => write!(f, "Geçersiz IHDR veri boyutu: {}", size),
            PngError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map PngError to FileSystemError
fn map_png_error_to_fs_error(e: PngError) -> FileSystemError {
    match e {
        PngError::UnexpectedEof(_) | PngError::SeekError(_) => FileSystemError::IOError(format!("PNG IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("PNG ayrıştırma hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilepdf.rs'den kopyalandı)
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


// Removed redundant arch, fs, SahneError imports (already imported via crate::).


/// Basic PNG header structure.
#[derive(Debug, PartialEq, Eq)] // Add PartialEq, Eq for tests
pub struct PngHeader {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub compression_method: u8,
    pub filter_method: u8,
    pub interlace_method: u8,
}

// PNG magic number
const PNG_MAGIC_NUMBER: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
// IHDR chunk type
const IHDR_CHUNK_TYPE: [u8; 4] = *b"IHDR";
// IHDR chunk data length
const IHDR_DATA_LENGTH: u32 = 13;


/// Basic PNG header parser. Extracts image dimensions and other IHDR properties.
/// Does NOT parse other chunks or pixel data.
pub struct PngParser<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size
}

impl<R: Read + Seek> PngParser<R> {
    /// Creates a new `PngParser` instance from a reader and parses the PNG magic number.
    /// This is used internally after opening the file/resource.
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Check PNG magic number (8 bytes)
        let mut magic_number = [0u8; 8];
        reader.read_exact(&mut magic_number).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_png_error_to_fs_error(PngError::UnexpectedEof(String::from("magic number"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        if magic_number != PNG_MAGIC_NUMBER {
             return Err(map_png_error_to_fs_error(PngError::InvalidMagicNumber(magic_number)));
        }

        Ok(PngParser {
            reader, // Store the reader
            handle,
            file_size,
        })
    }

    /// Reads and parses the IHDR chunk after the magic number.
    ///
    /// # Returns
    ///
    /// A Result containing the PngHeader or a FileSystemError if parsing fails.
    pub fn parse_ihdr(&mut self) -> Result<PngHeader, FileSystemError> { // Return FileSystemError

        // Read IHDR chunk length (4 bytes, Big Endian)
        let ihdr_length = self.reader.read_u32::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_png_error_to_fs_error(PngError::UnexpectedEof(String::from("IHDR length"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Validate IHDR length (must be 13)
        if ihdr_length != IHDR_DATA_LENGTH {
             return Err(map_png_error_to_fs_error(PngError::InvalidIhdrLength(ihdr_length)));
        }

        // Read IHDR chunk type (4 bytes)
        let mut ihdr_type = [0u8; 4];
        self.reader.read_exact(&mut ihdr_type).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_png_error_to_fs_error(PngError::UnexpectedEof(String::from("IHDR type"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Validate IHDR chunk type
        if ihdr_type != IHDR_CHUNK_TYPE {
             return Err(map_png_error_to_fs_error(PngError::InvalidChunkType(ihdr_type)));
        }

        // Read IHDR chunk data (13 bytes)
        let mut ihdr_data = [0u8; IHDR_DATA_LENGTH as usize];
        let bytes_read = self.reader.read(&mut ihdr_data).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use read to get bytes_read

        // Check if exactly 13 bytes were read for IHDR data
         if bytes_read != IHDR_DATA_LENGTH as usize {
             return Err(map_png_error_to_fs_error(PngError::InvalidIhdrDataSize(bytes_read)));
         }


        // Extract values from IHDR data (Big Endian)
        let width = u32::from_be_bytes([
            ihdr_data[0], ihdr_data[1], ihdr_data[2], ihdr_data[3],
        ]);
        let height = u32::from_be_bytes([
            ihdr_data[4], ihdr_data[5], ihdr_data[6], ihdr_data[7],
        ]);
        let bit_depth = ihdr_data[8];
        let color_type = ihdr_data[9];
        let compression_method = ihdr_data[10];
        let filter_method = ihdr_data[11];
        let interlace_method = ihdr_data[12];

        // Read and discard the CRC (4 bytes) after IHDR data
        let mut crc = [0u8; 4];
        // Use read_exact and map the error, don't ignore it
        self.reader.read_exact(&mut crc).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_png_error_to_fs_error(PngError::UnexpectedEof(String::from("IHDR CRC"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;


        // Return the parsed header information
        Ok(PngHeader {
            width,
            height,
            bit_depth,
            color_type,
            compression_method,
            filter_method,
            interlace_method,
        })
    }

    // You could add other methods here for parsing other chunks, pixel data, etc.
    // pub fn parse_next_chunk_header(&mut self) -> Result<Option<(u32, [u8; 4])>, FileSystemError> { ... }
    // pub fn read_idat_data(&mut self) -> Result<Vec<u8>, FileSystemError> { ... }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for PngParser<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the PngParser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: PngParser drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a PNG file from the given path (std) or resource ID (no_std)
/// and parses its header (magic number + IHDR chunk).
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the parsed PngHeader or a FileSystemError.
#[cfg(feature = "std")]
pub fn parse_png_header<P: AsRef<Path>>(file_path: P) -> Result<PngHeader, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create a PngParser and parse the header
    let mut parser = PngParser::from_reader(reader, None, file_size)?; // Pass None for handle in std version

    // Parse and return the IHDR chunk
    parser.parse_ihdr()
}

#[cfg(not(feature = "std"))]
pub fn parse_png_header(file_path: &str) -> Result<PngHeader, FileSystemError> { // Return FileSystemError
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

    // Create a PngParser and parse the header
    let mut parser = PngParser::from_reader(reader, Some(handle), file_size)?; // Pass the handle to the parser

    // Parse and return the IHDR chunk
    parser.parse_ihdr()
}


// Example main function (no_std)
#[cfg(feature = "example_png")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("PNG header parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy PNG file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/image.png" exists.
     // let png_res = parse_png_header("sahne://files/image.png");
     // match png_res {
     //     Ok(header) => {
     //         crate::println!("Parsed PNG Header: {:?}", header);
     //         crate::println!(" Dimensions: {}x{}", header.width, header.height);
     //     },
     //     Err(e) => crate::eprintln!("Error parsing PNG header: {:?}", e),
     // }

     eprintln!("PNG header parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_png")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("PNG header parser example (std) starting...");
     eprintln!("PNG header parser example (std) using header parsing.");

     // This example needs a dummy PNG file with at least the magic number and IHDR chunk.
     // Creating a minimal valid PNG header structure bytes in memory.
      let mut png_bytes: Vec<u8> = Vec::new();
       // Magic number (8 bytes)
      png_bytes.extend_from_slice(&PNG_MAGIC_NUMBER);

       // IHDR chunk (4 length + 4 type + 13 data + 4 CRC = 25 bytes)
       // Length (13, Big Endian)
       png_bytes.extend_from_slice(&13u32.to_be_bytes());
       // Type ("IHDR")
       png_bytes.extend_from_slice(&IHDR_CHUNK_TYPE);
       // Data (13 bytes: width, height, bit depth, color type, compression, filter, interlace)
       // Width (200, Big Endian)
       png_bytes.extend_from_slice(&200u32.to_be_bytes());
       // Height (160, Big Endian)
       png_bytes.extend_from_slice(&160u32.to_be_bytes());
       // Bit Depth (8)
       png_bytes.push(8);
       // Color Type (6 - Truecolour with alpha)
       png_bytes.push(6);
       // Compression Method (0 - deflate)
       png_bytes.push(0);
       // Filter Method (0 - adaptive)
       png_bytes.push(0);
       // Interlace Method (0 - no interlace)
       png_bytes.push(0);
       // CRC (4 bytes - dummy value, actual CRC calculation needed for valid PNG)
       png_bytes.extend_from_slice(&[0u8; 4]);


      let file_path = Path::new("example.png");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&png_bytes).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy PNG file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy PNG file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match parse_png_header(file_path) { // Call the function that opens and parses header
         Ok(header) => {
             println!("Parsed PNG Header: {:?}", header);
             println!(" Dimensions: {}x{}", header.width, header.height);

             // Basic assertion based on the dummy data
             assert_eq!(header.width, 200);
             assert_eq!(header.height, 160);
             assert_eq!(header.bit_depth, 8);
             assert_eq!(header.color_type, 6);
             assert_eq!(header.compression_method, 0);
             assert_eq!(header.filter_method, 0);
             assert_eq!(header.interlace_method, 0);
         }
         Err(e) => {
              eprintln!("Error parsing PNG header: {}", e); // std error display
              // Don't return error, let cleanup run
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy PNG file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("PNG header parser example (std) finished.");

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


     // Helper function to create dummy PNG header bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn create_dummy_png_header_bytes(width: u32, height: u32, bit_depth: u8, color_type: u8, compression: u8, filter: u8, interlace: u8) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(Vec::new());
           // Magic number
           buffer.write_all(&PNG_MAGIC_NUMBER)?;

           // IHDR chunk (length 13)
           buffer.write_u32::<BigEndian>(IHDR_DATA_LENGTH)?; // Length
           buffer.write_all(&IHDR_CHUNK_TYPE)?; // Type
           // Data (13 bytes)
           buffer.write_u32::<BigEndian>(width)?;
           buffer.write_u32::<BigEndian>(height)?;
           buffer.write_u8(bit_depth)?;
           buffer.write_u8(color_type)?;
           buffer.write_u8(compression)?;
           buffer.write_u8(filter)?;
           buffer.write_u8(interlace)?;

           // CRC (4 bytes - dummy)
           buffer.write_all(&[0u8; 4])?;

           Ok(buffer.into_inner())
       }


     // Test parsing a valid PNG header in memory
     #[test]
     fn test_parse_png_header_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
          // Create dummy PNG header bytes
          let dummy_png_bytes = create_dummy_png_header_bytes(
              640, 480, 8, 2, 0, 0, 0 // Example dimensions and properties
          ).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


          // Use Cursor as a Read + Seek reader
          let file_size = dummy_png_bytes.len() as u64;
          let cursor = Cursor::new(dummy_png_bytes.clone()); // Clone for potential re-reads in test

          // Create a PngParser using the cursor reader
          let mut parser = PngParser::from_reader(cursor, None, file_size)?; // Pass None for handle

          // Parse the IHDR header
          let header = parser.parse_ihdr()?;

          // Assert header fields are correct
          assert_eq!(header.width, 640);
          assert_eq!(header.height, 480);
          assert_eq!(header.bit_depth, 8);
          assert_eq!(header.color_type, 2);
          assert_eq!(header.compression_method, 0);
          assert_eq!(header.filter_method, 0);
          assert_eq!(header.interlace_method, 0);

          // Verify the reader is positioned after the IHDR chunk + CRC
          // Magic (8) + IHDR length (4) + IHDR type (4) + IHDR data (13) + CRC (4) = 33
          assert_eq!(parser.reader.stream_position().unwrap(), 33);


          Ok(())
     }

     // Test handling of invalid magic number
      #[test]
      fn test_parse_png_header_invalid_magic() {
           // Create dummy bytes with invalid magic number
           let mut dummy_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8]; // Invalid magic number
            // Add some dummy IHDR to make it long enough, though it won't be read
            dummy_bytes.extend_from_slice(&[0u8; 25]); // IHDR + CRC size

           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes);
           // Attempt to create PngParser (which checks magic number), expect an error
           let result = PngParser::from_reader(cursor, None, file_size);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from PngError::InvalidMagicNumber
                   assert!(msg.contains("Geçersiz PNG sihirli sayısı"));
                   assert!(msg.contains("0102030405060708")); // Hex representation
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of unexpected EOF during magic number reading
       #[test]
       fn test_parse_png_header_truncated_magic() {
            // Truncated magic (4 bytes)
            let dummy_bytes = vec![137, 80, 78, 71];

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            // Attempt to create PngParser, expect an error
            let result = PngParser::from_reader(cursor, None, file_size);
            assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from PngError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                    assert!(msg.contains("magic number"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
       }

       // Test handling of invalid IHDR length
        #[test]
        fn test_parse_png_header_invalid_ihdr_length() {
             // Valid magic + invalid IHDR length (e.g., 10 instead of 13)
             let mut dummy_bytes_cursor = Cursor::new(Vec::new());
             dummy_bytes_cursor.write_all(&PNG_MAGIC_NUMBER).unwrap();
             dummy_bytes_cursor.write_u32::<BigEndian>(10).unwrap(); // Invalid length 10
              // Add dummy data for the length+type (4) + data (10) + CRC (4)
              dummy_bytes_cursor.write_all(&IHDR_CHUNK_TYPE).unwrap();
              dummy_bytes_cursor.write_all(&[0u8; 10]).unwrap();
              dummy_bytes_cursor.write_all(&[0u8; 4]).unwrap();

             let dummy_bytes = dummy_bytes_cursor.into_inner();

             let file_size = dummy_bytes.len() as u64;
             let cursor = Cursor::new(dummy_bytes);
             let mut parser = PngParser::from_reader(cursor, None, file_size).unwrap();

             // Attempt to parse IHDR, expect an error
             let result = parser.parse_ihdr();

             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from PngError::InvalidIhdrLength
                     assert!(msg.contains("Geçersiz IHDR chunk uzunluğu: 10"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


      // Test handling of invalid IHDR chunk type
       #[test]
       fn test_parse_png_header_invalid_ihdr_type() {
            // Valid magic + valid IHDR length (13) + invalid IHDR type ("BAD!")
            let mut dummy_bytes_cursor = Cursor::new(Vec::new());
            dummy_bytes_cursor.write_all(&PNG_MAGIC_NUMBER).unwrap();
            dummy_bytes_cursor.write_u32::<BigEndian>(IHDR_DATA_LENGTH).unwrap(); // Valid length 13
            dummy_bytes_cursor.write_all(b"BAD!").unwrap(); // Invalid type
             // Add dummy data for the 13 bytes and 4 bytes CRC
             dummy_bytes_cursor.write_all(&[0u8; 13]).unwrap();
             dummy_bytes_cursor.write_all(&[0u8; 4]).unwrap();

            let dummy_bytes = dummy_bytes_cursor.into_inner();

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            let mut parser = PngParser::from_reader(cursor, None, file_size).unwrap();

            // Attempt to parse IHDR, expect an error
            let result = parser.parse_ihdr();

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from PngError::InvalidChunkType
                    assert!(msg.contains("Geçersiz PNG chunk tipi"));
                    assert!(msg.contains("42414421")); // Hex representation of "BAD!"
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }


       // Test handling of unexpected EOF during IHDR data reading
        #[test]
        fn test_parse_png_header_truncated_ihdr_data() {
             // Valid magic + valid IHDR length (13) + valid IHDR type + truncated IHDR data (e.g., 5 bytes instead of 13)
             let mut dummy_bytes_cursor = Cursor::new(Vec::new());
             dummy_bytes_cursor.write_all(&PNG_MAGIC_NUMBER).unwrap();
             dummy_bytes_cursor.write_u32::<BigEndian>(IHDR_DATA_LENGTH).unwrap(); // Valid length 13
             dummy_bytes_cursor.write_all(&IHDR_CHUNK_TYPE).unwrap(); // Valid type
             dummy_bytes_cursor.write_all(&[0u8; 5]).unwrap(); // Truncated data (5 bytes)
             // Don't add CRC, make it EOF after data.

             let dummy_bytes = dummy_bytes_cursor.into_inner();

             let file_size = dummy_bytes.len() as u64;
             let cursor = Cursor::new(dummy_bytes);
             let mut parser = PngParser::from_reader(cursor, None, file_size).unwrap();

             // Attempt to parse IHDR, expect an error during IHDR data reading
             let result = parser.parse_ihdr();
             assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from PngError::InvalidIhdrDataSize
                     assert!(msg.contains("Geçersiz IHDR veri boyutu: 5"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }
        }

        // Test handling of unexpected EOF during CRC reading
         #[test]
         fn test_parse_png_header_truncated_crc() {
              // Valid magic + valid IHDR length (13) + valid IHDR type + valid IHDR data (13) + truncated CRC (2 bytes instead of 4)
              let mut dummy_bytes_cursor = Cursor::new(Vec::new());
               dummy_bytes_cursor.write_all(&PNG_MAGIC_NUMBER).unwrap();
               dummy_bytes_cursor.write_u32::<BigEndian>(IHDR_DATA_LENGTH).unwrap(); // Valid length 13
               dummy_bytes_cursor.write_all(&IHDR_CHUNK_TYPE).unwrap(); // Valid type
               dummy_bytes_cursor.write_all(&[0u8; 13]).unwrap(); // Valid data (13 bytes)
               dummy_bytes_cursor.write_all(&[0u8; 2]).unwrap(); // Truncated CRC (2 bytes)

              let dummy_bytes = dummy_bytes_cursor.into_inner();

              let file_size = dummy_bytes.len() as u64;
              let cursor = Cursor::new(dummy_bytes);
              let mut parser = PngParser::from_reader(cursor, None, file_size).unwrap();

              // Attempt to parse IHDR, expect an error during CRC reading
              let result = parser.parse_ihdr();
              assert!(result.is_err());
               match result.unwrap_err() {
                  FileSystemError::IOError(msg) => { // Mapped from PngError::UnexpectedEof (via read_exact)
                      assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                      assert!(msg.contains("IHDR CRC"));
                  },
                  _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
               }
         }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during header reading,
     // and correctly parsing headers from mock data.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_png", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
