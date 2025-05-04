// srcfileoggvorbis.rs
// Ogg Vorbis parser for Sahne64 (Basic no_std implementation)

#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt}; // Include ReadExt for read_to_end, read_to_string
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use lewton::inside_ogg::OggStreamReader; // Use lewton crate for std parsing


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
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt}; // core::io, Include ReadExt


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


/// Custom error type for Ogg Vorbis parsing/decoding issues.
#[derive(Debug)]
pub enum OggVorbisError {
    // Use a generic error kind for errors from underlying libraries (like lewton)
    ParsingError(String), // Use String for error messages from underlying parser
    DecodingError(String), // Use String for error messages from underlying decoder
    UnexpectedEof, // During basic header read in no_std stub
    InvalidHeader, // Basic header check failed in no_std stub
    SeekError(u64), // Failed to seek
    // Add other Ogg Vorbis specific errors here
}

// Implement Display for OggVorbisError
impl fmt::Display for OggVorbisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OggVorbisError::ParsingError(msg) => write!(f, "Ogg Vorbis ayrıştırma hatası: {}", msg),
            OggVorbisError::DecodingError(msg) => write!(f, "Ogg Vorbis çözme hatası: {}", msg),
            OggVorbisError::UnexpectedEof => write!(f, "Beklenmedik dosya sonu (başlık okurken)"),
            OggVorbisError::InvalidHeader => write!(f, "Geçersiz Ogg Vorbis başlığı (basit kontrol)"),
            OggVorbisError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map OggVorbisError to FileSystemError
fn map_oggvorbis_error_to_fs_error(e: OggVorbisError) -> FileSystemError {
    match e {
        OggVorbisError::UnexpectedEof | OggVorbisError::SeekError(_) => FileSystemError::IOError(format!("Ogg Vorbis IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("Ogg Vorbis ayrıştırma hatası: {}", e)), // Map parsing/decoding errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfileodf.rs'den kopyalandı)
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


// Removed redundant arch, memory, process, sync, kernel, SahneError imports.
// Removed redundant print module and panic handler.
// Removed local collections::Vec definition. Assume alloc::vec::Vec.


/// Ogg Vorbis File parser/decoder.
/// In std environment, uses the lewton crate.
/// In no_std environment, this is currently a stub due to decoding complexity.
pub struct OggVorbisFile<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks

    pub sample_rate: u32,
    pub channels: u8,
    pub vendor: String, // Requires alloc
    pub comments: Vec<(String, String)>, // Requires alloc

    #[cfg(feature = "std")]
    ogg_reader: OggStreamReader<R>, // Store lewton reader in std
}

impl<R: Read + Seek> OggVorbisFile<R> {
    /// Creates a new `OggVorbisFile` instance from a reader and parses headers.
    /// This is used internally after opening the file/resource.
    #[cfg(feature = "std")]
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Use lewton to create the stream reader
        let mut ogg_reader = OggStreamReader::new(reader).map_err(|e| map_oggvorbis_error_to_fs_error(OggVorbisError::ParsingError(format!("Lewton init error: {}", e))))?; // Map lewton error to FileSystemError

        // Extract metadata from headers
        let sample_rate = ogg_reader.ident_hdr.audio_sample_rate;
        let channels = ogg_reader.ident_hdr.audio_channels;
        let vendor = ogg_reader.comment_hdr.vendor.clone(); // Requires alloc
        let comments = ogg_reader.comment_hdr.comments.clone(); // Requires alloc

        Ok(OggVorbisFile {
            reader: ogg_reader.into_inner(), // Get the reader back from lewton
            handle,
            file_size,
            sample_rate,
            channels,
            vendor,
            comments,
            ogg_reader, // Store the lewton reader
        })
    }

    /// Creates a new `OggVorbisFile` instance from a reader (no_std stub).
    #[cfg(not(feature = "std"))]
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Basic header read for identification (minimal stub)
        let mut header_buffer = [0u8; 4]; // Ogg page start pattern "OggS"
         reader.read_exact(&mut header_buffer).map_err(|e| match e.kind() {
              core::io::ErrorKind::UnexpectedEof => map_oggvorbis_error_to_fs_error(OggVorbisError::UnexpectedEof),
              _ => map_core_io_error_to_fs_error(e),
         })?;


        if &header_buffer != b"OggS" {
             return Err(map_oggvorbis_error_to_fs_error(OggVorbisError::InvalidHeader));
        }

        // Skip the rest of the minimal page header for now (22 bytes total for page header)
        // https://wiki.xiph.org/Ogg
        // After "OggS" (4 bytes), there are 22 more bytes in the page header.
        let remaining_header_size = 22;
        reader.seek(SeekFrom::Current(remaining_header_size as i64)).map_err(|e| map_core_io_error_to_fs_error(e))?;


        // A real parser would now read identification, comment, and setup headers
        // to get sample rate, channels, vendor, and comments. This is complex
        // without a library. We'll use default/empty values for the stub.

        let sample_rate = 0; // Stub value
        let channels = 0; // Stub value
        let vendor = String::new(); // Requires alloc
        let comments = Vec::new(); // Requires alloc


        Ok(OggVorbisFile {
            reader,
            handle,
            file_size,
            sample_rate,
            channels,
            vendor,
            comments,
        })
    }


    /// Reads decoded audio data packets from the Ogg Vorbis stream.
    #[cfg(feature = "std")]
    pub fn read_audio_data(&mut self) -> Result<Vec<i16>, FileSystemError> { // Return FileSystemError
        let mut audio_data = Vec::new(); // Requires alloc
        // Use the stored lewton reader to read decoded packets
        while let Some(packet) = self.ogg_reader.read_dec_packet_generic::<i16>().map_err(|e| map_oggvorbis_error_to_fs_error(OggVorbisError::DecodingError(format!("Lewton decoding error: {}", e))))? { // Map lewton error
            audio_data.extend(packet); // Requires alloc
        }

        Ok(audio_data)
    }

    /// Reads decoded audio data packets (no_std stub).
    #[cfg(not(feature = "std"))]
    pub fn read_audio_data(&mut self) -> Result<Vec<i16>, FileSystemError> { // Return FileSystemError
        // In a real no_std implementation, this would involve significant
        // Ogg bitstream parsing and Vorbis decoding logic or a no_std compatible library.
        // This is a placeholder.
        #[cfg(not(feature = "std"))]
        crate::eprintln!("WARNING: read_audio_data not implemented in no_std for OggVorbisFile.");
        Ok(Vec::new()) // Return empty vector in the stub
    }


    /// Provides a mutable reference to the internal reader. Use with caution.
     pub fn reader(&mut self) -> &mut R {
         &mut self.reader
     }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for OggVorbisFile<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the OggVorbisFile is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: OggVorbisFile drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens an Ogg Vorbis file from the given path (std) or resource ID (no_std)
/// and creates an OggVorbisFile instance by parsing headers.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the OggVorbisFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_oggvorbis_file<P: AsRef<Path>>(file_path: P) -> Result<OggVorbisFile<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (needed for SahneResourceReader in no_std, but good practice)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create OggVorbisFile by parsing headers from the reader
    OggVorbisFile::from_reader(reader, None, file_size) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_oggvorbis_file(file_path: &str) -> Result<OggVorbisFile<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    // Create OggVorbisFile by parsing headers (minimal stub)
    OggVorbisFile::from_reader(reader, Some(handle), file_size) // Pass the handle to the OggVorbisFile
}


// Example main function (no_std)
#[cfg(feature = "example_oggvorbis")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("Ogg Vorbis parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy Ogg Vorbis file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/audio.ogg" exists.
     // let ogg_res = open_oggvorbis_file("sahne://files/audio.ogg");
     // match ogg_res {
     //     Ok(mut ogg_file) => { // Need mut to read audio data
     //         crate::println!("Ogg Vorbis file loaded (stub).");
     //         crate::println!(" Sample Rate (stub): {}", ogg_file.sample_rate);
     //         crate::println!(" Channels (stub): {}", ogg_file.channels);
     //         crate::println!(" Vendor (stub): {}", ogg_file.vendor); // Requires String Display
     //         crate::println!(" Comments (stub): {} adet", ogg_file.comments.len());
     //
     //         // Reading audio data is not implemented in the no_std stub
     //         // match ogg_file.read_audio_data() {
     //         //     Ok(audio_data) => {
     //         //         crate::println!("Read {} samples of audio data (stub).", audio_data.len());
     //         //         // Process audio data here
     //         //     },
     //         //     Err(e) => crate::eprintln!("Error reading audio data: {:?}", e),
     //         // }
     //
     //         // File is automatically closed when ogg_file goes out of scope (due to Drop)
     //     },
     //     Err(e) => crate::eprintln!("Error opening Ogg Vorbis file: {:?}", e),
     // }

     eprintln!("Ogg Vorbis parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_oggvorbis")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("Ogg Vorbis parser example (std) starting...");
     eprintln!("Ogg Vorbis parser example (std) using lewton.");

     // This example needs a dummy Ogg Vorbis file. Creating a valid one from scratch is complex.
     // You might need a pre-existing minimal Ogg Vorbis file for this example.
     // For testing purposes, the test below creates a very minimal dummy file, but it's not a valid Ogg Vorbis file.

     let file_path = Path::new("example.ogg"); // This file needs to exist and be a valid Ogg Vorbis file

      // Create a very minimal dummy file just to open something (NOT a valid Ogg Vorbis)
      // This is ONLY for the example to have a file path to open.
      use std::fs::remove_file;
      use std::io::Write;
       if !file_path.exists() {
           match File::create(file_path) {
               Ok(mut file) => {
                   // A few bytes to make the file exist. Not valid Ogg Vorbis.
                   if let Err(e) = file.write_all(b"OggS\x00\x00\x00\x00").map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy file: {}", e);
                       // Continue even on error, open_oggvorbis_file will likely fail correctly
                   }
               },
               Err(e) => {
                   eprintln!("Error creating dummy file: {}", e);
                   // Continue even on error
               }
           }
       }


     match open_oggvorbis_file(file_path) { // Call the function that opens and parses
         Ok(mut ogg_file) => { // Need mut to read audio data
             println!("Ogg Vorbis file loaded (std, using lewton).");
             println!(" Sample Rate: {}", ogg_file.sample_rate);
             println!(" Channels: {}", ogg_file.channels);
             println!(" Vendor: {}", ogg_file.vendor);
             println!(" Comments: {} adet", ogg_file.comments.len());

             // Example: Print comments
             for (key, value) in &ogg_file.comments {
                  println!("  Comment: {} = {}", key, value);
             }


             // Example: Read audio data (requires a valid Ogg Vorbis file)
             match ogg_file.read_audio_data() {
                 Ok(audio_data) => {
                     println!("Read {} samples of audio data.", audio_data.len());
                     // Process audio data here (e.g., play it)
                     // For this example, just printing length is enough.
                 },
                 Err(e) => {
                     eprintln!("Error reading audio data: {}", e); // std error display
                     // This is expected if the dummy file was not valid Ogg Vorbis.
                 }
             }

             // File is automatically closed when ogg_file goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening Ogg Vorbis file: {}", e); // std error display
              // This is expected if the dummy file was not valid Ogg Vorbis.
         }
     }

     // Clean up the dummy file (if it was created)
      if file_path.exists() {
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("Ogg Vorbis parser example (std) finished.");

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


     // Test creating a dummy Ogg Vorbis file (minimal OggS header) for basic no_std stub testing
      #[test]
      fn test_create_dummy_ogg_stub_bytes() -> Result<(), Box<dyn std::error::Error>> {
          let mut buffer = Cursor::new(Vec::new());
           buffer.write_all(b"OggS")?; // OggS magic
           buffer.write_all(&[0u8; 22])?; // Rest of minimal page header
           // Add some dummy data
           buffer.write_all(b"some dummy data")?;

           let dummy_bytes = buffer.into_inner();
           assert!(dummy_bytes.len() > 26); // OggS (4) + header (22) = 26 minimum

           // Test reading with a Cursor and the no_std from_reader stub
           let file_size = dummy_bytes.len() as u64;
           let cursor = Cursor::new(dummy_bytes.clone());
           let mut parser_stub = JpegParser::from_reader(cursor, None, file_size); // Use JpegParser to get Read+Seek, doesn't matter which

           // This test can only verify that the no_std from_reader stub doesn't panic or return unexpected error
           // when reading minimal header. It can't verify correct Ogg Vorbis parsing.

           // Create a dummy OggVorbisFile using the no_std from_reader with the cursor
           let cursor_for_ogg_stub = Cursor::new(dummy_bytes.clone());
            // Note: open_oggvorbis_file wraps from_reader, but from_reader expects Reader.
            // We simulate calling from_reader directly for testing the stub logic.
            // In real no_std tests, we would mock fs/resource and use open_oggvorbis_file.

            // The no_std from_reader needs a Reader<R: Read+Seek>, let's use Cursor.
            let mut cursor_for_stub = Cursor::new(dummy_bytes.clone());
            let file_size_for_stub = dummy_bytes.len() as u64;

            // Call the no_std specific from_reader
            let result_stub = OggVorbisFile::from_reader(cursor_for_stub, None, file_size_for_stub);


           assert!(result_stub.is_ok());
           let ogg_file_stub = result_stub.unwrap();

           // Assert stub values
           assert_eq!(ogg_file_stub.sample_rate, 0);
           assert_eq!(ogg_file_stub.channels, 0);
           assert!(ogg_file_stub.vendor.is_empty());
           assert!(ogg_file_stub.comments.is_empty());

           // Test read_audio_data stub
           let audio_data_stub = ogg_file_stub.read_audio_data()?;
           assert!(audio_data_stub.is_empty()); // Expect empty vector from stub


           Ok(())
      }


     // Add std tests for actual Ogg Vorbis parsing using lewton.
     // These tests require a valid Ogg Vorbis file or generating one (complex).
     // For now, we will use a pre-existing minimal valid Ogg Vorbis file if available,
     // or skip full parsing tests if not.

     // Example: Test parsing a known minimal valid Ogg Vorbis file
      #[test]
      // #[ignore = "Requires a pre-existing minimal_valid.ogg file"] // Ignore if no file available
      fn test_parse_ogg_vorbis_std_valid() -> Result<(), FileSystemError> { // Return FileSystemError

          // This test REQUIRES a valid, minimal Ogg Vorbis file named "minimal_valid.ogg"
          // in the directory where tests are run.
          let file_path = Path::new("minimal_valid.ogg");

           if !file_path.exists() {
               // Skip the test if the required file doesn't exist.
               // In a real project, you might generate a test file or include it in the repo.
               #[cfg(test)]
               println!("Skipping test_parse_ogg_vorbis_std_valid: minimal_valid.ogg not found.");
               return Ok(());
           }


           // Use the std open_oggvorbis_file function
           let mut ogg_file = open_oggvorbis_file(file_path)?;

           // Assert expected metadata from the minimal valid file
           // These values depend on the specific minimal_valid.ogg file used.
           // Example: Assuming a file with 44100 sample rate, 2 channels, specific vendor/comments.
           // Replace with actual expected values from your test file.
           #[cfg(test)]
            {
               println!("Test: Parsed Sample Rate: {}", ogg_file.sample_rate);
               println!("Test: Parsed Channels: {}", ogg_file.channels);
               println!("Test: Parsed Vendor: {}", ogg_file.vendor);
               println!("Test: Parsed Comments Count: {}", ogg_file.comments.len());
            }


           assert!(ogg_file.sample_rate > 0); // Should be parsed correctly
           assert!(ogg_file.channels > 0); // Should be parsed correctly
           // assert_eq!(ogg_file.sample_rate, 44100); // Example specific assertion
           // assert_eq!(ogg_file.channels, 2); // Example specific assertion
           // assert_eq!(ogg_file.vendor, "Xiph.Org libVorbis I 20xxabcdef"); // Example
           // assert!(ogg_file.comments.iter().any(|(k, v)| k == "ARTIST" && v == "Test Artist")); // Example


           // Test reading some audio data
           let audio_data = ogg_file.read_audio_data()?;
           assert!(!audio_data.is_empty()); // Should read some data

           // Further assertions on audio_data content would depend on the test file.

           Ok(())
      }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during basic header read,
     // and verifying the stub behavior (returning default metadata and empty audio data).
     // Mocking the Ogg bitstream parsing for full no_std testing is complex.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_oggvorbis", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
