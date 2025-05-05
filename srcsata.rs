#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std")), no_std)] // Standart kütüphaneye ihtiyacımız yok

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;


// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "std")]
use std::io::{self, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Write as StdWrite, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt, WriteExt as StdWriteExt}; // Added ReadExt, WriteExt, ErrorKind
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::error::Error as StdError; // For std Error trait


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::{O_RDWR, O_CREAT}; // Import necessary fs flags for opening

// core::io traits for block device implementation
use core::io::{Read, Seek, SeekFrom, Write, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt, WriteExt as CoreWriteExt}; // core::io
use core::result::Result; // Use core::result::Result
use core::fmt; // For fmt::Display
use core::ops::Drop; // For Drop trait

// Import the standard BlockDevice trait and its error type
use crate::blockdevice::{BlockDevice, BlockDeviceError}; // Assuming these are in crate::blockdevice

// Assuming SataConfig is defined elsewhere (e.g., in crate::config)
use crate::config::SataConfig;


// Removed custom SataError enum and its From implementations to BlockDeviceError
// We will map SahneError/io::Error directly to BlockDeviceError::IoError
// and handle other logical errors as BlockDeviceError variants (e.g., InvalidParameter, DeviceError if they exist in the trait).


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

/// Helper function to map BlockDeviceError to FileSystemError (copied from srchdd.rs)
fn map_block_device_error_to_fs_error(e: BlockDeviceError) -> FileSystemError {
    match e {
        BlockDeviceError::IoError(io_err) => {
             #[cfg(feature = "std")]
             // In std, BlockDeviceError::IoError wraps std::io::Error
             map_std_io_error_to_fs_error(io_err)
             #[cfg(not(feature = "std"))]
             // In no_std, BlockDeviceError::IoError wraps SahneError
             map_sahne_error_to_fs_error(io_err)
        },
        BlockDeviceError::BlockSizeError(msg) => FileSystemError::InvalidData(format!("Block size mismatch or error: {}", msg)), // Map BlockSizeError to InvalidData
        // Map other BlockDeviceError variants if they exist in the standardized trait
        #[cfg(feature = "blockdevice_trait")] // Map specific trait errors if they exist
        BlockDeviceError::NotSupported(msg) => FileSystemError::NotSupported(msg),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::TimedOut => FileSystemError::TimedOut(String::from("Block device operation timed out")),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::InvalidParameter(msg) => FileSystemError::InvalidParameter(msg),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::DeviceNotFound(msg) => FileSystemError::NotFound(msg), // Assuming DeviceNotFound variant
         #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::PermissionDenied(msg) => FileSystemError::PermissionDenied(msg), // Assuming PermissionDenied variant
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::DeviceError(msg) => FileSystemError::DeviceError(msg), // Assuming a generic DeviceError variant
    }
}

// Use the standardized SahneResourceReadWriteSeek for no_std (copied from srchdd.rs)
#[cfg(not(feature = "std"))]
pub struct SahneResourceReadWriteSeek { // Read+Write+Seek resource wrapper
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu (read/write için güncellenmeli)
}

#[cfg(not(feature = "std"))]
impl SahneResourceReadWriteSeek {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReadWriteSeek { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReadWriteSeek { // Use core::io::Read trait
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> { // Return core::io::Error
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
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::read_at error: {:?}", e)))?; // Map SahneError to core::io::Error

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

#[cfg(not(feature = "std"))]
impl Write for SahneResourceReadWriteSeek { // Use core::io::Write trait (for write_at)
    fn write(&mut self, buf: &[u8]) -> Result<usize, CoreIOError> { // Return core::io::Error
         // Assuming fs::write_at(handle, offset, buf) Result<usize, SahneError>
         // This write implementation writes at the current position and updates it.
         let bytes_to_write = buf.len();
         if bytes_to_write == 0 { return Ok(0); }

         let bytes_written = fs::write_at(self.handle, self.position, buf)
             .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::write_at error: {:?}", e)))?; // Map SahneError to core::io::Error

         self.position += bytes_written as u64;

         // Update file_size if writing extends beyond current size
         if self.position > self.file_size {
              self.file_size = self.position;
         }

         Ok(bytes_written)
    }

     fn flush(&mut self) -> Result<(), CoreIOError> {
         #[cfg(not(feature = "no_fs_flush"))] // Assume fs::flush is available unless this feature is set
         fs::flush(self.handle).map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::flush error: {:?}", e)))?;
         #[cfg(feature = "no_fs_flush")] // If fs::flush is not available
         { /* No-op or alternative sync method */ }
         Ok(())
     }
}


#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReadWriteSeek { // Use core::io::Seek trait
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> { // Return core::io::Error
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize,
            SeekFrom::End(offset) => {
                file_size_isize.checked_add(offset)
                    .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from end)")))?
            },
            SeekFrom::Current(offset) => {
                (self.position as isize).checked_add(offset)
                     .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from current)")))?
            },
        };

        if new_pos < 0 {
            return Err(CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Invalid seek position (result is negative)")));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
}

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReadWriteSeek {
     fn drop(&mut self) {
         if let Some(handle) = self.handle.take() {
              #[cfg(not(feature = "no_fs_flush"))]
              let _ = fs::flush(handle);
              #[cfg(feature = "no_fs_flush")]
              { /* No-op */ }

              if let Err(e) = resource::release(handle) {
                  eprintln!("WARN: SahneResourceReadWriteSeek drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e);
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant fs, memory, process, sync, kernel, arch imports.
// Removed redundant print module and panic handler boilerplate.
// Removed SataError and its From implementations.
// Removed SataConfig definition (assuming it's imported).

/// Represents a SATA Block Device simulated over a file.
/// Implements the BlockDevice trait.
/// Note: This is a simulation using file I/O, not a real SATA driver.
pub struct SataDevice<RWS: Read + Write + Seek + Drop> { // Generic over the underlying reader/writer/seeker
    inner: RWS, // The underlying file/resource reader/writer/seeker
    config: SataConfig, // Store SataConfig which contains block size and count
    // Add total_blocks field if needed, can be calculated from file size and block size
     total_blocks: usize, // Should be derived from config.block_count
}

impl<RWS: Read + Write + Seek + Drop> SataDevice<RWS> {
    /// Creates a new SataDevice instance.
    ///
    /// # Arguments
    ///
    /// * `inner`: The underlying reader/writer/seeker for the file/resource.
    /// * `config`: The configuration for the SATA device (contains block size and count).
    ///
    /// # Returns
    ///
    /// A new SataDevice instance or BlockDeviceError::BlockSizeError if block_size is zero.
    pub fn new(inner: RWS, config: SataConfig) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
        if config.block_size == 0 {
            return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
        }
        // Calculate total blocks based on file size? Or use config.block_count?
        // Assuming config.block_count is the definitive source for total blocks.
        // If validating file size, need to seek(End) on 'inner', which needs mut inner or other approach.

        Ok(SataDevice {
            inner,
            config,
            // total_blocks: config.block_count as usize, // Store usize if BlockDevice trait needs it
        })
    }

    /// Opens a file from the given path and creates a SataDevice over it.
    /// This is a convenience constructor for file-based block devices.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to the file/resource.
    /// * `config`: The configuration for the SATA device.
    ///
    /// # Returns
    ///
    /// A Result containing the SataDevice or a FileSystemError.
    #[cfg(feature = "std")]
    pub fn open_file(path: &str, config: SataConfig) -> Result<Self, FileSystemError> { // Return FileSystemError
         if config.block_size == 0 {
             return Err(map_block_device_error_to_fs_error(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero.")))); // Requires alloc
         }
         let file = OpenOptions::new()
             .read(true)
             .write(true)
             .create(true) // Create the file if it doesn't exist
             .open(path).map_err(|e| map_std_io_error_to_fs_error(e))?; // Map std::io::Error to FileSystemError

         // Set file length based on config.block_size and config.block_count
         let file_size = config.block_size as u64 * config.block_count;
          if file.set_len(file_size).is_err() {
             // Error setting file length, map to FileSystemError
             // Note: set_len might fail or be slow depending on OS and device.
             eprintln!("WARN: Failed to set SATA device file size to {}. Continuing anyway, writes may extend file.", file_size); // Use std print
             // Decide if this is a fatal error or just a warning. For a simulation, maybe just warn.
              If it's a fatal error, return Err(map_std_io_error_to_fs_error(e)).
          }

         // In std, File implements Read + Write + Seek + Drop.
         // Use map_block_device_error_to_fs_error to map the internal new error
         SataDevice::new(file, config).map_err(|e| map_block_device_error_to_fs_error(e))
    }

    /// Opens a resource using Sahne64 fs calls and creates a SataDevice over it.
    /// Convenience constructor for Sahne64 resource-based block devices.
    ///
    /// # Arguments
    ///
    /// * `path`: The path/ID of the resource.
    /// * `config`: The configuration for the SATA device.
    ///
    /// # Returns
    ///
    /// A Result containing the SataDevice or a FileSystemError.
    #[cfg(not(feature = "std"))]
    pub fn open_resource(path: &str, config: SataConfig) -> Result<Self, FileSystemError> { // Return FileSystemError
         if config.block_size == 0 {
             return Err(map_block_device_error_to_fs_error(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero.")))); // Requires alloc
         }
         let flags = fs::O_RDWR | fs::O_CREAT;
         let handle = fs::open(path, flags).map_err(|e| map_sahne_error_to_fs_error(e))?; // Map SahneError to FileSystemError

         // Get or set file size for SahneResourceReadWriteSeek
         // In Sahne64, setting file size might require fs::ftruncate or similar.
         // Let's assume fstat gives the current size and ftruncate exists to set it.
         let file_size = config.block_size as u64 * config.block_count;
         let current_size_result = fs::fstat(handle);

         let actual_size = match current_size_result {
              Ok(stat) => stat.size as u64,
              Err(e) => {
                  // If fstat fails, try to release the handle and return error
                  let _ = resource::release(handle);
                  return Err(map_sahne_error_to_fs_error(e)); // Map SahneError to FileSystemError
              }
         };

         if actual_size != file_size {
              // Attempt to set file size if it doesn't match config (requires ftruncate syscall)
              #[cfg(not(feature = "no_fs_ftruncate"))] // Assume ftruncate is available unless this feature is set
              {
                   crate::println!("INFO: Resizing SATA device file from {} to {}", actual_size, file_size); // Use no_std print
                   if let Err(e) = fs::ftruncate(handle, file_size as usize).map_err(|e| map_sahne_error_to_fs_error(e)) {
                        // Error setting file size. Decide if fatal or warning.
                        // For a simulation, maybe warn and continue, assuming writes will extend.
                         crate::eprintln!("WARN: Failed to set SATA device file size to {}. Error: {:?}", file_size, e); // Use no_std print
                         // If it's a fatal error, release handle and return Err.
                          let _ = resource::release(handle); return Err(e);
                   }
              }
              #[cfg(feature = "no_fs_ftruncate")] // If ftruncate is not available
              {
                   crate::eprintln!("WARN: fs::ftruncate not available. Cannot ensure SATA device file size matches config. Writes may extend file."); // Use no_std print
              }
         }


         // SahneResourceReadWriteSeek implements Read + Write + Seek + Drop
         let resource_rws = SahneResourceReadWriteSeek::new(handle, file_size); // Pass configured size


         // Use map_block_device_error_to_fs_error to map the internal new error
         SataDevice::new(resource_rws, config).map_err(|e| map_block_device_error_to_fs_error(e))
    }


    // Removed send_command, read_data, write_data helpers.
    // The logic is unified in the BlockDevice trait methods.

     // Helper to map core::io::Error to BlockDeviceError (copied from srchdd.rs)
    fn map_core_io_error_to_block_device_error(e: core::io::Error) -> BlockDeviceError {
         #[cfg(not(feature = "std"))]
         {
              // In no_std, core::io::Error might be mapped from SahneError
              // Attempt to downcast or match the original SahneError if possible
              // For now, assume the error message contains info or map generically
              // Assuming CoreIOError from our SahneResource wrappers includes the original SahneError info.
              BlockDeviceError::IoError(crate::SahneError::Other(format!("Core IO Error during block op: {:?}", e))) // Re-wrap or map
         }
          #[cfg(feature = "std")]
         // In std, core::io::Error is std::io::Error, already handled by From impl
         BlockDeviceError::IoError(std::io::Error::new(e.kind(), format!("Core IO Error during block op: {:?}", e))) // Map kind
    }
}


// Implement BlockDevice for SataDevice
impl<RWS: Read + Write + Seek + Drop> BlockDevice for SataDevice<RWS> { // Implement BlockDevice for SataDevice
    // Use the standardized underlying reader/writer/seeker
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), BlockDeviceError> { // Use u64 for block_id, return BlockDeviceError
        // Check if block_id is out of bounds (based on config.block_count)
         if block_id >= self.config.block_count {
             return Err(BlockDeviceError::InvalidParameter(format!("Block ID {} is out of bounds. Total blocks: {}", block_id, self.config.block_count))); // Requires alloc
         }

        // Check if buffer size matches block size
        if buf.len() != self.config.block_size as usize { // Use config.block_size as usize
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.config.block_size) // Requires alloc
            ));
        }

        // Calculate the byte offset for the block (block_id is u64, block_size is u32)
        let offset = block_id * self.config.block_size as u64; // Ensure calculation uses u64


        // Seek to the correct offset in the underlying reader/writer/seeker
        // Map core::io::Error from seek to BlockDeviceError
        self.inner.seek(SeekFrom::Start(offset)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        // Read exactly the required number of bytes (one block)
        // Map core::io::Error from read_exact to BlockDeviceError
        self.inner.read_exact(buf).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        Ok(()) // Return success
    }

    // Use the standardized underlying reader/writer/seeker
    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> Result<(), BlockDeviceError> { // Use u64 for block_id, return BlockDeviceError
        // Check if block_id is out of bounds (based on config.block_count)
         if block_id >= self.config.block_count {
             return Err(BlockDeviceError::InvalidParameter(format!("Block ID {} is out of bounds. Total blocks: {}", block_id, self.config.block_count))); // Requires alloc
         }

        // Check if buffer size matches block size
        if buf.len() != self.config.block_size as usize { // Use config.block_size as usize
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.config.block_size) // Requires alloc
            ));
        }

        // Calculate the byte offset for the block (block_id is u64, config.block_size is u32)
        let offset = block_id * self.config.block_size as u64; // Ensure calculation uses u64


        // Seek to the correct offset in the underlying reader/writer/seeker
        // Map core::io::Error from seek to BlockDeviceError
        self.inner.seek(SeekFrom::Start(offset)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        // Write exactly the required number of bytes (one block)
        // Map core::io::Error from write_all to BlockDeviceError
        self.inner.write_all(buf).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        Ok(()) // Return success
    }

    fn block_size(&self) -> usize { // Return usize for block_size
        self.config.block_size as usize
    }

    fn block_count(&self) -> u64 { // Return u64 for block_count (assuming trait includes this)
        self.config.block_count
    }
}


#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Write, Seek, Cursor}; // For File/Cursor traits
    use std::fs::{remove_file, OpenOptions}; // For creating/managing test files
    use std::path::Path;
    use alloc::string::ToString; // For to_string()
    use alloc::vec::Vec; // For Vec
    use core::io::{ReadExt, WriteExt}; // For read_exact, write_all


    // Assuming a dummy SataConfig for tests
    #[derive(Clone, Copy)] // Add Clone, Copy for tests
    pub struct SataConfig {
        pub device_id: u32,
        pub block_size: u32,
        pub block_count: u64,
    }


    // Helper function to map std::io::Error to BlockDeviceError in tests
    fn map_std_io_error_to_block_device_error_test(e: std::io::Error) -> BlockDeviceError {
        BlockDeviceError::IoError(e) // Direct mapping in std tests
    }

    // Helper function to map core::io::Error to BlockDeviceError in tests (for Mock)
    fn map_core_io_error_to_block_device_error_test(e: core::io::Error) -> BlockDeviceError {
         #[cfg(not(feature = "std"))] // This mapping is only relevant in no_std tests with a mock core::io::Error
         {
              // Assuming CoreIOError has a debug impl or can be mapped to SahneError
              BlockDeviceError::IoError(crate::SahneError::Other(format!("Mock Core IO Error: {:?}", e))) // Map generically for mock
         }
          #[cfg(feature = "std")] // In std tests, core::io::Error is std::io::Error
         BlockDeviceError::IoError(std::io::Error::new(e.kind(), format!("Core IO Error in test: {:?}", e)))
    }


    // Mock SahneResourceReadWriteSeek for testing no_std path with in-memory buffer (Cursor)
    struct MockSahneResourceReadWriteSeek {
        cursor: Cursor<Vec<u8>>, // In-memory buffer using std::io::Cursor
        // No Handle needed for in-memory mock
        file_size: u64, // Track simulated file size
    }
    impl MockSahneResourceReadWriteSeek {
        fn new(initial_size: u64) -> Self {
             let initial_data = vec![0u8; initial_size as usize]; // Requires alloc
             MockSahneResourceReadWriteSeek { cursor: Cursor::new(initial_data), file_size: initial_size }
        }
        fn get_content(&self) -> &[u8] { self.cursor.get_ref().as_slice() } // Get the underlying data
         // Helper to simulate setting file size (like ftruncate)
         fn set_file_size(&mut self, new_size: u64) -> Result<(), CoreIOError> {
             let current_len = self.cursor.get_ref().len() as u64;
             if new_size > current_len {
                  // Extend with zeros
                  let bytes_to_add = (new_size - current_len) as usize;
                  let mut buffer_mut = self.cursor.get_mut(); // Get mutable reference to Vec
                  buffer_mut.extend(vec![0u8; bytes_to_add]); // Requires alloc and Extend trait
             } else {
                  // Truncate
                  let mut buffer_mut = self.cursor.get_mut(); // Get mutable reference to Vec
                  buffer_mut.truncate(new_size as usize);
             }
             self.file_size = new_size; // Update simulated size
             // Reset cursor position? SeekFrom::Start(0) might be needed depending on test
             self.cursor.seek(SeekFrom::Start(self.cursor.position())).unwrap(); // Keep current relative position if possible
             Ok(())
         }
    }
    // Implement core::io::Read, Write, Seek, Drop for the Mock
    impl Read for MockSahneResourceReadWriteSeek {
         fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> { self.cursor.read(buf).map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("Mock read error: {}", e))) }
         fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), CoreIOError> { self.cursor.read_exact(buf).map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("Mock read_exact error: {}", e))) }
    }
    impl Write for MockSahneResourceReadWriteSeek {
         fn write(&mut self, buf: &[u8]) -> Result<usize, CoreIOError> {
             let bytes_written = self.cursor.write(buf)
                  .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("Mock write error: {}", e)))?;
             // Simulate file size increase on write past current end
              if self.cursor.position() > self.file_size {
                   self.file_size = self.cursor.position();
              }
             Ok(bytes_written)
         }
         fn write_all(&mut self, buf: &[u8]) -> Result<(), CoreIOError> {
             self.cursor.write_all(buf)
                  .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("Mock write_all error: {}", e)))?;
             // Simulate file size increase on write past current end
              if self.cursor.position() > self.file_size {
                   self.file_size = self.cursor.position();
              }
             Ok(())
         }
         fn flush(&mut self) -> Result<(), CoreIOError> { self.cursor.flush().map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("Mock flush error: {}", e))) }
    }
    impl Seek for MockSahneResourceReadWriteSeek {
        fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
            self.cursor.seek(pos).map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("Mock seek error: {}", e)))
        }
    }
    impl Drop for MockSahneResourceReadWriteSeek { fn drop(&mut self) { println!("MockSahneResourceReadWriteSeek dropped."); } }


    #[test]
    fn test_sata_device_std_file() -> Result<(), BlockDeviceError> { // Return BlockDeviceError
        let test_file_path = Path::new("test_sata_device.img");
        let config = SataConfig { device_id: 1, block_size: 512, block_count: 100 }; // Use SataConfig
        let block_size_usize = config.block_size as usize;
        let total_test_bytes = config.block_size as u64 * config.block_count;


        // Create and open the file using the SataDevice open_file constructor
        let mut device = SataDevice::open_file(test_file_path.to_str().unwrap(), config)?; // Uses SataDevice::open_file


        // Ensure the file is created and has the correct size (if set_len worked)
        let file_metadata = std::fs::metadata(test_file_path).map_err(|e| map_std_io_error_to_block_device_error_test(e))?;
        // Note: std::fs::File::set_len might not zero the extended part.
        // We expect the file size to be set, even if content is garbage.
        assert_eq!(file_metadata.len(), total_test_bytes);


         // Test block_size and block_count methods
         assert_eq!(device.block_size(), block_size_usize);
         assert_eq!(device.block_count(), config.block_count);


        // Prepare data to write
        let mut write_buf = vec![0u8; block_size_usize]; // Requires alloc
        for i in 0..block_size_usize {
            write_buf[i] = (i % 256) as u8; // Fill with some pattern
        }

        // Write a block
        let block_id_to_write: u64 = 25; // Use u64 for block_id
        device.write_block(block_id_to_write, &write_buf)?; // Uses SataDevice::write_block


        // Prepare buffer to read into
        let mut read_buf = vec![0u8; block_size_usize]; // Requires alloc

        // Read the block back
        device.read_block(block_id_to_write, &mut read_buf)?; // Uses SataDevice::read_block


        // Verify the read data matches the written data
        assert_eq!(read_buf, write_buf);


         // Test read with incorrect buffer size
          let mut small_buf = vec![0u8; block_size_usize / 2]; // Requires alloc
          let result_read_small = device.read_block(0, &mut small_buf);
          assert!(result_read_small.is_err());
           match result_read_small.unwrap_err() {
               BlockDeviceError::BlockSizeError(msg) => {
                   assert!(msg.contains("Buffer size"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_read_small.unwrap_err()),
           }

         // Test write with incorrect buffer size
          let mut large_buf = vec![0u8; block_size_usize * 2]; // Requires alloc
          let result_write_large = device.write_block(0, &large_buf);
          assert!(result_write_large.is_err());
           match result_write_large.unwrap_err() {
               BlockDeviceError::BlockSizeError(msg) => {
                   assert!(msg.contains("Buffer size"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_write_large.unwrap_err()),
           }

        // Test read/write with out of bounds block ID
         let result_read_oob = device.read_block(config.block_count + 1, &mut read_buf);
         assert!(result_read_oob.is_err());
          match result_read_oob.unwrap_err() {
              BlockDeviceError::InvalidParameter(msg) => { // Mapped from InvalidParameter
                  assert!(msg.contains("Block ID") && msg.contains("is out of bounds"));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result_read_oob.unwrap_err()),
          }
          let result_write_oob = device.write_block(config.block_count + 1, &write_buf);
          assert!(result_write_oob.is_err());
          match result_write_oob.unwrap_err() {
              BlockDeviceError::InvalidParameter(msg) => { // Mapped from InvalidParameter
                  assert!(msg.contains("Block ID") && msg.contains("is out of bounds"));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result_write_oob.unwrap_err()),
          }


        // Test creating device with zero block size (using a different path to avoid conflict)
        let test_zero_block_size_path = Path::new("test_sata_zero_block_size.img");
        let config_zero_size = SataConfig { device_id: 2, block_size: 0, block_count: 100 };
        let result_zero_block_size = SataDevice::open_file(test_zero_block_size_path.to_str().unwrap(), config_zero_size);
         assert!(result_zero_block_size.is_err());
          match result_zero_block_size.unwrap_err() {
              FileSystemError::InvalidData(msg) => { // Mapped from BlockDeviceError::BlockSizeError
                  assert!(msg.contains("Block size cannot be zero."));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result_zero_block_size.unwrap_err()),
          }
         // Clean up any potentially created file
          if test_zero_block_size_path.exists() {
              remove_file(test_zero_block_size_path).unwrap_or_default();
          }


        // Clean up the test file
        remove_file(test_file_path).expect("Test dosyası silinemedi");


        Ok(()) // Return Ok from test function
    }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This requires simulating resource acquire/release, fs::fstat, fs::ftruncate, fs::read_at, fs::write_at, fs::lseek.
    // Test cases should cover opening resources, block reads/writes, invalid block sizes/IDs, and simulated IO errors.
    // Requires Mock implementations of fs functions and SahneError.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
