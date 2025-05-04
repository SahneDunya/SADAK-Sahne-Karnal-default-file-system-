#![allow(unused_imports)] // Gerekli olmayan importlar için uyarı vermesin
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader as StdBufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt, WriteExt as StdWriteExt};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::O_RDONLY; // Import necessary fs flags

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
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt, WriteExt as CoreWriteExt}; // core::io, Added Write, WriteExt


// tiff crate (requires specific features for no_std)
// Assuming tiff::decoder::Decoder, tiff::decoder::DecodingError, tiff::tags::{Tag, Type} are available with features.
#[cfg(feature = "tiff_parser")] // Assume a feature flag controls tiff parser availability
mod tiff_parser {
    // Re-export required types from the tiff crate
    pub use tiff::decoder::{Decoder, DecodingError};
    pub use tiff::tags::{Tag, Type, Rational}; // Import Rational if needed for tags
}
#[cfg(feature = "tiff_parser")]
use tiff_parser::{Decoder, DecodingError, Tag, Type, Rational};


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

/// Helper function to map tiff::decoder::DecodingError to FileSystemError.
#[cfg(feature = "tiff_parser")]
fn map_decoding_error_to_fs_error(e: DecodingError) -> FileSystemError {
     #[cfg(feature = "std")]
     { // Use std Error::source() or similar if available for better mapping
         if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<StdIOError>()) {
              return map_std_io_error_to_fs_error(io_err.clone()); // Clone is needed if source returns reference
         }
     }

    FileSystemError::InvalidData(format!("TIFF decoding error: {:?}", e)) // Generic mapping
    // TODO: Implement a proper mapping based on DecodingError variants if possible
}


/// Custom error type for TIFF handling issues.
#[derive(Debug)]
pub enum TiffError {
    DecodingError(String), // Errors from the underlying TIFF decoder
    TagNotFound(Tag),
    InvalidTagType(Tag, Type),
    ConversionError(Tag, String), // Error converting tag value to expected type
    UnexpectedEof(String), // During reading
    SeekError(u64), // Failed to seek
    NotSupported, // Functionality is not supported in the current configuration (e.g., no_std tiff)
    // Add other TIFF specific errors here
}

// Implement Display for TiffError
impl fmt::Display for TiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TiffError::DecodingError(msg) => write!(f, "TIFF kod çözme hatası: {}", msg),
            TiffError::TagNotFound(tag) => write!(f, "Etiket bulunamadı: {:?}", tag),
            TiffError::InvalidTagType(tag, type_enum) => write!(f, "Geçersiz etiket türü: {:?} için tür: {:?}", tag, type_enum),
            TiffError::ConversionError(tag, message) => write!(f, "{:?} etiketi dönüştürme hatası: {}", tag, message),
            TiffError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            TiffError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
            TiffError::NotSupported => write!(f, "TIFF desteği mevcut değil"),
        }
    }
}

// Helper function to map TiffError to FileSystemError
fn map_tiff_error_to_fs_error(e: TiffError) -> FileSystemError {
    match e {
        TiffError::UnexpectedEof(_) | TiffError::SeekError(_) => FileSystemError::IOError(format!("TIFF IO hatası: {}", e)), // Map IO related errors
        TiffError::NotSupported => FileSystemError::NotSupported(format!("TIFF hatası: {}", e)), // Map NotSupported
        _ => FileSystemError::InvalidData(format!("TIFF ayrıştırma/veri hatası: {}", e)), // Map decoding/parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfiletxt.rs'den kopyalandı)
// core::io::Write implementasyonu eklendi.
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at/fs::write_at ile okuma/yazma yapar.
// fstat ile dosya boyutını alarak seek(End) desteği sağlar.
// Sahne64 API'sının bu syscall'ları Handle üzerinde sağladığı varsayılır.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu (read/write için güncellenmeli)
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
impl core::io::Write for SahneResourceReader { // Use core::io::Write trait (for write_at)
    fn write(&mut self, buf: &[u8]) -> Result<usize, core::io::Error> { // Return core::io::Error
         // Assuming fs::write_at(handle, offset, buf) Result<usize, SahneError>
         // This write implementation writes at the current position and updates it.
         let bytes_to_write = buf.len();
         if bytes_to_write == 0 { return Ok(0); }

         let bytes_written = fs::write_at(self.handle, self.position, buf)
             .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::write_at error: {:?}", e)))?; // Map SahneError to core::io::Error

         self.position += bytes_written as u64;

         // Update file_size if writing extends beyond current size
         if self.position > self.file_size {
              self.file_size = self.position;
              // Note: In a real filesystem, updating file size might require a separate syscall (e.g., ftruncate)
              // or might be handled implicitly by write_at at the end of the file.
              // Assuming for this model that writing past file_size implicitly extends it and updates fstat.
         }


         Ok(bytes_written)
    }

     fn flush(&mut self) -> Result<(), core::io::Error> {
         // Assuming fs::flush(handle) or sync() is available for durability.
         // If not, this is a no-op or needs a different syscall.
         // For this model, assume no explicit flush syscall is needed for basic durability after write.
         Ok(())
     }
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
// Removed custom MetadataError re-declaration and From impls at top level.
// Removed redundant print module and panic handler boilerplate.
// Removed the #[cfg(feature = "std")] pub mod srcfiletıff { ... } structure.
// Removed the #[cfg(not(feature = "std"))] pub mod srcfiletıff { ... } structure.


// Define the TiffFileHandler struct and its methods directly in this module.
// This struct holds the reader and handle for resource management.
pub struct TiffFileHandler<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size
}

impl<R: Read + Seek> TiffFileHandler<R> {
    /// Creates a new `TiffFileHandler` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        TiffFileHandler {
            reader, // Store the reader
            handle,
            file_size,
        }
    }

    /// Reads metadata from the TIFF file.
    /// Requires the 'tiff_parser' feature flag to be enabled.
    ///
    /// # Returns
    ///
    /// A Result indicating success (metadata printed) or a FileSystemError.
    #[cfg(feature = "tiff_parser")] // Only compile if tiff parser is available
    pub fn read_tiff_metadata(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
         // Create a BufReader wrapping the internal reader
         #[cfg(feature = "std")] // Use std BufReader if std feature is enabled
         let mut buf_reader = StdBufReader::new(&mut self.reader); // Wrap reference to reader
         #[cfg(not(feature = "std"))] // Use custom no_std BufReader if std is not enabled
         // Assuming a custom no_std BufReader implementation exists and is in scope (e.g., crate::BufReader)
         let mut buf_reader = crate::BufReader::new(&mut self.reader); // Wrap reference to reader

         // Create a TIFF decoder from the buffered reader
         let mut decoder = Decoder::new(buf_reader).map_err(|e| map_decoding_error_to_fs_error(e))?; // Map initial decoder error

         println!("TIFF Dosyası Meta Verileri:"); // Use standardized print


         // Helper function to get and print a tag value
         let get_and_print_tag = |decoder: &mut Decoder<_>, tag: Tag, tag_name: &str| -> Result<(), FileSystemError> { // Return FileSystemError
             match decoder.get_tag(tag) {
                 Ok(value) => {
                     // Safely print based on known Type variants
                     match value {
                         Type::U32(v) => {
                              if let Some(val) = v.first() {
                                  println!("{}: {}", tag_name, val);
                              } else { println!("{}: Değer bulunamadı", tag_name); }
                         },
                         Type::U16(v) => {
                             if let Some(val) = v.first() {
                                 println!("{}: {}", tag_name, val);
                             } else { println!("{}: Değer bulunamadı", tag_name); }
                         },
                          Type::Ascii(v) => {
                             if let Some(val) = v.first() {
                                  // Attempt to convert ASCII to String for printing
                                  let ascii_str = alloc::string::String::from_utf8_lossy(val.as_bytes()); // Use lossy for basic printing
                                  println!("{}: {}", tag_name, ascii_str);
                             } else { println!("{}: Değer bulunamadı", tag_name); }
                          },
                         Type::Rational(v) => {
                             if let Some(val) = v.first() {
                                 println!("{}: {}/{}", tag_name, val.n, val.d);
                             } else { println!("{}: Değer bulunamadı", tag_name); }
                         },
                          // Add handling for other common tag types if needed (e.g., i8, u8, i16, i32, f32, f64, SRational)
                         _ => {
                              println!("{}: {:?} (Yazdırılamayan tür)", tag_name, value);
                         }
                     }
                     Ok(())
                 },
                 Err(DecodingError::TagNotFound) => {
                     println!("{}: Bulunamadı", tag_name); // Info that tag is missing
                     Ok(()) // Not an error if tag is optional
                 },
                 Err(e) => Err(map_decoding_error_to_fs_error(e)), // Map other TIFF decoding errors
             }
         };

         // Call helper for relevant tags
         get_and_print_tag(&mut decoder, Tag::ImageWidth, "Genişlik")?;
         get_and_print_tag(&mut decoder, Tag::ImageLength, "Yükseklik")?;
         get_and_print_tag(&mut decoder, Tag::BitsPerSample, "Bit/Örnek")?;
         get_and_print_tag(&mut decoder, Tag::PhotometricInterpretation, "Fotometrik Yorumlama")?;
         get_and_print_tag(&mut decoder, Tag::ImageDescription, "Dosya Açıklaması")?;
         get_and_print_tag(&mut decoder, Tag::Make, "Üretici")?;
         get_and_print_tag(&mut decoder, Tag::Model, "Model")?;
         get_and_print_tag(&mut decoder, Tag::Software, "Yazılım")?;
         get_and_print_tag(&mut decoder, Tag::DateTime, "Tarih ve Saat")?;
         get_and_print_tag(&mut decoder, Tag::Artist, "Sanatçı")?;
         get_and_print_tag(&mut decoder, Tag::Copyright, "Telif Hakkı")?;
         get_and_print_tag(&mut decoder, Tag::ResolutionUnit, "Çözünürlük Birimi")?;
         get_and_print_tag(&mut decoder, Tag::XResolution, "X Çözünürlüğü")?;
         get_and_print_tag(&mut decoder, Tag::YResolution, "Y Çözünürlüğü")?;


        Ok(())
    }

    /// Placeholder for read_tiff_metadata when the TIFF parser is not supported.
    #[cfg(not(feature = "tiff_parser"))]
    pub fn read_tiff_metadata(&mut self) -> Result<(), FileSystemError> {
         eprintln!("WARNING: TIFF parser feature ('tiff_parser') is not enabled.");
         Err(map_tiff_error_to_fs_error(TiffError::NotSupported)) // Use standardized error map
    }
}


#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for TiffFileHandler<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the TiffFileHandler is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: TiffFileHandler drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a TIFF file from the given path (std) or resource ID (no_std)
/// and creates a TiffFileHandler instance.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the TiffFileHandler or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_tiff_file<P: AsRef<Path>>(file_path: P) -> Result<TiffFileHandler<File>, FileSystemError> { // Use std::fs::File directly as Reader+Seek
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    // Note: BufReader can be wrapped around File later in read_tiff_metadata if needed.
    // Open the file and wrap it in a handler.

    // Get file size for the handler
    let mut temp_file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let file_size = temp_file.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    // No need to seek temp_file back, it will be dropped.


    Ok(TiffFileHandler::from_reader(file, None, file_size)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_tiff_file(file_path: &str) -> Result<TiffFileHandler<SahneResourceReader>, FileSystemError> { // Return FileSystemError
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutını al (needed for SahneResourceReader)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReader oluştur
    let reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek + Drop


    // Create a TiffFileHandler instance
    Ok(TiffFileHandler::from_reader(reader, Some(handle), file_size)) // Pass the handle to the handler struct
}


// Example main function (std)
#[cfg(feature = "example_tiff")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
#[cfg(feature = "tiff_parser")] // Only compile if tiff parser is available
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("TIFF metadata reader example (std) starting...");
     eprintln!("TIFF metadata reader example (std) using tiff crate.");

     // Create a dummy TIFF file for the example
     let test_file_path = "example.tiff";
     use std::fs::remove_file;
     use std::io::Write;
     use tiff::encoder::{TiffEncoder, colortype::ColorType};
     use tiff::ImageBuffer;
     use tiff::tags::Tag;

     // Helper to create a test TIFF file (copied from test module)
     fn create_test_tiff_file(file_path: &str) -> std::io::Result<()> {
         let mut file = File::create(file_path)?;
         let mut encoder = TiffEncoder::new(&mut file)?;

         let width: u32 = 100;
         let height: u32 = 100;
         let mut image_buffer: ImageBuffer<ColorType::Gray(8), Vec<u8>> = ImageBuffer::new(width, height);

         // Add some dummy data and tags
          for x in 0..width { for y in 0..height { image_buffer.put_pixel(x, y, tiff::ColorValue::Gray( (x % 255) as u8)); } }

          encoder.set_tag(Tag::ImageDescription, "Sahne64 Test Image").expect("Failed to set tag");
          encoder.set_tag(Tag::Software, "Sahne64 TIFF Example").expect("Failed to set tag");


         encoder.encode_image(image_buffer.as_raw(), width, height, ColorType::Gray(8))?;
         Ok(())
     }


     // Create the dummy TIFF file
      if let Err(e) = create_test_tiff_file(test_file_path) {
           eprintln!("Error creating dummy TIFF file: {}", e);
           return Err(map_std_io_error_to_fs_error(e));
      }
      println!("Dummy TIFF file created: {}", test_file_path);


     // Open the TIFF file and read metadata
     match open_tiff_file(test_file_path) { // Call the function that opens and creates handler
         Ok(mut tiff_handler) => { // Need mut to call read_tiff_metadata
             println!("TIFF file handler created.");

             // Read and print metadata
             match tiff_handler.read_tiff_metadata() {
                 Ok(_) => {
                      println!("TIFF metadata read successfully.");
                 },
                 Err(e) => {
                     eprintln!("Error reading TIFF metadata: {}", e); // std error display
                      // Don't return error, let cleanup run
                 }
             }

             // File is automatically closed when tiff_handler goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening TIFF file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if Path::new(test_file_path).exists() { // Check if file exists before removing
          if let Err(e) = remove_file(test_file_path) {
               eprintln!("Error removing dummy TIFF file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("TIFF metadata reader example (std) finished.");

     Ok(())
}

// Example main function (no_std)
#[cfg(feature = "example_tiff")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("TIFF metadata reader example (no_std) starting...");
     // This example will likely return NotSupported unless a no_std tiff parser is configured.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and a TIFF file exists at "sahne://files/image.tiff".
      let tiff_res = open_tiff_file("sahne://files/image.tiff");
      match tiff_res {
          Ok(mut tiff_handler) => { // Need mut to read metadata
              crate::println!("TIFF file handler created.");
     //
     //         // Read and print metadata (will likely return NotSupported in no_std without parser)
              match tiff_handler.read_tiff_metadata() {
                  Ok(_) => {
                       crate::println!("TIFF metadata read successfully (requires no_std parser).");
                  },
                  Err(e) => {
                      crate::eprintln!("Error reading TIFF metadata: {:?}", e); // no_std print
                  }
              }
     //
              // File is automatically closed when tiff_handler goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening TIFF file: {:?}", e),
      }

     eprintln!("TIFF metadata reader example (no_std) needs Sahne64 mocks and tiff parser to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. A TIFF file to be available at the specified path.
     // 3. A no_std compatible tiff parser crate configured.

      // Explicitly return NotSupported if the parser is not enabled in no_std main
     #[cfg(not(feature = "tiff_parser"))]
     return Err(map_tiff_error_to_fs_error(TiffError::NotSupported));

     #[cfg(feature = "tiff_parser")]
     Ok(()) // If tiff_parser is enabled, the example logic above would run (if mocks existed)
}


// Test modülü (std özelliği aktifse ve tiff parser aktifse çalışır)
#[cfg(test)]
#[cfg(feature = "std")] // Only run tests with std feature enabled
#[cfg(feature = "tiff_parser")] // Only run tests if tiff parser is available
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use std::fs::{remove_file, File}; // Add File for test setup
    use std::path::Path; // For Path
    use std::io::Cursor as StdCursor; // For in-memory testing
    use std::io::Write; // For test file creation
    use tiff::encoder::{TiffEncoder, colortype::ColorType};
    use tiff::ImageBuffer;
    use tiff::tags::Tag;
    use std::error::Error; // For Box<dyn Error> source()


    // Helper to create a test TIFF file (copied from example main)
    fn create_test_tiff_file(file_path: &str) -> std::io::Result<()> {
        let mut file = File::create(file_path)?;
        let mut encoder = TiffEncoder::new(&mut file)?;

        let width: u32 = 50;
        let height: u32 = 30;
        let mut image_buffer: ImageBuffer<ColorType::Gray(8), Vec<u8>> = ImageBuffer::new(width, height);

        // Add some dummy data and tags for test verification
         for x in 0..width { for y in 0..height { image_buffer.put_pixel(x, y, tiff::ColorValue::Gray( (x % 255) as u8)); } }

         encoder.set_tag(Tag::ImageWidth, width).expect("Failed to set width tag");
         encoder.set_tag(Tag::ImageLength, height).expect("Failed to set height tag");
         encoder.set_tag(Tag::BitsPerSample, vec![8u16]).expect("Failed to set BitsPerSample tag");
         encoder.set_tag(Tag::PhotometricInterpretation, 1u16).expect("Failed to set PhotometricInterpretation tag"); // Grayscale
         encoder.set_tag(Tag::Software, "Sahne64 Test Encoder").expect("Failed to set Software tag");
         encoder.set_tag(Tag::XResolution, Rational {n: 72u32, d: 1u32}).expect("Failed to set XResolution tag"); // Example Rational tag
         encoder.set_tag(Tag::YResolution, Rational {n: 72u32, d: 1u32}).expect("Failed to set YResolution tag");


        encoder.encode_image(image_buffer.as_raw(), width, height, ColorType::Gray(8))?;
        Ok(())
    }

    // Helper to create dummy TIFF bytes in memory using tiff encoder
     fn create_test_tiff_bytes() -> Result<Vec<u8>, Box<dyn Error>> {
         let mut buffer = Cursor::new(Vec::new());
         let mut encoder = TiffEncoder::new(&mut buffer)?;

         let width: u32 = 20;
         let height: u32 = 10;
         let mut image_buffer: ImageBuffer<ColorType::Gray(8), Vec<u8>> = ImageBuffer::new(width, height);

          // Add some dummy data and tags for test verification
           for x in 0..width { for y in 0..height { image_buffer.put_pixel(x, y, tiff::ColorValue::Gray( (x % 255) as u8)); } }

           encoder.set_tag(Tag::ImageWidth, width).expect("Failed to set width tag");
           encoder.set_tag(Tag::ImageLength, height).expect("Failed to set height tag");


         encoder.encode_image(image_buffer.as_raw(), width, height, ColorType::Gray(8))?;

         Ok(buffer.into_inner()) // Return the bytes
     }


    #[test]
    fn test_read_tiff_metadata_std_file() -> Result<(), FileSystemError> { // Return FileSystemError for std test
        let test_file_path = "test_metadata.tiff";

        // Create the dummy TIFF file for the test
        create_test_tiff_file(test_file_path).map_err(map_std_io_error_to_fs_error)?;


        // Open the TIFF file and read metadata using the Sahne64-like function
        match open_tiff_file(test_file_path) {
            Ok(mut tiff_handler) => { // Need mut to call read_tiff_metadata
                // Call the metadata reading method
                let result = tiff_handler.read_tiff_metadata(); // This should print to stdout/stderr


                // For automated testing, we can't easily check stdout.
                // We'd need to capture stdout or modify the function to return the metadata.
                // Assert that the operation was successful.
                assert!(result.is_ok());


                // A more robust test would involve parsing the metadata into a struct
                // and asserting the values, but the current function only prints.
                // TODO: Refactor read_tiff_metadata to return a struct with metadata if possible.
            }
            Err(e) => {
                panic!("Error opening/handling TIFF file: {}", e);
            }
        }


        // Clean up the test file
        remove_file(test_file_path).map_err(map_std_io_error_to_fs_error)?;


        Ok(()) // Return Ok from test function
    }

    // Test handling of invalid TIFF data using in-memory cursor
     #[test]
     fn test_read_tiff_metadata_invalid_data_cursor() {
          // Create invalid TIFF bytes (e.g., truncated data)
          let dummy_bytes = vec![0u8; 10]; // Too short for even the header

          let cursor = StdCursor::new(dummy_bytes);
          // Create a handler from the cursor
          let mut tiff_handler = TiffFileHandler::from_reader(cursor, None, 10); // Pass None for handle, dummy size

          // Attempt to read metadata, expect an error
          let result = tiff_handler.read_tiff_metadata();

          assert!(result.is_err());
          match result.unwrap_err() {
              FileSystemError::InvalidData(msg) => { // Mapped from DecodingError
                  assert!(msg.contains("TIFF kod çözme hatası"));
                  // Check if the underlying tiff error message is included (might vary)
                   #[cfg(feature = "std")] // std tiff error message check
                  assert!(msg.contains("unexpected EOF"));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
          }
     }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
    // Test cases should include opening valid/invalid files, handling IO errors during reading,
    // and verifying the NotSupported error if the tiff_parser feature is disabled in no_std.
    // If tiff_parser is enabled in no_std, test actual metadata reading with mock data.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_tiff", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
