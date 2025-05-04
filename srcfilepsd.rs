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
use alloc::vec::Vec; // For temporary buffers, dummy data
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


/// Custom error type for PSD parsing issues.
#[derive(Debug)]
pub enum PsdError {
    UnexpectedEof(String), // During header or section reading
    InvalidSignature([u8; 4]), // Expected '8BPS'
    UnsupportedVersion(u16), // Expected 1 or 2
    UnsupportedDepth(u16), // Expected 1, 8, 16, or 32
    InvalidHeaderSize, // Read incorrect number of bytes for the header
    SeekError(u64), // Failed to seek
    // Add other PSD specific parsing errors here (e.g., invalid section lengths)
}

// Implement Display for PsdError
impl fmt::Display for PsdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PsdError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            PsdError::InvalidSignature(signature) => write!(f, "Geçersiz PSD imzası: {:x?}", signature),
            PsdError::UnsupportedVersion(version) => write!(f, "Desteklenmeyen PSD sürümü: {}", version),
            PsdError::UnsupportedDepth(depth) => write!(f, "Desteklenmeyen bit derinliği: {}", depth),
            PsdError::InvalidHeaderSize => write!(f, "Geçersiz PSD başlık boyutu"),
            PsdError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map PsdError to FileSystemError
fn map_psd_error_to_fs_error(e: PsdError) -> FileSystemError {
    match e {
        PsdError::UnexpectedEof(_) | PsdError::SeekError(_) => FileSystemError::IOError(format!("PSD IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("PSD ayrıştırma hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilepng.rs'den kopyalandı)
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


// Removed redundant arch, SahneError, syscall, fs module definitions.
// Removed redundant memory, process, sync, ipc, kernel modules.
// Removed redundant print module and panic handler.


/// Basic PSD file header structure.
#[derive(Debug, PartialEq, Eq)] // Add PartialEq, Eq for tests
pub struct PsdHeader {
    pub signature: [u8; 4], // Always '8BPS'
    pub version: u16, // Always 1 or 2
    reserved: [u8; 6], // Must be zero
    pub channels: u16, // Number of color channels
    pub height: u32, // Height of the image in pixels
    pub width: u32, // Width of the image in pixels
    pub depth: u16, // Bits per channel (1, 8, 16, or 32)
    pub color_mode: u16, // Color mode of the file
}

// PSD Signature
const PSD_SIGNATURE: [u8; 4] = *b"8BPS";

/// PSD file parser. Reads the header and provides access to the underlying reader
/// for potential future parsing of other sections.
pub struct Psd<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size

    pub header: PsdHeader, // Store the parsed header
}

impl<R: Read + Seek> Psd<R> {
    /// Creates a new `Psd` instance by reading the file header from the specified reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        let header = Self::read_header(&mut reader)?; // Read header from the reader

        Ok(Psd {
            reader, // Store the reader
            handle,
            file_size,
            header, // Store the parsed header
        })
    }

    /// Reads and parses the PSD file header from the reader.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read + Seek.
    ///
    /// # Returns
    ///
    /// A Result containing the PsdHeader or a FileSystemError if parsing fails.
    fn read_header(reader: &mut R) -> Result<PsdHeader, FileSystemError> { // Return FileSystemError
        let mut header = PsdHeader {
            signature: [0u8; 4],
            version: 0,
            reserved: [0u8; 6],
            channels: 0,
            height: 0,
            width: 0,
            depth: 0,
            color_mode: 0,
        };

        // Signature (4 bytes): Always '8BPS' (Big Endian implicit for fixed size)
        reader.read_exact(&mut header.signature).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("signature"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        if header.signature != PSD_SIGNATURE {
             return Err(map_psd_error_to_fs_error(PsdError::InvalidSignature(header.signature)));
        }

        // Version (2 bytes): Always 1 or 2 (Big Endian)
        header.version = reader.read_u16::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("version"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        if header.version != 1 && header.version != 2 {
             return Err(map_psd_error_to_fs_error(PsdError::UnsupportedVersion(header.version)));
        }

        // Reserved (6 bytes): Must be zero. Read and validate (optional check).
        reader.read_exact(&mut header.reserved).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("reserved bytes"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        // Optional: Check if reserved bytes are all zero: if header.reserved != [0; 6] { /* Handle error */ }

        // Channels (2 bytes): Number of color channels (Big Endian)
        header.channels = reader.read_u16::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("channels"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Height (4 bytes): Height of the image in pixels (Big Endian)
        header.height = reader.read_u32::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("height"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Width (4 bytes): Width of the image in pixels (Big Endian)
        header.width = reader.read_u32::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("width"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Depth (2 bytes): Bits per channel (1, 8, 16, or 32) (Big Endian)
        header.depth = reader.read_u16::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("depth"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        if ![1, 8, 16, 32].contains(&header.depth) {
             return Err(map_psd_error_to_fs_error(PsdError::UnsupportedDepth(header.depth)));
        }

        // Color Mode (2 bytes): Color mode of the file (Big Endian)
        header.color_mode = reader.read_u16::<BigEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_psd_error_to_fs_error(PsdError::UnexpectedEof(String::from("color mode"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        // You might want to validate color_mode against known values if needed


        // The header section ends after the color mode.
        // The next section is the Color Mode Data section, which is variable size.
        // A full parser would read its length and data, then move to Image Resources, etc.
        // For this header parser, we stop here, leaving the reader positioned
        // at the start of the Color Mode Data section.

        Ok(header)
    }

    /// Provides a mutable reference to the internal reader. Use with caution.
    /// This allows reading subsequent sections of the PSD file.
     pub fn reader(&mut self) -> &mut R {
         &mut self.reader
     }

    // Add other methods here for parsing other sections (Color Mode Data, Image Resources, etc.)
     pub fn read_color_mode_data(&mut self) -> Result<Vec<u8>, FileSystemError> { ... }
     pub fn read_image_resources(&mut self) -> Result<ImageResources, FileSystemError> { ... }
     pub fn read_layer_and_mask_info(&mut self) -> Result<LayerAndMaskInfo, FileSystemError> { ... }
     pub fn read_image_data(&mut self) -> Result<ImageData, FileSystemError> { ... }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for Psd<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the Psd is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: Psd drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a PSD file from the given path (std) or resource ID (no_std)
/// and parses its header.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the parsed Psd struct with the header or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_psd_file<P: AsRef<Path>>(file_path: P) -> Result<Psd<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create a Psd parser by reading the header from the reader
    Psd::from_reader(reader, None, file_size) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_psd_file(file_path: &str) -> Result<Psd<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    // Create a Psd parser by reading the header from the reader
    Psd::from_reader(reader, Some(handle), file_size) // Pass the handle to the Psd struct
}


// Example main function (no_std)
#[cfg(feature = "example_psd")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("PSD header parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy PSD file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Create dummy example.psd data bytes for the mock filesystem
     let example_psd_data: Vec<u8> = vec![
         0x38, 0x42, 0x50, 0x53, // Signature "8BPS"
         0x00, 0x01,             // Version 1
         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Reserved
         0x00, 0x03,             // Channels (3)
         0x00, 0x00, 0x01, 0x00, // Height (256)
         0x00, 0x00, 0x01, 0x00, // Width (256)
         0x00, 0x08,             // Depth (8 bits)
         0x00, 0x03,             // Color Mode (CMYK)
          // Add dummy data for Color Mode Data section (4 bytes length, then data)
          0x00, 0x00, 0x00, 0x00, // Length 0
          // Add dummy data for Image Resources section (4 bytes length, then data)
          0x00, 0x00, 0x00, 0x00, // Length 0
          // Add dummy data for Layer and Mask Information section (4 bytes length, then data, or 8 bytes length for Version 2 files)
          0x00, 0x00, 0x00, 0x00, // Length 0 (for Version 1)
          // Add dummy data for Image Data section (2 bytes compression, then data)
          0x00, 0x00, // Compression method (0 = raw)
          // Image data would follow here (dummy bytes)
          0x01, 0x02, 0x03, 0x04 // Dummy pixel data
     ];
      // Assuming the mock filesystem is set up to provide this data for "sahne://files/example.psd"

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/example.psd" exists with the dummy data.
      let psd_res = open_psd_file("sahne://files/example.psd");
      match psd_res {
          Ok(mut psd) => { // Need mut if reading further data
              crate::println!("PSD file loaded (header parsed).");
              crate::println!(" Header: {:?}", psd.header); // Requires Debug on PsdHeader
              crate::println!(" Dimensions: {}x{}", psd.header.width, psd.header.height);
              crate::println!(" Channels: {}", psd.header.channels);
              crate::println!(" Depth: {}", psd.header.depth);
              crate::println!(" Color Mode: {}", psd.header.color_mode);
     //
     //         // Example: Read Color Mode Data (requires a read_color_mode_data method)
               match psd.read_color_mode_data() {
                   Ok(data) => crate::println!("Read {} bytes of Color Mode Data.", data.len()),
                   Err(e) => crate::eprintln!("Error reading Color Mode Data: {:?}", e),
               }
     
              // File is automatically closed when psd goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening PSD file: {:?}", e),
      }

     eprintln!("PSD header parser example (no_std) needs Sahne64 mocks to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The dummy PSD data to be available at the specified path.

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_psd")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("PSD header parser example (std) starting...");
     eprintln!("PSD header parser example (std) using header parsing.");

     // Create dummy example.psd data bytes
     let example_psd_data: Vec<u8> = vec![
         0x38, 0x42, 0x50, 0x53, // Signature "8BPS"
         0x00, 0x01,             // Version 1
         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Reserved
         0x00, 0x03,             // Channels (3)
         0x00, 0x00, 0x01, 0x00, // Height (256)
         0x00, 0x00, 0x01, 0x00, // Width (256)
         0x00, 0x08,             // Depth (8 bits)
         0x00, 0x03,             // Color Mode (CMYK)
          // Add dummy data for Color Mode Data section (4 bytes length, then data)
          0x00, 0x00, 0x00, 0x00, // Length 0
          // Add dummy data for Image Resources section (4 bytes length, then data)
          0x00, 0x00, 0x00, 0x00, // Length 0
          // Add dummy data for Layer and Mask Information section (4 bytes length, then data, or 8 bytes length for Version 2 files)
          0x00, 0x00, 0x00, 0x00, // Length 0 (for Version 1)
          // Add dummy data for Image Data section (2 bytes compression, then data)
          0x00, 0x00, // Compression method (0 = raw)
          // Image data would follow here (dummy bytes)
          0x01, 0x02, 0x03, 0x04 // Dummy pixel data
     ];


     let file_path = Path::new("example.psd");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&example_psd_data).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy PSD file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy PSD file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_psd_file(file_path) { // Call the function that opens and parses header
         Ok(mut psd) => { // Need mut if reading further data
             println!("PSD file loaded (header parsed).");
             println!(" Header: {:?}", psd.header); // Requires Debug on PsdHeader
             println!(" Dimensions: {}x{}", psd.header.width, psd.header.height);
             println!(" Channels: {}", psd.header.channels);
             println!(" Depth: {}", psd.header.depth);
             println!(" Color Mode: {}", psd.header.color_mode);

             // Assert basic header fields based on dummy data
             assert_eq!(psd.header.signature, *b"8BPS");
             assert_eq!(psd.header.version, 1);
             assert_eq!(psd.header.channels, 3);
             assert_eq!(psd.header.height, 256);
             assert_eq!(psd.header.width, 256);
             assert_eq!(psd.header.depth, 8);
             assert_eq!(psd.header.color_mode, 3);


             // Example: Read the length of the next section (Color Mode Data)
             // The reader is positioned right after the header (26 bytes).
             // The next 4 bytes are the length of the Color Mode Data section.
             match psd.reader.read_u32::<BigEndian>() {
                  Ok(color_mode_data_length) => {
                       println!("Color Mode Data Section Length: {}", color_mode_data_length);
                       // In our dummy data, this length is 0.
                       assert_eq!(color_mode_data_length, 0);
                  },
                  Err(e) => eprintln!("Error reading Color Mode Data Length: {:?}", e), // core::io::Error display
             }


             // Example: Read the length of the Image Resources section
             // After Color Mode Data (length 0), the reader is positioned at the start of Image Resources.
             // The next 4 bytes are the length of the Image Resources section.
             match psd.reader.read_u32::<BigEndian>() {
                  Ok(image_resources_length) => {
                       println!("Image Resources Section Length: {}", image_resources_length);
                       // In our dummy data, this length is 0.
                       assert_eq!(image_resources_length, 0);
                  },
                  Err(e) => eprintln!("Error reading Image Resources Length: {:?}", e), // core::io::Error display
             }


             // Example: Read the length of the Layer and Mask Information section
             // After Image Resources (length 0), the reader is positioned at the start of Layer and Mask Info.
             // The next 4 bytes are the length of this section (or 8 for Version 2).
              match psd.reader.read_u32::<BigEndian>() { // Assuming Version 1 for dummy
                   Ok(layer_mask_info_length) => {
                        println!("Layer and Mask Info Section Length: {}", layer_mask_info_length);
                        // In our dummy data (Version 1), this length is 0.
                        assert_eq!(layer_mask_info_length, 0);
                   },
                   Err(e) => eprintln!("Error reading Layer and Mask Info Length: {:?}", e), // core::io::Error display
              }


             // Example: Read the Compression Method of the Image Data section
             // After Layer and Mask Info (length 0), the reader is positioned at the start of Image Data.
             // The next 2 bytes are the compression method.
             match psd.reader.read_u16::<BigEndian>() {
                  Ok(compression_method) => {
                       println!("Image Data Compression Method: {}", compression_method);
                       // In our dummy data, this is 0 (raw).
                       assert_eq!(compression_method, 0);
                  },
                  Err(e) => eprintln!("Error reading Image Data Compression Method: {:?}", e), // core::io::Error display
             }

             // File is automatically closed when psd goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening PSD file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy PSD file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("PSD header parser example (std) finished.");

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


     // Helper function to create dummy PSD header bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn create_dummy_psd_header_bytes(version: u16, channels: u16, height: u32, width: u32, depth: u16, color_mode: u16) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(Vec::new());
           // Signature
           buffer.write_all(&PSD_SIGNATURE)?;
           // Version
           buffer.write_u16::<BigEndian>(version)?;
           // Reserved (6 bytes)
           buffer.write_all(&[0u8; 6])?;
           // Channels
           buffer.write_u16::<BigEndian>(channels)?;
           // Height
           buffer.write_u32::<BigEndian>(height)?;
           // Width
           buffer.write_u32::<BigEndian>(width)?;
           // Depth
           buffer.write_u16::<BigEndian>(depth)?;
           // Color Mode
           buffer.write_u16::<BigEndian>(color_mode)?;

           // Add minimal subsequent section headers (length 0) and image data compression method (raw)
           // This is needed so the parser can read beyond the fixed-size header.
           buffer.write_u32::<BigEndian>(0)?; // Color Mode Data length
           buffer.write_u32::<BigEndian>(0)?; // Image Resources length
           // Layer and Mask Info length (4 bytes for Version 1, 8 bytes for Version 2)
           if version == 1 {
                buffer.write_u32::<BigEndian>(0)?; // Version 1 length
           } else { // Version 2
                buffer.write_u64::<BigEndian>(0)?; // Version 2 length
           }
           // Image Data Compression Method
           buffer.write_u16::<BigEndian>(0)?; // Compression method (0 = raw)
            // Optional: Add dummy image data bytes here if testing reading image data section later.
            // For header test, we don't need actual image data.


           Ok(buffer.into_inner())
       }


     // Test parsing a valid PSD header in memory (Version 1)
     #[test]
     fn test_parse_psd_header_valid_v1_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
          // Create dummy PSD header bytes (Version 1)
          let dummy_psd_bytes = create_dummy_psd_header_bytes(
              1, 3, 256, 256, 8, 3 // Version 1, 3 channels, 256x256, 8 depth, CMYK
          ).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


          // Use Cursor as a Read + Seek reader
          let file_size = dummy_psd_bytes.len() as u64;
          let cursor = Cursor::new(dummy_psd_bytes.clone()); // Clone for potential re-reads in test

          // Create a Psd parser by reading the header from the reader
          let mut psd = Psd::from_reader(cursor, None, file_size)?; // Pass None for handle

          // Assert header fields are correct
          assert_eq!(psd.header.signature, *b"8BPS");
          assert_eq!(psd.header.version, 1);
          assert_eq!(psd.header.channels, 3);
          assert_eq!(psd.header.height, 256);
          assert_eq!(psd.header.width, 256);
          assert_eq!(psd.header.depth, 8);
          assert_eq!(psd.header.color_mode, 3);

          // Verify the reader is positioned after the fixed-size header (26 bytes)
          /// Signature (4) + Version (2) + Reserved (6) + Channels (2) + Height (4) + Width (4) + Depth (2) + Color Mode (2) = 26
          assert_eq!(psd.reader.stream_position().unwrap(), 26);


          Ok(())
     }

      // Test parsing a valid PSD header in memory (Version 2)
       #[test]
       fn test_parse_psd_header_valid_v2_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
            // Create dummy PSD header bytes (Version 2)
            let dummy_psd_bytes = create_dummy_psd_header_bytes(
                2, 4, 512, 512, 16, 1 // Version 2, 4 channels, 512x512, 16 depth, RGB
            ).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


           // Use Cursor as a Read + Seek reader
           let file_size = dummy_psd_bytes.len() as u64;
           let cursor = Cursor::new(dummy_psd_bytes.clone());

           // Create a Psd parser by reading the header from the reader
           let mut psd = Psd::from_reader(cursor, None, file_size)?;

           // Assert header fields are correct
           assert_eq!(psd.header.signature, *b"8BPS");
           assert_eq!(psd.header.version, 2);
           assert_eq!(psd.header.channels, 4);
           assert_eq!(psd.header.height, 512);
           assert_eq!(psd.header.width, 512);
           assert_eq!(psd.header.depth, 16);
           assert_eq!(psd.header.color_mode, 1);

            // Verify the reader is positioned after the fixed-size header (26 bytes)
           assert_eq!(psd.reader.stream_position().unwrap(), 26);


           Ok(())
       }


     // Test handling of invalid signature
      #[test]
      fn test_parse_psd_header_invalid_signature() {
           // Create dummy bytes with invalid signature
           let mut dummy_bytes_cursor = Cursor::new(Vec::new());
           dummy_bytes_cursor.write_all(b"BAD!").unwrap(); // Invalid signature
            // Add rest of header bytes (22 bytes) + minimal subsequent headers/data (enough to pass length checks)
            dummy_bytes_cursor.write_all(&[0u8; 22]).unwrap();
            dummy_bytes_cursor.write_all(&[0u8; 4+4+4+2]).unwrap(); // Minimal subsequent headers/data

           let dummy_bytes = dummy_bytes_cursor.into_inner();

           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes);
           // Attempt to create Psd parser, expect an error during header reading
           let result = Psd::from_reader(cursor, None, file_size);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from PsdError::InvalidSignature
                   assert!(msg.contains("Geçersiz PSD imzası"));
                   assert!(msg.contains("42414421")); // Hex representation of "BAD!"
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of unsupported version
       #[test]
       fn test_parse_psd_header_unsupported_version() {
            // Valid signature + unsupported version (e.g., 3)
            let mut dummy_bytes_cursor = Cursor::new(Vec::new());
            dummy_bytes_cursor.write_all(&PSD_SIGNATURE).unwrap();
            dummy_bytes_cursor.write_u16::<BigEndian>(3).unwrap(); // Unsupported version 3
             // Add rest of header bytes (20 bytes) + minimal subsequent headers/data
            dummy_bytes_cursor.write_all(&[0u8; 20]).unwrap();
             dummy_bytes_cursor.write_all(&[0u8; 4+4+4+2]).unwrap(); // Minimal subsequent headers/data


            let dummy_bytes = dummy_bytes_cursor.into_inner();

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            let result = Psd::from_reader(cursor, None, file_size);

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from PsdError::UnsupportedVersion
                    assert!(msg.contains("Desteklenmeyen PSD sürümü: 3"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }

       // Test handling of unsupported depth
        #[test]
        fn test_parse_psd_header_unsupported_depth() {
             // Valid signature + valid version (1) + rest of header up to depth + unsupported depth (e.g., 4)
             let mut dummy_bytes_cursor = Cursor::new(Vec::new());
             dummy_bytes_cursor.write_all(&PSD_SIGNATURE).unwrap();
             dummy_bytes_cursor.write_u16::<BigEndian>(1).unwrap(); // Version 1
             dummy_bytes_cursor.write_all(&[0u8; 6]).unwrap(); // Reserved
             dummy_bytes_cursor.write_u16::<BigEndian>(3).unwrap(); // Channels
             dummy_bytes_cursor.write_u32::<BigEndian>(256).unwrap(); // Height
             dummy_bytes_cursor.write_u32::<BigEndian>(256).unwrap(); // Width
             dummy_bytes_cursor.write_u16::<BigEndian>(4).unwrap(); // Unsupported depth 4
             dummy_bytes_cursor.write_u16::<BigEndian>(3).unwrap(); // Color Mode
              // Add minimal subsequent headers/data
             dummy_bytes_cursor.write_all(&[0u8; 4+4+4+2]).unwrap();


            let dummy_bytes = dummy_bytes_cursor.into_inner();

            let file_size = dummy_bytes.len() as u64;
            let cursor = Cursor::new(dummy_bytes);
            let result = Psd::from_reader(cursor, None, file_size);

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from PsdError::UnsupportedDepth
                    assert!(msg.contains("Desteklenmeyen bit derinliği: 4"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
        }


       // Test handling of unexpected EOF during header reading
        #[test]
        fn test_parse_psd_header_truncated_header() {
             // Truncated header (only first 10 bytes - Signature + Version + part of Reserved)
             let mut dummy_bytes_cursor = Cursor::new(Vec::new());
              dummy_bytes_cursor.write_all(&PSD_SIGNATURE).unwrap();
              dummy_bytes_cursor.write_u16::<BigEndian>(1).unwrap(); // Version 1
              dummy_bytes_cursor.write_all(&[0u8; 4]).unwrap(); // Part of Reserved (4 out of 6)

             let dummy_bytes = dummy_bytes_cursor.into_inner(); // Total 4 + 2 + 4 = 10 bytes

             let file_size = dummy_bytes.len() as u64;
             let cursor = Cursor::new(dummy_bytes);
             // Attempt to create Psd parser, expect an error during header reading (UnexpectedEof)
             let result = Psd::from_reader(cursor, None, file_size);
             assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from PsdError::UnexpectedEof (via read_exact/read_u16/read_u32)
                     assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                      // The message should ideally indicate which section was being read, but mapping core::io::Error doesn't provide that context easily.
                      // If using read_exact/read_u16/read_u32 directly and mapping their result with PsdError, we can add the section name.
                      // Let's check if any of the expected section names are in the error message (after refactoring).
                      assert!(msg.contains("signature") || msg.contains("version") || msg.contains("reserved bytes") ||
                              msg.contains("channels") || msg.contains("height") || msg.contains("width") ||
                              msg.contains("depth") || msg.contains("color mode"));
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
#[cfg(not(any(feature = "std", feature = "example_psd", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
