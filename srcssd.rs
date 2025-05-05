#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std")), no_std)] // Standart kütüphaneye ihtiyacımız yok

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
// Removed redundant imports like fs, memory, process, sync, kernel, arch, SahneError
use crate::FileSystemError; // Assuming FileSystemError is in crate


// Use alloc's Vec for both std and no_std builds
use alloc::vec::Vec;
use alloc::string::{String, ToString}; // For error messages
use alloc::format;


// core::result
use core::result::Result;


// Import the standard BlockDevice trait and its error type
use crate::blockdevice::{BlockDevice, BlockDeviceError}; // Assuming these are in crate::blockdevice

// Assuming BlockDeviceError has InvalidParameter and BlockSizeError variants


/// Helper function to map BlockDeviceError to FileSystemError (copied from other files)
fn map_block_device_error_to_fs_error(e: BlockDeviceError) -> FileSystemError {
    match e {
        BlockDeviceError::IoError(io_err) => {
             #[cfg(feature = "std")]
             // In std, BlockDeviceError::IoError wraps std::io::Error
             FileSystemError::IOError(format!("IO Error: {}", io_err)) // Assuming io_err is std::io::Error
             #[cfg(not(feature = "std"))]
             // In no_std, BlockDeviceError::IoError wraps SahneError
             FileSystemError::IOError(format!("SahneError: {:?}", io_err)) // Assuming io_err is SahneError
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


/// In-memory simulation of an SSD Block Device.
/// Stores block data in a Vec<Vec<u8>>.
/// WARNING: This stores the entire simulated device content in RAM,
/// which is only suitable for small sizes or testing.
pub struct SSD {
    // Physical characteristics (size, block size) - Redundant if derived from blocks Vec
    // size: usize, // Can be calculated as blocks.len() * block_size
    block_size: usize, // Logical block size
    block_count: u64, // Total number of blocks

    // Internal data structure holding block content in memory
    blocks: Vec<Vec<u8>>, // Requires alloc and Vec<Vec<u8>>
}

impl SSD {
    /// Creates a new in-memory SSD simulation.
    /// Allocates memory for all blocks and initializes them to zeros.
    ///
    /// # Arguments
    ///
    /// * `block_count`: The total number of blocks for the simulated device.
    /// * `block_size`: The size of each block in bytes. Must be non-zero.
    ///
    /// # Returns
    ///
    /// A new SSD instance or BlockDeviceError::BlockSizeError if block_size is zero.
    pub fn new(block_count: u64, block_size: usize) -> Result<Self, BlockDeviceError> { // Return BlockDeviceError
        if block_size == 0 {
            return Err(BlockDeviceError::BlockSizeError(String::from("Block size cannot be zero."))); // Requires alloc
        }

        // Allocate memory for all blocks and initialize them to zeros
        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(block_count as usize); // Requires alloc
        for _ in 0..block_count {
            // Create each block as a vector of zeros (Requires alloc)
            blocks.push(vec![0u8; block_size]);
        }

        Ok(SSD {
            // size: (block_count as usize) * block_size, // Calculated size
            block_size,
            block_count,
            blocks,
        })
    }

    // Add persistence methods (placeholders) if needed for saving/loading the in-memory state.
     pub fn load_from_file(path: &str, block_count: u64, block_size: usize) -> Result<Self, FileSystemError> { ... }
     pub fn save_to_file(&self, path: &str) -> Result<(), FileSystemError> { ... }
}

// Implement the standard BlockDevice trait for the in-memory SSD
impl BlockDevice for SSD {
    /// Reads a block from the in-memory SSD simulation into the provided buffer.
    ///
    /// # Arguments
    ///
    /// * `block_id`: The index of the block to read (0-based).
    /// * `buf`: The buffer to read the data into. Its length must match the block size.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a BlockDeviceError (InvalidParameter or BlockSizeError).
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), BlockDeviceError> { // Use u64 for block_id, &mut self, BlockDeviceError
        // Check if block_id is out of bounds
        if block_id >= self.block_count { // Use self.block_count
            return Err(BlockDeviceError::InvalidParameter(format!("Block ID {} is out of bounds. Total blocks: {}", block_id, self.block_count))); // Requires alloc
        }
        // Check if buffer size matches block size
        if buf.len() != self.block_size { // Use self.block_size
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.block_size) // Requires alloc
            ));
        }

        // Copy data from the in-memory block to the buffer
        // Accessing blocks Vec requires mutable reference if the method takes &mut self
        // Accessing the inner Vec<u8> element requires immutable reference if blocks[block_id] is used directly.
        // Since read_block only copies data out, it could conceptually take &self if the trait allowed.
        // However, the standardized trait uses &mut self. Access the block mutably then copy immutably.
        let source_block = &self.blocks[block_id as usize]; // Use as usize for Vec index
        buf.copy_from_slice(source_block.as_slice()); // Use as_slice() for clarity


        Ok(()) // Return success
    }

    /// Writes the provided buffer to a block in the in-memory SSD simulation.
    ///
    /// # Arguments
    ///
    /// * `block_id`: The index of the block to write to (0-based).
    /// * `buf`: The buffer containing the data to write. Its length must match the block size.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a BlockDeviceError (InvalidParameter or BlockSizeError).
    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> Result<(), BlockDeviceError> { // Use u64 for block_id, &mut self, BlockDeviceError
        // Check if block_id is out of bounds
        if block_id >= self.block_count { // Use self.block_count
            return Err(BlockDeviceError::InvalidParameter(format!("Block ID {} is out of bounds. Total blocks: {}", block_id, self.block_count))); // Requires alloc
        }
        // Check if buffer size matches block size
        if buf.len() != self.block_size { // Use self.block_size
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer size ({}) must match block size ({}).", buf.len(), self.block_size) // Requires alloc
            ));
        }

        // Copy data from the buffer to the in-memory block
        // Accessing blocks Vec requires mutable reference
        let destination_block = &mut self.blocks[block_id as usize]; // Use as usize for Vec index
        destination_block.copy_from_slice(buf);


        Ok(()) // Return success
    }

    /// Returns the logical block size of the in-memory SSD.
    fn block_size(&self) -> usize { // Return usize
        self.block_size // Use self.block_size
    }

    /// Returns the total number of blocks in the in-memory SSD.
    fn block_count(&self) -> u64 { // Return u64 (assuming BlockDevice trait includes this)
        self.block_count // Use self.block_count
    }

    // Implement size() if required by the standardized BlockDevice trait.
     fn size(&self) -> u64 { // Return u64 (total size in bytes)
         self.block_count * self.block_size as u64
     }
}


#[cfg(test)]
#[cfg(feature = "std")] // Although it's an in-memory device, std features make testing easier
mod tests {
    // Need alloc for Vec and String
    use super::*;
    use alloc::string::ToString; // For to_string()
    use alloc::vec::Vec; // For Vec

    // Helper function to map BlockDeviceError to FileSystemError in tests if needed
    // (though tests here return BlockDeviceError directly)
    fn map_block_device_error_to_fs_error_test(e: BlockDeviceError) -> FileSystemError {
         map_block_device_error_to_fs_error(e) // Reuse the production mapping
    }


    #[test]
    fn test_ssd_new_and_size() -> Result<(), BlockDeviceError> { // Return BlockDeviceError
        let block_count: u64 = 10;
        let block_size: usize = 256;
        let ssd = SSD::new(block_count, block_size)?; // Requires alloc

        // Check basic properties
        assert_eq!(ssd.block_size(), block_size);
        assert_eq!(ssd.block_count(), block_count);
         assert_eq!(ssd.size(), block_count * block_size as u64); // If size() is implemented


        // Check bitmap initialization (should be all zeros)
        // Iterate through blocks and check content
        for i in 0..block_count {
             let mut read_buf = vec![1u8; block_size]; // Fill with non-zero to ensure it's overwritten
             ssd.read_block(i, &mut read_buf)?; // Read block
             assert_eq!(read_buf, vec![0u8; block_size]); // Should be all zeros
        }


        // Test creating with zero block size
         let result_zero_block_size = SSD::new(10, 0);
         assert!(result_zero_block_size.is_err());
          match result_zero_block_size.unwrap_err() {
              BlockDeviceError::BlockSizeError(msg) => {
                  assert!(msg.contains("Block size cannot be zero."));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result_zero_block_size.unwrap_err()),
          }


        Ok(()) // Return Ok from test function
    }

    #[test]
    fn test_ssd_read_write() -> Result<(), BlockDeviceError> { // Return BlockDeviceError
        let block_count: u64 = 10;
        let block_size: usize = 256;
        let mut ssd = SSD::new(block_count, block_size)?; // Requires alloc

        // Prepare data to write
        let mut write_buf = vec![0u8; block_size]; // Requires alloc
        for i in 0..block_size {
            write_buf[i] = (i % 256) as u8; // Fill with some pattern
        }

        // Write a block
        let block_id_to_write: u64 = 5; // Use u64 for block_id
        ssd.write_block(block_id_to_write, &write_buf)?; // Use write_block


        // Prepare buffer to read into
        let mut read_buf = vec![0u8; block_size]; // Requires alloc

        // Read the block back
        ssd.read_block(block_id_to_write, &mut read_buf)?; // Use read_block


        // Verify the read data matches the written data
        assert_eq!(read_buf, write_buf);

        // Test reading a block that hasn't been written (should be zeros)
         let mut zero_buf = vec![0u8; block_size]; // Requires alloc
         let block_id_zero: u64 = 3; // A block that wasn't written to
         ssd.read_block(block_id_zero, &mut read_buf)?; // Read block
         assert_eq!(read_buf, zero_buf); // Should still be zeros


        Ok(()) // Return Ok from test function
    }

    #[test]
     fn test_ssd_invalid_parameters() -> Result<(), BlockDeviceError> { // Return BlockDeviceError
          let block_count: u64 = 10;
          let block_size: usize = 256;
          let mut ssd = SSD::new(block_count, block_size)?; // Requires alloc
          let mut dummy_buf = vec![0u8; block_size]; // Requires alloc


          // Test read with out of bounds block ID
           let result_read_oob = ssd.read_block(block_count, &mut dummy_buf); // block_count is out of bounds
           assert!(result_read_oob.is_err());
           match result_read_oob.unwrap_err() {
               BlockDeviceError::InvalidParameter(msg) => { // Mapped from InvalidParameter
                   assert!(msg.contains("Block ID") && msg.contains("is out of bounds"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_read_oob.unwrap_err()),
           }

          // Test write with out of bounds block ID
           let result_write_oob = ssd.write_block(block_count, &dummy_buf); // block_count is out of bounds
           assert!(result_write_oob.is_err());
           match result_write_oob.unwrap_err() {
               BlockDeviceError::InvalidParameter(msg) => { // Mapped from InvalidParameter
                   assert!(msg.contains("Block ID") && msg.contains("is out of bounds"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_write_oob.unwrap_err()),
           }

         // Test read with incorrect buffer size
          let mut small_buf = vec![0u8; block_size / 2]; // Requires alloc
          let result_read_small = ssd.read_block(0, &mut small_buf);
          assert!(result_read_small.is_err());
           match result_read_small.unwrap_err() {
               BlockDeviceError::BlockSizeError(msg) => { // Mapped from BlockSizeError
                   assert!(msg.contains("Buffer size"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_read_small.unwrap_err()),
           }

         // Test write with incorrect buffer size
          let mut large_buf = vec![0u8; block_size * 2]; // Requires alloc
          let result_write_large = ssd.write_block(0, &large_buf);
          assert!(result_write_large.is_err());
           match result_write_large.unwrap_err() {
               BlockDeviceError::BlockSizeError(msg) => { // Mapped from BlockSizeError
                   assert!(msg.contains("Buffer size"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_write_large.unwrap_err()),
           }


          Ok(()) // Return Ok from test function
     }


    // TODO: Add tests for concurrency if Spinlock/Mutex is added around SSD instance.
    // Requires simulating multiple threads accessing the same SSD instance.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
