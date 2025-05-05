#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Write as StdWrite, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt, WriteExt as StdWriteExt}; // Added ReadExt, WriteExt
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::error::Error as StdError; // For std Error trait


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::{O_RDWR, O_CREAT}; // Import necessary fs flags


// core::fmt, core::result, core::ops::Drop, core::io
use core::fmt;
use core::result::Result;
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Write, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt, WriteExt as CoreWriteExt}; // core::io


// BlockDevice trait definition (assuming this is defined elsewhere, e.g., in blockdevice.rs)
// We'll define a simple placeholder here for context, but the actual trait should be imported.
#[cfg(not(feature = "blockdevice_trait"))] // Define placeholder if the real trait isn't available via features
pub trait BlockDevice {
    /// Reads a block from the device into the provided buffer.
    fn read_block(&mut self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockDeviceError>;

    /// Writes the provided buffer to a block on the device.
    fn write_block(&mut self, block_id: usize, buf: &[u8]) -> Result<(), BlockDeviceError>;

    /// Returns the size of a block in bytes.
    fn block_size(&self) -> usize;
}
#[cfg(feature = "blockdevice_trait")] // Use the real trait if the feature is enabled
use crate::blockdevice::BlockDevice;


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

/// Helper function to map BlockDeviceError to FileSystemError.
fn map_block_device_error_to_fs_error(e: BlockDeviceError) -> FileSystemError {
    match e {
        BlockDeviceError::IOError(io_err) => {
             #[cfg(feature = "std")]
             // In std, BlockDeviceError::IOError wraps std::io::Error
             map_std_io_error_to_fs_error(io_err)
             #[cfg(not(feature = "std"))]
             // In no_std, BlockDeviceError::IOError wraps SahneError
             map_sahne_error_to_fs_error(io_err)
        },
        BlockDeviceError::BlockSizeError(msg) => FileSystemError::InvalidData(format!("Block size mismatch or error: {}", msg)), // Map BlockSizeError to InvalidData
    }
}


// Custom error type for Block Device operations
#[derive(Debug)]
pub enum BlockDeviceError {
    #[cfg(feature = "std")]
    IOError(io::Error), // Wrap std::io::Error in std
    #[cfg(not(feature = "std"))]
    IOError(SahneError), // Wrap SahneError in no_std
    BlockSizeError(String), // Buffer size or block size related error (Requires alloc)
}

impl fmt::Display for BlockDeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockDeviceError::IOError(e) => write!(f, "Giriş/Çıkış Hatası: {}", e), // Uses Display impl of wrapped error
            BlockDeviceError::BlockSizeError(msg) => write!(f, "Blok Boyutu Hatası: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl StdError for BlockDeviceError { // Implement std Error trait in std
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            BlockDeviceError::IOError(e) => Some(e), // Provide the underlying io::Error as the source
            _ => None,
        }
    }
}


#[cfg(feature = "std")]
impl From<io::Error> for BlockDeviceError { // From conversion for std::io::Error
    fn from(error: io::Error) -> Self {
        BlockDeviceError::IOError(error)
    }
}

#[cfg(not(feature = "std"))]
impl From<SahneError> for BlockDeviceError { // From conversion for SahneError in no_std
     fn from(error: SahneError) -> Self {
          BlockDeviceError::IOError(error)
     }
}


// Sahne64 Handle'ı için core::io::Read, Write ve Seek implementasyonu (copied from srcfreespacemanagement.rs)
// This requires fs::read_at, fs::write_at, fs::lseek and fstat.
// Assuming these are part of the standardized Sahne64 FS API.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReadWriteSeek { // Renamed to reflect Read+Write+Seek
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
impl core::io::Read for SahneResourceReadWriteSeek { // Use core::io::Read trait
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
}

#[cfg(not(feature = "std"))]
impl core::io::Write for SahneResourceReadWriteSeek { // Use core::io::Write trait (for write_at)
    fn write(&mut self, buf: &[u8]) -> Result<usize, core::io::Error> { // Return core::io::Error
         // Assuming fs::write_at(handle, offset, buf) Result<usize, SahneError>
         // This write implementation writes at the current position and updates it.
         let bytes_to_write = buf.len();
         if bytes_to_write == 0 { return Ok(0); }

         let bytes_written = fs::write_at(self.handle, self.position, buf)
             .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::write_at error: {:?}", e)))?; // Map SahneError to core::io::Error

         self.position += bytes_written as u64;

         // Update file_size if writing extends beyond current size
         // Note: In a real filesystem, updating file size might require a separate syscall (e.g., ftruncate)
         // or might be handled implicitly by write_at at the end of the file.
         // Assuming for this model that writing past file_size implicitly extends it and updates fstat.
         if self.position > self.file_size {
              self.file_size = self.position;
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
impl core::io::Seek for SahneResourceReadWriteSeek { // Use core::io::Seek trait
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
     stream_position has a default implementation in core::io::Seek that uses seek(Current(0))
}

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReadWriteSeek {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReadWriteSeek is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReadWriteSeek drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant fs, memory, process, sync, kernel, arch, SahneError imports.
// Removed redundant print module and panic handler boilerplate.


/// Represents a Block Device simulated over a file.
/// Implements the BlockDevice trait.
pub struct FileBlockDevice<RWS: Read + Write + Seek + Drop> { // Generic over the underlying reader/writer/seeker
    inner: RWS, // The underlying file/resource reader/writer/seeker
    block_size: usize,
    // Add total_blocks field if needed, can be calculated from file size and block size
     total_blocks: usize,
}

impl<RWS: Read + Write + Seek + Drop> FileBlockDevice<RWS> {
    /// Creates a new FileBlockDevice instance.
    ///
    /// # Arguments
    ///
    /// * `inner`: The underlying reader/writer/seeker for the file/resource.
    /// * `block_size`: The size of a block in bytes. Must be non-zero.
    ///
    /// # Returns
    ///
    /// A new FileBlockDevice instance or BlockDeviceError::BlockSizeError if block_size is zero.
    pub fn new(inner: RWS, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
        if block_size == 0 {
            return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
        }
        // Calculate total blocks based on file size? Or is total_blocks fixed/part of metadata?
        // For this simulation, let's assume total_blocks is implicit from file size.
         let file_size = inner.seek(SeekFrom::End(0)).map_err(|e| map_core_io_error_to_block_device_error(e))?; // Requires mapping core::io::Error
         let total_blocks = (file_size as usize) / block_size; // Integer division might lose blocks

        Ok(FileBlockDevice {
            inner,
            block_size,
            // total_blocks: total_blocks,
        })
    }

    /// Opens a file from the given path and creates a FileBlockDevice over it.
    /// This is a convenience constructor for file-based block devices.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to the file/resource.
    /// * `block_size`: The size of a block in bytes.
    ///
    /// # Returns
    ///
    /// A Result containing the FileBlockDevice or a BlockDeviceError.
    #[cfg(feature = "std")]
    pub fn open_file(path: &str, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
         if block_size == 0 {
             return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
         }
         let file = File::options()
             .read(true)
             .write(true)
             .create(true) // Create the file if it doesn't exist
             .open(path)?; // io::Error is mapped to BlockDeviceError by From impl

         // In std, File implements Read + Write + Seek + Drop
         FileBlockDevice::new(file, block_size)
    }

    /// Opens a resource using Sahne64 fs calls and creates a FileBlockDevice over it.
    /// This is the convenience constructor for Sahne64 resource-based block devices.
    ///
    /// # Arguments
    ///
    /// * `path`: The path/ID of the resource.
    /// * `block_size`: The size of a block in bytes.
    ///
    /// # Returns
    ///
    /// A Result containing the FileBlockDevice or a BlockDeviceError.
    #[cfg(not(feature = "std"))]
    pub fn open_resource(path: &str, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
         if block_size == 0 {
             return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
         }
         let flags = fs::O_RDWR | fs::O_CREAT;
         let handle = fs::open(path, flags)?; // SahneError is mapped to BlockDeviceError by From impl

         // Get file size for SahneResourceReadWriteSeek
         let file_stat = fs::fstat(handle).map_err(|e| {
             let _ = resource::release(handle); // Release handle on fstat error
             BlockDeviceError::IOError(e) // Map SahneError to BlockDeviceError
         })?;
         let file_size = file_stat.size as u64;


         // SahneResourceReadWriteSeek implements Read + Write + Seek + Drop
         let resource_rws = SahneResourceReadWriteSeek::new(handle, file_size);

         FileBlockDevice::new(resource_rws, block_size)
    }


    // Helper to map core::io::Error to BlockDeviceError
    fn map_core_io_error_to_block_device_error(e: core::io::Error) -> BlockDeviceError {
         // In no_std, core::io::Error might be mapped from SahneError
         #[cfg(not(feature = "std"))]
         if let core::io::ErrorKind::Other = e.kind() { // Check if it's our generic SahneError wrapper
             // Attempt to downcast or match the original SahneError if possible
             // For now, assume the error message contains info or map generically
             return BlockDeviceError::IOError(SahneError::Other(format!("Core IO Error during block op: {:?}", e))); // Re-wrap or map
         }
         // Otherwise, map core::io::Error kind
         BlockDeviceError::IOError(SahneError::Other(format!("Core IO Error during block op: {:?}", e))) // Map generically
         #[cfg(feature = "std")]
         // In std, core::io::Error is std::io::Error, already handled by From impl
         BlockDeviceError::IOError(StdIOError::new(e.kind(), format!("Core IO Error during block op: {:?}", e))) // Map kind
    }
}


impl<RWS: Read + Write + Seek + Drop> BlockDevice for FileBlockDevice<RWS> { // Implement BlockDevice for FileBlockDevice
    // Use the standardized underlying reader/writer/seeker
    fn read_block(&mut self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockDeviceError> { // Return BlockDeviceError
        // Check if buffer size matches block size
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.block_size) // Requires alloc
            ));
        }

        // Calculate the byte offset for the block
        let offset = block_id as u64 * self.block_size as u64;

        // Seek to the correct offset in the underlying reader/writer/seeker
        // Map core::io::Error from seek to BlockDeviceError
        self.inner.seek(SeekFrom::Start(offset)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        // Read exactly the required number of bytes (one block)
        // Map core::io::Error from read_exact to BlockDeviceError
        self.inner.read_exact(buf).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        Ok(()) // Return success
    }

    // Use the standardized underlying reader/writer/seeker
    fn write_block(&mut self, block_id: usize, buf: &[u8]) -> Result<(), BlockDeviceError> { // Return BlockDeviceError
        // Check if buffer size matches block size
        if buf.len() != self.block_size {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.block_size) // Requires alloc
            ));
        }

        // Calculate the byte offset for the block
        let offset = block_id as u64 * self.block_size as u64;

        // Seek to the correct offset in the underlying reader/writer/seeker
        // Map core::io::Error from seek to BlockDeviceError
        self.inner.seek(SeekFrom::Start(offset)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        // Write exactly the required number of bytes (one block)
        // Map core::io::Error from write_all to BlockDeviceError
        self.inner.write_all(buf).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;


        Ok(()) // Return success
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    // Add total_blocks() method if total_blocks was calculated and stored.
    // For a file-backed block device, total_blocks = file_size / block_size.
    // Getting file size requires Seek::seek(End).
     fn total_blocks(&mut self) -> Result<usize, BlockDeviceError> { // Needs mut self for seeking
         let file_size = self.inner.seek(SeekFrom::End(0)).map_err(|e| Self::map_core_io_error_to_block_device_error(e))?;
    //     // Seek back to the original position? Depends on trait requirements.
    //     // If BlockDevice trait doesn't require seeking, we can't do this here.
    //     // Assuming total_blocks might be stored in Superblock or determined at creation.
         Ok((file_size as usize) / self.block_size)
     }
}

// The HDD struct is replaced by the generic FileBlockDevice and its constructors (open_file, open_resource).
// The original HDD struct was just a file-backed block device.

#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Write, Seek, SeekFrom}; // For File/Cursor traits
    use std::fs::{remove_file, OpenOptions}; // For creating/managing test files
    use std::path::Path;
    use alloc::string::ToString; // For to_string()


    // Helper function to map std::io::Error to BlockDeviceError in tests
    fn map_std_io_error_to_block_device_error_test(e: std::io::Error) -> BlockDeviceError {
        BlockDeviceError::IOError(e) // Direct mapping in std tests
    }

    // Helper function to map core::io::Error to BlockDeviceError in tests (for Mock)
    fn map_core_io_error_to_block_device_error_test(e: core::io::Error) -> BlockDeviceError {
         #[cfg(not(feature = "std"))] // This mapping is only relevant in no_std tests with a mock core::io::Error
         {
              // Assuming CoreIOError has a debug impl or can be mapped to SahneError
              BlockDeviceError::IOError(crate::SahneError::Other(format!("Mock Core IO Error: {:?}", e))) // Map generically for mock
         }
          #[cfg(feature = "std")] // In std tests, core::io::Error is std::io::Error
         BlockDeviceError::IOError(std::io::Error::new(e.kind(), format!("Core IO Error in test: {:?}", e)))
    }


    #[test]
    fn test_file_block_device_std_file() -> Result<(), BlockDeviceError> { // Return BlockDeviceError for std test
        let test_file_path = Path::new("test_block_device.bin");
        let block_size = 512;
        let total_test_blocks = 10; // Create a file large enough for a few blocks


        // Create and open the file using the FileBlockDevice constructor
        let mut device = FileBlockDevice::open_file(test_file_path.to_str().unwrap(), block_size)?; // Uses FileBlockDevice::open_file


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
        let block_id_to_write = 2;
        device.write_block(block_id_to_write, &write_buf)?; // Uses FileBlockDevice::write_block


        // Prepare buffer to read into
        let mut read_buf = vec![0u8; block_size]; // Requires alloc

        // Read the block back
        device.read_block(block_id_to_write, &mut read_buf)?; // Uses FileBlockDevice::read_block


        // Verify the read data matches the written data
        assert_eq!(read_buf, write_buf);


         // Test reading a block that hasn't been written (should be zeros if file was extended)
         let mut zero_buf = vec![0u8; block_size]; // Requires alloc
         device.read_block(0, &mut read_buf)?; // Read block 0
         // This might fail if the file isn't explicitly sized or zeroed beforehand.
         // std::fs::File doesn't guarantee zeroing on extension.
         // Let's skip this assertion or explicitly truncate/zero the file in setup.
         // Or, rely on the write_block test being sufficient.

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


        // Clean up the test file
        remove_file(test_file_path).expect("Test dosyası silinemedi");


        Ok(()) // Return Ok from test function
    }

    #[test]
     fn test_file_block_device_zero_block_size() {
          let test_file_path = Path::new("test_zero_block_size.bin");
          let block_size = 0;

           // Attempt to create device with zero block size, expect error
          let result = FileBlockDevice::open_file(test_file_path.to_str().unwrap(), block_size);

          assert!(result.is_err());
          match result.unwrap_err() {
              BlockDeviceError::BlockSizeError(msg) => {
                  assert!(msg.contains("Block size cannot be zero."));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
          }

          // Clean up any potentially created file (open(create) might still create it)
          if test_file_path.exists() {
              remove_file(test_file_path).unwrap_or_default();
          }
     }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This requires simulating resource acquire/release, fs::fstat, fs::read_at, fs::write_at, fs::lseek.
    // Test cases should cover opening resources, block reads/writes, invalid block sizes, and simulated IO errors.
    // Requires Mock implementations of fs functions and SahneError.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "blockdevice_trait", test)))] // Only when not building std, the real blockdevice trait, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
