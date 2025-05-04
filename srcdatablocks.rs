#![allow(dead_code)]

use crate::{
    blockdevice::{BlockDevice, SeekFrom}, // Import the BlockDevice trait and SeekFrom enum
    config, // Import file system configuration
    SahneError, // Import SahneError
    // Add other necessary Sahne64 modules if needed, e.g., resource, sync
};

// Data block management structure
pub struct DataBlocksManager<'a> {
    // Reference to the underlying block device.
    // 'a lifetime ensures the manager doesn't outlive the device.
    device: &'a mut dyn BlockDevice,
    // We might need a reference to the Free Space Manager here later
    // free_space_manager: &'a mut dyn FreeSpaceManager,
}

impl<'a> DataBlocksManager<'a> {
    /// Creates a new DataBlocksManager.
    ///
    /// # Arguments
    ///
    /// * `device` - A mutable reference to the underlying BlockDevice.
    pub fn new(device: &'a mut dyn BlockDevice) -> Self {
        DataBlocksManager {
            device,
            // free_space_manager: ...,
        }
    }

    /// Reads a specific logical data block.
    ///
    /// # Arguments
    ///
    /// * `logical_block_number` - The number of the data block relative to DATA_BLOCKS_LOCATION.
    /// * `buffer` - The buffer to store the block data. Must be config::BLOCK_SIZE.
    ///
    /// # Returns
    ///
    /// Returns `Ok(bytes_read)` (expected to be config::BLOCK_SIZE) on success,
    /// or a `SahneError` on failure.
    pub fn read_data_block(&mut self, logical_block_number: u64, buffer: &mut [u8]) -> Result<usize, SahneError> {
        // Ensure the buffer size matches the configured block size
        if buffer.len() as u32 != config::BLOCK_SIZE {
            println!("ERROR: read_data_block buffer size mismatch. Expected: {}, Got: {}", config::BLOCK_SIZE, buffer.len());
            return Err(SahneError::InvalidParameter); // Or a more specific error
        }

        // Calculate the physical offset on the underlying device
        let physical_block_number = config::DATA_BLOCKS_LOCATION + logical_block_number;
        let offset = physical_block_number * config::BLOCK_SIZE as u64;

        // Read from the block device at the calculated offset.
        // NOTE: This relies on the underlying BlockDevice implementation (e.g., ResourceBlockDevice)
        // correctly handling the 'offset' parameter. As noted in ResourceBlockDevice,
        // the current Sahne64 resource::read/write API *ignores* the offset.
        // This call will likely read from the beginning of the resource unless seek works.
        self.device.read(offset, buffer)
         // The BlockDevice::read should return the number of bytes read.
         // For a block device layer, we typically expect to read the full block size.
         // Additional checks might be needed here to ensure the full block was read.
    }

    /// Writes data to a specific logical data block.
    ///
    /// # Arguments
    ///
    /// * `logical_block_number` - The number of the data block relative to DATA_BLOCKS_LOCATION.
    /// * `buffer` - The buffer containing the block data. Must be config::BLOCK_SIZE.
    ///
    /// # Returns
    ///
    /// Returns `Ok(bytes_written)` (expected to be config::BLOCK_SIZE) on success,
    /// or a `SahneError` on failure.
    pub fn write_data_block(&mut self, logical_block_number: u64, buffer: &[u8]) -> Result<usize, SahneError> {
        // Ensure the buffer size matches the configured block size
        if buffer.len() as u32 != config::BLOCK_SIZE {
            println!("ERROR: write_data_block buffer size mismatch. Expected: {}, Got: {}", config::BLOCK_SIZE, buffer.len());
            return Err(SahneError::InvalidParameter); // Or a more specific error
        }

        // Calculate the physical offset on the underlying device
        let physical_block_number = config::DATA_BLOCKS_LOCATION + logical_block_number;
        let offset = physical_block_number * config::BLOCK_SIZE as u64;

        // Write to the block device at the calculated offset.
        // NOTE: This relies on the underlying BlockDevice implementation (e.g., ResourceBlockDevice)
        // correctly handling the 'offset' parameter. As noted in ResourceBlockDevice,
        // the current Sahne64 resource::write/write API *ignores* the offset.
        // This call will likely write to the beginning of the resource unless seek works.
        self.device.write(offset, buffer)
        // The BlockDevice::write should return the number of bytes written.
        // For a block device layer, we typically expect to write the full block size.
        // Additional checks might be needed here to ensure the full block was written.
    }

    // Future methods would include block allocation and deallocation.
     pub fn allocate_block(...) -> Result<u64, SahneError> { ... }
     pub fn free_block(...) -> Result<(), SahneError> { ... }
}

// Example usage (requires a BlockDevice instance)
#[cfg(feature = "example_datablocks")] // Use a specific feature flag for this example
fn main() -> Result<(), SahneError> {
    // This example is illustrative and needs a concrete BlockDevice instance.
    // In a real scenario, you'd pass a ResourceBlockDevice (or MemBlockDevice if enabled).

    // Dummy BlockDevice implementation for example purposes only
    struct DummyBlockDevice {
        block_size: u64,
        // Simulate some data storage that respects offset
        data: alloc::vec::Vec<u8>, // Requires 'alloc'
    }

    #[cfg(any(feature = "std", feature = "alloc"))] // Dummy requires alloc
    impl BlockDevice for DummyBlockDevice {
         fn block_size(&self) -> u64 { self.block_size }
         fn size(&self) -> Result<u64, SahneError> { Ok(self.data.len() as u64) }
         fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError> {
             // Dummy seek - real seek updates internal state
             println!("DummyBlockDevice::seek called with {:?}", pos);
             Ok(0) // Just return 0 for simplicity in dummy
         }

         // Dummy read that actually respects offset
         fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, SahneError> {
             let offset_usize = offset as usize;
             let len = buf.len();
             println!("DummyBlockDevice::read called with offset {} and buffer len {}", offset, len);

             if offset_usize >= self.data.len() {
                 return Ok(0);
             }

             let read_len = min(len, self.data.len() - offset_usize);
             buf[..read_len].copy_from_slice(&self.data[offset_usize..offset_usize + read_len]);
             Ok(read_len)
         }

         // Dummy write that actually respects offset
         fn write(&mut self, offset: u64, buf: &[u8]) -> Result<usize, SahneError> {
             let offset_usize = offset as usize;
             let len = buf.len();
              println!("DummyBlockDevice::write called with offset {} and buffer len {}", offset, len);

             if offset_usize >= self.data.len() {
                 return Ok(0);
             }

             let write_len = min(len, self.data.len() - offset_usize);
             self.data[offset_usize..offset_usize + write_len].copy_from_slice(&buf[..write_len]);
             Ok(write_len)
         }
    }

    #[cfg(any(feature = "std", feature = "alloc"))] // Dummy requires alloc
    {
        // Create a dummy device that's 20 blocks large, with 512 byte blocks
        let total_device_size = (config::DATA_BLOCKS_LOCATION + 20) * config::BLOCK_SIZE as u64;
        let mut dummy_device = DummyBlockDevice {
            block_size: config::BLOCK_SIZE as u64,
            data: alloc::vec![0u8; total_device_size as usize],
        };

        // Create the DataBlocksManager with the dummy device
        let mut data_manager = DataBlocksManager::new(&mut dummy_device);

        let mut read_buffer = alloc::vec![0u8; config::BLOCK_SIZE as usize];
        let write_buffer_1 = alloc::vec![1u8; config::BLOCK_SIZE as usize];
        let write_buffer_2 = alloc::vec![2u8; config::BLOCK_SIZE as usize];

        // Write to logical data block 0 (physical block DATA_BLOCKS_LOCATION)
        println!("Writing to logical data block 0...");
        match data_manager.write_data_block(0, &write_buffer_1) {
            Ok(bytes_written) => println!("Wrote {} bytes to logical block 0.", bytes_written),
            Err(e) => eprintln!("Error writing to logical block 0: {:?}", e),
        }

        // Write to logical data block 5 (physical block DATA_BLOCKS_LOCATION + 5)
        println!("Writing to logical data block 5...");
        match data_manager.write_data_block(5, &write_buffer_2) {
            Ok(bytes_written) => println!("Wrote {} bytes to logical block 5.", bytes_written),
            Err(e) => eprintln!("Error writing to logical block 5: {:?}", e),
        }

        // Read from logical data block 0
        println!("Reading from logical data block 0...");
        match data_manager.read_data_block(0, &mut read_buffer) {
            Ok(bytes_read) => {
                println!("Read {} bytes from logical block 0. First 10 bytes: {:?}", bytes_read, &read_buffer[..min(10, bytes_read)]);
                 // In a real test, you'd assert read_buffer == write_buffer_1
            },
            Err(e) => eprintln!("Error reading from logical block 0: {:?}", e),
        }

        // Read from logical data block 5
        println!("Reading from logical data block 5...");
        match data_manager.read_data_block(5, &mut read_buffer) {
            Ok(bytes_read) => {
                println!("Read {} bytes from logical block 5. First 10 bytes: {:?}", bytes_read, &read_buffer[..min(10, bytes_read)]);
                 // In a real test, you'd assert read_buffer == write_buffer_2
            },
            Err(e) => eprintln!("Error reading from logical block 5: {:?}", e),
        }

        // Attempt to read from an unwritten block (e.g., logical block 1)
        println!("Reading from logical data block 1 (unwritten)...");
         match data_manager.read_data_block(1, &mut read_buffer) {
            Ok(bytes_read) => {
                 println!("Read {} bytes from logical block 1. First 10 bytes: {:?}", bytes_read, &read_buffer[..min(10, bytes_read)]);
                 // Expected to be all zeros if device was initialized to zero
            },
             Err(e) => eprintln!("Error reading from logical block 1: {:?}", e),
         }

    }

    Ok(())
}
