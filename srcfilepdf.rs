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

// alloc crate for String, Vec, Box, Arc, format!
use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box; // For Box<dyn Read + Seek>
use alloc::format;
use alloc::sync::Arc; // For potential Arc requirements of VFS or specific readers


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io


// lopdf crate (no_std compatible, requires alloc)
use lopdf::Document;
use lopdf::Error as LopdfError;
use lopdf::Object; // Needed for metadata extraction


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

/// Helper function to map lopdf::Error to FileSystemError.
fn map_lopdf_error_to_fs_error(e: LopdfError) -> FileSystemError {
    // Map specific lopdf errors to FileSystemError variants where appropriate.
    // Otherwise, use InvalidData or IOError.
    match e {
        // Map lopdf's internal IO error to our FileSystemError::IOError
        LopdfError::IOError(io_err) => {
            #[cfg(feature = "std")]
            {
                 map_std_io_error_to_fs_error(io_err) // Map std::io::Error to FileSystemError
            }
            #[cfg(not(feature = "std"))]
            {
                 // Assuming lopdf in no_std uses core::io::Error or similar internally that we can map.
                 // This mapping might need adjustment based on the no_std lopdf's specific error types.
                 // For now, a generic mapping from LopdfError::IOError is used.
                 FileSystemError::IOError(format!("Lopdf IO error: {:?}", io_err)) // Use Debug if core::io::Error Debug impl is available
            }
        },
        // Map parsing errors to FileSystemError::InvalidData
        LopdfError::Parse { pos, message } => FileSystemError::InvalidData(format!("PDF Parse Error at {}: {}", pos, message)),
        LopdfError::Type { object_id, expected_type, actual_type } => FileSystemError::InvalidData(format!("PDF Type Error: Object ID {:?}, Expected {:?}, Got {:?}", object_id, expected_type, actual_type)),
        LopdfError::Missing { object_id, missing_item } => FileSystemError::InvalidData(format!("PDF Missing Item Error: Object ID {:?}, Missing {}", object_id, missing_item)),
        LopdfError::Range { object_id, value, min, max } => FileSystemError::InvalidData(format!("PDF Range Error: Object ID {:?}, Value {}, Range {}-{}", object_id, value, min, max)),
        LopdfError::Other(msg) => FileSystemError::InvalidData(format!("PDF Other Error: {}", msg)),
        LopdfError::StringError(_) => FileSystemError::InvalidData(format!("PDF String encoding error")), // LopdfError::StringError does not expose details readily
        LopdfError::ReferenceValidation { object_id, trace } => FileSystemError::InvalidData(format!("PDF Reference Validation Error: Object ID {:?}, Trace {:?}", object_id, trace)),
        LopdfError::Encryption { .. } => FileSystemError::InvalidData(format!("PDF Encryption Error")),
        LopdfError::Xref(msg) => FileSystemError::InvalidData(format!("PDF Xref Error: {}", msg)),
        LopdfError::DictionaryNotFound => FileSystemError::InvalidData(format!("PDF Dictionary not found")),
        LopdfError::NullObject { object_id } => FileSystemError::InvalidData(format!("PDF Null Object: ID {:?}", object_id)),
        // Add mappings for other LopdfError variants if necessary
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfileoggvorbis.rs'den kopyalandı)
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
     read_exact has a default implementation in core::io::Read that uses read
     read_to_end has a default implementation in core::io::ReadExt that uses read
     read_to_string has a default implementation in core::io::ReadExt that uses read and from_utf8
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

// Removed redundant arch, fs, memory, process, sync, ipc, kernel, SahneError definitions.
// Removed redundant panic handler (assuming one in lib.rs or common module).


/// Represents basic parsed PDF document metadata.
pub struct PdfMetadata {
    pub title: Option<String>, // Requires alloc
    pub author: Option<String>, // Requires alloc
    pub creator: Option<String>, // Requires alloc
    pub producer: Option<Option<String>>, // Requires alloc (lopdf returns Option<String>)
    pub page_count: u32,
}

/// PDF document parser.
/// Uses the lopdf crate to parse the PDF structure from a Read + Seek source.
pub struct PdfParser<R: Read + Seek> {
    #[allow(dead_code)] // Reader might be used in future methods
    reader: R, // Reader implementing Read + Seek
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size

    document: Document, // Store the loaded lopdf Document (requires alloc)
}

impl<R: Read + Seek> PdfParser<R> {
    /// Creates a new `PdfParser` instance by loading the PDF document
    /// from the specified reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Use lopdf::Document::load with the reader
        let document = Document::load(&mut reader).map_err(map_lopdf_error_to_fs_error)?; // Map lopdf error

        Ok(PdfParser {
            reader, // Store the reader
            handle,
            file_size,
            document, // Store the loaded document
        })
    }

    /// Extracts basic metadata from the loaded PDF document.
    ///
    /// # Returns
    ///
    /// A Result containing the PdfMetadata or a FileSystemError if metadata cannot be extracted.
    pub fn get_metadata(&self) -> Result<PdfMetadata, FileSystemError> { // Return FileSystemError
        // Access the document's trailer dictionary and Info dictionary to get metadata
        let info_dict = self.document.get_trailer()
             .and_then(|trailer| trailer.get(b"Info").ok()) // Get the Info object reference
             .and_then(|info_obj| self.document.get_dictionary(info_obj).ok()); // Get the Info dictionary object


        let title = info_dict.and_then(|dict| get_metadata_string_from_dict(dict, b"Title"));
        let author = info_dict.and_then(|dict| get_metadata_string_from_dict(dict, b"Author"));
        let creator = info_dict.and_then(|dict| get_metadata_string_from_dict(dict, b"Creator"));

        // Producer can be a simple string or sometimes a more complex object, lopdf handles this.
        // get_metadata_string_from_dict handles the Option<String> result from lopdf.
         let producer = info_dict.and_then(|dict| dict.get(b"Producer").ok())
            .and_then(|object| object.as_string_custom(None).ok()) // Use as_string_custom to handle different string types if necessary
            .map(|s| s.to_string());


        // Get page count from the document's page tree
        let page_count = self.document.get_pages().len() as u32;


        Ok(PdfMetadata {
            title,
            author,
            creator,
            producer: Some(producer), // Wrap the Option<String> in Some for consistency
            page_count,
        })
    }

    // Add other methods here for accessing pages, objects, etc., using self.document.
     pub fn get_page(&self, page_number: u32) -> Result<Page, FileSystemError> { ... }
}

// Helper function to extract a string value from a PDF dictionary.
// Returns Option<String> if the key exists and the value is a string.
fn get_metadata_string_from_dict(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key)
        .and_then(|object| object.as_string_custom(None).ok()) // Use as_string_custom for robustness
        .map(|s| s.to_string()) // Convert Vec<u8> or Cow<str> to String
}


#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for PdfParser<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the PdfParser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: PdfParser drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a PDF file from the given path (std) or resource ID (no_std)
/// and parses its metadata.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the PdfMetadata or a FileSystemError.
#[cfg(feature = "std")]
pub fn read_pdf_metadata<P: AsRef<Path>>(file_path: P) -> Result<PdfMetadata, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create a PdfParser by loading the document from the reader
    let mut parser = PdfParser::from_reader(reader, None, file_size)?; // Pass None for handle in std version

    // Get and return the metadata
    parser.get_metadata()
}

#[cfg(not(feature = "std"))]
pub fn read_pdf_metadata(file_path: &str) -> Result<PdfMetadata, FileSystemError> { // Return FileSystemError
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

    // Create a PdfParser by loading the document from the reader
    let mut parser = PdfParser::from_reader(reader, Some(handle), file_size)?; // Pass the handle to the parser

    // Get and return the metadata
    parser.get_metadata()
}


// Example main function (no_std)
#[cfg(feature = "example_pdf")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("PDF metadata parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy PDF file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/document.pdf" exists.
      let pdf_res = read_pdf_metadata("sahne://files/document.pdf");
      match pdf_res {
          Ok(metadata) => {
              crate::println!("Parsed PDF Metadata:");
              crate::println!(" Title: {:?}", metadata.title); // Requires Option<String> Debug
              crate::println!(" Author: {:?}", metadata.author);
              crate::println!(" Creator: {:?}", metadata.creator);
              crate::println!(" Producer: {:?}", metadata.producer);
              crate::println!(" Page Count: {}", metadata.page_count);
          },
          Err(e) => crate::eprintln!("Error parsing PDF metadata: {:?}", e),
      }

     eprintln!("PDF metadata parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_pdf")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("PDF metadata parser example (std) starting...");
     eprintln!("PDF metadata parser example (std) using lopdf.");

     // This example needs a dummy PDF file. Using include_bytes for a minimal PDF.
      let pdf_content = include_bytes!("../assets/minimal.pdf");
      let file_path = Path::new("example_minimal.pdf");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(pdf_content).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy PDF file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy PDF file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match read_pdf_metadata(file_path) { // Call the function that opens and parses metadata
         Ok(metadata) => {
             println!("Parsed PDF Metadata:");
             println!(" Title: {:?}", metadata.title); // Requires Option<String> Debug
             println!(" Author: {:?}", metadata.author);
             println!(" Creator: {:?}", metadata.creator);
             println!(" Producer: {:?}", metadata.producer); // Requires Option<String> Debug
             println!(" Page Count: {}", metadata.page_count);

             // Basic assertion based on minimal.pdf content
             assert_eq!(metadata.page_count, 1);
             // assert_eq!(metadata.title, Some("Minimal PDF".to_string())); // Example assertion based on minimal.pdf content
         }
         Err(e) => {
              eprintln!("Error parsing PDF metadata: {}", e); // std error display
              // Don't return error, let cleanup run
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy PDF file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("PDF metadata parser example (std) finished.");

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


     // Include minimal PDF bytes for testing (assuming ../assets/minimal.pdf exists)
     #[cfg(test)]
     #[cfg(feature = "std")] // include_bytes works in std tests
     #[allow(unused)] // The bytes are used below
     const MINIMAL_PDF_BYTES: &[u8] = include_bytes!("../assets/minimal.pdf");


     // Test parsing metadata from a minimal valid PDF in memory
      #[test]
      // #[ignore = "Requires ../assets/minimal.pdf"] // Ignore if file not available
      fn test_read_pdf_metadata_minimal_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError

          // Check if minimal.pdf is available via include_bytes
          #[cfg(test)]
          #[cfg(feature = "std")]
          if MINIMAL_PDF_BYTES.is_empty() {
              println!("Skipping test_read_pdf_metadata_minimal_valid_cursor: ../assets/minimal.pdf not found or empty.");
              return Ok(());
          }


          // Use Cursor as a Read + Seek reader over the minimal PDF bytes
          let pdf_bytes = MINIMAL_PDF_BYTES.to_vec(); // Copy to Vec for Cursor
          let file_size = pdf_bytes.len() as u64;
          let cursor = Cursor::new(pdf_bytes.clone());

          // Create a PdfParser using the cursor reader
          let mut parser = PdfParser::from_reader(cursor, None, file_size)?; // Pass None for handle

          // Get the metadata
          let metadata = parser.get_metadata()?;

          // Assert expected metadata from minimal.pdf
           assert_eq!(metadata.page_count, 1);
           // Example assertions based on minimal.pdf content (adjust if your minimal.pdf is different)
            assert_eq!(metadata.title, Some("Minimal PDF".to_string()));
            assert_eq!(metadata.author, None);
            assert_eq!(metadata.creator, Some("Some Creator".to_string()));
            assert_eq!(metadata.producer, Some(Some("Some Producer".to_string()))); // lopdf::Object::as_string can return None or Some("")


          Ok(())
      }


     // Test handling of invalid PDF data
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_read_pdf_metadata_invalid_data() {
           // Create dummy data that is not a valid PDF
           let dummy_bytes = b"This is not a PDF file.";

           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes.to_vec());
           let mut parser = PdfParser::from_reader(cursor, None, file_size);

           // Attempt to get metadata, expect an error from lopdf parsing
           let result = parser.get_metadata();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from LopdfError::Parse or similar
                   assert!(msg.contains("PDF Parse Error") || msg.contains("PDF Other Error"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of IO errors during lopdf loading (e.g., truncated file)
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_read_pdf_metadata_truncated_file() {
            // Use a truncated version of minimal.pdf bytes
            #[cfg(test)]
            #[cfg(feature = "std")]
            if MINIMAL_PDF_BYTES.is_empty() {
                println!("Skipping test_read_pdf_metadata_truncated_file: ../assets/minimal.pdf not found or empty.");
                return; // Exit test function
            }
            let truncated_bytes = MINIMAL_PDF_BYTES[..MINIMAL_PDF_BYTES.len() / 2].to_vec(); // Truncate


           let file_size = truncated_bytes.len() as u64;
           let cursor = Cursor::new(truncated_bytes.clone()); // Cursor over truncated data
           let mut parser = PdfParser::from_reader(cursor, None, file_size);

           // Attempt to get metadata, expect an error from lopdf parsing due to EOF
           let result = parser.get_metadata();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from LopdfError::IOError(std::io::ErrorKind::UnexpectedEof)
                   assert!(msg.contains("IO Error") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
       }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during lopdf loading,
     // and verifying metadata extraction from mock data. Mocking PDF structure for metadata extraction is complex.
     // A simpler approach for no_std tests might be to mock lopdf's dependencies or return predefined results from the mocked reader.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_pdf", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
