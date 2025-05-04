#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel Sahne64 tipleri ve modülleri
use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri ve Handle
// Sahne64 resource modülü
#[cfg(not(feature = "std"))]
use crate::resource;
// Sahne64 fs modülü (fs::read_at, fs::fstat için varsayım)
#[cfg(not(feature = "std"))]
use crate::fs;

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box; // For Box<dyn VfsNode>
use alloc::format;

// core::io traits and types (might not be strictly needed if using fs::read_at directly, but useful context)
 use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io


// Helper function to map SahneError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
    // TODO: Implement a proper mapping based on SahneError variants
}

// Helper function to map FileSystemError to VfsError
fn map_fs_error_to_vfs_error(e: FileSystemError) -> VfsError {
    match e {
        // Map specific FileSystemError variants to VfsError variants
        FileSystemError::NotFound => VfsError::NotFound,
        FileSystemError::PermissionDenied => VfsError::PermissionDenied,
        FileSystemError::InvalidData(msg) => VfsError::InvalidData(msg), // Keep message
        FileSystemError::IOError(msg) => VfsError::IOError(msg), // Keep message
        FileSystemError::NotSupported => VfsError::NotSupported,
        // Add other mappings if FileSystemError gains more variants
        _ => VfsError::IOError(format!("Unknown FileSystemError: {:?}", e)), // Default mapping
    }
}

// VfsError tanımı
#[derive(Debug)]
pub enum VfsError {
    NotFound,
    PermissionDenied,
    InvalidDescriptor, // Corresponds to an invalid Handle in the underlying layer
    IOError(String), // Include details for better debugging
    InvalidData(String), // Include details for parsing issues
    NotSupported,
    AlreadyExists, // Example VFS error
    IsDirectory, // Example VFS error
    NotDirectory, // Example VFS error
    NotEmpty, // Example VFS error
    // ... diğer VFS hataları
}

// Implement Display for VfsError (useful for error reporting)
use core::fmt;
impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VfsError::NotFound => write!(f, "Entity not found"),
            VfsError::PermissionDenied => write!(f, "Permission denied"),
            VfsError::InvalidDescriptor => write!(f, "Invalid descriptor"),
            VfsError::IOError(msg) => write!(f, "IO error: {}", msg),
            VfsError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            VfsError::NotSupported => write!(f, "Operation not supported"),
            VfsError::AlreadyExists => write!(f, "Entity already exists"),
            VfsError::IsDirectory => write!(f, "Is a directory"),
            VfsError::NotDirectory => write!(f, "Not a directory"),
            VfsError::NotEmpty => write!(f, "Directory not empty"),
        }
    }
}

// Example VFS traits (these should ideally be defined in a VFS core module)
pub trait FileSystem {
    /// Opens a VFS node (file or directory) at the given path.
    fn open(&self, path: &str, flags: u32) -> Result<Box<dyn VfsNode>, VfsError>;
    // Add other file system operations like create, mkdir, remove, etc.
     fn create(&self, path: &str, flags: u32) -> Result<Box<dyn VfsNode>, VfsError>;
     fn mkdir(&self, path: &str) -> Result<(), VfsError>;
     fn remove(&self, path: &str) -> Result<(), VfsError>;
     fn rename(&self, old_path: &str, new_path: &str) -> Result<(), VfsError>;
     fn metadata(&self, path: &str) -> Result<Metadata, VfsError>; // Need Metadata struct
}

// Need a Metadata struct if the trait includes metadata operations
 pub struct Metadata {
    pub file_size: usize,
    pub is_directory: bool,
//    // Add other metadata fields like permissions, timestamps, etc.
 }


pub trait VfsNode {
    /// Reads data from the VFS node into the provided buffer at the specified offset.
    /// Returns the number of bytes read.
    fn read(&self, buffer: &mut [u8], offset: usize) -> Result<usize, VfsError>;

    // Add other VFS node operations like write, seek, close, get_size, etc.
    // fn write(&self, buffer: &[u8], offset: usize) -> Result<usize, VfsError>;
    // fn seek(&self, pos: SeekFrom) -> Result<u64, VfsError>; // SeekFrom from core::io
    fn size(&self) -> Result<usize, VfsError>; // Get the size of the node
    // fn close(&self) -> Result<(), VfsError>; // Explicit close might be needed for resource management
    // fn sync(&self) -> Result<(), VfsError>; // Flush buffered data
}


/// Represents a DOCX file as a VFS node, providing basic read functionality.
/// DOCX files are essentially ZIP archives containing XML files.
/// This implementation focuses on providing access to the raw bytes of the file.
#[cfg(not(feature = "std"))] // This VFS Node implementation is for no_std/Sahne64
pub struct DocxFile {
    /// Sahne64 dosya kaynağının Handle'ı.
    handle: Handle,
    /// Dosyanın boyutu (bayt olarak).
    file_size: usize,
    // Note: No internal position tracking here, relies on fs::read_at or equivalent.
}

#[cfg(not(feature = "std"))]
impl DocxFile {
    /// Creates a new `DocxFile` instance from a Sahne64 file Handle and size.
    /// This is typically called by a `FileSystem` implementation's `open` method.
    ///
    /// # Arguments
    ///
    /// * `handle` - The Sahne64 Handle for the opened file.
    /// * `file_size` - The size of the file in bytes.
    ///
    /// # Returns
    ///
    /// A new `DocxFile` instance.
    pub fn new(handle: Handle, file_size: usize) -> Self {
        DocxFile { handle, file_size }
    }

    // Removed read_all as it's redundant with VfsNode::read
}

#[cfg(not(feature = "std"))]
impl VfsNode for DocxFile {
    /// Reads data from the DOCX file (VFS node) into the provided buffer
    /// at the specified offset using the underlying Sahne64 `read_at` syscall.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to read data into.
    /// * `offset` - The offset in the file to start reading from.
    ///
    /// # Returns
    ///
    /// The number of bytes read, or a `VfsError` if an error occurred.
    fn read(&self, buffer: &mut [u8], offset: usize) -> Result<usize, VfsError> {
        // Check if the requested read is beyond the end of the file
        if offset >= self.file_size {
            return Ok(0); // Reading at or past EOF
        }

        // Calculate how many bytes are actually available to read from the offset
        let bytes_available = self.file_size.checked_sub(offset).unwrap_or(0); // Should not panic if offset <= file_size
        let bytes_to_read = cmp::min(buffer.len(), bytes_available);

        if bytes_to_read == 0 {
            return Ok(0); // No bytes to read
        }

        // Use the assumed Sahne64 fs::read_at syscall
        // fs::read_at(handle, offset, buffer) Result<usize, SahneError> döner (varsayım)
        let bytes_read = fs::read_at(self.handle, offset as u64, &mut buffer[..bytes_to_read])
            .map_err(map_sahne_error_to_fs_error) // SahneError -> FileSystemError
            .map_err(map_fs_error_to_vfs_error)?; // FileSystemError -> VfsError

        Ok(bytes_read)
    }

    /// Returns the size of the DOCX file (VFS node).
    fn size(&self) -> Result<usize, VfsError> {
        Ok(self.file_size)
    }

    // Add other VfsNode methods if needed, mapping underlying SahneError/FileSystemError to VfsError
     fn close(&self) -> Result<(), VfsError> {
          resource::release(self.handle)
              .map_err(map_sahne_error_to_fs_error) // SahneError -> FileSystemError
              .map_err(map_fs_error_to_vfs_error) // FileSystemError -> VfsError
     }
}

// TODO: Add a std implementation for DocxFile if needed, potentially wrapping std::fs::File
 #[cfg(feature = "std")]
 pub struct DocxFile {
     file: std::fs::File, // Or BufReader<File>
     file_size: usize,
 }
//
 #[cfg(feature = "std")]
 impl DocxFile {
     pub fn new(file: std::fs::File, file_size: usize) -> Self {
         DocxFile { file, file_size }
     }
 }
//
 #[cfg(feature = "std")]
 impl VfsNode for DocxFile {
     fn read(&self, buffer: &mut [u8], offset: usize) -> Result<usize, VfsError> {
         use std::io::{Read, Seek, SeekFrom};
//         // Use self.file.seek and self.file.read
//         // Need to map std::io::Error to VfsError
//         // This requires a mapping helper similar to map_sahne_error_to_fs_error
//         // and then mapping FileSystemError to VfsError (or map std::io::Error directly to VfsError)
         unimplemented!()
     }
//
     fn size(&self) -> Result<usize, VfsError> {
         Ok(self.file_size)
     }
//     // fn close(&self) -> Result<(), VfsError> { ... }
 }


// Redundant syscall/module definitions removed - assume they are defined elsewhere in Sahne64 API

// Example main function (no_std)
#[cfg(feature = "example_docx")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), VfsError> { // Return VfsError
     eprintln!("DocxFile example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock FileSystem and a mock Sahne64 fs/resource layer
     // to simulate opening and reading a file via the VFS.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
      let mock_fs = MockFileSystem::new(); // A mock that simulates fs/resource syscalls
      mock_fs.add_file("sahne://files/document.docx", b"PK\x03\x04...".to_vec()); // Add dummy DOCX data

      let docx_node_res = mock_fs.open("sahne://files/document.docx", 0 /* flags */);
      match docx_node_res {
          Ok(docx_node) => {
              let mut buffer = [0u8; 100];
              match docx_node.read(&mut buffer, 0) {
                  Ok(bytes_read) => {
                      println!("Read {} bytes from DOCX file.", bytes_read);
                      // Process the buffer (check for ZIP magic 'PK\x03\x04')
                      if bytes_read >= 4 && &buffer[0..4] == b"PK\x03\x04" {
                          println!("Detected ZIP signature (PK\\x03\\x04).");
                      } else {
                          println!("ZIP signature not found.");
                      }
                  },
                  Err(e) => eprintln!("Error reading DOCX file: {:?}", e),
              }
     //         // Need to explicitly close the node if VfsNode has a close method
               if let Some(closable) = docx_node.as_any().downcast_ref::<dyn VfsNode>() { // Requires downcasting if Box<dyn VfsNode> doesn't expose close
                    let _ = closable.close(); // Handle error
               } else {
                     // If VfsNode doesn't have close, Drop should handle it if DocxFile implements Drop
     //              // But Drop in no_std is tricky for resources.
     //              // If DocxFile has a close method, need to call it.
     //              // Example: If DocxFile implements Closeable trait and VfsNode trait object can be downcasted.
               }
     //         // Or, if DocxFile::close() is public:
               let concrete_node = docx_node.as_any().downcast_ref::<DocxFile>().unwrap(); // Requires Any trait
               concrete_node.close().unwrap(); // Requires close method on concrete type
     //
     //          // Simpler: if VfsNode trait has a close method
                docx_node.close().unwrap(); // If close is part of VfsNode
     //
     //          // Assuming resource release is handled by Drop in DocxFile (as implemented now for the Handle)
     //          // This requires careful Drop implementation in no_std.
     //          // Current DocxFile doesn't implement Drop. Explicit release is safer.
     //          // Let's add a close method to DocxFile and call it.
               // And add a close method to VfsNode trait.
          },
          Err(e) => eprintln!("Error opening DOCX file: {:?}", e),
      }

     eprintln!("DocxFile example (no_std) needs VFS and Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_docx")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), VfsError> { // Return VfsError for std example
     eprintln!("DocxFile example (std) starting...");
     eprintln!("DocxFile example (std) not fully implemented.");
     Ok(()) // Dummy return
}


// Test module (requires a mock VFS and Sahne64 environment for no_std)
#[cfg(test)]
#[cfg(not(feature = "std"))] // Only compile tests for no_std
mod tests_no_std {
    // Need a mock VFS and Sahne64 filesystem layer for testing
    // This is complex and requires a testing framework or simulation.

    // TODO: Implement tests for DocxFile using a mock Sahne64 VFS environment.
}

#[cfg(test)]
#[cfg(feature = "std")] // Only compile tests for std
mod tests_std {
    // Need a std VFS implementation or mock for testing
    // This is complex and requires a testing framework.

    // TODO: Implement tests for DocxFile using std::fs and a VFS wrapper.
}

// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_docx", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
