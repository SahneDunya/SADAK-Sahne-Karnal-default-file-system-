#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std")), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;


// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::{File, OpenOptions}; // Added OpenOptions for consistency with other file devices
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
    // read_exact has a default implementation in core::io::Read that uses read
    // read_to_end has a default implementation in core::io::ReadExt that uses read
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
// Removed SahneError usage from new and BlockDevice methods.

/// Represents a SAS Block Device simulated over a file.
/// Implements the BlockDevice trait.
/// Note: This is a simulation using file I/O, not a real SAS driver.
pub struct SasDevice<RWS: Read + Write + Seek + Drop> { // Generic over the underlying reader/writer/seeker
    inner: RWS, // The underlying file/resource reader/writer/seeker
    block_size: usize, // Standardize block_size to usize
    // Add total_blocks field if needed, can be calculated from file size and block size
    // total_blocks: usize,
}

impl<RWS: Read + Write + Seek + Drop> SasDevice<RWS> {
    /// Creates a new SasDevice instance.
    ///
    /// # Arguments
    ///
    /// * `inner`: The underlying reader/writer/seeker for the file/resource.
    /// * `block_size`: The size of a block in bytes. Must be non-zero.
    ///
    /// # Returns
    ///
    /// A new SasDevice instance or BlockDeviceError::BlockSizeError if block_size is zero.
    pub fn new(inner: RWS, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
        if block_size == 0 {
            return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
        }
        // Calculate total blocks based on file size? Or is total_blocks fixed/part of metadata?
        // For this simulation, let's assume total_blocks is implicit from file size.
        // let file_size = inner.seek(SeekFrom::End(0)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?; // Requires mapping core::io::Error
        // let total_blocks = (file_size as usize) / block_size; // Integer division might lose blocks

        Ok(SasDevice {
            inner,
            block_size,
            // total_blocks: total_blocks,
        })
    }

    /// Opens a file from the given path and creates a SasDevice over it.
    /// This is a convenience constructor for file-based block devices.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to the file/resource.
    /// * `block_size`: The size of a block in bytes.
    ///
    /// # Returns
    ///
    /// A Result containing the SasDevice or a BlockDeviceError.
    #[cfg(feature = "std")]
    pub fn open_file(path: &str, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
         if block_size == 0 {
             return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
         }
         let file = OpenOptions::new()
             .read(true)
             .write(true)
             .create(true) // Create the file if it doesn't exist
             .open(path)?; // io::Error is mapped to BlockDeviceError by From impl

         // In std, File implements Read + Write + Seek + Drop
         SasDevice::new(file, block_size)
    }

    /// Opens a resource using Sahne64 fs calls and creates a SasDevice over it.
    /// This is the convenience constructor for Sahne64 resource-based block devices.
    ///
    /// # Arguments
    ///
    /// * `path`: The path/ID of the resource.
    /// * `block_size`: The size of a block in bytes.
    ///
    /// # Returns
    ///
    /// A Result containing the SasDevice or a BlockDeviceError.
    #[cfg(not(feature = "std"))]
    pub fn open_resource(path: &str, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
         if block_size == 0 {
             return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
         }
         let flags = fs::O_RDWR | fs::O_CREAT;
         let handle = fs::open(path, flags).map_err(|e| BlockDeviceError::IoError(e))?; // Map SahneError to BlockDeviceError::IoError

         // Get file size for SahneResourceReadWriteSeek
         // This might fail if the file doesn't exist yet or device doesn't support fstat
         let file_stat_result = fs::fstat(handle);

         let file_size = match file_stat_result {
              Ok(stat) => stat.size as u64,
              Err(e) => {
                  // If fstat fails, try to release the handle and return error
                  let _ = resource::release(handle);
                  return Err(BlockDeviceError::IoError(e)); // Map SahneError to BlockDeviceError::IoError
              }
         };


         // SahneResourceReadWriteSeek implements Read + Write + Seek + Drop
         let resource_rws = SahneResourceReadWriteSeek::new(handle, file_size);

         SasDevice::new(resource_rws, block_size)
    }


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


// Implement BlockDevice for SasDevice, using u64 for block_id, usize for block_size
impl<RWS: Read + Write + Seek + Drop> BlockDevice for SasDevice<RWS> { // Implement BlockDevice for SasDevice
    // Use the standardized underlying reader/writer/seeker
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), BlockDeviceError> { // Use u64 for block_id, return BlockDeviceError
        // Check if buffer size matches block size
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.block_size) // Requires alloc
            ));
        }

        // Calculate the byte offset for the block (block_id is u64, block_size is usize)
        let offset = block_id * self.block_size as u64; // Ensure calculation uses u64


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
        // Check if buffer size matches block size
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.block_size) // Requires alloc
            ));
        }

        // Calculate the byte offset for the block (block_id is u64, block_size is usize)
        let offset = block_id * self.block_size as u64; // Ensure calculation uses u64


        // Seek to the correct offset in the underlying reader/writer/seeker
        // Map core::io::Error from seek to BlockDeviceError
        self.inner.seek(SeekFrom::Start(offset)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        // Write exactly the required number of bytes (one block)
        // Map core::io::Error from write_all to BlockDeviceError
        self.inner.write_all(buf).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        Ok(()) // Return success
    }

    fn block_size(&self) -> usize { // Return usize for block_size
        self.block_size
    }

    // Add total_blocks() method if needed, consistent with BlockDevice trait
    // For a file-backed block device, total_blocks = file_size / block_size.
    // Getting file size requires Seek::seek(End), which needs mut self.
    // If BlockDevice trait requires a const or &self method, this isn't possible here
    // unless total_blocks is stored in the struct (e.g., obtained at creation).
    // Let's assume total_blocks is not a required BlockDevice trait method for now.
}

// The original SasDevice struct definitions are effectively replaced by the generic FileBlockDevice pattern.
// We can keep the name SasDevice but make it a type alias or use the generic struct directly.
// Keeping the struct name SasDevice but making it generic over RWS is better for clarity.


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


    #[test]
    fn test_sas_device_std_file() -> Result<(), BlockDeviceError> { // Return BlockDeviceError for std test
        let test_file_path = Path::new("test_sas_device.bin");
        let block_size: usize = 256; // Use usize for block_size
        let total_test_blocks = 10; // Create a file large enough for a few blocks


        // Create and open the file using the SasDevice open_file constructor
        let mut device = SasDevice::open_file(test_file_path.to_str().unwrap(), block_size)?; // Uses SasDevice::open_file


        // Ensure the file is created and initially contains zeros (or is extended on write)
        // File::options().create(true) should create an empty file.
        // Writing beyond the end should extend it automatically in std.

         // Test block_size method
         assert_eq!(device.block_size(), block_size);


        // Prepare data to write
        let mut write_buf = vec![0u8; block_size]; // Requires alloc
        for i in 0..block_size {
            write_buf[i] = (i % 256) as u8; // Fill with some pattern
        }

        // Write a block
        let block_id_to_write: u64 = 5; // Use u64 for block_id
        device.write_block(block_id_to_write, &write_buf)?; // Uses SasDevice::write_block


        // Prepare buffer to read into
        let mut read_buf = vec![0u8; block_size]; // Requires alloc

        // Read the block back
        device.read_block(block_id_to_write, &mut read_buf)?; // Uses SasDevice::read_block


        // Verify the read data matches the written data
        assert_eq!(read_buf, write_buf);


         // Test reading a block that hasn't been written (should be zeros if file was extended)
         // Similar issue as in srchdd.rs test - depends on OS file behavior.
         // Let's skip this assertion or explicitly manage file size/zeroing in setup.


         // Test read with incorrect buffer size
          let mut small_buf = vec![0u8; block_size / 2]; // Requires alloc
          let result_read_small = device.read_block(0, &mut small_buf);
          assert!(result_read_small.is_err());
           match result_read_small.unwrap_err() {
               BlockDeviceError::BlockSizeError(msg) => {
                   assert!(msg.contains("Buffer size"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_read_small.unwrap_err()),
           }

         // Test write with incorrect buffer size
          let mut large_buf = vec![0u8; block_size * 2]; // Requires alloc
          let result_write_large = device.write_block(0, &large_buf);
          assert!(result_write_large.is_err());
           match result_write_large.unwrap_err() {
               BlockDeviceError::BlockSizeError(msg) => {
                   assert!(msg.contains("Buffer size"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_write_large.unwrap_err()),
           }

        // Test creating device with zero block size
        let test_zero_block_size_path = Path::new("test_sas_zero_block_size.bin");
        let result_zero_block_size = SasDevice::open_file(test_zero_block_size_path.to_str().unwrap(), 0);
         assert!(result_zero_block_size.is_err());
          match result_zero_block_size.unwrap_err() {
              BlockDeviceError::BlockSizeError(msg) => {
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
    // This requires simulating resource acquire/release, fs::fstat, fs::read_at, fs::write_at, fs::lseek.
    // Test cases should cover opening resources, block reads/writes, invalid block sizes, and simulated IO errors.
    // Requires Mock implementations of fs functions and SahneError.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
