#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::collections::HashMap;

// no_std uyumlu HashMap (hashbrown)
#[cfg(not(feature = "std"))]
use hashbrown::HashMap;


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
// We won't use SahneError directly here, but FileSystemError is used for return types.


use crate::sync::spinlock::Spinlock; // Spinlock for concurrency control
use spin::Mutex; // Mutex for concurrency control (from spin crate)


use alloc::string::{String, ToString};
use alloc::vec::Vec; // Use alloc's Vec
use alloc::sync::Arc; // Use alloc's Arc for sharing

use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp;
use core::ops::{Index, IndexMut}; // For Vec indexing (if needed directly, though methods are preferred)

// Helper function to map SahneError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
    // TODO: Implement a proper mapping based on SahneError variants
}

// Helper function to map std::io::Error to FileSystemError (copied from other files)
#[cfg(feature = "std")]
fn map_std_io_error_to_fs_error(e: std::io::Error) -> FileSystemError {
    FileSystemError::IOError(format!("IO Error: {}", e))
}

// Helper function to map CoreIOError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_core_io_error_to_fs_error(e: core::io::Error) -> FileSystemError {
     FileSystemError::IOError(format!("CoreIOError: {:?}", e))
     // TODO: Implement a proper mapping based on CoreIOErrorKind
}

// Add specific FileSystemError variants for Free Space Management
impl FileSystemError {
    pub fn OutOfSpace(msg: String) -> Self { FileSystemError::Other(format!("Out of space: {}", msg)) } // Generic Out of Space for now
    pub fn InvalidBlockIndex(msg: String) -> Self { FileSystemError::InvalidParameter(format!("Invalid block index: {}", msg)) } // Invalid Block Index
    // TODO: Add more specific variants like BlockAlreadyFree, BlockAlreadyAllocated
}


/// Manages free and allocated blocks using a bitmap.
/// Designed for a single storage device/partition.
pub struct FreeSpaceManager {
    bitmap: Vec<u8>, // Bitmap where each bit represents a block (0 = free, 1 = allocated)
    block_size: usize,
    total_blocks: usize,
    // Add fields for persistence: e.g., superblock reference, bitmap start block/offset, dirty flag.
     superblock: Arc<Spinlock<Superblock>>, // Reference to the superblock (for persistence)
     bitmap_start_block: u64, // Starting block address of the bitmap on disk
     is_dirty: Mutex<bool>, // Flag to indicate if the bitmap has changed and needs to be written to disk
}

impl FreeSpaceManager {
    /// Creates a new in-memory FreeSpaceManager instance.
    /// The bitmap is initialized to all free (all bits are 0).
    /// This should ideally be loaded from disk for a persistent filesystem.
    ///
    /// # Arguments
    ///
    /// * `total_blocks`: The total number of blocks on the device.
    /// * `block_size`: The size of each block in bytes.
    ///
    /// # Returns
    ///
    /// A new FreeSpaceManager instance.
    pub fn new(total_blocks: usize, block_size: usize) -> Self {
        let bitmap_size_bytes = (total_blocks + 7) / 8;
        // Initialize bitmap with all zeros (all blocks free)
        let bitmap = Vec::with_capacity(bitmap_size_bytes); // Requires alloc
        // Fill the vector with zeros up to the calculated size.
        // Vec::resize is available in alloc::vec.
         #[cfg(feature = "std")] // std::vec::Vec has resize
         let mut bitmap = std::vec::Vec::new();
         #[cfg(feature = "std")]
         bitmap.resize(bitmap_size_bytes, 0);

         #[cfg(not(feature = "std"))] // alloc::vec::Vec needs alloc feature
         let mut bitmap = alloc::vec::Vec::new();
         #[cfg(not(feature = "std"))]
         bitmap.resize(bitmap_size_bytes, 0); // resize is in alloc::vec::Vec


        FreeSpaceManager {
            bitmap,
            block_size,
            total_blocks,
            // is_dirty: Mutex::new(false), // Initialize dirty flag
        }
    }

    /// Loads a FreeSpaceManager from raw bitmap data (e.g., read from disk).
    ///
    /// # Arguments
    ///
    /// * `bitmap_data`: Raw bytes of the bitmap read from disk.
    /// * `total_blocks`: The total number of blocks on the device.
    /// * `block_size`: The size of each block in bytes.
    ///
    /// # Returns
    ///
    /// A new FreeSpaceManager instance loaded from the provided data.
    pub fn load_from_data(bitmap_data: Vec<u8>, total_blocks: usize, block_size: usize) -> Result<Self, FileSystemError> { // Return FileSystemError
         let expected_bitmap_size = (total_blocks + 7) / 8;
         if bitmap_data.len() != expected_bitmap_size {
             return Err(FileSystemError::InvalidData(format!("Bitmap data size mismatch. Expected {}, found {}.", expected_bitmap_size, bitmap_data.len()))); // Requires alloc
         }

         // Check if the last byte has unexpected bits set if total_blocks is not a multiple of 8
          let last_byte_index = bitmap_data.len().saturating_sub(1);
          if total_blocks % 8 != 0 && last_byte_index < bitmap_data.len() {
               let used_bits_in_last_byte = total_blocks % 8;
               let unused_bits_mask: u8 = !((1 << used_bits_in_last_byte) - 1);
               if (bitmap_data[last_byte_index] & unused_bits_mask) != 0 {
                    // There are unexpected set bits in the unused portion of the last byte.
                    // This might indicate corruption.
                    eprintln!("WARN: Unexpected set bits in unused portion of bitmap last byte."); // Use standardized print
                    // Decide whether to return error or just warn. Returning error is safer.
                    return Err(FileSystemError::InvalidData(String::from("Bitmap corrupted: Unexpected bits set in last byte."))); // Requires alloc
               }
          }


        Ok(FreeSpaceManager {
            bitmap: bitmap_data, // Requires alloc (takes ownership)
            block_size,
            total_blocks,
             is_dirty: Mutex::new(false), // Initialize dirty flag (assume not dirty on load unless specified)
        })
    }

    /// Writes the bitmap data to a writer (e.g., file/device block).
    ///
    /// # Arguments
    ///
    /// * `writer`: A mutable reference to the writer implementing Write + Seek.
    ///             The writer should be positioned correctly to write the bitmap.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError.
    pub fn save_to_writer<W: core::io::Write + core::io::Seek>(&self, mut writer: W) -> Result<(), FileSystemError> {
    ///     // Writer must be positioned correctly before calling this function.
     Example: writer.seek(core::io::SeekFrom::Start(self.bitmap_start_block * self.block_size as u64))?;
    ///
          writer.write_all(&self.bitmap).map_err(|e| map_core_io_error_to_fs_error(e))?; // Requires core::io::WriteExt
          writer.flush().map_err(|e| map_core_io_error_to_fs_error(e))?; // Requires core::io::WriteExt
    ///
    ///      // Clear dirty flag after successful save
     *self.is_dirty.lock() = false;
    
         Ok(())
     }


    /// Finds and allocates the first free block.
    /// Marks the block as allocated in the bitmap.
    ///
    /// # Returns
    ///
    /// A Result containing the index of the allocated block, or
    /// FileSystemError::OutOfSpace if no free blocks are available.
    pub fn allocate_block(&mut self) -> Result<usize, FileSystemError> { // Return Result<usize, FileSystemError>
        // Find the first byte that is not all 1s (optimized check)
        for (byte_index, byte) in self.bitmap.iter().enumerate() {
            if *byte != 255 { // Check if the whole byte is not full (all bits are 1)
                // Iterate through the bits in this byte to find the first zero bit
                for bit_index in 0..8 {
                    // Calculate the potential block index
                    let block_index = byte_index * 8 + bit_index;
                    // Ensure the block index is within the total number of blocks
                    if block_index < self.total_blocks {
                         // Check if the specific bit is 0 (free)
                        if (*byte & (1 << bit_index)) == 0 {
                            // Mark the block as allocated by setting the bit to 1
                            // Needs mutable access to the bitmap Vec
                             self.bitmap[byte_index] |= (1 << bit_index); // Bit manipulation requires mut ref

                            // Mark the manager as dirty (needs saving to disk)
                            // *self.is_dirty.lock() = true;

                             return Ok(block_index); // Return the index of the allocated block
                        }
                    } else {
                         // We've iterated past the total_blocks boundary within the bitmap bytes.
                         // This can happen if total_blocks is not a multiple of 8 and the last byte
                         // contains bits beyond total_blocks. We should stop checking bits for this byte.
                         break; // Stop checking bits in this byte and move to the next byte (if any)
                    }
                }
            }
        }

        // If the loop finishes without finding a free block, there is no space.
        Err(FileSystemError::OutOfSpace(String::from("No free blocks available."))) // Requires alloc
    }

    /// Deallocates a previously allocated block.
    /// Marks the block as free in the bitmap.
    ///
    /// # Arguments
    ///
    /// * `block_index`: The index of the block to deallocate.
    ///
    /// # Returns
    ///
    /// A Result indicating success or FileSystemError::InvalidBlockIndex if the index is out of bounds.
    pub fn deallocate_block(&mut self, block_index: usize) -> Result<(), FileSystemError> { // Return Result<(), FileSystemError>
        // Check if the block index is within the valid range
        if block_index >= self.total_blocks {
            return Err(FileSystemError::InvalidBlockIndex(format!("Block index {} is out of bounds. Total blocks: {}.", block_index, self.total_blocks))); // Requires alloc
        }

        let byte_index = block_index / 8;
        let bit_index = block_index % 8;

        // Check if the block is currently allocated (bit is 1) before deallocating
        if (self.bitmap[byte_index] & (1 << bit_index)) != 0 {
            // Mark the block as free by clearing the bit to 0
             self.bitmap[byte_index] &= !(1 << bit_index); // Bit manipulation requires mut ref

            // Mark the manager as dirty (needs saving to disk)
             *self.is_dirty.lock() = true;

            Ok(()) // Deallocation successful
        } else {
             // Attempting to deallocate a block that is already free.
             // This might be an error or just a warning depending on filesystem policy.
             // For now, return an error indicating the block was not allocated.
             // TODO: Add a specific error variant like FileSystemError::BlockAlreadyFree
             Err(FileSystemError::InvalidBlockIndex(format!("Block index {} is already free or invalid.", block_index))) // Requires alloc
        }
    }

    /// Checks if a block is free.
    ///
    /// # Arguments
    ///
    /// * `block_index`: The index of the block to check.
    ///
    /// # Returns
    ///
    /// A Result indicating whether the block is free (Ok(true/false)) or
    /// FileSystemError::InvalidBlockIndex if the index is out of bounds.
    pub fn is_block_free(&self, block_index: usize) -> Result<bool, FileSystemError> { // Return Result<bool, FileSystemError>
        // Check if the block index is within the valid range
        if block_index >= self.total_blocks {
            return Err(FileSystemError::InvalidBlockIndex(format!("Block index {} is out of bounds. Total blocks: {}.", block_index, self.total_blocks))); // Requires alloc
        }

        let byte_index = block_index / 8;
        let bit_index = block_index % 8;

        // Check if the specific bit is 0 (free)
        // Accessing bitmap element requires immutable access.
        let is_free = (self.bitmap[byte_index] & (1 << bit_index)) == 0;

        Ok(is_free) // Return the free status
    }

    /// Gets the total number of blocks managed.
    pub fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    /// Gets the block size.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Gets the size of the bitmap in bytes.
     pub fn bitmap_size_bytes(&self) -> usize {
         self.bitmap.len()
     }

     /// Gets a reference to the raw bitmap data.
      pub fn raw_bitmap_data(&self) -> &[u8] {
          &self.bitmap
      }

     // Add persistence methods (placeholders)
     /// Marks the free space manager as dirty, indicating it needs to be saved to disk.
      pub fn mark_dirty(&self) {
          *self.is_dirty.lock() = true;
      }
     ///
     /// /// Checks if the free space manager is dirty and needs to be saved.
      pub fn is_dirty(&self) -> bool {
          *self.is_dirty.lock()
      }
     ///
     /// /// Saves the bitmap to disk.
     /// /// This requires access to the underlying storage device (e.g., via a trait or function).
      pub fn save(&self, device_writer: &mut impl core::io::Write + core::io::Seek) -> Result<(), FileSystemError> {
     /// ///     // This function needs the starting block/offset of the bitmap on the device
     /// ///     // and potentially the block size to calculate the write position.
     /// ///     // Using save_to_writer helper.
     /// ///     // This requires knowing the correct seek position for the bitmap.
     /// ///     // Let's assume the caller ensures the writer is positioned correctly.
          self.save_to_writer(device_writer)?;
          Ok(())
      }
}


/// Manages FreeSpaceManager instances for multiple storage devices.
/// Access to FreeSpaceManager instances within the map should be thread-safe.
pub struct DeviceManager {
     // Store FreeSpaceManager instances, protected by a Mutex for thread-safe access.
     // Using Arc<Mutex<FreeSpaceManager>> allows multiple threads to get a clone of the Arc
     // and lock the Mutex to access the FreeSpaceManager concurrently.
     // HashMap needs to be Sync if DeviceManager is used across threads.
     // hashbrown::HashMap is Send + Sync if K and V are Send + Sync.
    devices: HashMap<String, Arc<Mutex<FreeSpaceManager>>>, // Requires alloc, String, Arc, Mutex, HashMap
    // Add fields for persistence: e.g., filesystem reference.
    // filesystem: Arc<Spinlock<FileSystem>>, // Reference to the parent filesystem
}

impl DeviceManager {
    /// Creates a new DeviceManager instance.
    pub fn new() -> Self {
        DeviceManager {
            devices: HashMap::new(), // Requires alloc and HashMap
        }
    }

    /// Adds a new device with its FreeSpaceManager to the manager.
    /// The FreeSpaceManager should typically be loaded from disk for persistence.
    ///
    /// # Arguments
    ///
    /// * `device_name`: The unique name of the device.
    /// * `fsm`: The FreeSpaceManager instance for the device (likely loaded from disk).
    pub fn add_device(&mut self, device_name: String, fsm: FreeSpaceManager) { // Takes owned String and FreeSpaceManager
        let shared_fsm = Arc::new(Mutex::new(fsm)); // Wrap FSM in Arc<Mutex> for thread safety (Requires alloc, Arc, Mutex)
        self.devices.insert(device_name, shared_fsm); // Requires alloc and HashMap
    }

    /// Finds a device's FreeSpaceManager by name and returns a clone of its Arc<Mutex>.
    /// This allows multiple callers to hold references to the FSM and lock it.
    ///
    /// # Arguments
    ///
    /// * `device_name`: The name of the device.
    ///
    /// # Returns
    ///
    /// An Option containing a clone of the Arc<Mutex<FreeSpaceManager>>, or None if the device is not found.
     pub fn get_device_fsm(&self, device_name: &str) -> Option<Arc<Mutex<FreeSpaceManager>>> {
         self.devices.get(device_name).cloned() // Requires HashMap::get and Arc::cloned
     }


    /// Allocates a block from the specified device.
    /// Locks the device's FreeSpaceManager to perform the allocation.
    ///
    /// # Arguments
    ///
    /// * `device_name`: The name of the device.
    ///
    /// # Returns
    ///
    /// A Result containing a tuple of (device_name: String, block_index: usize) if successful,
    /// or a FileSystemError if the device is not found or allocation fails.
    pub fn allocate_block(&self, device_name: &str) -> Result<(String, usize), FileSystemError> { // Take &self, return Result
        // Get the FreeSpaceManager for the device (requires Arc<Mutex>)
        if let Some(fsm_arc) = self.get_device_fsm(device_name) { // Use get_device_fsm helper
            // Lock the FreeSpaceManager to perform allocation
            let mut fsm = fsm_arc.lock(); // Acquire the Mutex lock

            // Perform the allocation using the locked FreeSpaceManager
            match fsm.allocate_block() { // This returns Result<usize, FileSystemError>
                Ok(block_index) => {
                    // Allocation successful, return the device name and block index
                    Ok((device_name.to_string(), block_index)) // Requires alloc and String
                }
                Err(e) => {
                    // Allocation failed (e.g., out of space), return the error
                    Err(e)
                }
            } // Mutex lock is released when fsm goes out of scope
        } else {
            // Device not found
            Err(FileSystemError::NotFound(format!("Device '{}' not found.", device_name))) // Requires alloc and String
        }
    }

    /// Deallocates a block on the specified device.
    /// Locks the device's FreeSpaceManager to perform the deallocation.
    ///
    /// # Arguments
    ///
    /// * `device_name`: The name of the device.
    /// * `block_index`: The index of the block to deallocate.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError if the device is not found
    /// or deallocation fails (e.g., invalid block index, block already free).
    pub fn deallocate_block(&self, device_name: &str, block_index: usize) -> Result<(), FileSystemError> { // Take &self, return Result
        // Get the FreeSpaceManager for the device
        if let Some(fsm_arc) = self.get_device_fsm(device_name) { // Use get_device_fsm helper
            // Lock the FreeSpaceManager to perform deallocation
            let mut fsm = fsm_arc.lock(); // Acquire the Mutex lock

            // Perform the deallocation using the locked FreeSpaceManager
            fsm.deallocate_block(block_index) // This returns Result<(), FileSystemError>
            // Mutex lock is released when fsm goes out of scope
        } else {
            // Device not found
            Err(FileSystemError::NotFound(format!("Device '{}' not found.", device_name))) // Requires alloc and String
        }
    }

    /// Checks if a block is free on the specified device.
    /// Locks the device's FreeSpaceManager to perform the check.
    ///
    /// # Arguments
    ///
    /// * `device_name`: The name of the device.
    /// * `block_index`: The index of the block to check.
    ///
    /// # Returns
    ///
    /// A Result indicating whether the block is free (Ok(true/false)) or
    /// a FileSystemError if the device is not found or the block index is out of bounds.
    pub fn is_block_free(&self, device_name: &str, block_index: usize) -> Result<bool, FileSystemError> { // Take &self, return Result
        // Get the FreeSpaceManager for the device
        if let Some(fsm_arc) = self.get_device_fsm(device_name) { // Use get_device_fsm helper
            // Lock the FreeSpaceManager to perform the check
            let fsm = fsm_arc.lock(); // Acquire the Mutex lock

            // Perform the check using the locked FreeSpaceManager
            fsm.is_block_free(block_index) // This returns Result<bool, FileSystemError>
            // Mutex lock is released when fsm goes out of scope
        } else {
            // Device not found
            Err(FileSystemError::NotFound(format!("Device '{}' not found.", device_name))) // Requires alloc and String
        }
    }

    // Add persistence management methods (placeholders)
    /// Checks if any FreeSpaceManager managed by this DeviceManager is dirty.
     pub fn any_dirty(&self) -> bool {
         self.devices.iter().any(|(_, fsm_arc)| fsm_arc.lock().is_dirty())
     }
    ///
    /// /// Saves all dirty FreeSpaceManagers to their respective devices.
    /// /// This requires a mechanism to access the device writers.
     pub fn save_dirty(&self, device_writer_factory: &impl DeviceWriterFactory) -> Result<(), FileSystemError> {
         for (device_name, fsm_arc) in self.devices.iter() {
             let mut fsm = fsm_arc.lock();
             if fsm.is_dirty() {
    /// ///             // Get the writer for this device (requires a factory or lookup)
    /// ///             // This needs careful synchronization if device writers are shared.
                  let mut device_writer = device_writer_factory.get_writer(device_name)?;
                  fsm.save(&mut device_writer)?; // Save the bitmap
                  drop(device_writer); // Ensure writer is dropped and potentially flushed
             }
         }
         Ok(())
     }
}


#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use alloc::string::ToString; // For .to_string() in tests


    #[test]
    fn test_free_space_manager_allocation() -> Result<(), FileSystemError> { // Return FileSystemError
        let total_blocks = 20; // A few bytes in the bitmap
        let block_size = 512;
        let mut fsm = FreeSpaceManager::new(total_blocks, block_size); // Requires alloc

         // Bitmap size should be (20 + 7) / 8 = 3 bytes
         assert_eq!(fsm.bitmap_size_bytes(), 3);
         // All blocks should be free initially
          for i in 0..total_blocks {
              assert!(fsm.is_block_free(i)?); // Use ? to propagate error
          }

        // Allocate a few blocks
        let block1 = fsm.allocate_block()?; // Allocate block 0
        let block2 = fsm.allocate_block()?; // Allocate block 1
        let block3 = fsm.allocate_block()?; // Allocate block 2

        assert_eq!(block1, 0);
        assert_eq!(block2, 1);
        assert_eq!(block3, 2);


        // Check their status
         assert!(!fsm.is_block_free(block1)?);
         assert!(!fsm.is_block_free(block2)?);
         assert!(!fsm.is_block_free(block3)?);
         assert!(fsm.is_block_free(3)?); // Block 3 should be free

        // Deallocate block1
        fsm.deallocate_block(block1)?;
        assert!(fsm.is_block_free(block1)?); // Should be free now

        // Deallocate block3
         fsm.deallocate_block(block3)?;
         assert!(fsm.is_block_free(block3)?); // Should be free now

        // Allocate again, should reuse block1
        let block4 = fsm.allocate_block()?;
        assert_eq!(block4, block1); // Should be block 0 again
        assert!(!fsm.is_block_free(block4)?);

        // Allocate again, should reuse block3
         let block5 = fsm.allocate_block()?;
         assert_eq!(block5, block3); // Should be block 2 again
         assert!(!fsm.is_block_free(block5)?);


        Ok(()) // Return Ok from test function
    }

    #[test]
     fn test_free_space_manager_out_of_space() {
          let total_blocks = 8; // Exactly one byte in bitmap
          let block_size = 1024;
          let mut fsm = FreeSpaceManager::new(total_blocks, block_size); // Requires alloc

          // Allocate all blocks
           for _ in 0..total_blocks {
               fsm.allocate_block().expect("Should be able to allocate block");
           }

          // Attempt to allocate one more block, expect OutOfSpace error
          let result = fsm.allocate_block();

          assert!(result.is_err());
          match result.unwrap_err() {
              FileSystemError::Other(msg) => { // Mapped from OutOfSpace
                  assert!(msg.contains("Out of space"));
                  assert!(msg.contains("No free blocks available"));
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
          }
     }

     #[test]
      fn test_free_space_manager_invalid_index() {
           let total_blocks = 10;
           let block_size = 512;
           let mut fsm = FreeSpaceManager::new(total_blocks, block_size); // Requires alloc

           // Test is_block_free with out of bounds index
           let result_is_free = fsm.is_block_free(total_blocks + 5);
           assert!(result_is_free.is_err());
           match result_is_free.unwrap_err() {
               FileSystemError::InvalidParameter(msg) => { // Mapped from InvalidBlockIndex
                   assert!(msg.contains("Invalid block index"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_is_free.unwrap_err()),
           }

           // Test deallocate_block with out of bounds index
           let result_deallocate = fsm.deallocate_block(total_blocks + 5);
           assert!(result_deallocate.is_err());
           match result_deallocate.unwrap_err() {
               FileSystemError::InvalidParameter(msg) => { // Mapped from InvalidBlockIndex
                   assert!(msg.contains("Invalid block index"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_deallocate.unwrap_err()),
           }

            // Test deallocate_block on an already free block (should also be an error)
            let block_index_free = 0;
            assert!(fsm.is_block_free(block_index_free).unwrap()); // Ensure it's free first
            let result_deallocate_free = fsm.deallocate_block(block_index_free);
            assert!(result_deallocate_free.is_err());
             match result_deallocate_free.unwrap_err() {
                 FileSystemError::InvalidParameter(msg) => { // Mapped from InvalidBlockIndex (or custom BlockAlreadyFree if added)
                      assert!(msg.contains("Block index") && (msg.contains("is already free") || msg.contains("invalid")));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result_deallocate_free.unwrap_err()),
             }
      }


    #[test]
    fn test_device_manager() -> Result<(), FileSystemError> { // Return FileSystemError
        let mut dm = DeviceManager::new(); // Requires alloc and HashMap

        // Create FreeSpaceManager instances (requires alloc)
        let fsm_hdd = FreeSpaceManager::new(1024, 4096);
        let fsm_ssd = FreeSpaceManager::new(2048, 4096);

        dm.add_device("hdd".to_string(), fsm_hdd); // Requires alloc and String, HashMap insert
        dm.add_device("ssd".to_string(), fsm_ssd); // Requires alloc and String, HashMap insert

        // Allocate blocks using DeviceManager (uses Arc<Mutex> and calls FSM methods)
        let (device1, block1) = dm.allocate_block("hdd")?; // Calls FSM allocate_block
        let (device2, block2) = dm.allocate_block("ssd")?; // Calls FSM allocate_block

        assert_eq!(device1, "hdd".to_string()); // Requires String
        assert_eq!(device2, "ssd".to_string()); // Requires String

        // Check block status using DeviceManager (uses Arc<Mutex> and calls FSM methods)
        assert!(!dm.is_block_free("hdd", block1)?); // Calls FSM is_block_free
        assert!(!dm.is_block_free("ssd", block2)?); // Calls FSM is_block_free

        // Deallocate a block using DeviceManager (uses Arc<Mutex> and calls FSM methods)
        dm.deallocate_block("hdd", block1)?; // Calls FSM deallocate_block

        // Check status after deallocation
        assert!(dm.is_block_free("hdd", block1)?); // Calls FSM is_block_free

        // Test allocation after deallocation (should reuse the block)
        let (device3, block3) = dm.allocate_block("hdd")?;
        assert_eq!(device3, "hdd".to_string()); // Requires String
        assert_eq!(block3, block1); // Should allocate the block that was just freed


        // Test accessing non-existent device
         let result_allocate_nonexistent = dm.allocate_block("nonexistent_device");
         assert!(result_allocate_nonexistent.is_err());
          match result_allocate_nonexistent.unwrap_err() {
              FileSystemError::NotFound(msg) => {
                  assert!(msg.contains("Device 'nonexistent_device' not found.")); // Requires String
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result_allocate_nonexistent.unwrap_err()),
          }

         let result_deallocate_nonexistent = dm.deallocate_block("nonexistent_device", 0);
         assert!(result_deallocate_nonexistent.is_err());
         match result_deallocate_nonexistent.unwrap_err() {
              FileSystemError::NotFound(msg) => {
                  assert!(msg.contains("Device 'nonexistent_device' not found.")); // Requires String
              },
              _ => panic!("Beklenenden farklı hata türü: {:?}", result_deallocate_nonexistent.unwrap_err()),
         }

          let result_is_free_nonexistent = dm.is_block_free("nonexistent_device", 0);
          assert!(result_is_free_nonexistent.is_err());
          match result_is_free_nonexistent.unwrap_err() {
               FileSystemError::NotFound(msg) => {
                   assert!(msg.contains("Device 'nonexistent_device' not found.")); // Requires String
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_is_free_nonexistent.unwrap_err()),
          }


        Ok(()) // Return Ok from test function
    }

     // TODO: Add tests specifically for the no_std implementation.
     // This requires mocking the Sahne64 environment (fs, resource, SahneError).
     // It also requires using hashbrown::HashMap and spin::Mutex in the tests.
     // Test cases should cover FreeSpaceManager allocation/deallocation/checking,
     // DeviceManager operations, and error scenarios (out of space, invalid index, device not found).
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
