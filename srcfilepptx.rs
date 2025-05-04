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
#[cfg(feature = "std")]
use zip::ZipArchive; // Use std zip crate
#[cfg(feature = "std")]
use zip::read::ZipFile as StdZipFile; // Use std zip file type
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // Use std Cursor for in-memory read in tests/examples


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

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

/// Helper function to map zip::Error to FileSystemError.
#[cfg(any(feature = "std", doc))] // zip crate is typically std-dependent, include for std or documentation builds
fn map_zip_error_to_fs_error(e: zip::Error) -> FileSystemError {
    match e {
        zip::Error::Io(io_err) => {
            #[cfg(feature = "std")]
             map_std_io_error_to_fs_error(io_err) // Map std::io::Error
            #[cfg(not(feature = "std"))] // This branch should ideally not be reached if zip is std-only
            FileSystemError::IOError(format!("Zip IO error: {:?}", io_err)) // Fallback mapping
        },
        zip::Error::InvalidArchive(msg) => FileSystemError::InvalidData(format!("Zip Invalid Archive: {}", msg)),
        zip::Error::UnsupportedArchive(msg) => FileSystemError::InvalidData(format!("Zip Unsupported Archive: {}", msg)),
        zip::Error::UnsupportedFeature(msg) => FileSystemError::InvalidData(format!("Zip Unsupported Feature: {}", msg)),
        zip::Error::FileNotFound => FileSystemError::FileNotFound(format!("Zip entry not found")), // Map zip's FileNotFound
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


/// PPTX file handler. Represents a PPTX file as a ZIP archive.
/// Currently provides basic functionality to open the file and read a specific entry.
/// A full PPTX handler would involve XML parsing of specific entries.
pub struct PptxFile<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    handle: Option<Handle>, // Use Option<Handle> for resource management
    #[allow(dead_code)] // File size might be useful
    file_size: u64, // Store file size

    #[cfg(feature = "std")]
    zip_archive: ZipArchive<R>, // Store ZipArchive in std
}

impl<R: Read + Seek> PptxFile<R> {
    /// Creates a new `PptxFile` instance by opening the underlying file and attempting
    /// to treat it as a ZIP archive.
    /// This is used internally after opening the file/resource.
    #[cfg(feature = "std")]
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Use zip::ZipArchive::new with the reader
        let zip_archive = ZipArchive::new(reader).map_err(map_zip_error_to_fs_error)?; // Map zip error

        Ok(PptxFile {
            reader: zip_archive.get_mut().map_err(|_| FileSystemError::InvalidData(String::from("Failed to get reader from ZipArchive"))).expect("ZipArchive should contain a reader"), // Get the reader back from ZipArchive
            handle,
            file_size,
            zip_archive, // Store the ZipArchive
        })
    }

    /// Creates a new `PptxFile` instance from a reader (no_std stub).
    /// A real no_std implementation would require a no_std compatible zip crate
    /// and potentially parse basic ZIP header information here.
    #[cfg(not(feature = "std"))]
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // In a real no_std implementation, you would validate the file as a ZIP archive
        // by reading the End of Central Directory Record or Central Directory Header.
        // This requires seeking to the end of the file and reading backwards, or parsing
        // the Central Directory header entries.
        // For this refactor, this remains a stub.

        #[cfg(not(feature = "std"))]
        crate::eprintln!("WARNING: PptxFile::from_reader not implemented in no_std for ZIP parsing.");
        // TODO: Add basic ZIP header check in no_std if possible without full zip crate.
        // For now, assume it's a valid file and return a stub.

        Ok(PptxFile {
            reader, // Store the reader
            handle,
            file_size,
        })
    }


    /// Reads the uncompressed data of a specific entry (file) within the PPTX (ZIP) archive.
    ///
    /// # Arguments
    ///
    /// * `entry_name` - The name of the entry (e.g., "ppt/presentation.xml").
    ///
    /// # Returns
    ///
    /// A Result containing the entry's data as Vec<u8> or FileSystemError.
    #[cfg(feature = "std")]
    pub fn read_entry_data(&mut self, entry_name: &str) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
        // Open the specified entry within the zip archive
        let mut zip_entry = self.zip_archive.by_name(entry_name).map_err(map_zip_error_to_fs_error)?; // Map zip error

        // Read the entry's data into a Vec<u8>
        let mut data = Vec::with_capacity(zip_entry.size() as usize); // Allocate with entry size hint
        zip_entry.read_to_end(&mut data).map_err(map_std_io_error_to_fs_error)?; // Map std::io::Error from zip entry reader

        Ok(data)
    }

    /// Reads the uncompressed data of a specific entry (no_std stub).
    #[cfg(not(feature = "std"))]
    pub fn read_entry_data(&mut self, _entry_name: &str) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
         // In a real no_std implementation, this would require a no_std compatible zip crate
         // or manual parsing of the ZIP structure using self.reader to find the entry,
         // seek to its data, and decompress it.
         #[cfg(not(feature = "std"))]
         crate::eprintln!("WARNING: read_entry_data not implemented in no_std for PptxFile.");
         Err(FileSystemError::NotSupported(String::from("Reading zip entries not supported in no_std stub"))) // Indicate functionality is not supported
    }


    /// Provides a mutable reference to the internal reader (use with caution).
    /// Useful if external libraries need a Read+Seek instance.
     pub fn reader(&mut self) -> &mut R {
         &mut self.reader
     }

     #[cfg(feature = "std")]
     /// Provides a mutable reference to the internal ZipArchive (std only).
     pub fn zip_archive(&mut self) -> &mut ZipArchive<R> {
         &mut self.zip_archive
     }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for PptxFile<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the PptxFile is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: PptxFile drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a PPTX file from the given path (std) or resource ID (no_std)
/// and creates a PptxFile instance.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the PptxFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_pptx_file<P: AsRef<Path>>(file_path: P) -> Result<PptxFile<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create PptxFile by opening the zip archive from the reader
    PptxFile::from_reader(reader, None, file_size) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_pptx_file(file_path: &str) -> Result<PptxFile<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    // Create PptxFile from the reader (no_std stub)
    PptxFile::from_reader(reader, Some(handle), file_size) // Pass the handle to the PptxFile
}


// Example main function (no_std)
#[cfg(feature = "example_pptx")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("PPTX file handler example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy PPTX (ZIP) file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/presentation.pptx" exists.
      let pptx_res = open_pptx_file("sahne://files/presentation.pptx");
      match pptx_res {
          Ok(mut pptx_file) => { // Need mut to read entries
              crate::println!("PPTX file loaded (no_std stub).");
               Reading specific entries is not supported in the no_std stub
               match pptx_file.read_entry_data("ppt/presentation.xml") {
                   Ok(data) => {
                       crate::println!("Read {} bytes for entry 'ppt/presentation.xml' (stub).", data.len());
     //                  // Process data here (requires XML parsing in no_std)
                   },
                   Err(e) => crate::eprintln!("Error reading entry data: {:?}", e),
               }
     //
     //         // File is automatically closed when pptx_file goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening PPTX file: {:?}", e),
      }

     eprintln!("PPTX file handler example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_pptx")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("PPTX file handler example (std) starting...");
     eprintln!("PPTX file handler example (std) using zip crate.");

     // This example needs a dummy PPTX file (which is a ZIP archive).
     // Creating a valid PPTX is complex. For demonstration, we can create a minimal ZIP
     // with a single entry to simulate the structure.
      let mut zip_bytes: Vec<u8> = Vec::new();
       // Use the zip crate to create a minimal zip archive in memory
       let writer = StdCursor::new(&mut zip_bytes);
       let mut zip_writer = zip::ZipWriter::new(writer);

       let entry_name = "ppt/presentation.xml";
       let entry_data = b"<p:presentation/>"; // Minimal XML content

       zip_writer.start_file(entry_name, zip::write::FileOptions::default()).map_err(|e| map_zip_error_to_fs_error(e))?;
       zip_writer.write_all(entry_data).map_err(|e| map_std_io_error_to_fs_error(e))?;

       zip_writer.finish().map_err(|e| map_zip_error_to_fs_error(e))?;

       // Now zip_bytes contains the minimal zip archive.
       let file_path = Path::new("example.pptx");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&zip_bytes).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy PPTX file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy PPTX file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_pptx_file(file_path) { // Call the function that opens and creates the handler
         Ok(mut pptx_file) => { // Need mut to read entries
             println!("PPTX file loaded (std, using zip crate).");
             #[cfg(feature = "std")]
             println!(" Number of entries: {}", pptx_file.zip_archive().len());

             // Example: Read a specific entry's data
             match pptx_file.read_entry_data(entry_name) {
                 Ok(data) => {
                     println!("Read {} bytes for entry '{}'.", data.len(), entry_name);
                     // Process data here (requires XML parsing)
                     #[cfg(feature = "std")] // Only in std where String::from_utf8 is readily available
                     match String::from_utf8(data) {
                         Ok(content) => println!(" Entry content: {}", content),
                         Err(_) => println!(" Entry data is not valid UTF-8."),
                     }
                 },
                 Err(e) => {
                     eprintln!("Error reading entry data: {}", e); // std error display
                 }
             }

             // File is automatically closed when pptx_file goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening PPTX file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy PPTX file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("PPTX file handler example (std) finished.");

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
      use zip::ZipWriter;
      use zip::write::FileOptions;


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec::Vec; // For Vec
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests


     // Helper function to create a minimal ZIP archive in memory
      #[cfg(feature = "std")] // Uses std::io::Cursor and zip::ZipWriter
       fn create_minimal_zip_bytes(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(Vec::new());
           let mut zip_writer = ZipWriter::new(buffer);

           for (name, data) in entries {
               zip_writer.start_file(name, FileOptions::default())?;
               zip_writer.write_all(data)?;
           }

           let finished_buffer = zip_writer.finish()?;
           Ok(finished_buffer.into_inner())
       }


     // Test opening a minimal zip archive and reading an entry
     #[test]
     fn test_open_pptx_file_read_entry_std_cursor() -> Result<(), FileSystemError> { // Return FileSystemError

          // Create minimal zip bytes with one entry
          let entry_name = String::from("test_entry.txt");
          let entry_data = b"This is test data inside the zip.";
          let zip_bytes = create_minimal_zip_bytes(&[(entry_name.clone(), entry_data.to_vec())])
               .map_err(|e| FileSystemError::IOError(format!("Test zip creation error: {}", e)))?;


          // Use Cursor as a Read + Seek reader
          let file_size = zip_bytes.len() as u64;
          let cursor = Cursor::new(zip_bytes.clone());

          // Create a PptxFile from the cursor reader (std version uses ZipArchive)
          let mut pptx_file = PptxFile::from_reader(cursor, None, file_size)?; // Pass None for handle

          // Assert the zip archive contains the expected entry
          #[cfg(feature = "std")]
          assert!(pptx_file.zip_archive().by_name(&entry_name).is_ok());

          // Read the entry data
          let read_data = pptx_file.read_entry_data(&entry_name)?;

          // Assert the read data is correct
          assert_eq!(read_data, entry_data);

          // Test reading a non-existent entry
           let result = pptx_file.read_entry_data("non_existent.txt");
           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::FileNotFound(msg) => { // Mapped from zip::Error::FileNotFound
                   assert!(msg.contains("Zip entry not found"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }


          Ok(())
     }

     // Test handling of invalid zip archive data
      #[test]
      fn test_open_pptx_file_invalid_archive_std_cursor() {
           // Create dummy data that is not a valid zip archive
           let dummy_bytes = b"This is not a zip file.";

           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes.to_vec());

           // Attempt to create PptxFile, expect an error from zip::ZipArchive::new
           let result = PptxFile::from_reader(cursor, None, file_size);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from zip::Error::InvalidArchive or Io
                   assert!(msg.contains("Zip Invalid Archive") || msg.contains("Zip IO error"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors,
     // and verifying the stub behavior for entry reading (returning NotSupported error).
     // Mocking the zip structure parsing for full no_std testing is complex.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_pptx", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
