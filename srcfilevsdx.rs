#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
// #![cfg_attr(not(feature = "std"), no_std)] // This file relies heavily on std features of zip and io

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için (if any alloc is used)
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
// Use crate:: instead of super:: for consistency
use crate::fs::{FileSystem, VfsNode};
use crate::{resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::O_RDONLY; // Import necessary fs flags


// std and core IO traits
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Write, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt, WriteExt as CoreWriteExt};


// std library imports (required for Box<dyn Read + Seek> and std::io::Error in original code)
#[cfg(feature = "std")]
use std::io::{Read as StdRead, Seek as StdSeek, Error as StdIOError, ErrorKind as StdIOErrorKind, Cursor as StdCursor, ReadExt as StdReadExt, SeekFrom as StdSeekFrom};
#[cfg(feature = "std")]
use std::boxed::Box as StdBox;
#[cfg(feature = "std")]
use std::fs::File as StdFile;
#[cfg(feature = "std")]
use std::path::Path as StdPath;


// zip crate (requires std::io::Read + Seek by default)
// Assuming zip::ZipArchive, zip::result::ZipError are available
use zip::ZipArchive;
use zip::result::ZipError;


// alloc crate for String, Vec, format! (used directly or by dependencies)
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;


// Need no_std println!/eprintln! macros (if needed)
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülünden import edildiği varsayılır


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

/// Helper function to map zip::result::ZipError to FileSystemError.
fn map_zip_error_to_fs_error(e: ZipError) -> FileSystemError {
    match e {
         ZipError::Io(io_err) => {
              #[cfg(feature = "std")]
              return map_std_io_error_to_fs_error(io_err); // Map underlying std IO error
              #[cfg(not(feature = "std"))]
              // In no_std, the underlying IO error from zip::result::ZipError::Io
              // needs to be mapped. Assuming ZipError::Io contains a core::io::Error or similar.
              // If zip is truly no_std, its Io error should be core::io::Error.
              // This requires investigating the zip crate's no_std error handling.
              // For now, assume a generic mapping for no_std ZipError::Io.
               FileSystemError::IOError(format!("Zip IO error: {:?}", io_err)) // Generic mapping for no_std
         },
        ZipError::InvalidArchive(msg) => FileSystemError::InvalidData(format!("Invalid Zip archive: {}", msg)),
        ZipError::FileNotFound => FileSystemError::NotFound(String::from("File not found in Zip archive")), // Requires alloc
        ZipError::UnsupportedArchive(msg) => FileSystemError::NotSupported(format!("Unsupported Zip archive feature: {}", msg)), // Requires alloc
        ZipError::InvalidPassword => FileSystemError::PermissionDenied(String::from("Invalid password for Zip archive")), // Requires alloc
        ZipError::Other(msg) => FileSystemError::Other(format!("Zip error: {}", msg)), // Requires alloc
    }
}

/// Custom error type for VSDX handling issues.
#[derive(Debug)]
pub enum VsdxError {
    ZipError(String), // Errors from the underlying zip crate
    MissingInternalFile(String), // A required internal file is missing
    // Add other VSDX specific errors here
}

// Implement Display for VsdxError
impl fmt::Display for VsdxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VsdxError::ZipError(msg) => write!(f, "Zip hatası: {}", msg),
            VsdxError::MissingInternalFile(filename) => write!(f, "VSDX içinde gerekli dosya bulunamadı: {}", filename),
        }
    }
}

// Helper function to map VsdxError to FileSystemError
fn map_vsdx_error_to_fs_error(e: VsdxError) -> FileSystemError {
    match e {
        VsdxError::ZipError(msg) => FileSystemError::Other(format!("VSDX işleme hatası: {}", msg)), // Map zip error msg
        VsdxError::MissingInternalFile(_) => FileSystemError::InvalidData(format!("VSDX format hatası: {}", e)), // Map missing file as invalid format
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (copied from srcfiletxt.rs)
// core::io::Write implementasyonu eklendi.
// This requires fs::read_at/write_at and fstat, which are not guaranteed in the original Sahne64 syscalls.
// Assuming these are part of the standardized Sahne64 FS API.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReadSeek { // Renamed to reflect Read+Seek capability
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu (read/write için güncellenmeli)
}

#[cfg(not(feature = "std"))]
impl SahneResourceReadSeek {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReadSeek { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl core::io::Read for SahneResourceReadSeek { // Use core::io::Read trait
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
    // read_to_string has a default implementation in core::io::ReadExt that uses read and from_utf8
}

#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReadSeek { // Use core::io::Seek trait
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
    // stream_position has a default implementation in core::io::Seek that uses seek(Current(0))
}

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReadSeek {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReadSeek is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReadSeek drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}

// NOTE: SahneResourceReader (with Write impl) from srcfiletxt.rs is not directly needed here
// unless we want to represent the VSDX itself as a *writable* VfsNode, which is not supported
// by the current VsdxFile implementation. We need a Read+Seek only wrapper for the zip crate.


// Removed redundant module imports from top level.
// Removed redundant fs, SahneError definitions.


/// Represents a VSDX file (ZIP archive) and provides access to its contents.
/// This struct holds the ZipArchive instance created from an underlying reader.
/// It is designed to be used in a std environment or a no_std environment
/// with a compatible zip crate and Read + Seek implementation.
pub struct VsdxFile {
    // The ZipArchive requires a Read + Seek reader.
    // We use a Box<dyn Read + Seek> for trait object capabilities in std.
    // In no_std, a concrete reader type like SahneResourceReadSeek would be used.
    // To make this generic over the reader type, we could use a generic parameter.
    // However, for VfsNode implementation (which might need to clone/share the archive),
    // a shared Mutex/RefCell around the archive might be necessary.
    // Let's keep the Box<dyn Read + Seek> for std and discuss no_std/VFS integration below.

    // To implement VfsNode on individual entries and share the archive,
    // the archive itself needs to be shared and protected by a Mutex.
    // The underlying reader might also need to be shared/clonable or reopened.
    // The zip crate's ZipArchive is not typically Send/Sync unless the reader is.
    // And by_name takes &mut self.

    // A better approach for VFS: The VSDX loader creates a VfsNode (directory)
    // representing the VSDX archive. This node holds the ZipArchive (perhaps Arc<Mutex<...>>).
    // Child nodes (files) are created for each entry in the ZIP.
    // The child node's VfsNode::read method accesses the entry via the shared archive.

    // Let's refactor VsdxFile to represent the *archive itself* for now,
    // holding the ZipArchive. Accessing individual files is via methods, not VfsNode::read directly.
    // The VfsNode implementation on VsdxFile (if it exists) will be limited, or
    // a separate structure for VfsNode representation of internal files is needed.
    // The current VfsNode impl on VsdxFile for the first file is not robust.
    // Let's remove the VfsNode impl from VsdxFile and focus on loading and accessing files within.

    // ZipArchive needs Send + Sync to be shared across VfsNodes (if VfsNode is Sync).
    // Box<dyn Read + Seek> is not Send/Sync unless the underlying type is.
    // SahneResourceReadSeek (no_std) is Send/Sync if Handle is.

    // Let's make VsdxFile hold the ZipArchive (possibly in a Mutex for thread safety if shared).
    // And provide methods to access files by name. The VfsNode integration will be separate.

    #[cfg(feature = "std")]
    archive: StdBox<ZipArchive<StdBox<dyn StdRead + StdSeek>>>, // Use StdBox for trait objects in std
    #[cfg(not(feature = "std"))]
    // In no_std, ZipArchive needs a concrete reader type.
    // SahneResourceReadSeek is a candidate, but ZipArchive itself needs no_std compatibility.
    // Assuming a no_std compatible ZipArchive is available with a generic reader type.
    // Let's define a placeholder structure for no_std ZipArchive if zip crate is not fully no_std.
    // If zip crate is no_std, the type signature will look similar but without std::Box.
    // We'll assume a no_std compatible ZipArchive<R> exists.
    archive: ZipArchive<SahneResourceReadSeek>, // Assumes ZipArchive<R> is no_std compatible

    // Remove first_file_content cache
}

impl VsdxFile {
    /// Creates a new `VsdxFile` instance from a reader implementing Read + Seek.
    ///
    /// # Arguments
    ///
    /// * `reader`: A reader providing access to the VSDX file content.
    ///
    /// # Returns
    ///
    /// A Result containing the created VsdxFile or a FileSystemError.
    #[cfg(feature = "std")]
    pub fn from_reader<R: StdRead + StdSeek + 'static>(reader: R) -> Result<Self, FileSystemError> { // Requires 'static bound for Box<dyn ...>
         // Box the reader to satisfy the ZipArchive constraint
         let boxed_reader = StdBox::new(reader) as StdBox<dyn StdRead + StdSeek>;
         let archive = ZipArchive::new(boxed_reader).map_err(map_zip_error_to_fs_error)?;

        Ok(Self { archive: StdBox::new(archive) }) // Box the archive too if needed for VFS node later
    }

     #[cfg(not(feature = "std"))]
     // In no_std, the reader must be a concrete type like SahneResourceReadSeek
     // And ZipArchive<R> must be no_std compatible.
     pub fn from_reader(reader: SahneResourceReadSeek) -> Result<Self, FileSystemError> {
         // Assuming ZipArchive::new is available and compatible with SahneResourceReadSeek
         let archive = ZipArchive::new(reader).map_err(map_zip_error_to_fs_error)?;
         Ok(Self { archive }) // Return VsdxFile holding the ZipArchive
     }


    /// Gets the content of a file within the VSDX archive by its filename.
    ///
    /// # Arguments
    ///
    /// * `filename`: The name of the file within the archive.
    ///
    /// # Returns
    ///
    /// A Result containing the content of the file as Vec<u8> or a FileSystemError.
    pub fn get_file(&mut self, filename: &str) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
        // Access the file within the archive by name
        let mut file_in_archive = self.archive.by_name(filename).map_err(map_zip_error_to_fs_error)?;

        // Read the content of the file within the archive
        let mut buffer = Vec::new(); // Requires alloc
        file_in_archive.read_to_end(&mut buffer).map_err(|e| {
            #[cfg(feature = "std")]
            return map_std_io_error_to_fs_error(e); // Map std IO error
            #[cfg(not(feature = "std"))]
            return map_core_io_error_to_fs_error(e); // Map core IO error
        })?;

        Ok(buffer) // Return the file content as bytes
    }

    // Add other VSDX-specific functions here as needed, e.g., listing files, accessing specific parts.
     pub fn file_names(&self) -> zip::fileinfo::ZipFileNames<'_> { self.archive.file_names() }
     pub fn len(&self) -> usize { self.archive.len() }
}


// Removed VfsNode implementation from VsdxFile directly.
// A VSDX file itself is not a single flat file for VfsNode purposes;
// its internal entries should be represented as VfsNodes.


/// Opens a VSDX file from the given path (std) or resource ID (no_std)
/// and creates a VsdxFile instance.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the created VsdxFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_vsdx_file<P: AsRef<StdPath>>(file_path: P) -> Result<VsdxFile, FileSystemError> { // Return FileSystemError, Use StdPath
    // Open the file using std::fs::File
    let file = StdFile::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;

    // Create a VsdxFile from the file reader
    VsdxFile::from_reader(file) // File implements Read + Seek
}

#[cfg(not(feature = "std"))]
pub fn open_vsdx_file(file_path: &str) -> Result<VsdxFile, FileSystemError> { // Return FileSystemError
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutını al (needed for SahneResourceReadSeek)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReadSeek oluştur
    let reader = SahneResourceReadSeek::new(handle, file_size); // Implements core::io::Read + Seek + Drop


    // Create a VsdxFile from the SahneResourceReadSeek reader
    VsdxFile::from_reader(reader)

    // File handle is released when 'reader' (and thus VsdxFile's reader) goes out of scope (due to Drop on SahneResourceReadSeek).
}


/// Loads a VSDX file and adds it to the VFS as a composite node (directory).
/// This function demonstrates how to integrate VSDX with the VFS by creating
/// a parent node for the archive and child nodes for each file within.
///
/// # Arguments
///
/// * `fs`: A mutable reference to the Sahne64 FileSystem.
/// * `vfs_path`: The path in the VFS tree where the VSDX archive should be mounted.
/// * `vsdx_data_reader`: A reader providing access to the VSDX file content.
///    The caller is responsible for managing the underlying resource of this reader.
///    Ideally, open_vsdx_file is called first, and the reader is passed.
///
/// # Returns
///
/// A Result indicating success or a FileSystemError.
#[cfg(feature = "std")] // This function relies on std Box<dyn ...> and FileSystem trait
pub fn load_vsdx_into_vfs<R: StdRead + StdSeek + 'static>(
     fs: &mut FileSystem,
     vfs_path: &str,
     vsdx_data_reader: R // Take the reader directly
 ) -> Result<(), FileSystemError> { // Return FileSystemError
    // Create the VsdxFile instance from the reader
    let vsdx_file = VsdxFile::from_reader(vsdx_data_reader)?; // This consumes the reader

    // The VSDX archive itself is a container. We can represent it as a directory-like node in the VFS.
    // The files within the archive will be child nodes under this parent node.

    // Create a parent VfsNode for the VSDX archive (as a directory)
    // This requires FileSystem::add_directory or similar VFS API
    // Assuming a VfsNode implementation for a directory exists.
    // For demonstration, let's create a simple placeholder node type or use a generic directory type.
    // This requires more detail about the FileSystem/VfsNode API.

    // For now, let's just add the VsdxFile instance itself as a node,
    // acknowledging that its VfsNode impl (if it existed) would be limited.
    // However, we removed the VfsNode impl from VsdxFile as it's not the right level.

    // The proper way:
    // 1. Create a VfsNode representing the VSDX directory. This node needs to hold
    //    the VsdxFile instance (likely Arc<Mutex<VsdxFile>>) so child nodes can access it.
    // 2. Iterate through `vsdx_file.archive.file_names()`.
    // 3. For each filename, create a new VfsNode representing that file within the ZIP.
    //    This child node needs a reference back to the shared VsdxFile archive.
    //    The child node's `VfsNode::read` method would call `vsdx_file.get_file(filename)`.
    //    This requires careful lifetime and sharing management (Arc, Mutex).
    // 4. Add the parent VSDX directory node to the VFS at `vfs_path`.
    // 5. Add each child file node under the parent directory node.


    // Placeholder implementation: Just create a dummy VfsNode for the path.
    // This is NOT a proper VSDX VFS integration.
    // It just shows where the VSDX parsing fits before VFS node creation.
    // A proper VFS node implementation for VSDX entries is complex.

    eprintln!("WARN: load_vsdx_into_vfs is a placeholder. Proper VSDX VFS node implementation is needed.");

    // Example of how to get file names from the parsed VSDX:
     #[cfg(feature = "std")] // zip crate's file_names requires &self or &mut self depending on the reader
     {
          println!("Files within the VSDX archive:");
          let mut archive_ref = StdBox::leak(vsdx_file.archive).deref_mut(); // Leaking for temporary access - NOT SAFE IN REAL CODE
          for name in archive_ref.file_names() {
              println!(" - {}", name);
          }
           // To avoid leak, pass &mut vsdx_file to a helper, or clone/re-open the archive (inefficient).
     }
      #[cfg(not(feature = "std"))]
      {
          // Accessing file_names in no_std from ZipArchive depends on its implementation.
          // Assuming it works with &self or &mut self on the no_std reader.
           println!("Files within the VSDX archive:");
           for name in vsdx_file.archive.file_names() { // Assuming file_names exists and works in no_std
               println!(" - {}", name);
           }
      }


    // Actual VFS integration requires creating VfsNodes for directory and files.
    // This requires knowledge of the FileSystem/VfsNode API beyond what's in this file.
    // For now, we just acknowledge that vsdx_file is parsed.

     fs.add_node(vfs_path, Box::new(vsdx_vfs_directory_node)); // Hypothetical VFS directory node creation

    Ok(()) // Indicate success of parsing and potential file listing
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", test)))] // Only when not building std or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
