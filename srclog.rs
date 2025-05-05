#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std")), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "std")]
use std::io::{self, Write as StdWrite, Error as StdIOError, ErrorKind as StdIOErrorKind, WriteExt as StdWriteExt}; // Added WriteExt
#[cfg(feature = "std")]
use std::sync::Mutex as StdMutex; // Use std Mutex in std
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::error::Error as StdError; // For std Error trait
#[cfg(feature = "std")]
use chrono::Local; // For std timestamp

// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::{O_CREAT, O_APPEND, O_WRONLY}; // Import necessary fs flags


use spin::Mutex; // Use spin Mutex for no_std consistency


use alloc::string::{String, ToString}; // Requires alloc
use alloc::vec::Vec; // Requires alloc
use alloc::format; // Requires alloc


// core::fmt, core::result, core::ops::Drop, core::io
use core::fmt;
use core::result::Result;
use core::ops::Drop; // For Drop trait
use core::io::{Write, Error as CoreIOError, ErrorKind as CoreIOErrorKind, WriteExt as CoreWriteExt}; // core::io


// Removed redundant imports from core, cfg(not(feature="std")), etc.
// Removed redundant panic handler boilerplate.


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


// Sahne64 Handle'ı için core::io::Write implementasyonu (simplified from SahneResourceReadWriteSeek)
// This requires fs::write_at. Append mode might need explicit seeking to end before writing.
#[cfg(not(feature = "std"))]
pub struct SahneResourceWrite { // Write-only resource wrapper
    handle: Handle,
    // Append mode often implies writing at the end, explicit position tracking might be complex.
    // Relying on fs::write_at at offset 0 or implicitly at the end for append mode.
    // Let's assume fs::write_at with offset 0 for simplicity here, simulating append.
    // A true append mode might require seeking to the end before each write or a dedicated fs::append_at syscall.
    // For this logger, append is crucial. Let's assume `fs::write` *always* appends when opened with O_APPEND.
    // If not, we need Seek and seek to End before each write.
    // Let's assume fs::write with O_APPEND works as expected without explicit seek to end.
}

#[cfg(not(feature = "std"))]
impl SahneResourceWrite {
    pub fn new(handle: Handle) -> Self {
        SahneResourceWrite { handle }
    }
}

#[cfg(not(feature = "std"))]
impl core::io::Write for SahneResourceWrite { // Use core::io::Write trait
    fn write(&mut self, buf: &[u8]) -> Result<usize, core::io::Error> { // Return core::io::Error
         // Assuming fs::write(handle, buf) writes at the current position or appends with O_APPEND.
         // Let's assume it appends for O_APPEND.
         let bytes_to_write = buf.len();
         if bytes_to_write == 0 { return Ok(0); }

         // Assuming fs::write(handle, buf) Result<usize, SahneError>
         let bytes_written = fs::write(self.handle, buf) // Use the basic fs::write
             .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::write error: {:?}", e)))?; // Map SahneError to core::io::Error

         Ok(bytes_written) // Return bytes written
    }

     fn flush(&mut self) -> Result<(), core::io::Error> {
         // Assuming fs::flush(handle) or sync() is available for durability.
         // If not, this is a no-op or needs a different syscall.
         // For logging, flushing is important for durability. Assume fs::flush is available.
         #[cfg(not(feature = "no_fs_flush"))] // Assume fs::flush is available unless this feature is set
         fs::flush(self.handle).map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::flush error: {:?}", e)))?;
         #[cfg(feature = "no_fs_flush")] // If fs::flush is not available
         { /* No-op or alternative sync method */ }
         Ok(())
     }
}

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceWrite {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceWrite is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              // Attempt to flush before closing for better durability, ignore errors in drop
              #[cfg(not(feature = "no_fs_flush"))]
              let _ = fs::flush(handle);
              #[cfg(feature = "no_fs_flush")]
              { /* No-op */ }

              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceWrite drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Günlük seviyeleri
#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

// Sürücü tipleri
#[derive(Debug, Clone, Copy)] // Add Clone, Copy for easier handling
pub enum DriveType {
    HDD,
    SSD,
    SATA,
    SAS,
    NVMe,
    UFS,
    eMMC,
    USB,
    Unknown,
}

/// File-based logger for Sahne66.
/// Writes log messages to a file with timestamps, levels, and drive type.
pub struct Logger<W: Write + Drop> { // Generic over the underlying writer
     // The underlying file/resource writer, protected by a Mutex for thread safety.
    file: Mutex<W>, // Use spin Mutex for both std and no_std consistency
    level: LogLevel, // Minimum level to log
    drive_type: DriveType, // Associated drive type
    // Add persistence mechanism if logger configuration needs to be saved.
}

impl<W: Write + Drop> Logger<W> {
    /// Creates a new Logger instance with the given writer.
    ///
    /// # Arguments
    ///
    /// * `writer`: The underlying writer for the log output (implements Write + Drop).
    /// * `level`: The minimum log level for this logger.
    /// * `drive_type`: The associated drive type.
    ///
    /// # Returns
    ///
    /// A new Logger instance.
    pub fn new(writer: W, level: LogLevel, drive_type: DriveType) -> Self {
        Logger {
            file: Mutex::new(writer), // Wrap writer in Mutex (Requires Mutex)
            level,
            drive_type,
        }
    }

    /// Opens a file/resource and creates a Logger instance over it.
    /// Convenience constructor for file-based loggers.
    ///
    /// # Arguments
    ///
    /// * `filename`: The path to the log file/resource.
    /// * `level`: The minimum log level for this logger.
    /// * `drive_type`: The associated drive type.
    ///
    /// # Returns
    ///
    /// A Result containing the Logger instance or a FileSystemError.
    #[cfg(feature = "std")]
    pub fn open_file(filename: &str, level: LogLevel, drive_type: DriveType) -> Result<Logger<File>, FileSystemError> { // Return FileSystemError
        let file = OpenOptions::new()
            .create(true) // Create if doesn't exist
            .append(true) // Append to the end
            .write(true) // Enable writing
            .open(filename).map_err(|e| map_std_io_error_to_fs_error(e))?; // Map std::io::Error


        // In std, File implements Write + Drop.
        Ok(Logger::new(file, level, drive_type))
    }

    /// Opens a resource using Sahne64 fs calls and creates a Logger instance over it.
    /// Convenience constructor for Sahne64 resource-based loggers.
    ///
    /// # Arguments
    ///
    /// * `filename`: The path/ID of the resource.
    /// * `level`: The minimum log level for this logger.
    /// * `drive_type`: The associated drive type.
    ///
    /// # Returns
    ///
    /// A Result containing the Logger instance or a FileSystemError.
    #[cfg(not(feature = "std"))]
    pub fn open_resource(filename: &str, level: LogLevel, drive_type: DriveType) -> Result<Logger<SahneResourceWrite>, FileSystemError> { // Return FileSystemError
        let flags = fs::O_CREAT | fs::O_APPEND | fs::O_WRONLY;
        let handle = fs::open(filename, flags).map_err(|e| map_sahne_error_to_fs_error(e))?; // Map SahneError

        // SahneResourceWrite implements Write + Drop.
        let resource_writer = SahneResourceWrite::new(handle);


        Ok(Logger::new(resource_writer, level, drive_type))
    }


    /// Writes a log message if its level is sufficient.
    /// Formats the message with timestamp, level, and drive type.
    ///
    /// # Arguments
    ///
    /// * `level`: The level of the log message.
    /// * `message`: The log message string.
    ///
    /// Note: Write errors are currently ignored.
    pub fn log(&self, level: LogLevel, message: &str) { // Does not return Result, errors are ignored
        if self.should_log(&level) {
            let timestamp = self.get_timestamp(); // Requires alloc and timestamp logic
            let level_str = format!("{:?}", level).to_uppercase(); // Requires alloc
            let drive_type_str = format!("{:?}", self.drive_type).to_uppercase(); // Requires alloc
            // Format the log message string (Requires alloc)
            let log_message = format!("[{}] {} ({}) {}\n", timestamp, level_str, drive_type_str, message);

            // Acquire the lock on the file writer
            let mut file = self.file.lock(); // Use spin Mutex lock
            // Check if lock acquisition was successful in case of panic (though spin Mutex doesn't panic on acquisition)
             #[cfg(feature = "std")] // std Mutex::lock can panic
             let mut file = file.expect("Logger file mutex poisoned"); // Handle poisoned mutex in std


            // Write the message to the file. Ignore errors for basic logging.
            // In a robust system, write errors should be handled (e.g., retry, fallback to console).
            let _ = file.write_all(log_message.as_bytes()); // Requires WriteExt
            let _ = file.flush(); // Flush for durability (Requires WriteExt)

            // Lock is released when 'file' goes out of scope.
        }
    }

    /// Checks if a log message with the given level should be logged by this logger.
    ///
    /// # Arguments
    ///
    /// * `level`: The level of the log message.
    ///
    /// # Returns
    ///
    /// True if the message should be logged, false otherwise.
    fn should_log(&self, level: &LogLevel) -> bool { // Takes reference to LogLevel
        level >= &self.level // Comparison uses PartialOrd derive
    }

    /// Gets the current timestamp string.
    ///
    /// # Returns
    ///
    /// A String containing the formatted timestamp.
    #[cfg(feature = "std")]
    fn get_timestamp(&self) -> String { // Returns String (Requires alloc)
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string() // Uses chrono in std
    }

    #[cfg(not(feature = "std"))]
    fn get_timestamp(&self) -> String { // Returns String (Requires alloc)
        // Sahne64'te gerçek zamanı almak için bir sistem çağrısı gerekebilir.
        // Şimdilik basit bir placeholder kullanıyoruz ve alloc gerektiriyor.
        String::from("YYYY-MM-DD HH:MM:SS") // Requires alloc
    }
}


// Removed example main functions - they are typically outside the library crate file.

#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Write, Seek, Cursor}; // For File/Cursor traits
    use std::fs::{remove_file, OpenOptions}; // For creating/managing test files
    use std::path::Path;
    use alloc::string::ToString; // For to_string()
    use alloc::vec::Vec; // For Vec
    use core::io::WriteExt; // For flush() and write_all() on Cursor

    // Helper function to map std::io::Error to FileSystemError in tests
    fn map_std_io_error_to_fs_error_test(e: std::io::Error) -> FileSystemError {
        FileSystemError::IOError(format!("IO Error in test: {}", e))
    }


    // Mock SahneResourceWrite for testing no_std path with in-memory buffer (Cursor)
    struct MockSahneResourceWrite {
        cursor: Cursor<Vec<u8>>, // In-memory buffer using std::io::Cursor
        // No Handle needed for in-memory mock
    }
    impl MockSahneResourceWrite {
        fn new() -> Self { MockSahneResourceWrite { cursor: Cursor::new(Vec::new()) } } // Requires alloc
        fn get_content(&self) -> &[u8] { self.cursor.get_ref().as_slice() } // Get the underlying data
    }
    // Implement core::io::Write for the Mock
    impl Write for MockSahneResourceWrite {
        fn write(&mut self, buf: &[u8]) -> Result<usize, core::io::Error> {
            // Write to the inner Cursor
             self.cursor.write(buf)
                 .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("Mock write error: {}", e))) // Map std io error to core io error
        }
         fn flush(&mut self) -> Result<(), core::io::Error> {
             // Flush the inner Cursor (mostly a no-op for Cursor)
             self.cursor.flush()
                 .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("Mock flush error: {}", e))) // Map std io error to core io error
         }
    }
    // Implement core::ops::Drop for the Mock
    impl Drop for MockSahneResourceWrite {
        fn drop(&mut self) {
            println!("MockSahneResourceWrite dropped."); // Indicate drop occurred
        }
    }


    #[test]
    fn test_logger_std_file() -> Result<(), FileSystemError> { // Return FileSystemError
        let test_file_path = Path::new("test_logger.log");
        let level = LogLevel::Info;
        let drive_type = DriveType::SSD;

        // Create logger using the file constructor
        let logger = Logger::open_file(test_file_path.to_str().unwrap(), level, drive_type)?; // Uses Logger::open_file


        // Log messages at different levels
        logger.log(LogLevel::Debug, "This is a debug message (should NOT appear)."); // Below logger level
        logger.log(LogLevel::Info, "This is an info message (should appear)."); // At logger level
        logger.log(LogLevel::Warning, "This is a warning message (should appear)."); // Above logger level
        logger.log(LogLevel::Error, "This is an error message (should appear)."); // Above logger level


         // The logger should be dropped here, closing the file.
         drop(logger); // Explicitly drop to ensure file is closed before reading it.


        // Read the content of the log file
         let mut file = OpenOptions::new().read(true).open(test_file_path).map_err(|e| map_std_io_error_to_fs_error_test(e))?;
         let mut contents = String::new(); // Requires alloc
         file.read_to_string(&mut contents).map_err(|e| map_std_io_error_to_fs_error_test(e))?;


        // Assert the content of the log file
        println!("Log file contents:\n{}", contents); // Print for inspection


        // Check for the presence of expected messages
        assert!(!contents.contains("This is a debug message")); // Debug message should be filtered out
        assert!(contents.contains("INFO (SSD) This is an info message."));
        assert!(contents.contains("WARNING (SSD) This is a warning message."));
        assert!(contents.contains("ERROR (SSD) This is an error message."));

         // Check timestamp format (basic check for the placeholder format in std)
          assert!(contents.contains("YYYY-MM-DD HH:MM:SS") || contents.contains(Local::now().format("%Y-%m-%d %H:%M:%S").to_string().split(" ").next().unwrap())); // Check for either placeholder or actual date start


        // Clean up the test file
        remove_file(test_file_path).expect("Test dosyası silinemedi");


        Ok(()) // Return Ok from test function
    }

    #[test]
     fn test_logger_no_std_mock_resource() {
          let level = LogLevel::Warning;
          let drive_type = DriveType::HDD;

          // Create a mock SahneResourceWrite (in-memory)
          let mock_writer = MockSahneResourceWrite::new(); // Requires alloc

          // Create logger using the generic constructor with the mock writer
          let logger = Logger::new(mock_writer, level, drive_type);

          // Log messages at different levels
          logger.log(LogLevel::Info, "This is an info message (should NOT appear)."); // Below logger level
          logger.log(LogLevel::Warning, "This is a warning message (should appear)."); // At logger level
          logger.log(LogLevel::Error, "This is an error message (should appear)."); // Above logger level


          // Get the content from the mock writer after logging
          // We need to access the inner MockSahneResourceWrite from the Logger.
          // This requires the Mock to be accessible or the Logger to provide a debug method.
          // Let's capture the MockSahneResourceWrite before passing it, or make the Mutex public for testing.
          // Making the Mutex public is acceptable for testing.

           // Re-create logger and access the mock writer content
           let mock_writer_captured = MockSahneResourceWrite::new(); // Requires alloc
           let logger_test = Logger::new(mock_writer_captured, level, drive_type);

           logger_test.log(LogLevel::Info, "This is an info message (should NOT appear)."); // Below logger level
           logger_test.log(LogLevel::Warning, "This is a warning message (should appear)."); // At logger level
           logger_test.log(LogLevel::Error, "This is an error message (should appear)."); // Above logger level

           // Get the content from the mock writer via the logger's mutex
           let mut file_lock = logger_test.file.lock();
           let mock_writer_content = file_lock.get_content(); // Access helper on the mock writer

           let contents = String::from_utf8_lossy(mock_writer_content); // Convert bytes to String for assertion

           println!("Mock log content:\n{}", contents); // Print for inspection


           // Assert the content of the mock log
           assert!(!contents.contains("This is an info message")); // Info message should be filtered out
           assert!(contents.contains("WARNING (HDD) This is a warning message."));
           assert!(contents.contains("ERROR (HDD) This is an error message."));

           // Check timestamp placeholder in no_std test
            assert!(contents.contains("YYYY-MM-DD HH:MM:SS"));


          // Drop the logger explicitly
          drop(logger_test);
     }


    // TODO: Add tests for error handling in open_file/open_resource.
    // TODO: Add test for concurrent logging (requires multiple threads).
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
