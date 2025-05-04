#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::io::{Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::boxed::Box as StdBox; // Use std Box
#[cfg(feature = "std")]
use std::sync::Arc as StdArc; // Use std Arc

// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
// Assuming VFile, VFileOps, FileSystem, VFileRef are defined in crate::fs
// Assuming FileSystemError, SahneError, Handle, resource are defined in crate
use crate::{fs::{FileSystem, VFile, VFileOps, VFileRef}, FileSystemError, SahneError, resource}; // fs, FileSystemError, SahneError

// core::result, core::fmt, core::io
use core::result::Result;
use core::fmt;
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io

// alloc crate for Box, Arc (if VFileRef is Arc), String, Vec
use alloc::boxed::Box; // Use alloc Box
#[cfg(not(feature = "std"))]
use alloc::sync::Arc; // Use alloc Arc if needed for VFileRef

// Assuming VFileRef is defined as something like Arc<dyn VFileOps + Send + Sync>
// The bounds Send + Sync are often required by Arc for multi-threading safety.
// We will add these bounds to the dyn traits used in this file.
use core::marker::Send;
#[cfg(any(feature = "std", target_has_atomic = "ptr"))] // Sync requires atomics for no_std
use core::marker::Sync;


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

// No need to redefine Read, Seek, SeekFrom traits and impls.
// Use core::io traits directly and map errors where needed.


/// `PSTFile` structure representing a PST file adapted to VFileOps.
/// It wraps an underlying `Read + Seek + Send + Sync` data source.
/// PST files are treated as read-only.
pub struct PSTFile {
    /// The underlying data source for the PST file.
    /// `Box<dyn core::io::Read + core::io::Seek + Send + Sync>`.
    /// The `Send + Sync` bounds are likely required by the VFileRef (Arc).
    data: Box<dyn core::io::Read + core::io::Seek + Send + Sync>,
    /// The size of the PST file in bytes.
    size: u64,
}

impl PSTFile {
    /// Creates a new `PSTFile` instance.
    ///
    /// # Arguments
    ///
    /// * `data`: The underlying readable and seekable data for the PST file.
    ///           Must implement `core::io::Read`, `core::io::Seek`, `Send`, and `Sync`.
    /// * `size`: The size of the PST file.
    pub fn new(data: Box<dyn core::io::Read + core::io::Seek + Send + Sync>, size: u64) -> Self {
        PSTFile { data, size }
    }
}

// Assuming VFileOps trait is defined like:
 pub trait VFileOps: Send + Sync { // Requires Send + Sync for Arc
    fn read(&mut self, buf: &mut [u8], offset: u64) -> Result<usize, FileSystemError>;
    fn write(&mut self, buf: &[u8], offset: u64) -> Result<usize, FileSystemError>;
    fn size(&self) -> u64;
 }
// And VFileRef is type alias: type VFileRef = Arc<dyn VFileOps>; or Arc<dyn VFileOps + Send + Sync>;

impl VFileOps for PSTFile {
    /// Reads data from the file into `buf` starting at the given `offset`.
    ///
    /// # Arguments
    ///
    /// * `buf`: The byte slice to read data into.
    /// * `offset`: The file offset to start reading from.
    ///
    /// # Returns
    ///
    /// On success, returns `Ok(usize)` number of bytes read.
    /// On failure, returns a `FileSystemError`.
    // Assuming the VFileOps::read signature is &mut self
    fn read(&mut self, buf: &mut [u8], offset: u64) -> Result<usize, FileSystemError> { // Return FileSystemError
        // Seek to the specified offset. Map core::io::Error to FileSystemError.
        self.data.seek(core::io::SeekFrom::Start(offset)).map_err(map_core_io_error_to_fs_error)?;

        // Read data into buf and return the result. Map core::io::Error to FileSystemError.
        self.data.read(buf).map_err(map_core_io_error_to_fs_error)
    }

    /// Writes data to the file (Unsupported for PST files).
    ///
    /// This method always returns a `PermissionDenied` error as writing to PST files is not supported.
    // Assuming the VFileOps::write signature is &mut self
    fn write(&mut self, _buf: &[u8], _offset: u64) -> Result<usize, FileSystemError> { // Return FileSystemError
        Err(FileSystemError::PermissionDenied)
    }

    /// Returns the size of the file.
    fn size(&self) -> u64 {
        self.size
    }
}

/// `PSTFileSystem` structure representing a file system that can handle PST files.
pub struct PSTFileSystem {}

impl PSTFileSystem {
    /// Creates a new `PSTFileSystem` instance.
    pub fn new() -> Self {
        PSTFileSystem {}
    }
}

// Assuming FileSystem trait is defined like:
 pub trait FileSystem: Send + Sync { // Requires Send + Sync for Arc
    fn open(&self, path: &str, data: Box<dyn core::io::Read + core::io::Seek + Send + Sync>, size: u64) -> Result<VFileRef, FileSystemError>;
//    // Other filesystem operations like create, unlink, stat, etc.
 }

impl FileSystem for PSTFileSystem {
    /// Opens a `VFile` instance for the given `path`.
    ///
    /// If the `path` ends with ".pst", a `PSTFile` wrapping the provided `data`
    /// and `size` is created and returned within a `VFileRef`.
    /// Otherwise, a `FileNotFound` error is returned.
    ///
    /// # Arguments
    ///
    /// * `path`: The path of the file to open.
    /// * `data`: The underlying file data source, must implement `core::io::Read + core::io::Seek + Send + Sync`.
    /// * `size`: The size of the file.
    ///
    /// # Returns
    ///
    /// On success, returns `Ok` with the `VFileRef` wrapping the `PSTFile`.
    /// On failure, returns a `FileSystemError`.
    // Assuming the FileSystem::open signature matches the trait definition assumed above
    fn open(&self, path: &str, data: Box<dyn core::io::Read + core::io::Seek + Send + Sync>, size: u64) -> Result<VFileRef, FileSystemError> { // Return FileSystemError
        // Check if the file path ends with ".pst".
        if path.ends_with(".pst") {
            // If it's a PST file, create a PSTFile and wrap it in a VFileRef.
            let pst_file_ops: Box<dyn VFileOps> = Box::new(PSTFile::new(data, size));
            // VFile::new typically wraps VFileOps in Arc<Mutex<...>> or similar for thread safety and shared ownership.
            // Assuming VFile::new handles the Box<dyn VFileOps> and returns VFileRef (Arc<dyn VFileOps + Send + Sync>).
            Ok(crate::fs::VFile::new(pst_file_ops)) // Assuming crate::fs::VFile::new takes Box<dyn VFileOps> and returns VFileRef
        } else {
            // Return FileSystemError::FileNotFound for unsupported file types or paths.
            Err(FileSystemError::FileNotFound(format!("Unsupported file type for PSTFileSystem: {}", path))) // Use FileSystemError::FileNotFound
        }
    }
}

// No test module or example main function provided in the original, so none added here.
// These would typically be in separate test files or example binaries.

// Redundant print module and panic handler are also removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
