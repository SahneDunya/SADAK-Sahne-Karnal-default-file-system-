 #![no_std] // This file likely part of the main crate, no need for redundant no_std if lib.rs has it
 #![allow(dead_code)] // Keep if needed, but removing redundant one from top level is better

// Necessary imports from the crate (assuming they are defined in lib.rs or other modules)
use crate::vfs::{VfsNode, VfsNodeType, VFileOps}; // Assuming VFileOps is here and uses &self, u64, FileSystemError
use crate::FileSystemError; // Assuming FileSystemError is defined

// alloc crate for String, Vec
extern crate alloc; // Ensure alloc is available
use alloc::string::String;
use alloc::vec::Vec;

// core sync primitive
use core::sync::Mutex; // Requires target_has_atomic = "cas" or similar

// core cmp for min
use core::cmp;

// core result type
use core::result::Result;


/// Represents an in-memory file within the VFS.
/// Stores data in a Mutex-protected vector.
pub struct File {
    pub name: String,
    data: Mutex<Vec<u8>>, // Mutex for thread-safe access to the data vector
    pub node: VfsNode, // Associated VFS node
}

// Define a specific error type for in-memory file operations if needed,
// or map directly to FileSystemError. Let's map directly for simplicity here.
 #[derive(Debug)]
 pub enum InMemoryFileError {
     InvalidOffset,
//     // Add other specific errors if necessary
 }
//
 impl fmt::Display for InMemoryFileError { ... }
//
 fn map_in_memory_file_error_to_fs_error(e: InMemoryFileError) -> FileSystemError { ... }


impl File {
    /// Creates a new in-memory file instance.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the file.
    /// * `node`: The associated VFS node.
    pub fn new(name: String, node: VfsNode) -> Self {
        File {
            name,
            data: Mutex::new(Vec::new()), // Initialize with an empty vector
            node,
        }
    }

    // The read, write, size methods will now be part of the VFileOps implementation
    // and will match the VFileOps trait signature.
    // We keep them here as internal helpers if needed, but the primary interface
    // should be through VFileOps. Let's remove the redundant implementations here
    // and rely solely on the VFileOps impl below.
}

// Assuming VFileOps trait is defined like:
 pub trait VFileOps: Send + Sync { // Requires Send + Sync for Arc (if VFileRef is Arc)
    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, FileSystemError>; // &self if managing own position
    fn write(&self, buf: &[u8], offset: u64) -> Result<usize, FileSystemError>; // &self if managing own position
    fn size(&self) -> u64;
//    // Potentially other methods like flush, truncate, etc.
 }
//
// Based on the original code's use of `&self` and explicit `offset`, we assume
// VFileOps::read and VFileOps::write take `&self`. The offset type should be `u64`.

// Add Send and Sync bounds if required by VFileOps and VfsNode (e.g., if VfsNode is Arc<Mutex<...>>)
 #[cfg(any(feature = "std", target_has_atomic = "ptr"))] // Sync requires atomics for no_std
 use core::marker::Sync;
 use core::marker::Send;

impl VFileOps for File {
    /// Reads data from the file into `buf` starting at the given `offset`.
    ///
    /// # Arguments
    ///
    /// * `buf`: The byte slice to read data into.
    /// * `offset`: The file offset (in bytes) to start reading from.
    ///
    /// # Returns
    ///
    /// On success, returns `Ok(usize)` number of bytes read.
    /// On failure, returns a `FileSystemError`.
    // Assuming the VFileOps::read signature is &self, u64 offset, Result<usize, FileSystemError>
    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, FileSystemError> { // Return FileSystemError
        let data = self.data.lock().map_err(|_| FileSystemError::Other(String::from("Failed to lock mutex")))?; // Lock the mutex, map poisoning error

        // Check if offset is out of bounds (u64 to usize conversion safety)
        if offset > usize::MAX as u64 || offset as usize >= data.len() {
             return Ok(0); // EOF or offset beyond end
        }
        let offset_usize = offset as usize; // Safe conversion after check


        let len = data.len();
        let read_len = cmp::min(buffer.len(), len - offset_usize); // Calculate bytes to read

        // Copy data from the vector to the buffer
        buffer[..read_len].copy_from_slice(&data[offset_usize..offset_usize + read_len]);

        Ok(read_len) // Return number of bytes read
    }

    /// Writes data from `buffer` to the file starting at the given `offset`.
    ///
    /// # Arguments
    ///
    /// * `buf`: The byte slice containing data to write.
    /// * `offset`: The file offset (in bytes) to start writing to.
    ///
    /// # Returns
    ///
    /// On success, returns `Ok(usize)` number of bytes written.
    /// On failure, returns a `FileSystemError`.
    // Assuming the VFileOps::write signature is &self, u64 offset, Result<usize, FileSystemError>
    fn write(&self, buf: &[u8], offset: u64) -> Result<usize, FileSystemError> { // Return FileSystemError
        let mut data = self.data.lock().map_err(|_| FileSystemError::Other(String::from("Failed to lock mutex")))?; // Lock the mutex mutably, map poisoning error

         // Check if offset is out of bounds for writing (can write at or after end)
        if offset > usize::MAX as u64 || (offset as usize) > data.len() { // Can write exactly at data.len()
            return Err(FileSystemError::InvalidParameter(format!("Invalid write offset: {}", offset))); // Invalid offset for writing
        }
        let offset_usize = offset as usize; // Safe conversion


        let write_len = buf.len();
        let current_len = data.len();

        if offset_usize == current_len {
            // Appending to the end
            data.extend_from_slice(buf);
        } else {
            // Overwriting or extending within the vector
            let required_len = offset_usize.checked_add(write_len).ok_or(FileSystemError::OutOfMemory)?; // Check for overflow
             if required_len > current_len {
                 data.resize(required_len, 0); // Resize if extending beyond current length, fill with zeros
             }
             // Copy data into the specified range
            data[offset_usize..required_len].copy_from_slice(buf);
        }

        Ok(write_len) // Return number of bytes written
    }

    /// Returns the size of the file in bytes.
    fn size(&self) -> u64 { // Return u64 as per VFileOps assumption
        // Acquire lock, get vector length, and return as u64
        // Use unwrap() on lock() here is generally safe if lock poisoning is acceptable,
        // or map the poisoning error to FileSystemError if VFileOps allowed returning error.
        // Since size() returns u64, mapping to FileSystemError is not possible in return type.
        // Assuming VFileOps::size() cannot fail, unwrap is used, implying panic on lock poisoning.
        self.data.lock().unwrap().len() as u64 // Convert usize to u64
    }

    // Implement other VFileOps methods if required by the trait,
    // such as flush, truncate, etc. For an in-memory file, flush might do nothing,
    // truncate would resize the vector.
     fn flush(&self) -> Result<(), FileSystemError> { Ok(()) }
     fn truncate(&self, size: u64) -> Result<(), FileSystemError> {
         let mut data = self.data.lock().map_err(|_| FileSystemError::Other(String::from("Failed to lock mutex")))?;
          // Check for u64 to usize conversion safety
          if size > usize::MAX as u64 {
               return Err(FileSystemError::InvalidParameter(format!("Truncate size too large: {}", size)));
          }
          data.resize(size as usize, 0);
          Ok(())
     }
}


// This file defines an in-memory file type. Its usage within a filesystem
// would be handled by a specific FileSystem implementation that manages
// VfsNode structures and creates instances of this File struct.
// No main function or tests are typically included in such a library component file.
// Example usage and tests would be in other files that use this module.

// Redundant print module and panic handler are also removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
