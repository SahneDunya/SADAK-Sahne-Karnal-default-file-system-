#![allow(dead_code)] // Allow unused code for a skeleton
#![allow(unused_imports)] // Allow unused imports for a skeleton
#![cfg_attr(not(feature = "std")), no_std)] // This is primarily a no_std driver

// Re-export necessary Sahne64 modules and types with correct paths
#[cfg(not(feature = "std"))]
use crate::{
    error::SahneError, // Assuming SahneError is in crate::error
    fs, // Assuming fs module is in crate
    resource, // Assuming resource module is in crate
    FileSystemError, // Assuming FileSystemError is in crate
    Handle, // Assuming Handle is in crate
    // Removed redundant imports like memory, process, sync, kernel, arch
};

#[cfg(not(feature = "std"))]
use crate::blockdevice::BlockDevice; // Use the standard BlockDevice trait

// Assuming BlockDeviceError is defined in the blockdevice module
#[cfg(not(feature = "blockdevice_trait"))]
#[derive(Debug)] // Define placeholder error if the real one isn't available
pub enum BlockDeviceError {
    IoError(String), // Simplified IO Error string for placeholder
    BlockSizeError(String),
    // Add other specific block device errors as needed
    NotSupported(String),
    TimedOut,
    // Add NVMe-specific errors mapped here if the real BlockDeviceError includes them
     NvmeQueueFull,
     NvmeCompletionError(u16),
}
// Add placeholder Display and potentially Error impls if needed for the placeholder

#[cfg(feature = "blockdevice_trait")]
use crate::blockdevice::BlockDeviceError; // Use the real BlockDeviceError


// core library imports
use core::{
    fmt, // For Debug, Display
    ptr, // For volatile reads/writes
    result::Result, // Use core::result::Result
    sync::atomic::{AtomicU16, Ordering}, // For atomic operations
};

// alloc crate imports (needed for String, Vec if used, and error formatting)
#[cfg_attr(not(feature = "std")), macro_use]
extern crate alloc;
use alloc::string::{String, ToString};
use alloc::format;


// Standard library imports (only for std build)
#[cfg(feature = "std")]
use std::{
    io::{self, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Write as StdWrite, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt, WriteExt as StdWriteExt},
    path::Path,
    error::Error as StdError,
    fmt as std_fmt, // Use alias to avoid conflict with core::fmt
    sync::Mutex as StdMutex, // Use std Mutex if in std
};


// Define NVMe device addresses and configuration parameters (Specific to the hardware and kernel mapping)
// These should be treated as kernel-provided values or obtained via PCI probing/configuration.
#[cfg(not(feature = "std"))] // These addresses are specific to no_std hardware interaction
const NVME_BASE_ADDRESS: usize = 0xFEE00000; // Example MMIO base address
#[cfg(not(feature = "std"))]
const NVME_QUEUE_SIZE: usize = 64; // Example Submission/Completion Queue size (number of entries)
#[cfg(not(feature = "std"))]
const NVME_BLOCK_SIZE: usize = 512; // Example NVMe block size (Logical Block Size)

// Standard NVMe Opcodes
#[cfg(not(feature = "std"))]
const NVME_OPCODE_READ: u8 = 0x02;
#[cfg(not(feature = "std"))]
const NVME_OPCODE_WRITE: u8 = 0x01;


// Custom error type for low-level NVMe driver operations.
// This is distinct from BlockDeviceError, but will be mapped to it.
#[derive(Debug, Clone, Copy)] // Add Clone, Copy for easier handling
pub enum NvmeError {
    QueueFull, // Submission queue is full
    CompletionError(u16), // Error reported in the completion queue status field
    Timeout, // Command timed out waiting for completion
    InvalidParameter, // Invalid parameter provided to driver function
    // Add other NVMe specific errors as needed
}

// Implement Display for NvmeError
impl fmt::Display for NvmeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NvmeError::QueueFull => write!(f, "NVMe Submission Queue Full"),
            NvmeError::CompletionError(status) => write!(f, "NVMe Completion Error (Status: {})", status),
            NvmeError::Timeout => write!(f, "NVMe Command Timeout"),
            NvmeError::InvalidParameter => write!(f, "NVMe Invalid Parameter"),
        }
    }
}


/// Helper function to map NvmeError to BlockDeviceError.
fn map_nvme_error_to_block_device_error(e: NvmeError) -> BlockDeviceError {
    #[cfg(feature = "blockdevice_trait")] // If the real BlockDeviceError has specific variants
    {
        match e {
            NvmeError::QueueFull => BlockDeviceError::NotSupported(String::from("NVMe Queue Full")), // Map QueueFull
            NvmeError::CompletionError(status) => BlockDeviceError::IoError(format!("NVMe Completion Error Status: {}", status)), // Map CompletionError
            NvmeError::Timeout => BlockDeviceError::TimedOut, // Map Timeout
            NvmeError::InvalidParameter => BlockDeviceError::BlockSizeError(String::from("NVMe Invalid Parameter")), // Map InvalidParameter (e.g., buffer size)
            // Map other NvmeError variants if added
        }
    }
    #[cfg(not(feature = "blockdevice_trait"))] // If using the placeholder BlockDeviceError
    {
        match e {
            NvmeError::QueueFull => BlockDeviceError::NotSupported(String::from("NVMe Queue Full")),
            NvmeError::CompletionError(status) => BlockDeviceError::IoError(format!("NVMe Completion Error Status: {}", status)),
            NvmeError::Timeout => BlockDeviceError::TimedOut,
             NvmeError::InvalidParameter => BlockDeviceError::BlockSizeError(String::from("NVMe Invalid Parameter")),
        }
    }
}


// NVMe command and completion queue structures (Low-level hardware interface)
// These are specific to the NVMe specification and physical memory layout.
#[cfg(not(feature = "std"))] // These structures are for no_std hardware interaction
#[repr(C, align(64))] // Alignment is crucial for hardware access
struct NvmeQueue<T> {
    entries: [T; NVME_QUEUE_SIZE], // Array of queue entries
    head: AtomicU16, // Controller's head pointer (used by driver for completion queue) / Driver's head pointer (used by controller for submission queue)
    tail: AtomicU16, // Driver's tail pointer (used by driver for submission queue) / Controller's tail pointer (used by controller for completion queue)
}

#[cfg(not(feature = "std"))]
impl<T> NvmeQueue<T> {
    /// Creates a new NvmeQueue instance (in memory, not yet mapped to hardware).
    /// Requires Zeroable trait or manual initialization for T.
    fn new() -> Self where T: Sized {
        // Manual initialization is safer than zeroed(), especially if T has non-zero invariants.
        // For hardware structs, zeroing is often the expected initial state.
        // Using a loop for clarity and safety if T is not truly Zeroable.
        let mut entries: [core::mem::MaybeUninit<T>; NVME_QUEUE_SIZE] = unsafe { core::mem::MaybeUninit::uninit().assume_init() };
         for entry in &mut entries[..] {
             unsafe { ptr::write(entry.as_mut_ptr(), core::mem::zeroed()); } // Initialize each entry to zero
         }
         let entries = unsafe { core::mem::transmute::<_, [T; NVME_QUEUE_SIZE]>(entries) }; // Transmute back

        NvmeQueue {
            entries,
            head: AtomicU16::new(0),
            tail: AtomicU16::new(0),
        }
    }
    // NOTE: This 'new' method is likely *not* how queues are allocated and initialized
    // in a real OS. Queues are usually allocated from specific kernel memory regions
    // and their physical addresses are provided to the NVMe controller.
}

#[cfg(not(feature = "std"))] // These structures are for no_std hardware interaction
#[repr(C)] // No strict alignment needed unless specified by spec for this struct alone
struct NvmeCommand {
    opcode: u8, // Command opcode (e.g., Read, Write)
    flags: u8, // Command flags
    cid: u16, // Command Identifier (matches completion CID)
    nsid: u32, // Namespace Identifier
    // DWORDs 2-9 (command-specific) - Example fields for Read/Write
    cdw2: u32, // Start LBA Low (for Read/Write)
    cdw3: u32, // Start LBA High (for Read/Write)
    cdw4: u32, // Number of Logical Blocks (0-based, so count-1)
    cdw5: u32, // Reserved or other command-specific fields
    metadata_ptr: u64, // Metadata Pointer (if metadata is used)
    data_ptr: u64, // Data Pointer (PRP or SGL) - Physical Address for DMA
    // DWORDs 10-15 (command-specific) - Example fields
    cdw10: u32, // Number of Logical Blocks (LSB) - overlaps with cdw4 in some command formats
    cdw11: u32, // Number of Logical Blocks (MSB) - overlaps with cdw4 in some command formats
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
}

#[cfg(not(feature = "std"))] // These structures are for no_std hardware interaction
#[repr(C)] // No strict alignment needed unless specified by spec for this struct alone
struct NvmeCompletion {
    cdw0: u32, // Command Specific DWORD 0
    cdw1: u32, // Reserved or other fields
    sq_head: u16, // Submission Queue Head pointer (Controller's view)
    sq_id: u16,   // Submission Queue Identifier
    cid: u16,     // Command Identifier (matches command CID)
    status: u16,  // Command Status (Success, Error codes)
}

// NVMe driver structure.
// This represents the state of the driver interacting with the NVMe controller.
#[cfg(not(feature = "std"))] // This is a no_std driver structure
pub struct NvmeDriver {
    // References to the command and completion queues in device memory.
    // These must be obtained through kernel memory mapping services.
    // &'static mut is unsafe if the memory is not truly static or exclusively owned.
    // Consider using raw pointers or a safe wrapper provided by the kernel.
    // For this skeleton, we keep &'static mut but emphasize unsafety.
    command_queue: &'static mut NvmeQueue<NvmeCommand>,
    completion_queue: &'static mut NvmeQueue<NvmeCompletion>,
    command_id_counter: AtomicU16, // Counter for generating command IDs
    namespace_id: u32, // The NVMe Namespace ID this driver instance manages (often 1)
    block_size: usize, // Logical Block Size of the namespace
    // Add pointer to the NVMe controller's Register structure for doorbells and status
    // controller_registers: &'static mut NvmeRegisters, // Example: MMIO registers struct
    // Add other necessary fields: e.g., interrupt handler registration, DMA buffer management state.
}

#[cfg(not(feature = "std"))]
impl NvmeDriver {
    /// Creates a new NVMe driver instance.
    ///
    /// This function should be called by the kernel during device initialization.
    /// It requires memory-mapped access to the NVMe controller registers and queues.
    ///
    /// # Safety
    ///
    /// This function is inherently unsafe as it deals with raw memory addresses and hardware.
    /// The caller must ensure that the `NVME_BASE_ADDRESS` and subsequent addresses are
    /// correctly mapped and represent the NVMe controller and its queues, and that
    /// there is exclusive mutable access to these memory regions.
    ///
    /// # Returns
    ///
    /// A Result containing the initialized NvmeDriver instance.
    pub fn new(namespace_id: u32, block_size: usize) -> Result<Self, BlockDeviceError> { // Return Result<Self, BlockDeviceError>
        // --- Kernel Interaction Placeholder ---
        // In a real Sahne64 kernel, obtaining access to the NVMe controller
        // and its queues involves:
        // 1. PCI enumeration to find the NVMe device.
        // 2. Reading PCI configuration space to get the BAR (Base Address Register)
        //    for the controller's MMIO registers.
        // 3. Mapping the MMIO region into the kernel's virtual address space.
        // 4. Allocating physically contiguous memory for the command and completion queues.
        // 5. Providing the physical addresses of these queues to the NVMe controller
        //    via its Admin Queue commands (e.g., Create I/O Submission/Completion Queue).
        // 6. Mapping the allocated queue memory into the kernel's virtual address space.
        //
        // The code below *simulates* direct access to mapped memory at a fixed address.
        // This is UNSAFE and must be replaced with actual kernel memory management APIs.
        // --------------------------------------

        let command_queue_ptr = NVME_BASE_ADDRESS as *mut NvmeQueue<NvmeCommand>;
        let completion_queue_ptr = (NVME_BASE_ADDRESS + 4096) as *mut NvmeQueue<NvmeCompletion>; // Example offset

        // Obtain mutable references to the queues. This assumes these addresses
        // are correctly mapped and we have exclusive mutable access.
        let command_queue = unsafe {
             command_queue_ptr.as_mut().ok_or(BlockDeviceError::IoError(String::from("Failed to get mutable reference to command queue")))? // Use Option::ok_or for mapping ptr to Result
        };
        let completion_queue = unsafe {
             completion_queue_ptr.as_mut().ok_or(BlockDeviceError::IoError(String::from("Failed to get mutable reference to completion queue")))? // Use Option::ok_or
        };


        // Initialize queue head and tail pointers (atomically)
        command_queue.head.store(0, Ordering::Relaxed);
        command_queue.tail.store(0, Ordering::Relaxed);
        completion_queue.head.store(0, Ordering::Relaxed);
        completion_queue.tail.store(0, Ordering::Relaxed);

        // --- Kernel Interaction Placeholder ---
        // Perform NVMe controller initialization steps here:
        // - Set up Admin Queue (if not already done by boot firmware)
        // - Create I/O Submission and Completion Queues using Admin Commands
        // - Enable the controller
        // - Configure namespace(s)
        // - Register interrupt handlers (if using interrupts instead of polling)
        // --------------------------------------

        Ok(NvmeDriver {
            command_queue,
            completion_queue,
            command_id_counter: AtomicU16::new(0),
            namespace_id,
            block_size,
            // Initialize other fields...
        })
    }

    /// Generates a new, unique command identifier.
    fn get_command_id(&self) -> u16 {
        // Fetch-and-add operation for atomic counter.
        self.command_id_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Submits a command to the NVMe submission queue.
    ///
    /// # Arguments
    ///
    /// * `command`: The NVMe command structure to submit.
    ///
    /// # Returns
    ///
    /// A Result containing the command ID if successful, or NvmeError::QueueFull.
    fn submit_command(&mut self, command: NvmeCommand) -> Result<u16, NvmeError> { // Return NvmeError
        let tail = self.command_queue.tail.load(Ordering::Relaxed); // Load tail atomically
        let next_tail = (tail + 1) % NVME_QUEUE_SIZE as u16;

        // Check if the queue is full (simplified: next_tail equals head)
        if next_tail == self.command_queue.head.load(Ordering::Relaxed) {
            return Err(NvmeError::QueueFull);
        }

        // Place the command in the next available entry in the submission queue
        // Using volatile write as this is hardware memory.
        unsafe {
            ptr::write_volatile(&mut self.command_queue.entries[tail as usize], command);
        }

        // Update the submission queue tail pointer (atomically)
        self.command_queue.tail.store(next_tail, Ordering::Release); // Use Release ordering before doorbell write

        // --- Kernel Interaction Placeholder ---
        // Notify the NVMe controller by writing to the submission queue doorbell register.
        // This requires mapping the controller registers and using kernel I/O functions.
        // Example: arch::io::write_u32(submission_queue_0_doorbell_address, next_tail as u32);
        // --------------------------------------
         #[cfg(not(feature = "std"))] // Doorbell write is specific to no_std hardware
         unsafe {
             // This address is an example, replace with actual MMIO address + doorbell offset
             // Accessing raw MMIO address directly is UNSAFE and needs kernel I/O functions.
             ptr::write_volatile((NVME_BASE_ADDRESS + 0x1000) as *mut u32, next_tail as u32); // Example SQ 0 doorbell
         }


        Ok(command.cid) // Return the command ID
    }

    /// Polls the completion queue for a specific command ID.
    /// This is a busy-wait polling implementation.
    /// A real driver would typically use interrupts for completion notification.
    ///
    /// # Arguments
    ///
    /// * `expected_cid`: The command ID to wait for completion.
    ///
    /// # Returns
    ///
    /// A Result containing the NvmeCompletion on success, or NvmeError on completion error or timeout.
    fn poll_completion(&mut self, expected_cid: u16) -> Result<NvmeCompletion, NvmeError> { // Return NvmeError
        // Simple timeout loop (replace with proper kernel timer/wait mechanism)
        for _ in 0..100_000 { // Example polling limit
            let head = self.completion_queue.head.load(Ordering::Acquire); // Load head atomically (Acquire before reading completion)
            let tail = self.completion_queue.tail.load(Ordering::Relaxed); // Load tail atomically

            // Check if there are new completions to process
            if head != tail {
                 // Read the completion entry from the head of the completion queue
                let completion = unsafe {
                     // Using volatile read as this is hardware memory.
                    ptr::read_volatile(&self.completion_queue.entries[head as usize])
                };

                // Check if this is the completion we are waiting for
                if completion.cid == expected_cid {
                    // Check the status field in the completion entry
                    if completion.status != 0 {
                        // Command completed with an error
                        return Err(NvmeError::CompletionError(completion.status));
                    }

                    // Command completed successfully.
                    // Update the completion queue head pointer (atomically).
                    let next_head = (head + 1) % NVME_QUEUE_SIZE as u16;
                    self.completion_queue.head.store(next_head, Ordering::Release); // Use Release ordering before doorbell write

                    // --- Kernel Interaction Placeholder ---
                    // Notify the NVMe controller that we have processed this completion
                    // by writing to the completion queue doorbell register.
                    // Example: arch::io::write_u32(completion_queue_0_doorbell_address, next_head as u32);
                    // --------------------------------------
                     #[cfg(not(feature = "std"))] // Doorbell write is specific to no_std hardware
                     unsafe {
                         // This address is an example, replace with actual MMIO address + doorbell offset
                         // Accessing raw MMIO address directly is UNSAFE and needs kernel I/O functions.
                         ptr::write_volatile((NVME_BASE_ADDRESS + 0x1004) as *mut u32, next_head as u32); // Example CQ 0 doorbell
                     }


                    return Ok(completion); // Return the completion entry
                }
                 // If it's not the expected CID, we might need to process it anyway if using interrupts,
                 // or if polling for any completion. For simple polling by CID, we just ignore it here
                 // and continue polling/waiting.

            }
            // No completion found or not the expected one, continue polling.
            core::hint::spin_loop(); // Hint to the CPU to spin efficiently
        }
        // Timeout occurred
        Err(NvmeError::Timeout)
    }

    /// Performs a block read operation on the NVMe device.
    ///
    /// # Arguments
    ///
    /// * `block_number`: The starting Logical Block Address (LBA) to read from.
    /// * `buffer`: The buffer to read the data into. Its length must be a multiple of the block size.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a BlockDeviceError.
    pub fn read_block(&mut self, block_number: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> { // Return BlockDeviceError
        // Check if the buffer length is a multiple of the device's block size
        if buffer.len() % self.block_size != 0 {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer length ({}) must be a multiple of device block size ({}).", buffer.len(), self.block_size) // Requires alloc
            ));
        }
        let block_count = buffer.len() / self.block_size; // Number of blocks to read


        // --- Kernel Interaction Placeholder ---
        // Get the physical address of the buffer for DMA.
        // This requires kernel services for virtual-to-physical translation.
        // Example: let physical_address = kernel::vm::virt_to_phys(buffer.as_ptr() as u64)?;
        // --------------------------------------
        // For this skeleton, we use the virtual address directly, which is UNSAFE for DMA.
        let data_ptr_phys = buffer.as_ptr() as u64; // UNSAFE: Using virtual address for DMA pointer


        // Construct the NVMe Read command (NVM Command Set, Read command opcode 0x02)
        let command = NvmeCommand {
            opcode: NVME_OPCODE_READ, // Read command opcode
            flags: 0, // No specific flags for basic read
            cid: self.get_command_id(), // Get a unique command ID
            nsid: self.namespace_id, // Target namespace ID
            // LBA and block count are split into multiple Command Dword fields for Read/Write
            cdw2: block_number as u32, // Starting LBA (Lower 32 bits)
            cdw3: (block_number >> 32) as u32, // Starting LBA (Upper 32 bits)
            cdw4: (block_count - 1) as u32, // Number of Logical Blocks (0-based)
            cdw5: 0, // Reserved
            metadata_ptr: 0, // No metadata used in this basic command
            data_ptr: data_ptr_phys, // Physical address of the data buffer for DMA
            cdw10: 0, // Reserved
            cdw11: 0, // Reserved
            cdw12: 0, // Reserved
            cdw13: 0, // Reserved
            cdw14: 0, // Reserved
            cdw15: 0, // Reserved
        };

        // Submit the command to the controller's submission queue
        let cid = self.submit_command(command).map_err(|e| map_nvme_error_to_block_device_error(e))?; // Map NvmeError to BlockDeviceError

        // Wait for the command to complete by polling the completion queue
        self.poll_completion(cid).map_err(|e| map_nvme_error_to_block_device_error(e))?; // Map NvmeError to BlockDeviceError


        Ok(()) // Read operation successful
    }

    /// Performs a block write operation on the NVMe device.
    ///
    /// # Arguments
    ///
    /// * `block_number`: The starting Logical Block Address (LBA) to write to.
    /// * `buffer`: The buffer containing the data to write. Its length must be a multiple of the block size.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a BlockDeviceError.
    pub fn write_block(&mut self, block_number: u64, buffer: &[u8]) -> Result<(), BlockDeviceError> { // Return BlockDeviceError
         // Check if the buffer length is a multiple of the device's block size
        if buffer.len() % self.block_size != 0 {
            return Err(BlockDeviceError::BlockSizeError(
                format!("Buffer length ({}) must be a multiple of device block size ({}).", buffer.len(), self.block_size) // Requires alloc
            ));
        }
        let block_count = buffer.len() / self.block_size; // Number of blocks to write


        // --- Kernel Interaction Placeholder ---
        // Get the physical address of the buffer for DMA.
        // This requires kernel services for virtual-to-physical translation.
        // Example: let physical_address = kernel::vm::virt_to_phys(buffer.as_ptr() as u64)?;
        // --------------------------------------
        // For this skeleton, we use the virtual address directly, which is UNSAFE for DMA.
        let data_ptr_phys = buffer.as_ptr() as u64; // UNSAFE: Using virtual address for DMA pointer


        // Construct the NVMe Write command (NVM Command Set, Write command opcode 0x01)
        let command = NvmeCommand {
            opcode: NVME_OPCODE_WRITE, // Write command opcode
            flags: 0, // No specific flags for basic write
            cid: self.get_command_id(), // Get a unique command ID
            nsid: self.namespace_id, // Target namespace ID
            // LBA and block count are split into multiple Command Dword fields
            cdw2: block_number as u32, // Starting LBA (Lower 32 bits)
            cdw3: (block_number >> 32) as u32, // Starting LBA (Upper 32 bits)
            cdw4: (block_count - 1) as u32, // Number of Logical Blocks (0-based)
            cdw5: 0, // Reserved
            metadata_ptr: 0, // No metadata
            data_ptr: data_ptr_phys, // Physical address of the data buffer for DMA
            cdw10: 0, // Reserved
            cdw11: 0, // Reserved
            cdw12: 0, // Reserved
            cdw13: 0, // Reserved
            cdw14: 0, // Reserved
            cdw15: 0, // Reserved
        };

        // Submit the command to the controller's submission queue
        let cid = self.submit_command(command).map_err(|e| map_nvme_error_to_block_device_error(e))?; // Map NvmeError to BlockDeviceError

        // Wait for the command to complete by polling the completion queue
        self.poll_completion(cid).map_err(|e| map_nvme_error_to_block_device_error(e))?; // Map NvmeError to BlockDeviceError


        Ok(()) // Write operation successful
    }

    /// Returns the logical block size of the NVMe namespace managed by this driver.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    // Add other NVMe driver functions as needed:
    // - Controller initialization and shutdown
    // - Admin queue commands (Identify, Create I/O Queue, etc.)
    // - Interrupt handling logic
    // - Namespace discovery and attachment
    // - Power state management
    // - Error handling and logging specifics
    // - DMA buffer allocation/management helpers
}


#[cfg(not(feature = "std"))] // Implement BlockDevice trait only in no_std
impl BlockDevice for NvmeDriver {
    /// Reads a block using the NVMe driver.
    /// Maps BlockDeviceError from the internal read_block.
    fn read_block(&mut self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockDeviceError> {
        // block_id is usize for BlockDevice trait, but NVMe uses u64 LBA.
        // Need to ensure block_id fits in u64 or handle larger addresses if necessary.
        // Assuming usize block_id can be cast to u64 safely for typical block device sizes.
        self.read_block(block_id as u64, buf)
    }

    /// Writes a block using the NVMe driver.
    /// Maps BlockDeviceError from the internal write_block.
    fn write_block(&mut self self, block_id: usize, buf: &[u8]) -> Result<(), BlockDeviceError> {
         // Assuming usize block_id can be cast to u64 safely.
        self.write_block(block_id as u64, buf)
    }

    /// Returns the logical block size from the driver instance.
    fn block_size(&self) -> usize {
        self.block_size() // Delegate to internal method
    }

    // Need to implement other BlockDevice trait methods if they exist in the real trait.
    // E.g., size(), erase(), etc.
    // Size of an NVMe namespace needs to be obtained via Identify command (Admin Queue).
    // Let's add a placeholder method for size based on a hypothetical field or method.
    // For now, assume size is not a required BlockDevice trait method based on previous files.
}


// No tests provided, adding a basic test module skeleton.
#[cfg(test)]
mod tests {
    // No std tests are meaningful for this hardware-specific no_std driver.
    // Unit tests would require extensive mocking of the hardware interface,
    // MMIO, atomics, and potentially Sahne64 kernel services.

    // TODO: Add no_std unit tests with mocks.
    // Test cases could include:
    // - Command ID generation.
    // - Simplified queue full condition check.
    // - Error mapping from NvmeError to BlockDeviceError.
    // - Buffer size validation in read_block/write_block.
    // - Mocking queue memory and testing command submission (checking queue entry contents, tail pointer updates).
    // - Mocking queue memory and testing completion polling (simulating completion entries, head pointer updates, status checks).
    // - Testing the BlockDevice trait implementation delegates correctly.
}

// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
