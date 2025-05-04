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
use alloc::vec::Vec; // For read buffer
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait


// core::io traits and types needed for SahneResourceReader (if used)
#[cfg(not(feature = "std"))]
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io


// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types


// Need no_std println!/eprintln! macros
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


/// Custom error type for MKV/EBML parsing issues.
#[derive(Debug)]
pub enum MkvError {
    InvalidMagicNumber([u8; 4]),
    UnexpectedEof, // During element ID or size reading
    InvalidEbmlData(String), // For VINT parsing errors or other EBML structure issues
    // Add other MKV specific parsing errors here
}

// Implement Display for MkvError
impl fmt::Display for MkvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MkvError::InvalidMagicNumber(magic) => write!(f, "Geçersiz MKV sihirli sayısı: {:x?}", magic),
            MkvError::UnexpectedEof => write!(f, "Beklenmedik dosya sonu"),
            MkvError::InvalidEbmlData(msg) => write!(f, "Geçersiz EBML verisi: {}", msg),
        }
    }
}

// Helper function to map MkvError to FileSystemError
fn map_mkv_error_to_fs_error(e: MkvError) -> FileSystemError {
    match e {
        MkvError::InvalidMagicNumber(magic) => FileSystemError::InvalidData(format!("Geçersiz MKV sihirli sayısı: {:x?}", magic)),
        MkvError::UnexpectedEof => FileSystemError::IOError(format!("Beklenmedik dosya sonu")), // Map parsing EOF to IO Error
        MkvError::InvalidEbmlData(msg) => FileSystemError::InvalidData(format!("Geçersiz EBML verisi: {}", msg)),
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilemd.rs'den kopyalandı)
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


/// Basic MKV (Matroska) file parser.
/// Focuses on reading top-level EBML elements (IDs and sizes).
/// Does NOT fully parse the entire MKV structure or media data.
pub struct MkvParser<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks
}

impl<R: Read + Seek> MkvParser<R> {
    /// Creates a new `MkvParser` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        Self { reader, handle, file_size }
    }

    /// Parses the MKV (EBML) header.
    /// Assumes the reader is positioned at the start of the file (offset 0).
    /// Reads the 4-byte EBML magic number.
    pub fn parse_ebml_header(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
        self.reader.seek(SeekFrom::Start(0)).map_err(map_core_io_error_to_fs_error)?; // Ensure at start

        let mut magic = [0u8; 4];
        self.reader.read_exact(&mut magic).map_err(|e| map_core_io_error_to_fs_error(e))?;

        if magic != [0x1A, 0x45, 0xDF, 0xA3] {
            return Err(map_mkv_error_to_fs_error(MkvError::InvalidMagicNumber(magic))); // MkvError -> FileSystemError
        }

        #[cfg(not(feature = "std"))]
        crate::println!("MKV (EBML) başlığı doğrulandı.");
        #[cfg(feature = "std")]
        println!("MKV (EBML) başlığı doğrulandı.");

        // Optional: Parse EBML version, read version, max size, etc. here if needed.
        // These are also VINTs/fixed size integers following the magic number.

        Ok(())
    }

    /// Iterates through the top-level EBML elements (segments) in the file.
    /// Reads Element IDs and Sizes. Skips the element data.
    /// Note: This is a simplified parser and does not handle the full EBML structure.
    pub fn parse_segments(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
        // After the EBML header, the rest of the file consists of EBML elements.
        // Top-level elements are typically the Segment element (ID 0x18538067).
        // Read elements until EOF is reached.

        loop {
            // Read Element ID (Variable Length Integer)
            let element_id = match self.read_ebml_id() {
                Ok(id) => id,
                Err(e) => {
                    // If we get an EOF error while reading an ID, it means we are at the end of the file.
                    if let FileSystemError::IOError(_) = e { // Check if it's an IO error mapped from UnexpectedEof
                         // Check if the underlying core::io::Error was UnexpectedEof
                         #[cfg(not(feature = "std"))]
                         if let FileSystemError::IOError(ref msg) = e {
                             if msg.contains("UnexpectedEof") { // Crude check based on error message format
                                 break Ok(()); // Reached end of file
                             }
                         }
                         #[cfg(feature = "std")]
                         if let FileSystemError::IOError(ref msg) = e {
                             if msg.contains("UnexpectedEof") || msg.contains("end of file") { // Check std error message too
                                break Ok(()); // Reached end of file
                             }
                         }
                    }
                    return Err(e); // Return other errors
                }
            };

             // If element_id is 0x00, it might be padding at the end. Read its size and skip.
             if element_id == 0x00 {
                 // Read size of padding and skip
                  let padding_size = self.read_ebml_size()?;
                  #[cfg(not(feature = "std"))]
                  crate::println!("Padding element (ID: 0x{:X}) found with size: {} bayt. Atlaniyor.", element_id, padding_size);
                  #[cfg(feature = "std")]
                  println!("Padding element (ID: 0x{:X}) found with size: {} bayt. Atlaniyor.", element_id, padding_size);
                 self.reader.seek(SeekFrom::Current(padding_size as i64)).map_err(map_core_io_error_to_fs_error)?;
                 continue; // Continue loop to read next element
             }


            // Read Element Size (Variable Length Integer)
            let element_size = self.read_ebml_size()?;

            #[cfg(not(feature = "std"))]
            crate::println!("Element ID: 0x{:X}, Boyut: {} bayt", element_id, element_size);
            #[cfg(feature = "std")]
            println!("Element ID: 0x{:X}, Boyut: {} bayt", element_id, element_size);

            // Process based on known element IDs (simplified)
            match element_id {
                0x18538067 => { // Segment element
                    #[cfg(not(feature = "std"))]
                    crate::println!("Segment elementi bulundu (boyut: {} bayt). Alt elementler ayrıştırilabilir.", element_size);
                    #[cfg(feature = "std")]
                    println!("Segment elementi bulundu (boyut: {} bayt). Alt elementler ayrıştırilabilir.", element_size);
                    // To parse nested elements within the Segment, we would need to recursively
                    // call a function that reads elements within a given size boundary (the segment size).
                    // For this basic parser, we just skip the segment data.
                    self.reader.seek(SeekFrom::Current(element_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Skip element data
                }
                 0x1549A966 => { // Info element
                    #[cfg(not(feature = "std"))]
                    crate::println!("Info elementi bulundu (boyut: {} bayt). Atlaniyor.", element_size);
                    #[cfg(feature = "std"))]
                    println!("Info elementi bulundu (boyut: {} bayt). Atlaniyor.", element_size);
                    self.reader.seek(SeekFrom::Current(element_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Skip element data
                 }
                  0x1654AE6B => { // Tracks element
                    #[cfg(not(feature = "std"))]
                    crate::println!("Tracks elementi bulundu (boyut: {} bayt). Atlaniyor.", element_size);
                    #[cfg(feature = "std"))]
                    println!("Tracks elementi bulundu (boyut: {} bayt). Atlaniyor.", element_size);
                    self.reader.seek(SeekFrom::Current(element_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Skip element data
                 }
                _ => { // Unknown element
                    #[cfg(not(feature = "std"))]
                    crate::println!("Bilinmeyen element ID: 0x{:X} (boyut: {} bayt). Atlaniyor.", element_id, element_size);
                    #[cfg(feature = "std"))]
                    println!("Bilinmeyen element ID: 0x{:X} (boyut: {} bayt). Atlaniyor.", element_id, element_size);

                    // Check if skipping would go beyond file bounds
                    let current_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
                    let bytes_remaining = self.file_size.checked_sub(current_pos).unwrap_or(0);

                    if element_size > bytes_remaining {
                         eprintln!("WARN: Element boyutu ({}) kalan dosya boyutundan ({}) büyük. Ayrıştırma durduruluyor.", element_size, bytes_remaining);
                         // Depending on strictness, this could be an error
                         return Err(map_mkv_error_to_fs_error(MkvError::InvalidEbmlData(format!("Element boyutu dosya sonunu aşıyor"))));
                    }


                    self.reader.seek(SeekFrom::Current(element_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Skip element data
                }
            }
        }
    }

    /// Reads an EBML Variable Length Integer (VINT) representing an Element ID.
    /// Returns the ID as u32.
    /// See https://www.matroska.org/technical/specs/index.html#vint_id
    fn read_ebml_id(&mut self) -> Result<u32, FileSystemError> { // Return FileSystemError
        let mut lead_byte = [0u8; 1];
        // Read the first byte. If EOF, it's the end of the file.
        let bytes_read = self.reader.read(&mut lead_byte).map_err(|e| map_core_io_error_to_fs_error(e))?;
        if bytes_read == 0 {
            return Err(map_mkv_error_to_fs_error(MkvError::UnexpectedEof)); // No more data
        }

        let lead = lead_byte[0];
        let mut length = 0;
        let mut id_value = 0;

        // Determine the length of the VINT ID from the leading bit pattern
        if (lead & 0x80) != 0 { length = 1; id_value = (lead & 0x7F) as u32; }
        else if (lead & 0x40) != 0 { length = 2; id_value = (lead & 0x3F) as u32; }
        else if (lead & 0x20) != 0 { length = 3; id_value = (lead & 0x1F) as u32; }
        else if (lead & 0x10) != 0 { length = 4; id_value = (lead & 0x0F) as u32; }
        // EBML IDs are typically 1 to 4 bytes long. Longer IDs are reserved or not common.
        // Let's support up to 4 bytes for IDs.

        if length == 0 || length > 4 {
            return Err(map_mkv_error_to_fs_error(MkvError::InvalidEbmlData(format!("Geçersiz VINT ID lider baytı: {:x}", lead))));
        }

        // Read the remaining bytes of the VINT ID
        let mut remaining_bytes = vec![0u8; length - 1]; // Requires alloc
        self.reader.read_exact(&mut remaining_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // core::io::Read

        for byte in remaining_bytes {
            id_value = (id_value << 8) | (byte as u32);
        }

        Ok(id_value)
    }

    /// Reads an EBML Variable Length Integer (VINT) representing an Element Size.
    /// Returns the size as u64.
    /// See https://www.matroska.org/technical/specs/index.html#vint_size
    fn read_ebml_size(&mut self) -> Result<u64, FileSystemError> { // Return FileSystemError
        let mut lead_byte = [0u8; 1];
        let bytes_read = self.reader.read(&mut lead_byte).map_err(|e| map_core_io_error_to_fs_error(e))?;
        if bytes_read == 0 {
            return Err(map_mkv_error_to_fs_error(MkvError::UnexpectedEof)); // No more data
        }

        let lead = lead_byte[0];
        let mut length = 0;
        let mut size_value = 0;

        // Determine the length of the VINT Size from the leading bit pattern
        if (lead & 0x80) != 0 { length = 1; size_value = (lead & 0x7F) as u64; }
        else if (lead & 0x40) != 0 { length = 2; size_value = (lead & 0x3F) as u64; }
        else if (lead & 0x20) != 0 { length = 3; size_value = (lead & 0x1F) as u64; }
        else if (lead & 0x10) != 0 { length = 4; size_value = (lead & 0x0F) as u64; }
        else if (lead & 0x08) != 0 { length = 5; size_value = (lead & 0x07) as u64; }
        else if (lead & 0x04) != 0 { length = 6; size_value = (lead & 0x03) as u64; }
        else if (lead & 0x02) != 0 { length = 7; size_value = (lead & 0x01) as u64; }
        else if (lead & 0x01) != 0 { length = 8; size_value = 0; } // Special case: VINTs starting with 0x01 are not used for size currently

        if length == 0 || length > 8 {
             // EBML sizes can be up to 8 bytes. If lead is 0x01, it's invalid for size.
            return Err(map_mkv_error_to_fs_error(MkvError::InvalidEbmlData(format!("Geçersiz VINT Size lider baytı: {:x}", lead))));
        }

        // Read the remaining bytes of the VINT Size
        let mut remaining_bytes = vec![0u8; length - 1]; // Requires alloc
        self.reader.read_exact(&mut remaining_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // core::io::Read

        for byte in remaining_bytes {
            size_value = (size_value << 8) | (byte as u64);
        }

        Ok(size_value)
    }

    /// Provides access to the underlying reader.
    pub fn reader(&mut self) -> &mut R {
        &mut self.reader
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for MkvParser<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the parser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: MkvParser drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens an MKV file from the given path (std) or resource ID (no_std)
/// and creates a basic MkvParser.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the MkvParser or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_mkv_file<P: AsRef<Path>>(file_path: P) -> Result<MkvParser<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (required by MkvParser constructor)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    Ok(MkvParser::from_reader(reader, None, file_size)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_mkv_file(file_path: &str) -> Result<MkvParser<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    Ok(MkvParser::from_reader(reader, Some(handle), file_size)) // Pass the handle to the parser
}


// Example main functions
#[cfg(feature = "example_mkv")] // Different feature flag
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     #[cfg(not(feature = "std"))]
     {
          eprintln!("MKV parser example (no_std) starting...");
          // TODO: Call init_console(crate::Handle(3)); if needed
     }
     #[cfg(feature = "std")]
     {
          eprintln!("MKV parser example (std) starting...");
     }

     // Test with a hypothetical file path (std) or resource ID (no_std)
     #[cfg(feature = "std")]
     let mkv_path = Path::new("example.mkv"); // This file needs to exist for the std example
     #[cfg(not(feature = "std"))]
     let mkv_path = "sahne://files/example.mkv"; // This resource needs to exist for the no_std example


     match open_mkv_file(mkv_path) { // Call the function that opens and creates the parser
         Ok(mut parser) => { // Need mut to call parse_header/parse_segments
             if let Err(e) = parser.parse_ebml_header() {
                  #[cfg(not(feature = "std"))]
                  crate::eprintln!("EBML başlığı ayrıştırma hatası: {:?}", e);
                  #[cfg(feature = "std"))]
                  eprintln!("EBML başlığı ayrıştırma hatası: {}", e);
                  return Err(e);
             }

             if let Err(e) = parser.parse_segments() {
                 #[cfg(not(feature = "std"))]
                 crate::eprintln!("Segment ayrıştırma hatası: {:?}", e);
                 #[cfg(feature = "std"))]
                 eprintln!("Segment ayrıştırma hatası: {}", e);
                 return Err(e);
             }
             #[cfg(not(feature = "std"))]
             crate::println!("MKV ayrıştırma tamamlandı.");
             #[cfg(feature = "std"))]
             println!("MKV ayrıştırma tamamlandı.");

             // The Handle is automatically released when 'parser' goes out of scope (due to Drop)
         }
         Err(e) => {
              #[cfg(not(feature = "std"))]
              crate::eprintln!("MKV dosyası açma hatası: {:?}", e);
              #[cfg(feature = "std"))]
              eprintln!("MKV dosyası açma hatası: {}", e); // std error display
              return Err(e);
         }
     }

     #[cfg(not(feature = "std"))]
     eprintln!("MKV parser example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("MKV parser example (std) finished.");

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


     // Helper function to create dummy MKV data bytes in memory (EBML header + simple elements)
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
      fn create_dummy_mkv_data(elements_data: &[u8]) -> Vec<u8> {
          let mut buffer = Cursor::new(Vec::new());

          // Write EBML header (4 byte magic)
          buffer.write_all(&[0x1A, 0x45, 0xDF, 0xA3]).unwrap();

          // Write sample EBML elements after header
          buffer.write_all(elements_data).unwrap();

          buffer.into_inner()
      }

      // Helper to write an EBML element (ID + Size + Data)
       #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn write_ebml_element<W: Write + Seek>(writer: &mut W, id: u32, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
           let size = data.len() as u64;
            // This is simplified and does not write VINT IDs/Sizes correctly.
            // Need proper VINT encoding helper here.

            // Let's create a helper that writes VINT IDs/Sizes for the test data.
            let mut id_vint = Vec::new();
            let mut temp_id = id;
             if temp_id == 0 { id_vint.push(0x80); } // Special case for ID 0
             else {
                 let mut mask = 0x80000000; // For 4-byte ID
                 let mut len = 4;
                 while len > 1 && (temp_id & mask) == 0 {
                     mask >>= 8;
                     len -= 1;
                 }
                 id_vint.push(mask.leading_zeros() as u8 | ((temp_id >> ((len - 1) * 8)) & ((1 << mask.leading_zeros()) - 1)) as u8);
                 for i in (0..len - 1).rev() {
                     id_vint.push(((temp_id >> (i * 8)) & 0xFF) as u8);
                 }
             }

           writer.write_all(&id_vint)?;

           let mut size_vint = Vec::new();
           let mut temp_size = size;
            // This is simplified and does not encode VINT sizes correctly.
            // Need proper VINT encoding helper here based on Matroska spec.
            // For simplicity in test data creation, let's assume sizes fit in 8 bytes VINT (prefix 0x01).
             if temp_size == 0 { size_vint.push(0x80); } // Size 0
             else {
                 let mut mask = 0x8000000000000000; // For 8-byte size
                  let mut len = 8;
                  while len > 1 && (temp_size & mask) == 0 {
                      mask >>= 8;
                      len -= 1;
                  }
                  size_vint.push(mask.leading_zeros() as u8 | ((temp_size >> ((len - 1) * 8)) & ((1 << mask.leading_zeros()) - 1)) as u8);
                  for i in (0..len - 1).rev() {
                      size_vint.push(((temp_size >> (i * 8)) & 0xFF) as u8);
                  }
             }
            // Correct VINT Size encoding is different from ID.
            // Let's manually encode a few common sizes for test data creation.
            // Size 0: 0x80
            // Size < 128: 1 byte, MSB 1, value in lower 7 bits
            // Size < 16384: 2 bytes, MSB 01, value in lower 14 bits
            // ... up to 8 bytes

            // For test simplicity, let's assume sizes fit in a few bytes VINTs.
            // Size 10 (0x0A): 0x8A (1-byte VINT)
            // Size 256 (0x100): 0x40 0x00 (2-byte VINT)
            // Size 65536 (0x10000): 0x20 0x01 0x00 (3-byte VINT)

            // Let's write a function to encode a VINT size correctly.
            let encoded_size = encode_vint_size(size)?; // Needs implementation


           writer.write_all(&encoded_size)?;
           writer.write_all(data)?; // Write element data
           Ok(())
       }

      // Helper function to encode a u64 as an EBML VINT Size
       #[cfg(feature = "std")] // Uses std byteorder
       fn encode_vint_size(value: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           if value == 0 {
               return Ok(vec![0x80]);
           }
           let mut buffer = Vec::new();
           let mut temp_value = value;
           let mut len = 0;
           let mut mask = 0x80u64; // Leading bit patterns for lengths
           while len < 8 {
               if (temp_value + 1) <= mask { // +1 because size 0 maps to 0x80 (length 1)
                   buffer.push(((mask | temp_value) & 0xFF) as u8); // Write the lead byte
                   break;
               }
               mask = (mask << 7) | 0x80; // Shift mask for next length
               len += 1;
           }
            if len == 8 { // Max length 8 (prefix 0x01)
                buffer.push(0x01); // Lead byte for length 8
            }


           let mut data_bytes = Vec::new();
            if value > 0 {
                let mut temp = value;
                 for _ in 0..len {
                     data_bytes.push((temp & 0xFF) as u8);
                     temp >>= 8;
                 }
                 data_bytes.reverse(); // Most significant byte first
            }


            // The VINT encoding is complex. Re-reading the spec.
            // Length L is determined by the position of the first '1' bit from the MSB.
            // If the first byte is 1xxxxxxx, length is 1. The value is in the lower 7 bits.
            // If the first byte is 01xxxxxx, length is 2. The value is in the lower 6 bits of byte 1 and 8 bits of byte 2.
            // ...
            // If the first byte is 00000001, length is 8. The value is in the lower 0 bits of byte 1 and 63 bits of bytes 2-8.

            // Let's implement a correct VINT size encoding.
            let mut bytes = Vec::new();
             if value < (1u64 << 7) - 1 { bytes.push((value | 0x80) as u8); } // Length 1
             else if value < (1u64 << 14) - 1 { bytes.push((value >> 8 | 0x4000) as u8); bytes.push((value & 0xFF) as u8); } // Length 2 (incorrect)
             // The encoding is value | (prefix << (length * 8))
             // Example: Size 10 (0x0A). Length 1 (0x80 prefix). (0x0A | (0x80 << 0)) = 0x8A. Bytes: [0x8A]
             // Example: Size 256 (0x100). Length 2 (0x40 prefix). (0x100 | (0x40 << 8)) = 0x4100. Bytes: [0x41, 0x00] (Big Endian order after adding prefix)
             // No, the value is NOT OR-ed with the prefix shifted.
             // The prefix determines the length. The value fills the remaining bits.

            let mut buffer = Vec::new();
            if value == 0 { return Ok(vec![0x80]); } // Length 1, value 0
            let mut temp_value = value;
            let mut len_bytes = 0;
             // Find the number of bytes needed for the value itself
             while temp_value > 0 {
                 temp_value >>= 8;
                 len_bytes += 1;
             }
            if len_bytes == 0 { len_bytes = 1; } // Value 0 still needs 1 byte for itself (after stripping prefix)

             let vint_len = match len_bytes {
                 1 => 1, // Value fits in 7 bits (1 byte VINT)
                 _ => len_bytes + (8 - len_bytes).leading_zeros() as usize, // This is complex.
             };

             // Re-reading the VINT Size encoding spec carefully.
             // The first byte contains the length information in its leading bits.
             // 1xxxxxxx: length 1, value in xxxxxxx
             // 01xxxxxx: length 2, value in xxxxxx (byte 1) + 8 bits (byte 2)
             // 001xxxxx: length 3, value in xxxxx (byte 1) + 16 bits (bytes 2-3)
             // ...
             // 00000001: length 8, value in 63 bits (bytes 2-8)

             // Let's encode value 10 (0x0A). Fits in 7 bits. Length 1. Lead byte: 0x80 | 0x0A = 0x8A. Bytes: [0x8A]
             // Let's encode value 256 (0x100). Needs 2 bytes (0x01 0x00). Fits in 14 bits (value 0x0100). Length 2. Lead byte: 0x40. Remaining 14 bits: 0x0100. Bytes: [0x40, 0x01, 0x00] (incorrect order)
             // Remaining bits are written Big Endian.

             let mut bytes = Vec::new();
             let mut temp_value = value;
             let mut len_marker = 0x80; // Starting with 1-byte marker
             let mut len = 1;

             while len <= 8 {
                 // Calculate the max value that fits in remaining bits for current length
                 let max_value = (1u64 << (len * 7 + (8 - len))) - 1; // This seems incorrect.

                 // Let's encode based on the number of bytes needed for the value itself
                 let mut value_bytes = Vec::new();
                  let mut temp = value;
                  while temp > 0 {
                      value_bytes.push((temp & 0xFF) as u8);
                      temp >>= 8;
                  }
                 if value_bytes.is_empty() { value_bytes.push(0); } // For value 0


                 // Find the smallest length 'L' such that value < 2^(7*L + (8-L)). No, this is wrong.

                 // Let's use a simple approach: find the number of bytes required for the value.
                 // Then add the prefix byte.
                 let mut value_bytes_only = Vec::new();
                  let mut temp = value;
                  if temp == 0 { value_bytes_only.push(0); }
                  else {
                       while temp > 0 {
                           value_bytes_only.push((temp & 0xFF) as u8);
                           temp >>= 8;
                       }
                  }
                 value_bytes_only.reverse(); // Big Endian order

                 let len_bytes_value = value_bytes_only.len();
                 let vint_len = match len_bytes_value {
                      1 if value < 0x80 => 1,
                      1 => 2, // Value 0x80 to 0xFF need length 2
                      2 if value < 0x4000 => 2,
                      2 => 3, // Value 0x4000 to 0xFFFF need length 3
                       // This is getting complex and error prone.

                       // Let's encode size 10 (0x0A): [0x8A]
                       // Let's encode size 256 (0x100): [0x40, 0x01, 0x00]
                       // Let's encode size 65536 (0x10000): [0x20, 0x01, 0x00, 0x00]

                       // Encode the length marker byte and the value bytes
                       let mut encoded = Vec::new();
                        if value < 0x80 { encoded.push((value | 0x80) as u8); }
                        else if value < 0x4000 { // Fits in 14 bits
                            encoded.push((value >> 8 | 0x40) as u8);
                            encoded.push((value & 0xFF) as u8);
                        } else if value < 0x200000 { // Fits in 21 bits
                             encoded.push((value >> 16 | 0x20) as u8);
                             encoded.push(((value >> 8) & 0xFF) as u8);
                             encoded.push((value & 0xFF) as u8);
                        } else if value < 0x10000000 { // Fits in 28 bits
                            encoded.push((value >> 24 | 0x10) as u8);
                             encoded.push(((value >> 16) & 0xFF) as u8);
                             encoded.push(((value >> 8) & 0xFF) as u8);
                             encoded.push((value & 0xFF) as u8);
                         } else {
                             // For larger sizes, need more cases up to 8 bytes.
                             // For test data, let's only encode sizes that fit in up to 4 bytes for now.
                              return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "VINT size encoding not implemented for values >= 0x10000000")));
                         }

                     return Ok(encoded);

                 };


           // This implementation is complex and requires careful bit manipulation.
           // For the test data creation helper, let's just manually write some common EBML element bytes.
           // This avoids implementing the full VINT encoding here.

           unimplemented!("encode_vint_size not implemented correctly yet");


       }


     // Test the EBML header parsing
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_parse_ebml_header() -> Result<(), FileSystemError> { // Return FileSystemError
          // Create dummy data with valid EBML header
          let dummy_data = create_dummy_mkv_data(&[]); // Only header

          // Use Cursor as a reader for the in-memory data
          let file_size = dummy_data.len() as u64;
          let mut cursor = Cursor::new(dummy_data);

          // Create a dummy MkvParser with the cursor reader
          let mut parser = MkvParser::from_reader(cursor, None, file_size);

          // Call the parsing function
          parser.parse_ebml_header()?; // Should not return error

          Ok(())
     }

      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_ebml_header_invalid_magic() {
           // Create dummy data with invalid magic number
           let dummy_data = b"XXXX\x45\xDF\xA3".to_vec(); // Invalid magic, rest doesn't matter for this test

           // Use Cursor as a reader
           let file_size = dummy_data.len() as u64;
           let mut cursor = Cursor::new(dummy_data);
           let mut parser = MkvParser::from_reader(cursor, None, file_size);

           // Call the parsing function, expect an error
           let result = parser.parse_ebml_header();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => {
                   assert!(msg.contains("Geçersiz MKV sihirli sayısı"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_ebml_header_truncated() {
           // Create dummy data that is too short for the header
           let dummy_data = b"\x1A\x45".to_vec(); // Only 2 bytes

           // Use Cursor as a reader
           let file_size = dummy_data.len() as u64;
           let mut cursor = Cursor::new(dummy_data);
           let mut parser = MkvParser::from_reader(cursor, None, file_size);

           // Call the parsing function, expect an error
           let result = parser.parse_ebml_header();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenenden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }


     // Test the basic segment parsing (requires correct EBML VINT reading)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_parse_segments_basic() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error> for easier test error handling

         // Create dummy MKV data with EBML header and a few simple elements
         // EBML elements: ID (VINT) + Size (VINT) + Data (Size bytes)
         // Let's create a dummy Segment element (ID 0x18538067), Info (ID 0x1549A966), Tracks (ID 0x1654AE6B).
         // Use hardcoded VINT encoding for these known IDs and small sizes for simplicity in test data.
         // Segment ID (0x18538067): 4 bytes. 0x18 0x53 0x80 0x67. VINT encoding: Need 4 bytes for value. Length 4. Prefix 0x10. Value: 0x08538067.
         // Encoding is value | (prefix << (length * 8 - leading_zeros - 1))
         // For 0x18538067: Leading 1 is at bit 28 (from right, 0-indexed). Length 4. Prefix 0x10.
         // The encoding is more complex.
         // Let's use the actual ID bytes: [0x18, 0x53, 0x80, 0x67]. VINT ID [0x18, 0x53, 0x80, 0x67]. This is not a VINT ID.
         // A VINT ID always starts with a byte that indicates length.

         // Correct VINT ID encoding examples:
         // ID 0x1A45DFA3 (EBML): 4 bytes. Starts with 0x1A (00011010). Length 4 (prefix 0x10). Value 0x0A45DFA3.
         // Encoded: [0x1A, 0x45, 0xDF, 0xA3] (incorrect VINT, this is the raw ID)
         // The VINT ID for EBML header is 0x1A45DFA3, but it is represented as a 4-byte VINT.
         // The VINT ID for Segment (0x18538067).

         // Let's manually encode a few VINT IDs and Sizes for test data.
         // VINT ID for Segment (0x18538067):
         // Needs 4 bytes. The ID value is 0x18538067.
         // The VINT ID must start with a byte indicating length.
         // If the ID value needs N bytes, the VINT ID will be N+1 bytes long.
         // Example: ID 0x0F (1 byte value). Needs 1 byte. VINT ID length 2. Prefix 0x40. Value 0x0F. [0x40 | 0x0F] incorrect.

         // Re-reading EBML spec for VINT ID encoding.
         // The first byte's leading bits determine length.
         // 1xxxxxxx -> length 1. Value in lower 7 bits. Example: 0x81 -> ID 1.
         // 01xxxxxx -> length 2. Value in lower 6 bits + next 8 bits. Example: 0x40 0x01 -> ID 1. [0x40 | (1 << 8)]
         // 001xxxxx -> length 3. Value in lower 5 bits + next 16 bits.
         // 0001xxxx -> length 4. Value in lower 4 bits + next 24 bits.
         // 00001xxx -> length 5.
         // 000001xx -> length 6.
         // 0000001x -> length 7.
         // 00000001 -> length 8.

         // ID 0x18538067. Needs 4 bytes (0x18 0x53 0x80 0x67). This value is > 2^21 (for length 3). It's < 2^28 (for length 4).
         // Needs 4 bytes. Prefix 0x10. Value in lower 4 bits of byte 1 + bytes 2-4.
         // First byte: 0x10 | ((0x18538067 >> 24) & 0x0F) = 0x10 | (0x18 & 0x0F) = 0x10 | 0x08 = 0x18.
         // Bytes 2-4: (0x18538067 >> 16) & 0xFF = 0x53. (0x18538067 >> 8) & 0xFF = 0x80. (0x18538067 >> 0) & 0xFF = 0x67.
         // Encoded VINT ID for Segment (0x18538067): [0x18, 0x53, 0x80, 0x67]. This matches the original code's assumption for ID bytes.
         // So the original code was reading the RAW ID bytes as the ID, not the VINT ID.
         // A proper parser reads the VINT ID.

         // Let's implement read_ebml_id correctly based on the VINT ID format.
         // The `read_ebml_id` function has been updated to read VINT IDs.
         // Now create test data with correctly encoded VINT IDs and Sizes.

         // Common top-level elements and their VINT IDs:
         // EBML (header): 0x1A45DFA3 (VINT ID: [0x1A, 0x45, 0xDF, 0xA3] - This is incorrect, the EBML ID is not represented as a VINT itself)
         // The EBML header STARTS with the 4-byte ID 0x1A45DFA3, followed by VINTs for EBML version, read version, etc.

         // Okay, the magic number is a fixed 4 bytes. Elements after that are ID (VINT) + Size (VINT) + Data.

         // Segment (root element): ID 0x18538067. VINT ID: [0x10 | (0x18>>24)&0x0F], [0x18>>16]&0xFF, [0x18>>8]&0xFF, [0x18>>0]&0xFF
         // Let's use simpler elements for testing VINT reading.
         // Info (ID 0x1549A966). VINT ID: [0x10 | (0x15>>24)&0x0F], [0x15>>16]&0xFF, [0x15>>8]&0xFF, [0x15>>0]&0xFF = [0x15, 0x49, 0xA9, 0x66].

         // Let's create a dummy file with EBML header, a small Info element, and a small Tracks element.
         // EBML Header: [0x1A, 0x45, 0xDF, 0xA3]
         // Info Element:
         //   VINT ID for Info (0x1549A966): [0x15, 0x49, 0xA9, 0x66] (This is wrong, VINT ID should start with length indicator)
         //   Correct VINT ID for 0x1549A966: Needs 4 bytes. Prefix 0x10. Value in lower 4 bits of byte 1 + bytes 2-4.
         //   Value 0x1549A966. First byte: 0x10 | ((0x1549A966 >> 24) & 0x0F) = 0x10 | (0x15 & 0x0F) = 0x10 | 0x05 = 0x15.
         //   Bytes 2-4: (0x1549A966 >> 16) & 0xFF = 0x49. (0x1549A966 >> 8) & 0xFF = 0xA9. (0x1549A966 >> 0) & 0xFF = 0x66.
         //   Encoded VINT ID for Info (0x1549A966): [0x15, 0x49, 0xA9, 0x66]. Okay, it seems the VINT ID bytes for common elements are the raw ID bytes themselves. This is counter-intuitive but seems to be the convention. Let's trust this observation for test data.

         // Info Element (ID 0x1549A966): [0x15, 0x49, 0xA9, 0x66] (VINT ID)
         //   Size: Let's say size is 10 bytes. VINT Size for 10 (0x0A): [0x8A]
         //   Data: 10 bytes of dummy data [0u8; 10]
         // Tracks Element (ID 0x1654AE6B): [0x16, 0x54, 0xAE, 0x6B] (VINT ID)
         //   Size: Let's say size is 20 bytes. VINT Size for 20 (0x14): [0x94]
         //   Data: 20 bytes of dummy data [1u8; 20]

         let mut elements_cursor = Cursor::new(Vec::new());
          // Write Info element (ID 0x1549A966, Size 10)
         elements_cursor.write_all(&[0x15, 0x49, 0xA9, 0x66]).unwrap(); // Info VINT ID
         elements_cursor.write_u8(0x8A).unwrap(); // Size 10 VINT
         elements_cursor.write_all(&[0u8; 10]).unwrap(); // Info data
          // Write Tracks element (ID 0x1654AE6B, Size 20)
         elements_cursor.write_all(&[0x16, 0x54, 0xAE, 0x6B]).unwrap(); // Tracks VINT ID
         elements_cursor.write_u8(0x94).unwrap(); // Size 20 VINT
         elements_cursor.write_all(&[1u8; 20]).unwrap(); // Tracks data

         let elements_data = elements_cursor.into_inner();
         let dummy_mkv_data = create_dummy_mkv_data(&elements_data); // Add EBML header

         // Use Cursor as a reader
         let file_size = dummy_mkv_data.len() as u64;
         let mut cursor = Cursor::new(dummy_mkv_data.clone()); // Clone for potential re-reads in test

         // Create a dummy MkvParser with the cursor reader
         let mut parser = MkvParser::from_reader(cursor, None, file_size);

         // Parse header first
         parser.parse_ebml_header()?;

         // Parse segments (should find and skip Info and Tracks)
         parser.parse_segments()?; // Should complete without error

          // Verify the cursor is at the end of the data after parsing segments
          assert_eq!(parser.reader.stream_position().unwrap(), file_size);


         Ok(())
     }

     // Test handling of unexpected EOF during VINT reading
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_segments_truncated_vint() {
           // Create dummy data with valid header, but truncated VINT ID
           let mut dummy_data_cursor = Cursor::new(Vec::new());
           dummy_data_cursor.write_all(&[0x1A, 0x45, 0xDF, 0xA3]).unwrap(); // EBML Header
           dummy_data_cursor.write_all(&[0x15, 0x49]).unwrap(); // Truncated VINT ID (should be 4 bytes)
           let dummy_data = dummy_data_cursor.into_inner(); // 4 + 2 = 6 bytes total

           let file_size = dummy_data.len() as u64;
           let mut cursor = Cursor::new(dummy_data);
           let mut parser = MkvParser::from_reader(cursor, None, file_size);

           let header_result = parser.parse_ebml_header();
           assert!(header_result.is_ok()); // Header should parse

           // Attempt to parse segments, expect an error during VINT ID reading
           let result = parser.parse_segments();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact in read_ebml_id)
                   assert!(msg.contains("Beklenenden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of unexpected EOF during data skipping
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_parse_segments_truncated_data() {
            // Create dummy data with valid header, valid VINT ID/Size, but truncated data
            let mut dummy_data_cursor = Cursor::new(Vec::new());
            dummy_data_cursor.write_all(&[0x1A, 0x45, 0xDF, 0xA3]).unwrap(); // EBML Header
            // Info Element (ID 0x1549A966, Size 10)
            dummy_data_cursor.write_all(&[0x15, 0x49, 0xA9, 0x66]).unwrap(); // Info VINT ID
            dummy_data_cursor.write_u8(0x8A).unwrap(); // Size 10 VINT
            dummy_data_cursor.write_all(&[0u8; 5]).unwrap(); // Only 5 bytes of data (truncated)
            let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 1 + 5 = 14 bytes total

            let file_size = dummy_data.len() as u64;
            let mut cursor = Cursor::new(dummy_data);
            let mut parser = MkvParser::from_reader(cursor, None, file_size);

            let header_result = parser.parse_ebml_header();
            assert!(header_result.is_ok()); // Header should parse

            // Attempt to parse segments, expect an error during data skipping (seek)
            let result = parser.parse_segments();

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError (seek failure)
                     // The Seek::Current(offset) implementation in Cursor would likely return Ok
                     // even if seeking past EOF. The error would occur on the NEXT read.
                     // The parse_segments logic checks if skipping goes beyond file bounds.
                     assert!(msg.contains("Element boyutu dosya sonunu aşıyor")); // Check the bounds check error
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This involves simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include reading valid content, handling file not found, IO errors,
     // and correctly parsing VINT IDs and Sizes.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_mkv", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
