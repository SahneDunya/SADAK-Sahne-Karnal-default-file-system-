#![allow(dead_code)] // Allow unused code for a skeleton
#![cfg_attr(not(feature = "std")), no_std)] // Needs alloc and core features


// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;


// Core library imports
use core::result::Result; // Use core::result::Result
use core::fmt; // For Debug, Display
use core::mem; // For size_of, align_of


// Import alloc for Vec and String
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use alloc::boxed::Box; // For Box<dyn BlockDevice> if used


// Import the standard BlockDevice trait and its error types
use crate::blockdevice::{BlockDevice, BlockDeviceError}; // Assuming these are in crate::blockdevice
use crate::FileSystemError; // Assuming FileSystemError is in crate


// Import Superblock for accessing filesystem metadata
use crate::superblock::Superblock; // Assuming Superblock is in crate::superblock


// Import spin for synchronization (if InodeTable is shared)
#[cfg(feature = "spin")]
use spin::Mutex; // Or Spinlock


// Helper function to map BlockDeviceError to FileSystemError (copied from other files)
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
        BlockDeviceError::BlockSizeError(msg) => FileSystemError::InvalidData(format!("Block size mismatch or error: {}", msg)),
        #[cfg(feature = "blockdevice_trait")] // Map specific trait errors if they exist
        BlockDeviceError::NotSupported(msg) => FileSystemError::NotSupported(msg),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::TimedOut => FileSystemError::TimedOut(String::from("Block device operation timed out")),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::InvalidParameter(msg) => FileSystemError::InvalidParameter(msg),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::DeviceNotFound(msg) => FileSystemError::NotFound(msg),
         #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::PermissionDenied(msg) => FileSystemError::PermissionDenied(msg),
        #[cfg(feature = "blockdevice_trait")]
        BlockDeviceError::DeviceError(msg) => FileSystemError::DeviceError(msg),
    }
}


/// Represents a filesystem Inode (On-disk structure).
/// Contains metadata about a file or directory.
#[repr(C, packed)] // packed requires careful handling for alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // Add Eq
pub struct Inode {
    pub mode: u16,       // File mode (permissions, type - e.g., 0o755, S_IFREG, S_IFDIR)
    pub uid: u32,        // User ID
    pub gid: u32,        // Group ID
    pub links: u32,      // Number of hard links to this inode
    pub size: u64,       // File size (bytes)
    pub blocks: u64,     // Number of data blocks used by this inode (in filesystem block size)
    pub atime: u64,      // Last access time (e.g., Unix timestamp)
    pub mtime: u64,      // Last modification time (e.g., Unix timestamp)
    pub ctime: u64,      // Inode change time (e.g., Unix timestamp)
    // Data block pointers: Direct, Indirect, Double Indirect, etc.
    // Let's assume a simple scheme: N direct pointers.
    // Adjust size/types based on desired addressable space and filesystem structure.
    pub direct_blocks: [u64; 12], // Example: 12 direct data block pointers
     pub indirect_block: u64, // Example: Pointer to a block containing more block pointers
     pub double_indirect_block: u64, // Example: Pointer to a block containing indirect block pointers
    // ... other fields ...
}

impl Inode {
    /// Creates a new in-memory Inode instance with default values.
    pub fn new(mode: u16, uid: u32, gid: u32) -> Self { // Return Self
        Inode {
            mode,
            uid,
            gid,
            links: 1, // Typically 1 link when created (from directory entry)
            size: 0,
            blocks: 0,
            atime: 0, // Placeholder
            mtime: 0, // Placeholder
            ctime: 0, // Placeholder
            direct_blocks: [0; 12], // Initialize direct pointers to 0 (invalid block ID)
             indirect_block: 0, // Placeholder
             double_indirect_block: 0, // Placeholder
        }
    }

    /// Returns the size of the Inode structure in bytes.
    /// Note: Due to `packed`, this is the exact size without padding.
    pub fn size() -> usize { // Associated function
        mem::size_of::<Inode>()
    }

    /// Safely serializes an Inode struct into a byte buffer.
    /// Handles the `packed` structure byte-by-byte.
    ///
    /// # Arguments
    ///
    /// * `inode`: The Inode instance to serialize.
    /// * `buffer`: The destination buffer to write the bytes into. Must be at least `Inode::size()` bytes long.
    ///
    /// # Safety
    ///
    /// The caller must ensure the buffer is large enough.
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error if the buffer is too small.
    pub fn serialize_into_buffer(inode: &Inode, buffer: &mut [u8]) -> Result<(), FileSystemError> {
        let inode_size = Inode::size();
        if buffer.len() < inode_size {
            return Err(FileSystemError::InvalidParameter(format!(
                "Buffer size ({}) is too small for Inode serialization ({}).",
                buffer.len(),
                inode_size
            )));
        }

        // Safely copy field by field. This avoids unsafe transmute and is safer with `packed`.
        // Requires manual copying for each field.
        let mut offset = 0;

        buffer[offset..offset + mem::size_of_val(&inode.mode)].copy_from_slice(&inode.mode.to_le_bytes());
        offset += mem::size_of_val(&inode.mode);

        buffer[offset..offset + mem::size_of_val(&inode.uid)].copy_from_slice(&inode.uid.to_le_bytes());
        offset += mem::size_of_val(&inode.uid);

        buffer[offset..offset + mem::size_of_val(&inode.gid)].copy_from_slice(&inode.gid.to_le_bytes());
        offset += mem::size_of_val(&inode.gid);

        buffer[offset..offset + mem::size_of_val(&inode.links)].copy_from_slice(&inode.links.to_le_bytes());
        offset += mem::size_of_val(&inode.links);

        buffer[offset..offset + mem::size_of_val(&inode.size)].copy_from_slice(&inode.size.to_le_bytes());
        offset += mem::size_of_val(&inode.size);

        buffer[offset..offset + mem::size_of_val(&inode.blocks)].copy_from_slice(&inode.blocks.to_le_bytes());
        offset += mem::size_of_val(&inode.blocks);

         buffer[offset..offset + mem::size_of_val(&inode.atime)].copy_from_slice(&inode.atime.to_le_bytes());
        offset += mem::size_of_val(&inode.atime);

         buffer[offset..offset + mem::size_of_val(&inode.mtime)].copy_from_slice(&inode.mtime.to_le_bytes());
        offset += mem::size_of_val(&inode.mtime);

         buffer[offset..offset + mem::size_of_val(&inode.ctime)].copy_from_slice(&inode.ctime.to_le_bytes());
        offset += mem::size_of_val(&inode.ctime);

        // Copy direct block pointers
        for ptr in &inode.direct_blocks {
            buffer[offset..offset + mem::size_of_val(ptr)].copy_from_slice(&ptr.to_le_bytes());
            offset += mem::size_of_val(ptr);
        }

        // Handle indirect pointers if they exist...
         if let Some(indirect) = inode.indirect_block {
             buffer[offset..offset + mem::size_of_val(&indirect)].copy_from_slice(&indirect.to_le_bytes());
             offset += mem::size_of_val(&indirect);
         }


        Ok(()) // Serialization successful
    }

    /// Safely deserializes an Inode struct from a byte buffer.
    /// Handles the `packed` structure byte-by-byte.
    ///
    /// # Arguments
    ///
    /// * `buffer`: The source buffer containing the byte data. Must be at least `Inode::size()` bytes long.
    ///
    /// # Safety
    ///
    /// The caller must ensure the buffer is large enough and contains valid inode data.
    ///
    /// # Returns
    ///
    /// A Result containing the deserialized Inode instance, or an error if the buffer is too small or data is invalid.
    pub fn deserialize_from_buffer(buffer: &[u8]) -> Result<Inode, FileSystemError> {
        let inode_size = Inode::size();
        if buffer.len() < inode_size {
            return Err(FileSystemError::InvalidData(format!(
                "Buffer size ({}) is too small for Inode deserialization ({}).",
                buffer.len(),
                inode_size
            )));
        }

        // Safely read field by field. Handles `packed`.
        let mut offset = 0;

        let mode = u16::from_le_bytes(buffer[offset..offset + mem::size_of::<u16>()].try_into().unwrap());
        offset += mem::size_of::<u16>();

        let uid = u32::from_le_bytes(buffer[offset..offset + mem::size_of::<u32>()].try_into().unwrap());
        offset += mem::size_of::<u32>();

        let gid = u32::from_le_bytes(buffer[offset..offset + mem::size_of::<u32>()].try_into().unwrap());
        offset += mem::size_of::<u32>();

        let links = u32::from_le_bytes(buffer[offset..offset + mem::size_of::<u32>()].try_into().unwrap());
        offset += mem::size_of::<u32>();

        let size = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
        offset += mem::size_of::<u64>();

        let blocks = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
        offset += mem::size_of::<u64>();

        let atime = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
        offset += mem::size_of::<u64>();

        let mtime = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
        offset += mem::size_of::<u64>();

        let ctime = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
        offset += mem::size_of::<u64>();

        let mut direct_blocks = [0u64; 12];
        for ptr in direct_blocks.iter_mut() {
            *ptr = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
            offset += mem::size_of::<u64>();
        }

        // Handle indirect pointers if they exist...
         let indirect_block = u64::from_le_bytes(buffer[offset..offset + mem::size_of::<u64>()].try_into().unwrap());
         offset += mem::size_of::<u64>();


        Ok(Inode {
            mode, uid, gid, links, size, blocks, atime, mtime, ctime, direct_blocks,
            // indirect_block,
        })
    }

    // Update methods remain the same
     pub fn update_size(&mut self, size: u64) { ... }
     pub fn set_block_ptr(&mut self, index: usize, block_ptr: u64) { ... }

    // Add methods for setting/getting timestamps, links, etc.
    // Add helper methods for block pointer management (mapping file offset to block ID chain)
}


/// Manages the collection of Inodes, potentially in memory or backed by storage.
/// This implementation keeps all inodes in memory (simple for small filesystems).
/// For larger filesystems, on-demand loading from block device is needed.
pub struct InodeTable {
     // Consider protecting this Vec with a Mutex if multiple threads access it concurrently
    inodes: Vec<Inode>, // In-memory cache of inodes (Requires alloc)
    // The underlying block device and Superblock are needed to load/save.
    // These are typically managed externally and passed in when needed.
     device: Mutex<Box<dyn BlockDevice>>, // Example: Store device and protect with Mutex
    superblock: Superblock, // Example: Store a copy of the superblock info
}

impl InodeTable {
    /// Creates a new InodeTable instance with a vector of inodes.
    /// This might be used for creating a new filesystem.
    ///
    /// # Arguments
    ///
    /// * `inodes`: A vector of Inode instances to initialize the table with.
    ///
    /// # Returns
    ///
    /// A new InodeTable instance.
    pub fn new(inodes: Vec<Inode>) -> Self { // Takes Vec<Inode>
        InodeTable {
            // Wrap inodes Vec in Mutex if concurrent access is expected
            inodes, // Requires alloc
        }
    }


    /// Loads the Inode Table from the specified block device based on Superblock information.
    /// Reads blocks containing inodes and deserializes them into a Vec<Inode>.
    ///
    /// # Arguments
    ///
    /// * `device`: A mutable reference to the block device.
    /// * `superblock`: A reference to the loaded Superblock.
    ///
    /// # Returns
    ///
    /// A Result containing the loaded InodeTable, or a FileSystemError.
    pub fn load_from_device(
        device: &mut impl BlockDevice,
        superblock: &Superblock,
    ) -> Result<Self, FileSystemError> { // Returns Result<Self, FileSystemError>
        let inode_count = superblock.inodes_count;
        let inode_size = superblock.inode_size as usize; // Convert to usize
        let fs_block_size = superblock.block_size as usize; // Filesystem block size
        let inode_table_start_block = superblock.inode_table_start; // Start block of inode table

        if inode_size == 0 || fs_block_size == 0 {
            return Err(FileSystemError::InvalidData(String::from("Superblock contains zero inode size or block size.")));
        }

        // Calculate how many inodes fit in one filesystem block
        let inodes_per_block = fs_block_size / inode_size;
        if inodes_per_block == 0 {
             return Err(FileSystemError::InvalidData(format!(
                 "Filesystem block size ({}) is smaller than inode size ({}).",
                 fs_block_size, inode_size
             )));
        }

        // Calculate the total number of blocks occupied by the inode table
        let inode_table_blocks = (inode_count as usize + inodes_per_block - 1) / inodes_per_block;

        // Allocate a buffer to read one filesystem block at a time
        let mut block_buffer = alloc::vec![0u8; fs_block_size]; // Requires alloc

        // Allocate a vector to hold all loaded inodes
        let mut loaded_inodes: Vec<Inode> = Vec::with_capacity(inode_count as usize); // Requires alloc

        // Read each block of the inode table
        for block_idx_in_table in 0..inode_table_blocks {
            let device_block_id = inode_table_start_block + block_idx_in_table as u64;

            // Read the block from the device
            device.read_block(device_block_id, &mut block_buffer)
                  .map_err(|e| map_block_device_error_to_fs_error(e))?; // Map BlockDeviceError

            // Deserialize inodes from the block buffer
            for inode_idx_in_block in 0..inodes_per_block {
                let buffer_offset = inode_idx_in_block * inode_size;
                 // Ensure we don't read beyond the buffer for the last block's partial inodes
                if buffer_offset >= block_buffer.len() {
                     break; // No more partial inodes in this block
                }
                 let inode_bytes = &block_buffer[buffer_offset..buffer_offset + inode_size];

                // Deserialize the inode from the buffer slice
                let inode = Inode::deserialize_from_buffer(inode_bytes)?; // Use safe deserialize

                 // Add the deserialized inode to the list (Only if we haven't loaded all inodes yet)
                 if loaded_inodes.len() < inode_count as usize {
                    loaded_inodes.push(inode); // Requires alloc
                 } else {
                     // Should not happen if calculations are correct, but as a safeguard.
                     // Maybe return an error indicating more inodes found than expected.
                     break; // Stop if we've loaded the required count
                 }
            }
             // Stop if we've loaded the required number of inodes after processing a block
            if loaded_inodes.len() >= inode_count as usize {
                 break;
            }
        }

        // Verify that we loaded the expected number of inodes
        if loaded_inodes.len() != inode_count as usize {
             // This indicates an inconsistency between superblock.inodes_count and the actual data.
             return Err(FileSystemError::InvalidData(format!(
                 "Loaded inode count ({}) does not match superblock count ({}).",
                 loaded_inodes.len(),
                 inode_count
             ))); // Requires alloc
        }


        // Return the loaded InodeTable instance
        Ok(InodeTable::new(loaded_inodes)) // Use the new constructor
    }

    /// Saves the Inode Table to the specified block device based on Superblock information.
    /// Serializes inodes into blocks and writes them to storage.
    ///
    /// # Arguments
    ///
    /// * `device`: A mutable reference to the block device.
    /// * `superblock`: A reference to the loaded Superblock.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError.
    pub fn save_to_device(
        &self, // Takes &self or &mut self depending on internal needs. &self is sufficient here.
        device: &mut impl BlockDevice,
        superblock: &Superblock,
    ) -> Result<(), FileSystemError> { // Returns Result<(), FileSystemError>
        let inode_count = self.inodes.len() as u64; // Get count from in-memory table
        let inode_size = superblock.inode_size as usize;
        let fs_block_size = superblock.block_size as usize;
        let inode_table_start_block = superblock.inode_table_start;

        if inode_size == 0 || fs_block_size == 0 {
             return Err(FileSystemError::InvalidParameter(String::from("Superblock has zero inode size or block size.")));
        }
         // Ensure the number of inodes in memory matches the superblock count (optional check)
         if inode_count != superblock.inodes_count {
             // This indicates an inconsistency in memory state vs superblock.
             return Err(FileSystemError::InvalidData(format!(
                 "In-memory inode count ({}) does not match superblock count ({}). Cannot save.",
                 inode_count,
                 superblock.inodes_count
             ))); // Requires alloc
         }


        let inodes_per_block = fs_block_size / inode_size;
        if inodes_per_block == 0 {
            return Err(FileSystemError::InvalidParameter(format!(
                 "Filesystem block size ({}) is smaller than inode size ({}). Cannot save.",
                 fs_block_size, inode_size
             )));
        }

        let inode_table_blocks = (inode_count as usize + inodes_per_block - 1) / inodes_per_block;

        // Allocate a buffer for one filesystem block
        let mut block_buffer = alloc::vec![0u8; fs_block_size]; // Requires alloc

        let mut current_inode_index = 0;

        // Iterate through blocks needed for the inode table
        for block_idx_in_table in 0..inode_table_blocks {
            let device_block_id = inode_table_start_block + block_idx_in_table as u64;
             let mut buffer_offset = 0;

            // Serialize inodes into the block buffer
            for inode_idx_in_block in 0..inodes_per_block {
                 // Check if we have more inodes to serialize
                if current_inode_index < inode_count as usize {
                     let inode = &self.inodes[current_inode_index];

                    // Serialize the current inode into the buffer slice
                     let buffer_slice = &mut block_buffer[buffer_offset..buffer_offset + inode_size];
                    Inode::serialize_into_buffer(inode, buffer_slice)?; // Use safe serialize

                    buffer_offset += inode_size;
                    current_inode_index += 1;
                } else {
                    // No more inodes, fill the rest of the block with zeros (optional, good practice)
                      block_buffer[buffer_offset..].fill(0); // Requires Fill trait (core?)
                     for byte in &mut block_buffer[buffer_offset..] {
                          *byte = 0;
                     }
                    break; // Move to writing the block
                }
            }

            // Write the filled (or partially filled) block buffer to the device
            device.write_block(device_block_id, &block_buffer)
                  .map_err(|e| map_block_device_error_to_fs_error(e))?; // Map BlockDeviceError

             // Reset buffer for the next block (optional, write_block takes ownership of slice reference)
              block_buffer.fill(0); // Requires Fill trait (core?)
             for byte in &mut block_buffer[..] {
                  *byte = 0;
             }
        }


        // Verify that all inodes were processed (optional consistency check)
         if current_inode_index != inode_count as usize {
             // Should not happen if logic is correct
              return Err(FileSystemError::Other(String::from("Internal error: Mismatch in inodes processed during save.")));
         }


        Ok(()) // Save operation successful
    }


    /// Gets a reference to an inode by its index (inode number).
    /// Returns None if the index is out of bounds.
    pub fn get_inode(&self, index: usize) -> Option<&Inode> { // Takes &self
        self.inodes.get(index) // Accesses in-memory Vec
    }

    /// Gets a mutable reference to an inode by its index (inode number).
    /// Returns None if the index is out of bounds.
    pub fn get_inode_mut(&mut self, index: usize) -> Option<&mut Inode> { // Takes &mut self
        self.inodes.get_mut(index) // Accesses in-memory Vec
    }

    // TODO: Add methods for inode allocation and deallocation.
    // This involves finding a free inode in the in-memory table, marking it as used,
    // and updating free inode counts (in Superblock and potentially in-memory cache).
    // Allocation might need synchronization if multiple threads allocate concurrently.

    // TODO: Add methods for updating inodes (requires getting mutable reference and then saving).

    // TODO: Add methods for managing inode block pointers (mapping file offset to device block ID).
    // This is complex and involves handling direct, indirect, double indirect blocks.

    // TODO: Implement on-demand inode loading if the in-memory approach is not scalable.
    // This would involve reading specific inode blocks from the device when an inode is requested,
    // potentially caching recently used inodes.
}


#[cfg(test)]
#[cfg(feature = "std")] // Use std for easier testing with mocks
mod tests {
    use super::*;
    use alloc::vec::Vec; // For Vec
    use std::io::{Read, Write, Seek, Cursor}; // For MockBlockDevice
    use core::io::{ReadExt, WriteExt, SeekFrom}; // For MockBlockDevice
    use alloc::string::ToString; // For to_string()


    // Mock BlockDevice (copied from srcsuperblock.rs tests)
    struct MockBlockDevice {
        cursor: Cursor<Vec<u8>>, // In-memory buffer
        block_size: usize,
         simulate_io_error_on_read: bool,
         simulate_io_error_on_write: bool,
         simulate_block_size: usize, // Simulate a different block size than actual buffer if needed for tests
    }

    impl MockBlockDevice {
        fn new(total_blocks: u64, block_size: usize) -> Self {
             let total_size = total_blocks as usize * block_size;
             let initial_data = vec![0u8; total_size]; // Requires alloc
             MockBlockDevice {
                 cursor: Cursor::new(initial_data),
                 block_size,
                 simulate_io_error_on_read: false,
                 simulate_io_error_on_write: false,
                 simulate_block_size: block_size, // Default to actual size
             }
        }

        // Helper to manually get the underlying data (for inspection)
        fn get_data(&self) -> &[u8] {
            self.cursor.get_ref().as_slice()
        }

        // Helper to manually set the underlying data (for simulating pre-filled device)
         fn set_data(&mut self, data: Vec<u8>) {
              self.cursor = Cursor::new(data);
         }


        // Helper to simulate errors
        fn set_simulate_io_error_on_read(&mut self, value: bool) { self.simulate_io_error_on_read = value; }
        fn set_simulate_io_error_on_write(&mut self, value: bool) { self.simulate_io_error_on_write = value; }
        fn set_simulate_block_size(&mut self, size: usize) { self.simulate_block_size = size; } // For testing SB size mismatch


    }

    impl BlockDevice for MockBlockDevice {
        fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), BlockDeviceError> {
             if self.simulate_io_error_on_read {
                 return Err(BlockDeviceError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "Simulated read error"))); // Use std error in std test
             }
              if buf.len() != self.block_size {
                   return Err(BlockDeviceError::BlockSizeError(format!("Mock buffer size ({}) != block size ({})", buf.len(), self.block_size)));
              }


            let offset = block_id * self.block_size as u64;
            // Ensure offset is within bounds of mock device data
            if offset + self.block_size as u64 > self.cursor.get_ref().len() as u64 {
                 return Err(BlockDeviceError::InvalidParameter(format!("Attempted read beyond mock device bounds at block {}", block_id)));
            }


            self.cursor.seek(SeekFrom::Start(offset))
                .map_err(|e| BlockDeviceError::IoError(e))?;
            self.cursor.read_exact(buf)
                .map_err(|e| BlockDeviceError::IoError(e))?;
            Ok(())
        }

        fn write_block(&mut self, block_id: u64, buf: &[u8]) -> Result<(), BlockDeviceError> {
             if self.simulate_io_error_on_write {
                 return Err(BlockDeviceError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "Simulated write error"))); // Use std error in std test
             }
              if buf.len() != self.block_size {
                   return Err(BlockDeviceError::BlockSizeError(format!("Mock buffer size ({}) != block size ({})", buf.len(), self.block_size)));
              }

             let offset = block_id * self.block_size as u64;
            // Ensure offset is within bounds of mock device data
            if offset + self.block_size as u64 > self.cursor.get_ref().len() as u64 {
                 return Err(BlockDeviceError::InvalidParameter(format!("Attempted write beyond mock device bounds at block {}", block_id)));
            }


            self.cursor.seek(SeekFrom::Start(offset))
                .map_err(|e| BlockDeviceError::IoError(e))?;
            self.cursor.write_all(buf)
                .map_err(|e| BlockDeviceError::IoError(e))?;
            Ok(())
        }

        fn block_size(&self) -> usize {
            self.simulate_block_size // Return simulated block size for testing SB size mismatch
        }

         // Add block_count if BlockDevice trait includes it
         fn block_count(&self) -> u64 {
              (self.cursor.get_ref().len() / self.block_size) as u64
         }
         // Add size() if BlockDevice trait includes it
          fn size(&self) -> u64 {
               self.cursor.get_ref().len() as u64
          }
    }

    // Dummy SataConfig and DeviceType for Superblock creation in tests
    #[derive(Clone, Copy, Debug, PartialEq)] // Add Debug, PartialEq for tests
    pub enum DeviceType { Dummy }
    #[derive(Clone, Copy)] // Add Clone, Copy for tests
    pub struct SataConfig { pub device_id: u32, pub block_size: u32, pub block_count: u64, } // Keep dummy SataConfig


    // Dummy Superblock for testing InodeTable
    fn create_dummy_superblock(fs_block_size: u32, total_blocks: u64, total_inodes: u64) -> Superblock {
         let inode_size = Inode::size() as u32; // Use actual Inode size
         let block_bitmap_blocks = (total_blocks as usize + fs_block_size as usize * 8 - 1) / (fs_block_size as usize * 8);
         let inode_table_blocks = (total_inodes as usize * inode_size as usize + fs_block_size as usize - 1) / fs_block_size as usize;

         Superblock::new(
              fs_block_size,
              inode_size,
              total_blocks,
              total_inodes,
              DeviceType::Dummy,
              99999,
              1, // Assume Block Bitmap starts at block 1
              1 + block_bitmap_blocks as u64, // Assume Inode Table starts after Block Bitmap
              1 + block_bitmap_blocks as u64 + inode_table_blocks as u64, // Assume Data Blocks start after Inode Table
         )
    }


    #[test]
    fn test_inode_serialization_deserialization() -> Result<(), FileSystemError> {
        let original_inode = Inode {
            mode: 0o644,
            uid: 100,
            gid: 200,
             links: 2,
            size: 4096,
            blocks: 8, // Assuming 512 byte blocks for inode count
             atime: 1678886400, // Example timestamps
             mtime: 1678886500,
             ctime: 1678886600,
            direct_blocks: [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21], // Example block pointers
             // indirect_block: 30,
        };
        let inode_size = Inode::size();
        let mut buffer = vec![0u8; inode_size]; // Requires alloc

        // Serialize the inode into the buffer
        Inode::serialize_into_buffer(&original_inode, &mut buffer)?;

        // Ensure the buffer is filled to inode_size (important for fixed-size serialization)
        assert_eq!(buffer.len(), inode_size);

        // Deserialize the inode from the buffer
        let deserialized_inode = Inode::deserialize_from_buffer(&buffer)?;

        // Verify the deserialized inode matches the original
        assert_eq!(deserialized_inode, original_inode);


         // Test deserialization with too small buffer
          let small_buffer = vec![0u8; inode_size / 2];
          let result_deserialize_small = Inode::deserialize_from_buffer(&small_buffer);
           assert!(result_deserialize_small.is_err());
           match result_deserialize_small.unwrap_err() {
               FileSystemError::InvalidData(msg) => {
                   assert!(msg.contains("Buffer size") && msg.contains("is too small for Inode deserialization"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_deserialize_small.unwrap_err()),
           }

         // Test serialization with too small buffer
          let mut small_buffer_serialize = vec![0u8; inode_size / 2];
          let result_serialize_small = Inode::serialize_into_buffer(&original_inode, &mut small_buffer_serialize);
           assert!(result_serialize_small.is_err());
           match result_serialize_small.unwrap_err() {
               FileSystemError::InvalidParameter(msg) => { // serialize_into_buffer returns InvalidParameter
                   assert!(msg.contains("Buffer size") && msg.contains("is too small for Inode serialization"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result_serialize_small.unwrap_err()),
           }


        Ok(())
    }

    #[test]
    fn test_inodetable_persistence() -> Result<(), FileSystemError> {
        let fs_block_size: u32 = 512;
        let total_blocks: u64 = 1000;
        let total_inodes: u64 = 10; // Keep inode count small for this test

        // Create a dummy superblock to define layout
        let superblock = create_dummy_superblock(fs_block_size, total_blocks, total_inodes);
        let inode_size = Inode::size();
        let inodes_per_block = fs_block_size as usize / inode_size;
        let inode_table_blocks_needed = (total_inodes as usize + inodes_per_block - 1) / inodes_per_block;


        // Create some dummy inodes
        let mut original_inodes: Vec<Inode> = Vec::new(); // Requires alloc
        for i in 0..total_inodes {
             original_inodes.push(Inode::new(0o644, i as u32, 100));
             original_inodes.get_mut(i as usize).unwrap().update_size(i * 100);
             original_inodes.get_mut(i as usize).unwrap().set_block_ptr(0, i + 100); // Example block pointer
        }

        // Create an InodeTable in memory
        let original_inode_table = InodeTable::new(original_inodes.clone()); // Requires alloc


        // Create a mock block device large enough for the inode table and some data blocks
        // Need enough blocks for Superblock (1), Block Bitmap, Inode Table, and some data.
        // Let's use a fixed size that's definitely enough based on dummy SB calculations.
        let mock_device_size_blocks = superblock.data_blocks_start + 100; // Enough blocks
        let mut mock_device = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Requires alloc


        // Save the InodeTable to the mock device
        original_inode_table.save_to_device(&mut mock_device, &superblock)?;


        // Create a new mock device (or reset) to simulate loading
        let mut mock_device_load = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Requires alloc
        // Manually write the Superblock to the load device (needed by load_from_device)
        let mut sb_buffer = vec![0u8; fs_block_size as usize];
        Superblock::new(fs_block_size, Inode::size() as u32, total_blocks, total_inodes, DeviceType::Dummy, 99999, 1, 1 + (total_blocks as usize + fs_block_size as usize * 8 - 1) / (fs_block_size as usize * 8) as u64, 1 + (total_blocks as usize + fs_block_size as usize * 8 - 1) / (fs_block_size as usize * 8) as u64 + (total_inodes as usize * inode_size as usize + fs_block_size as usize - 1) / fs_block_size as usize as u64).save_to_device(&mut mock_device_load, &create_dummy_superblock(fs_block_size, total_blocks, total_inodes)).map_err(|e| map_block_device_error_to_fs_error(e))?; // Create and save a dummy SB to the load device


        // Manually copy the inode table blocks from the save device to the load device
        let inode_table_start_byte = superblock.inode_table_start * fs_block_size as u64;
        let inode_table_size_bytes = inode_table_blocks_needed * fs_block_size as u64;
        let original_device_data = mock_device.get_data(); // Get data from the save device
        let inode_table_data = &original_device_data[inode_table_start_byte as usize .. (inode_table_start_byte + inode_table_size_bytes) as usize];

        // Write this data to the same location on the load device
        let mut temp_device_for_write = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Temp device for writing
        temp_device_for_write.set_data(mock_device_load.get_data().to_vec()); // Copy current load device state
        let mut load_device_buffer = vec![0u8; inode_table_size_bytes as usize];
        load_device_buffer.copy_from_slice(inode_table_data); // Copy inode table data
        // This requires a write operation starting at inode_table_start_block
        // MockBlockDevice needs a write_at or a loop using write_block
        // Let's use write_block in a loop
         let mut current_byte_offset = 0;
         for block_idx_in_table in 0..inode_table_blocks_needed {
              let device_block_id = superblock.inode_table_start + block_idx_in_table;
              let buffer_slice = &load_device_buffer[current_byte_offset..current_byte_offset + fs_block_size as usize];
              mock_device_load.write_block(device_block_id, buffer_slice)?; // Write each block
              current_byte_offset += fs_block_size as usize;
         }



        // Load the InodeTable from the mock device
        let loaded_inode_table = InodeTable::load_from_device(&mut mock_device_load, &superblock)?;


        // Verify the loaded InodeTable matches the original
        assert_eq!(loaded_inode_table.inodes.len(), original_inode_table.inodes.len());
         assert_eq!(loaded_inode_table.inodes, original_inode_table.inodes);


         // Test loading from device with insufficient block size (should be caught by BlockDevice trait or earlier)
         // Test saving to device with insufficient block size (should be caught by BlockDevice trait or earlier)


         // Test loading from a device where inode_size in SB is > fs_block_size
          let mut sb_invalid_inode_size = create_dummy_superblock(fs_block_size, total_blocks, total_inodes);
          sb_invalid_inode_size.inode_size = (fs_block_size + 1) as u32; // Invalid inode size
          let mut mock_device_invalid_sb = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Requires alloc
          // Need to save the invalid SB to the device first
          let invalid_sb_buffer = {
              let mut buf = vec![0u8; fs_block_size as usize];
              Superblock::serialize_into_buffer(&sb_invalid_inode_size, &mut buf).unwrap();
              buf
          };
           mock_device_invalid_sb.write_block(0, &invalid_sb_buffer)?; // Write invalid SB
          let result_load_invalid_inode_size = InodeTable::load_from_device(&mut mock_device_invalid_sb, &sb_invalid_inode_size);
           assert!(result_load_invalid_inode_size.is_err());
            match result_load_invalid_inode_size.unwrap_err() {
                FileSystemError::InvalidData(msg) => {
                    assert!(msg.contains("Filesystem block size") && msg.contains("is smaller than inode size"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result_load_invalid_inode_size.unwrap_err()),
            }


         // Test saving to a device where inode_size in SB is > fs_block_size
          let mut sb_invalid_inode_size_save = create_dummy_superblock(fs_block_size, total_blocks, total_inodes);
          sb_invalid_inode_size_save.inode_size = (fs_block_size + 1) as u32; // Invalid inode size
          let mut mock_device_invalid_sb_save = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Requires alloc
          let result_save_invalid_inode_size = original_inode_table.save_to_device(&mut mock_device_invalid_sb_save, &sb_invalid_inode_size_save);
           assert!(result_save_invalid_inode_size.is_err());
            match result_save_invalid_inode_size.unwrap_err() {
                FileSystemError::InvalidParameter(msg) => { // save_to_device maps this to InvalidParameter
                    assert!(msg.contains("Filesystem block size") && msg.contains("is smaller than inode size. Cannot save."));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result_save_invalid_inode_size.unwrap_err()),
            }


         // Test loading from device with simulated IO error during read
          let mut mock_device_io_error = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Requires alloc
          // Need to write valid inode table data first so the error happens during load read
           original_inode_table.save_to_device(&mut mock_device_io_error, &superblock)?; // Save valid data
          mock_device_io_error.set_simulate_io_error_on_read(true); // Enable simulated error
          let result_load_io_error = InodeTable::load_from_device(&mut mock_device_io_error, &superblock);
           assert!(result_load_io_error.is_err());
            match result_load_io_error.unwrap_err() {
                FileSystemError::IOError(_) => { /* Expected IO error */ },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result_load_io_error.unwrap_err()),
            }


         // Test saving to device with simulated IO error during write
          let mut mock_device_io_error_save = MockBlockDevice::new(mock_device_size_blocks, fs_block_size as usize); // Requires alloc
           mock_device_io_error_save.set_simulate_io_error_on_write(true); // Enable simulated error
          let result_save_io_error = original_inode_table.save_to_device(&mut mock_device_io_error_save, &superblock);
           assert!(result_save_io_error.is_err());
            match result_save_io_error.unwrap_err() {
                FileSystemError::IOError(_) => { /* Expected IO error */ },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result_save_io_error.unwrap_err()),
            }


    }


    // TODO: Add tests for InodeTable::get_inode and get_inode_mut with bounds check.
    // TODO: Add tests for Inode allocation and deallocation (when implemented).
    // TODO: Add tests for Inode block pointer management (when implemented).
    // TODO: Consider tests for concurrency if Mutex is added around InodeTable.
}

// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
