#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec; // For SOF0 data buffer
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


/// Custom error type for JPEG parsing issues.
#[derive(Debug)]
pub enum JpegError {
    UnexpectedEof(String), // During marker, length, or data reading
    InvalidMarker([u8; 2]), // Expected marker (FF xx) but got something else
    InvalidSOIMarker([u8; 2]), // Expected FF D8 but got something else
    InvalidSegmentLength(u16, String), // Segment length too short or inconsistent
    SOF0NotFound, // SOF0 marker not found before EOI
    InvalidDimensions, // Width or height is 0 after parsing SOF0
    SeekError(u64), // Failed to seek
    // Add other JPEG specific parsing errors here
}

// Implement Display for JpegError
impl fmt::Display for JpegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JpegError::UnexpectedEof(segment) => write!(f, "Beklenmedik dosya sonu ({} okurken)", segment),
            JpegError::InvalidMarker(marker) => write!(f, "Geçersiz JPEG marker: {:x?}", marker),
            JpegError::InvalidSOIMarker(marker) => write!(f, "Geçersiz SOI marker: {:x?}", marker),
            JpegError::InvalidSegmentLength(len, name) => write!(f, "Geçersiz {} segment uzunluğu: {}", name, len),
            JpegError::SOF0NotFound => write!(f, "SOF0 markerı EOI'dan önce bulunamadı"),
            JpegError::InvalidDimensions => write!(f, "Resim boyutları ayrıştırılamadı (0 veya geçersiz)"),
            JpegError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map JpegError to FileSystemError
fn map_jpeg_error_to_fs_error(e: JpegError) -> FileSystemError {
    match e {
        JpegError::UnexpectedEof(_) | JpegError::SeekError(_) => FileSystemError::IOError(format!("JPEG IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("JPEG ayrıştırma hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfileobj.rs'den kopyalandı)
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


/// Represents basic parsed JPEG image metadata (dimensions).
pub struct JpegImage {
    pub width: u32,
    pub height: u32,
    // More JPEG metadata can be added here later
}

/// Basic JPEG header parser. Extracts image dimensions from the SOF0 segment.
/// Does NOT parse pixel data or complex segments.
pub struct JpegParser<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks
}

impl<R: Read + Seek> JpegParser<R> {
    /// Creates a new `JpegParser` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        Self { reader, handle, file_size }
    }

    /// Parses the JPEG file header and extracts image dimensions.
    ///
    /// # Returns
    ///
    /// A Result containing the JpegImage metadata or a FileSystemError.
    pub fn parse_header(&mut self) -> Result<JpegImage, FileSystemError> { // Return FileSystemError
        // JPEG file structure: SOI marker, segments, SOS marker, compressed data, EOI marker

        // Read SOI marker (Start of Image) - FF D8
        let mut soi_buffer = [0u8; 2];
        self.reader.read_exact(&mut soi_buffer).map_err(|e| map_core_io_error_to_fs_error(e))?;
        if soi_buffer != [0xFF, 0xD8] {
             return Err(map_jpeg_error_to_fs_error(JpegError::InvalidSOIMarker(soi_buffer)));
        }

        let mut width: u32 = 0;
        let mut height: u32 = 0;
        let mut found_sof0 = false;

        // Loop through segments until SOF0 or EOI is found
        loop {
            // Read segment marker (FF xx)
            let mut marker_buffer = [0u8; 2];
            // Use read_exact, mapping IO errors including UnexpectedEof
            self.reader.read_exact(&mut marker_buffer).map_err(|e| match e.kind() {
                 core::io::ErrorKind::UnexpectedEof => map_jpeg_error_to_fs_error(JpegError::UnexpectedEof(String::from("segment marker"))), // Requires alloc
                 _ => map_core_io_error_to_fs_error(e),
            })?;


            // Validate marker start byte (should be FF)
            if marker_buffer[0] != 0xFF {
                 return Err(map_jpeg_error_to_fs_error(JpegError::InvalidMarker(marker_buffer)));
            }

            match marker_buffer[1] {
                 0xC0 => { // SOF0 marker (Start-of-frame, baseline DCT)
                    found_sof0 = true;
                    // Read segment length (2 bytes, Big Endian) - length includes the 2 bytes of the length field itself
                    let sof0_length = self.reader.read_u16::<BigEndian>().map_err(|e| map_core_io_error_to_fs_error(e))? as usize;

                    // Minimum SOF0 segment length is 8 bytes (2 marker + 2 length + 1 precision + 2 height + 2 width + >=1 components)
                    if sof0_length < 8 {
                         return Err(map_jpeg_error_to_fs_error(JpegError::InvalidSegmentLength(sof0_length as u16, String::from("SOF0")))); // Requires alloc
                    }

                    // Read SOF0 data (length - 2 bytes)
                    // The first byte is precision, then 2 bytes height, then 2 bytes width.
                    // We only need the height and width bytes.
                    let sof0_data_size = sof0_length - 2; // Size of the data following the length field

                     // Read the first 5 bytes of SOF0 data: 1 byte precision, 2 bytes height, 2 bytes width
                     let mut dimensions_buffer = [0u8; 5];
                     self.reader.read_exact(&mut dimensions_buffer).map_err(|e| match e.kind() {
                          core::io::ErrorKind::UnexpectedEof => map_jpeg_error_to_fs_error(JpegError::UnexpectedEof(String::from("SOF0 dimensions"))), // Requires alloc
                          _ => map_core_io_error_to_fs_error(e),
                     })?;


                    // Extract height and width (Big Endian)
                    height = u16::from_be_bytes([dimensions_buffer[1], dimensions_buffer[2]]) as u32;
                    width = u16::from_be_bytes([dimensions_buffer[3], dimensions_buffer[4]]) as u32;

                    // Skip the rest of the SOF0 segment data (if any bytes remaining)
                    let remaining_sof0_data_size = sof0_data_size.checked_sub(5).unwrap_or(0); // Remaining bytes after reading dimensions
                    if remaining_sof0_data_size > 0 {
                         self.reader.seek(SeekFrom::Current(remaining_sof0_data_size as i64)).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use reader.seek
                    }


                    // Found SOF0 and extracted dimensions, break loop
                    break;
                 }
                 0xD9 => { // EOI marker (End of Image)
                    // Reached end of image before finding SOF0
                    return Err(map_jpeg_error_to_fs_error(JpegError::SOF0NotFound));
                 }
                 _ => { // Skip other segments (APPn, DQT, DHT, etc.)
                    // These segments have a length field after the marker (FF xx).
                    // Read segment length (2 bytes, Big Endian) - length includes the 2 bytes of the length field itself
                    let segment_length = self.reader.read_u16::<BigEndian>().map_err(|e| map_core_io_error_to_fs_error(e))? as usize;

                     // Minimum segment length is 2 bytes (for the length field itself).
                     // Segments like COM, APPn, DQT, DHT have length >= 2 + data.
                     if segment_length < 2 {
                           return Err(map_jpeg_error_to_fs_error(JpegError::InvalidSegmentLength(segment_length as u16, String::from("unknown")))); // Requires alloc
                     }


                    // Skip the rest of the segment data (length - 2 bytes)
                    let skip_length = segment_length.checked_sub(2).unwrap_or(0); // Size of the data part
                    if skip_length > 0 {
                        self.reader.seek(SeekFrom::Current(skip_length as i64)).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use reader.seek
                    }
                 }
            }
        }

        // After loop, check if SOF0 was found and dimensions are valid
        if !found_sof0 || width == 0 || height == 0 {
             return Err(map_jpeg_error_to_fs_error(JpegError::InvalidDimensions));
        }


        Ok(JpegImage { width, height })
    }

    // You can add other methods here for reading pixel data, other segments, etc.
    // fn read_scan_data(&mut self) -> Result<(), FileSystemError> { ... }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for JpegParser<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the JpegParser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: JpegParser drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a JPEG file from the given path (std) or resource ID (no_std)
/// and parses its header to extract dimensions.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the JpegImage metadata or a FileSystemError.
#[cfg(feature = "std")]
pub fn parse_jpeg_file<P: AsRef<Path>>(file_path: P) -> Result<JpegImage, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create a JpegParser with the reader
    let mut parser = JpegParser::from_reader(reader, None, file_size); // Pass None for handle in std version

    // Parse the header using the parser
    parser.parse_header()
}

#[cfg(not(feature = "std"))]
pub fn parse_jpeg_file(file_path: &str) -> Result<JpegImage, FileSystemError> { // Return FileSystemError
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

    // Create a JpegParser with the reader
    let mut parser = JpegParser::from_reader(reader, Some(handle), file_size); // Pass the handle to the parser

    // Parse the header using the parser
    parser.parse_header()
}


// Example main function (no_std)
#[cfg(feature = "example_jpg")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("JPEG parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy JPEG file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/image.jpg" exists.
      let jpeg_res = parse_jpeg_file("sahne://files/image.jpg");
      match jpeg_res {
          Ok(image) => {
              crate::println!("Parsed JPEG dimensions: Width={}, Height={}", image.width, image.height);
          },
          Err(e) => crate::eprintln!("Error parsing JPEG file: {:?}", e),
      }

     eprintln!("JPEG parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_jpg")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("JPEG parser example (std) starting...");
     eprintln!("JPEG parser example (std) using header parsing.");

     // This example needs a dummy JPEG file.
     use std::fs::remove_file;
     use std::io::Write;
      use byteorder::BigEndian as StdBigEndian;
      use byteorder::WriteBytesExt as StdWriteBytesExt;
      use std::io::Cursor;


     let jpeg_path = Path::new("example.jpg");

     // Create a minimal valid JPEG file for testing in std environment
     // (Copied from test data, adapted to use std Write/Byteorder)
     let mut jpeg_bytes_cursor = Cursor::new(Vec::new());
      // SOI
      jpeg_bytes_cursor.write_all(&[0xFF, 0xD8]).unwrap();
      // APP0 segment (minimal JFIF header - not strictly necessary for basic decode)
      jpeg_bytes_cursor.write_all(&[0xFF, 0xE0]).unwrap(); // APP0 marker
      jpeg_bytes_cursor.write_u16::<StdBigEndian>(16).unwrap(); // Length (16 bytes, Big Endian)
      jpeg_bytes_cursor.write_all(&[0x4A, 0x46, 0x49, 0x46, 0x00]).unwrap(); // JFIF identifier
      jpeg_bytes_cursor.write_all(&[0x01, 0x01]).unwrap(); // JFIF version 1.1
      jpeg_bytes_cursor.write_u8(0x00).unwrap(); // Density units
      jpeg_bytes_cursor.write_u16::<StdBigEndian>(0x0001).unwrap(); // X density
      jpeg_bytes_cursor.write_u16::<StdBigEndian>(0x0001).unwrap(); // Y density
      jpeg_bytes_cursor.write_u8(0x00).unwrap(); // Thumbnail width
      jpeg_bytes_cursor.write_u8(0x00).unwrap(); // Thumbnail height (Correction: Thumbnail size is 2 bytes each) - Minimal JFIF ends after density. Let's fix this.

       // Minimal JFIF ends after density units (1 byte). Total 2 + 2 + 5 + 2 + 1 + 2 + 2 = 16. Correct.
       // The original APP0 test data was 16 bytes including marker and length, meaning 12 bytes of data.
       // Marker (2) + Length (2) = 4. Data = 12. JFIF (5) + Version (2) + Density Units (1) + X Density (2) + Y Density (2) = 12. Correct.
       // The original dummy APP0 was correct.

       // SOF0 segment (Start of Frame 0) - Minimal version
       jpeg_bytes_cursor.write_all(&[0xFF, 0xC0]).unwrap(); // SOF0 marker
       jpeg_bytes_cursor.write_u16::<StdBigEndian>(17).unwrap(); // Length (17 bytes, Big Endian) - 2 marker + 2 length + 1 precision + 2 height + 2 width + 6 components = 15? No, components depends on num_components * 3 bytes.
       // SOF0 segment data: 1 byte precision, 2 bytes height, 2 bytes width, (num_components * 3) bytes component data.
       // Minimal SOF0 length is 8 (2 marker + 2 length + 1 precision + 2 height + 2 width - min 1 component * 3 bytes).
       // For 3 components: 2 + 2 + 1 + 2 + 2 + (3 * 3) = 18. The length 17 in the original test data seems wrong.
       // Let's use a length of 18 for a minimal 3-component SOF0.
        jpeg_bytes_cursor.write_u16::<StdBigEndian>(18).unwrap(); // Length (18 bytes)
        jpeg_bytes_cursor.write_u8(0x08).unwrap();       // Sample precision (8 bits)
        jpeg_bytes_cursor.write_u16::<StdBigEndian>(0x00A0).unwrap(); // Height (160 pixels)
        jpeg_bytes_cursor.write_u16::<StdBigEndian>(0x00C8).unwrap(); // Width (200 pixels)
        jpeg_bytes_cursor.write_u8(0x03).unwrap();       // Number of components (3 - YCbCr)
        jpeg_bytes_cursor.write_all(&[0x01, 0x22, 0x00]).unwrap(); // Component 1: Y, sampling factors 2x2, quantization table 0
        jpeg_bytes_cursor.write_all(&[0x02, 0x11, 0x01]).unwrap(); // Component 2: Cb, sampling factors 1x1, quantization table 1
        jpeg_bytes_cursor.write_all(&[0x03, 0x11, 0x01]).unwrap(); // Component 3: Cr, sampling factors 1x1, quantization table 1


       // Minimal SOS (Start of Scan) - Just header
       jpeg_bytes_cursor.write_all(&[0xFF, 0xDA]).unwrap(); // SOS marker
        jpeg_bytes_cursor.write_u16::<StdBigEndian>(12).unwrap(); // Length (12 bytes)
        jpeg_bytes_cursor.write_u8(0x03).unwrap(); // Number of components in scan (3)
        jpeg_bytes_cursor.write_all(&[0x01, 0x00]).unwrap(); // Component 1: ID 1, table 0
        jpeg_bytes_cursor.write_all(&[0x02, 0x11]).unwrap(); // Component 2: ID 2, table 1
        jpeg_bytes_cursor.write_all(&[0x03, 0x11]).unwrap(); // Component 3: ID 3, table 1
        jpeg_bytes_cursor.write_all(&[0x00, 0x3F, 0x00]).unwrap(); // Spectral selection start, spectral selection end, approximation bit position (default)


       // EOI
       jpeg_bytes_cursor.write_all(&[0xFF, 0xD9]).unwrap();


       let dummy_data = jpeg_bytes_cursor.into_inner();


       // Write dummy data to a temporary file for std test
        match File::create(jpeg_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_data) {
                       eprintln!("Error writing dummy JPEG file: {}", e);
                       return Err(map_std_io_error_to_fs_error(e));
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy JPEG file: {}", e);
                  return Err(map_std_io_error_to_fs_error(e));
             }
        }


     match parse_jpeg_file(jpeg_path) { // Call the function that opens and parses
         Ok(image) => {
             println!("Parsed JPEG dimensions: Width={}, Height={}", image.width, image.height);
              // Verify the dimensions match the dummy data
              assert_eq!(image.width, 200);
              assert_eq!(image.height, 160);
         }
         Err(e) => {
              eprintln!("Error parsing JPEG file: {}", e); // std error display
              // Don't return error, let cleanup run
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(jpeg_path) {
          eprintln!("Error removing dummy JPEG file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("JPEG parser example (std) finished.");

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
      use byteorder::{BigEndian as StdBigEndian, WriteBytesExt as StdWriteBytesExt}; // Use std byteorder for writing test data


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests


     // Helper function to create dummy JPEG bytes with specific segments
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
      fn create_dummy_jpeg_bytes(segments_data: &[(u8, Option<Vec<u8>>)]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
          let mut buffer = Cursor::new(Vec::new());
           buffer.write_all(&[0xFF, 0xD8])?; // SOI marker

           for (marker_byte, data_option) in segments_data {
                buffer.write_all(&[0xFF, *marker_byte])?; // Segment marker (FF xx)

               // Segments like EOI (D9) have no length or data. Other markers typically have length + data.
               if *marker_byte != 0xD9 && (*marker_byte < 0xD0 || *marker_byte > 0xD7) { // Exclude RSTn markers (D0-D7) which also have no length/data
                   let data = data_option.as_ref().ok_or_else(|| {
                       Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Segment with marker {:x} requires data", marker_byte)))
                   })?;
                   let segment_length = (2 + data.len()) as u16; // Length includes the 2-byte length field itself
                    buffer.write_u16::<StdBigEndian>(segment_length)?; // Write length
                    buffer.write_all(data)?; // Write data
               }
                // For RSTn markers (D0-D7) and EOI (D9), just the marker is present.
                // For SOS (DA), the length is present, then the header, then compressed data (not handled here).

           }

           // Add EOI if not already present and required (for a valid file structure)
           if !segments_data.iter().any(|(m, _)| *m == 0xD9) {
              buffer.write_all(&[0xFF, 0xD9])?; // EOI marker
           }


           Ok(buffer.into_inner())
      }


     // Test parsing a minimal valid JPEG header (SOI, SOF0, EOI)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_parse_jpeg_minimal_valid() -> Result<(), FileSystemError> { // Return FileSystemError

          // Create dummy minimal JPEG bytes: SOI, SOF0 (with dimensions), EOI
           let sof0_data: Vec<u8> = vec![
               0x08, // Sample precision (1 byte)
               0x00, 0xA0, // Height (2 bytes, Big Endian = 160)
               0x00, 0xC8, // Width (2 bytes, Big Endian = 200)
               0x03, // Number of components (1 byte)
               // Component data (num_components * 3 bytes)
               0x01, 0x22, 0x00, // Component 1
               0x02, 0x11, 0x01, // Component 2
               0x03, 0x11, 0x01, // Component 3
           ]; // Total SOF0 data size: 1 + 2 + 2 + 1 + (3 * 3) = 15. Length field would be 15 + 2 = 17.

           let segments = vec![
               (0xC0, Some(sof0_data.clone())), // SOF0 segment
               (0xD9, None), // EOI marker
           ];

           let dummy_jpeg_bytes = create_dummy_jpeg_bytes(&segments)
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?; // SOI added by helper


          // Use Cursor as a reader
          let file_size = dummy_jpeg_bytes.len() as u64;
          let cursor = Cursor::new(dummy_jpeg_bytes.clone());

          // Create a JpegParser with the cursor reader
          let mut parser = JpegParser::from_reader(cursor, None, file_size);

          // Parse the header
          let image = parser.parse_header()?;

          // Assert dimensions are correct
          assert_eq!(image.width, 200);
          assert_eq!(image.height, 160);

          // Verify the reader is positioned at the end of the SOF0 segment data
          // SOI (2) + SOF0 marker (2) + SOF0 length (2) + SOF0 data (15) = 21.
          // After parsing SOF0, the reader should be positioned after SOF0 data, which is at offset 21.
          assert_eq!(parser.reader.stream_position().unwrap(), (2 + 2 + 2 + sof0_data.len()) as u64);


          Ok(())
     }

     // Test parsing a JPEG with other segments before SOF0
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_jpeg_with_other_segments() -> Result<(), FileSystemError> { // Return FileSystemError

           // Create dummy JPEG bytes: SOI, APP0, DQT, SOF0, EOI
           let app0_data = vec![0u8; 10]; // APP0 length 12 (2 marker + 2 length + 8 data)
           let dqt_data = vec![0u8; 10]; // DQT length 12
           let sof0_data: Vec<u8> = vec![
               0x08, 0x00, 0xA0, 0x00, 0xC8, 0x03, // Precision, Height, Width, NumComponents
               0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, // Component data
           ]; // SOF0 length 17 (2 marker + 2 length + 13 data)


           let segments = vec![
               (0xE0, Some(app0_data)), // APP0 segment
               (0xDB, Some(dqt_data)), // DQT segment
               (0xC0, Some(sof0_data.clone())), // SOF0 segment
               (0xD9, None), // EOI marker
           ];

           let dummy_jpeg_bytes = create_dummy_jpeg_bytes(&segments)
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


           // Use Cursor as a reader
           let file_size = dummy_jpeg_bytes.len() as u64;
           let cursor = Cursor::new(dummy_jpeg_bytes.clone());

           // Create a JpegParser
           let mut parser = JpegParser::from_reader(cursor, None, file_size);

           // Parse the header
           let image = parser.parse_header()?;

           // Assert dimensions are correct (from SOF0)
           assert_eq!(image.width, 200);
           assert_eq!(image.height, 160);

           // Verify the reader is positioned at the end of the SOF0 segment data
           // SOI (2) + APP0 (2+2+10) + DQT (2+2+10) + SOF0 marker (2) + SOF0 length (2) + SOF0 data (15) = 2 + 14 + 14 + 2 + 2 + 15 = 49.
           assert_eq!(parser.reader.stream_position().unwrap(), (2 + 14 + 14 + 17) as u64); // Total size up to end of SOF0


           Ok(())
      }

     // Test handling of invalid SOI marker
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_jpeg_invalid_soi() {
           // Create dummy bytes with invalid SOI marker
           let dummy_bytes = vec![0xFF, 0x00, 0xFF, 0xD8]; // Invalid SOI

           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes);
           let mut parser = JpegParser::from_reader(cursor, None, file_size);

           // Attempt to parse, expect an error
           let result = parser.parse_header();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from JpegError::InvalidSOIMarker
                   assert!(msg.contains("Geçersiz SOI marker"));
                   assert!(msg.contains("ff00")); // Hex representation
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of unexpected EOF during SOI reading
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_parse_jpeg_truncated_soi() {
            // Truncated SOI (1 byte)
            let dummy_bytes = vec![0xFF];

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            let mut parser = JpegParser::from_reader(cursor, None, file_size);

            // Attempt to parse, expect an error
            let result = parser.parse_header();
            assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
       }

      // Test handling of unexpected EOF during marker reading
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_parse_jpeg_truncated_marker() {
            // SOI + truncated marker (1 byte)
            let dummy_bytes = vec![0xFF, 0xD8, 0xFF];

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            let mut parser = JpegParser::from_reader(cursor, None, file_size);

            // Attempt to parse, expect an error
            let result = parser.parse_header();
            assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from JpegError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
       }

       // Test handling of invalid marker byte 0 (not FF)
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_parse_jpeg_invalid_marker_byte0() {
            // SOI + invalid marker (00 E0)
            let dummy_bytes = vec![0xFF, 0xD8, 0x00, 0xE0];

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            let mut parser = JpegParser::from_reader(cursor, None, file_size);

            // Attempt to parse, expect an error
            let result = parser.parse_header();
            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from JpegError::InvalidMarker
                     assert!(msg.contains("Geçersiz JPEG marker"));
                     assert!(msg.contains("00e0"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }

       // Test handling of unexpected EOF during segment length reading
        #[test]
        #[cfg(feature = "std")] // Run this test only with std feature
        fn test_parse_jpeg_truncated_segment_length() {
             // SOI + APP0 marker + truncated length (1 byte)
             let dummy_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00];

             let file_size = dummy_bytes.len() as u64;
             let cursor = Cursor::new(dummy_bytes);
             let mut parser = JpegParser::from_reader(cursor, None, file_size);

             // Attempt to parse, expect an error
             let result = parser.parse_header();
             assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_u16)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }
        }

        // Test handling of unexpected EOF during segment data reading
         #[test]
         #[cfg(feature = "std")] // Run this test only with std feature
         fn test_parse_jpeg_truncated_segment_data() {
              // SOI + APP0 marker + length (10 bytes) + truncated data (5 bytes instead of 8)
              let mut dummy_bytes_cursor = Cursor::new(Vec::new());
               dummy_bytes_cursor.write_all(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap(); // SOI + APP0 marker
               dummy_bytes_cursor.write_u16::<StdBigEndian>(10).unwrap(); // Length (10 bytes) -> 8 bytes data
               dummy_bytes_cursor.write_all(&[1, 2, 3, 4, 5]).unwrap(); // 5 bytes data (truncated)
              let dummy_bytes = dummy_bytes_cursor.into_inner(); // Total 2 + 2 + 2 + 5 = 11 bytes. Length says 10. Inconsistent but tests read_exact.

              let file_size = dummy_bytes.len() as u64;
              let cursor = Cursor::new(dummy_bytes);
              let mut parser = JpegParser::from_reader(cursor, None, file_size);

              // Attempt to parse, expect an error during segment data reading (read_exact will fail)
              let result = parser.parse_header();
              assert!(result.is_err());
               match result.unwrap_err() {
                  FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact)
                     assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                  },
                  _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
               }
         }


        // Test handling of missing SOF0 marker before EOI
         #[test]
         #[cfg(feature = "std")] // Run this test only with std feature
         fn test_parse_jpeg_missing_sof0() {
              // SOI + APP0 + EOI (no SOF0)
              let app0_data = vec![0u8; 10];
              let segments = vec![
                  (0xE0, Some(app0_data)), // APP0 segment
                  (0xD9, None), // EOI marker
              ];
              let dummy_jpeg_bytes = create_dummy_jpeg_bytes(&segments)
                  .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;

              let file_size = dummy_jpeg_bytes.len() as u64;
              let cursor = Cursor::new(dummy_jpeg_bytes);
              let mut parser = JpegParser::from_reader(cursor, None, file_size);

              // Attempt to parse, expect an error
              let result = parser.parse_header();
              assert!(result.is_err());
               match result.unwrap_err() {
                  FileSystemError::InvalidData(msg) => { // Mapped from JpegError::SOF0NotFound
                      assert!(msg.contains("SOF0 markerı EOI'dan önce bulunamadı"));
                  },
                  _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
               }
         }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This involves simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include opening valid/invalid files, handling IO errors,
     // and correctly parsing headers from mock data.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_jpg", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
