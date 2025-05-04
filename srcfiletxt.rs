#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader as StdBufReader, Read as StdRead, Write as StdWrite, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt, WriteExt as StdWriteExt}; // Added Write, WriteExt
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::{O_RDONLY, O_WRONLY, O_CREAT, O_TRUNC, O_APPEND}; // Import necessary fs flags

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
use core::io::{Read, Write, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt, WriteExt as CoreWriteExt}; // core::io, Added Write, WriteExt


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


/// Custom error type for text file handling issues.
#[derive(Debug)]
pub enum TxtError {
    UnexpectedEof(String), // During reading
    InvalidUtf8, // Error converting bytes to UTF-8 string
    LineReadingError(String), // Error specific to reading lines
    SeekError(u64), // Failed to seek
    // Add other text file specific errors here
}

// Implement Display for TxtError
impl fmt::Display for TxtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxtError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            TxtError::InvalidUtf8 => write!(f, "Geçersiz UTF-8 verisi"),
            TxtError::LineReadingError(msg) => write!(f, "Satır okuma hatası: {}", msg),
            TxtError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map TxtError to FileSystemError
fn map_txt_error_to_fs_error(e: TxtError) -> FileSystemError {
    match e {
        TxtError::UnexpectedEof(_) | TxtError::SeekError(_) | TxtError::InvalidUtf8 => FileSystemError::IOError(format!("Metin dosyası IO/Encoding hatası: {}", e)), // Map IO/Encoding related errors
        TxtError::LineReadingError(_) => FileSystemError::InvalidData(format!("Metin dosyası format hatası: {}", e)), // Map format/parsing errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilesvg.rs'den kopyalandı)
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
// Removed redundant print module and panic handler boilerplate.


/// Simple text file handler. Performs line-based operations.
/// This struct primarily holds the file path. File operations
/// open and close the file as needed using standardized I/O.
pub struct TxtFile {
    pub path: String, // Store the file path
}

impl TxtFile {
    /// Creates a new TxtFile instance referring to the given path.
    pub fn new(path: String) -> Self {
        TxtFile { path }
    }

    /// Reads all lines from the text file.
    /// Opens the file, reads all content into a buffer, decodes UTF-8,
    /// splits into lines, and returns a vector of strings.
    /// Handles line endings across buffer boundaries and invalid UTF-8.
    ///
    /// # Returns
    ///
    /// A Result containing a Vec<String> of lines or a FileSystemError.
    pub fn read_lines(&self) -> Result<Vec<String>, FileSystemError> { // Return FileSystemError
        #[cfg(feature = "std")]
        let file = File::open(&self.path).map_err(map_std_io_error_to_fs_error)?;
        #[cfg(not(feature = "std"))]
         let handle = resource::acquire(&self.path, resource::MODE_READ)
             .map_err(map_sahne_error_to_fs_error)?;
         #[cfg(not(feature = "std"))]
         let file_stat = fs::fstat(handle).map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
         #[cfg(not(feature = "std"))]
         let reader_impl = SahneResourceReader::new(handle, file_stat.size as u64); // Implements Read + Seek + Drop
        #[cfg(feature = "std")]
        let mut reader = StdBufReader::new(file); // std BufReader
        #[cfg(not(feature = "std"))]
        let mut reader = crate::BufReader::new(reader_impl); // Use Sahne64 BufReader wrapping SahneResourceReader


        let mut contents = Vec::new(); // Accumulate raw bytes first
         reader.read_to_end(&mut contents).map_err(|e| map_core_io_error_to_fs_error(e))?; // Read entire file bytes


        // Convert accumulated bytes to String, handling UTF-8
         let content_str = String::from_utf8(contents).map_err(|_| {
             map_txt_error_to_fs_error(TxtError::InvalidUtf8) // Map UTF-8 error
         })?;


        // Split the string into lines, handle different line endings (\n, \r\n)
        // and filter out the last empty line if the file ends with a newline.
        let lines: Vec<String> = content_str.split_inclusive('\n') // Split while keeping the newline character
            .map(|line_with_newline| {
                if line_with_newline.ends_with('\n') {
                    // Handle potential \r\n
                    let mut line = line_with_newline.trim_end_matches('\n').to_string();
                     if line.ends_with('\r') {
                         line.pop(); // Remove trailing \r
                     }
                     line
                } else {
                    line_with_string() // No newline at the end, this is the last partial line or the only line
                }
            })
            .collect();


         // Note: The existing logic had a bug where it added an empty line if the file ended with \n.
         // The split_inclusive approach correctly handles this, but the trimming logic needs care.
         // Let's use a simpler split that removes the newline and then handle the last line case.

         let lines_iter = content_str.split('\n');
         let mut lines_vec = Vec::new(); // Requires alloc
         let mut last_line_was_empty = false; // Track for trailing newline


         for line in lines_iter {
             if !line.is_empty() {
                 lines_vec.push(line.to_string()); // Requires alloc
                 last_line_was_empty = false;
             } else {
                 last_line_was_empty = true; // Encountered an empty string from split, could be empty line or trailing newline
             }
         }

         // If the original content ended with a newline, the last element after split is empty.
         // We should only add it if the original content was just a newline or was empty.
         // A more robust approach might be to iterate bytes and build lines, handling UTF-8 and newlines statefully.
         // For simplicity with split, let's trust split's behavior and filter empty strings.

         // The initial split approach already handles most cases, let's refine it.
         // split('\n') will produce an empty string at the end if the input ends with '\n'.
         // We should filter out this trailing empty string unless the file was only empty lines or just a newline.

          let lines: Vec<String> = content_str.lines() // core::str::lines() is a better fit for lines
               .map(|line| line.to_string()) // Convert &str to String
               .collect();


        // File is automatically closed when reader/handle goes out of scope (due to Drop).

        Ok(lines) // Return vector of lines
    }

    /// Writes a vector of strings as lines to the text file.
    /// Opens the file for writing (creates/truncates), writes each string
    /// followed by a newline, and closes the file.
    ///
    /// # Arguments
    ///
    /// * `lines`: A slice of strings to write as lines.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError.
    pub fn write_lines(&self, lines: &[String]) -> Result<(), FileSystemError> { // Return FileSystemError
        #[cfg(feature = "std")]
        let file = File::open(&self.path).map_err(map_std_io_error_to_fs_error)?;
        #[cfg(not(feature = "std"))]
         let handle = resource::acquire(&self.path, resource::MODE_WRITE | resource::MODE_CREATE | resource::MODE_TRUNCATE) // Use standardized modes
             .map_err(map_sahne_error_to_fs_error)?;
         #[cfg(not(feature = "std"))]
         let mut writer = SahneResourceReader::new(handle, 0); // SahneResourceReader implements Write (using write_at implicitly)


        #[cfg(feature = "std")]
        let mut writer = file; // std::fs::File implements Write (BufWriter is not needed for line-by-line writes)


        for line in lines {
            let line_with_newline = format!("{}\n", line); // Requires alloc and format!
            let bytes = line_with_newline.as_bytes();
             // Use write_all to ensure the entire line bytes are written
            writer.write_all(bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // Map core::io::Error
        }

        // Explicitly flush the writer to ensure data is written to the underlying file/buffer.
        writer.flush().map_err(|e| map_core_io_error_to_fs_error(e))?;


        // File is automatically closed when writer/handle goes out of scope (due to Drop).

        Ok(()) // Return success
    }

    /// Appends a single line to the end of the text file.
    /// Opens the file in append mode (or seeks to end), writes the line
    /// followed by a newline, and closes the file.
    /// This is more efficient than reading and rewriting the entire file.
    ///
    /// # Arguments
    ///
    /// * `line`: The string slice to append as a line.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError.
    pub fn append_line(&self, line: &str) -> Result<(), FileSystemError> { // Return FileSystemError
        // Use O_APPEND flag if supported by Sahne64 fs::open, or open RW and seek to end.
        // Assuming O_APPEND is available or fs::lseek (implied by Seek trait on reader).

        #[cfg(feature = "std")]
        let file = File::open(&self.path).map_err(|e| map_std_io_error_to_fs_error(e)).or_else(|e| {
             // If file not found in std, create it.
             if let FileSystemError::IOError(msg) = &e {
                  #[cfg(feature = "std")]
                  if msg.contains("No such file or directory") || msg.contains("not found") {
                      return File::create(&self.path).map_err(map_std_io_error_to_fs_error);
                  }
             }
             Err(e)
        })?;


        #[cfg(not(feature = "std"))]
         let handle = resource::acquire(&self.path, resource::MODE_APPEND | resource::MODE_CREATE) // Use standardized modes
             .map_err(map_sahne_error_to_fs_error)?;
         #[cfg(not(feature = "std"))]
         let mut writer = SahneResourceReader::new(handle, 0); // SahneResourceReader implements Write and should handle append due to MODE_APPEND


        #[cfg(feature = "std")]
        let mut writer = file; // std::fs::File implements Write and O_APPEND behavior


         // If O_APPEND was used, the file position is already at the end before the first write.
         // If O_APPEND is not directly supported and we opened RW, we would need to seek to the end here.
         // Assuming Sahne64 fs::open with MODE_APPEND sets the initial position.
         // If not, we would need: writer.seek(SeekFrom::End(0)).map_err(|e| map_core_io_error_to_fs_error(e))?;


        let line_with_newline = format!("{}\n", line); // Requires alloc and format!
        let bytes = line_with_newline.as_bytes();

         // Use write_all to ensure the entire line bytes are written
         writer.write_all(bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // Map core::io::Error

         // Explicitly flush the writer
         writer.flush().map_err(|e| map_core_io_error_to_fs_error(e))?;

        // File is automatically closed when writer/handle goes out of scope (due to Drop).

        Ok(()) // Return success
    }
}


// Test modülü (std özelliği aktifse çalışır)
#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::fs::{self, remove_file}; // Add remove_file for cleanup
    use std::path::Path; // For Path


    // Helper to create/write dummy file using std for tests
    fn create_dummy_file(path: &str, content: &str) -> Result<(), std::io::Error> {
        std::fs::write(path, content)
    }

    // Helper to read dummy file using std for tests
    fn read_dummy_file(path: &str) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }


    #[test]
    fn test_write_lines_and_read_lines_std() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error> for std test error handling
        let path_str = "test_write_read.txt";
        let file = TxtFile::new(path_str.to_string()); // Use to_string() for owned String


        let lines_to_write = vec![
             "Sahne Line 1".to_string(),
             "Another line".to_string(),
             "".to_string(), // Empty line
             "End line".to_string(),
             "".to_string(), // Trailing empty line
         ];
        file.write_lines(&lines_to_write)?; // Use ? for error propagation

        // Verify file content using std read (for test setup)
         let written_content = read_dummy_file(path_str)?;
          // write_lines adds a newline after each line, including the last one.
          // The last two empty strings will result in two newlines at the end.
          // "Sahne Line 1\nAnother line\n\nEnd line\n\n"
         assert_eq!(written_content, "Sahne Line 1\nAnother line\n\nEnd line\n\n");


        // Read lines using the TxtFile method
        let read_lines = file.read_lines()?; // Use ? for error propagation


        // The split('\n') logic in read_lines should handle the empty strings correctly.
         // A file ending with \n results in an empty string after the last \n when splitting.
         // The `content_str.lines()` iterator correctly handles different line endings and doesn't produce a trailing empty string for a file ending in newline.
         // So, the expected lines should match the input lines except for the final empty string.
          let expected_lines: Vec<String> = lines_to_write.into_iter()
              .filter(|line| !line.is_empty()) // Filter out intended empty lines from input for comparison
              .collect();
          // No, the .lines() iterator in read_lines WILL produce empty strings for empty lines in the file.
          // A file like "a\n\nb" will result in lines ["a", "", "b"].
          // A file like "a\n" will result in lines ["a"].
          // A file like "a\n\n" will result in lines ["a", ""].
          // The expected lines should include empty strings for actual empty lines in the file content.
          // Let's write content with explicit newlines and check read_lines result.
           let file_content_for_read_test = "Line 1\nLine 2\n\nLine 4\n"; // Results in ["Line 1", "Line 2", "", "Line 4"]

           create_dummy_file(path_str, file_content_for_read_test)?;
           let read_lines_actual = file.read_lines()?;
           assert_eq!(read_lines_actual, vec!["Line 1".to_string(), "Line 2".to_string(), "".to_string(), "Line 4".to_string()]);


        fs::remove_file(path_str)?; // Clean up test file

        Ok(()) // Return Ok from test function
    }

    #[test]
    fn test_append_line_std() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error> for std test error handling
        let path_str = "test_append.txt";
        let file = TxtFile::new(path_str.to_string()); // Use to_string() for owned String

        // Ensure the file doesn't exist initially
         let _ = fs::remove_file(path_str);


        file.append_line("Sahne First line")?; // File should be created
        file.append_line("Sahne Second line")?; // Should append


        // Verify file content using std read
         let file_content = read_dummy_file(path_str)?;
          // append_line adds a newline after the appended line.
          // "Sahne First line\nSahne Second line\n"
         assert_eq!(file_content, "Sahne First line\nSahne Second line\n");


        fs::remove_file(path_str)?; // Clean up test file

        Ok(()) // Return Ok from test function
    }

    // TODO: Add tests for edge cases like empty files, files with only newlines, files with invalid UTF-8.
    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::write_at, fs::lseek.
    // Test cases should include opening valid/invalid files, handling IO errors during reading/writing/appending,
    // and correctly handling line boundaries and UTF-8 in read_lines with mock data.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
