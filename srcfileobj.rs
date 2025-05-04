#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::collections::BTreeMap as StdBTreeMap; // Use std BTreeMap for std test lookup


// Gerekli Sahne64 modüllerini ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// byteorder crate (no_std compatible)
use byteorder::{LittleEndian, ReadBytesExt, ByteOrder}; // LittleEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec, BTreeMap
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use alloc::collections::BTreeMap; // Use BTreeMap from alloc


// core::result, core::option, core::str, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::str; // For from_utf8
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io


// Need no_std println!/eprintln! macros
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


/// Custom error type for object file parsing issues.
#[derive(Debug)]
pub enum ObjError {
    UnexpectedEof, // During header or length reading
    InvalidMagicNumber([u8; 4]),
    NameLengthExceeded(u32), // Name length > MAX_NAME_LENGTH
    DataLengthExceeded(u32), // Data length > MAX_DATA_LENGTH
    InvalidUtf8Name(core::str::Utf8Error),
    SeekError(u64), // Failed to seek to a specific position
    ObjectNotFound(String), // Object with given name not found
    InvalidObjectIndex(usize), // Object index out of bounds
    // Add other specific parsing errors here
}

// Implement Display for ObjError
impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjError::UnexpectedEof => write!(f, "Beklenmedik dosya sonu"),
            ObjError::InvalidMagicNumber(magic) => write!(f, "Geçersiz OBJ sihirli sayısı: {:x?}", magic),
            ObjError::NameLengthExceeded(len) => write!(f, "Nesne adı uzunluğu çok büyük: {}", len),
            ObjError::DataLengthExceeded(len) => write!(f, "Nesne veri uzunluğu çok büyük: {}", len),
            ObjError::InvalidUtf8Name(e) => write!(f, "Geçersiz UTF-8 nesne adı: {}", e),
            ObjError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
            ObjError::ObjectNotFound(name) => write!(f, "Nesne bulunamadı: {}", name),
            ObjError::InvalidObjectIndex(idx) => write!(f, "Geçersiz nesne indeksi: {}", idx),
        }
    }
}

// Helper function to map ObjError to FileSystemError
fn map_obj_error_to_fs_error(e: ObjError) -> FileSystemError {
    match e {
        ObjError::UnexpectedEof | ObjError::SeekError(_) => FileSystemError::IOError(format!("OBJ IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("OBJ ayrıştırma hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilemp4.rs'den kopyalandı)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
// fstat ile dosya boyutını alarak seek(End) desteği sağlar.
// Sahne64 API'sının bu syscall'ları Handle üzerinde sağladığı varsayılır.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu
}

#[cfg(not(feature = "std"))]
impl SahneResourceReader {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReader { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl core::io::Read for SahneResourceReader { // Use core::io::Read trait
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
}

#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReader { // Use core::io::Seek trait
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


// Removed the incomplete no_std collections::Vec and its use.
// Assuming alloc::vec::Vec is available.


/// Represents the metadata of an object within the object file.
/// Does NOT store the object's data by default.
#[derive(Debug)]
pub struct ObjObjectMetadata {
    pub name: String,
    pub data_size: u32,
    pub data_offset: u64, // File offset where the object's data begins
}


/// Parser for a custom object file format (.obj?).
/// Reads object metadata upon loading and provides methods to access object data on demand.
pub struct ObjFile<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks

    // Store object metadata, mapping name to metadata or in a Vec
    objects_metadata: BTreeMap<String, ObjObjectMetadata>, // Use BTreeMap for name lookup
    objects_order: Vec<String>, // Store names in order of appearance for index lookup
}

const OBJ_MAGIC_NUMBER: &[u8; 4] = b"OBJ\0";
const MAX_NAME_LENGTH: u32 = 256; // Maksimum nesne adı uzunluğu
const MAX_DATA_LENGTH: u32 = 1024 * 1024; // Maksimum veri uzunluğu (1MB)

impl<R: Read + Seek> ObjFile<R> {
    /// Creates a new `ObjFile` instance by reading the object metadata
    /// from the specified reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(mut reader: R, handle: Option<Handle>, file_size: u64) -> Result<Self, FileSystemError> { // Return FileSystemError

        // Check magic number
        let mut magic_number = [0u8; 4];
        reader.read_exact(&mut magic_number).map_err(|e| map_core_io_error_to_fs_error(e))?;
        if magic_number != *OBJ_MAGIC_NUMBER {
             return Err(map_obj_error_to_fs_error(ObjError::InvalidMagicNumber(magic_number)));
        }

        // Read object count
        let object_count = reader.read_u32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;

        let mut objects_metadata = BTreeMap::new(); // Use BTreeMap from alloc
        let mut objects_order = Vec::with_capacity(object_count as usize); // Use Vec from alloc

        // Read metadata for each object
        for _ in 0..object_count {
            // Read name length
            let name_length = reader.read_u32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;

            // Security check: Max name length
            if name_length > MAX_NAME_LENGTH {
                 return Err(map_obj_error_to_fs_error(ObjError::NameLengthExceeded(name_length)));
            }

            // Read object name bytes
            let mut name_buffer = vec![0u8; name_length as usize]; // Requires alloc
            reader.read_exact(&mut name_buffer).map_err(|e| map_core_io_error_to_fs_error(e))?;

            // Convert name bytes to String (UTF-8)
            let name = str::from_utf8(&name_buffer)
                 .map_err(|e| map_obj_error_to_fs_error(ObjError::InvalidUtf8Name(e)))? // Map Utf8Error to ObjError
                 .to_string(); // Requires alloc

            // Read data length
            let data_length = reader.read_u32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;

            // Security check: Max data length
            if data_length > MAX_DATA_LENGTH {
                 return Err(map_obj_error_to_fs_error(ObjError::DataLengthExceeded(data_length)));
            }

            // Store the current position as the data offset before skipping data
            let data_offset = reader.stream_position().map_err(|e| map_core_io_error_to_fs_error(e))?;


            // Skip object data
            if data_length > 0 {
                 reader.seek(SeekFrom::Current(data_length as i64)).map_err(|e| map_core_io_error_to_fs_error(e))?;
            }

            // Store object metadata
            let metadata = ObjObjectMetadata { name: name.clone(), data_size: data_length, data_offset };
            objects_metadata.insert(name.clone(), metadata); // Requires alloc (String clone)
            objects_order.push(name); // Requires alloc (String)
        }

        Ok(ObjFile { reader, handle, file_size, objects_metadata, objects_order })
    }

    /// Gets the metadata for an object by its name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the object.
    ///
    /// # Returns
    ///
    /// An Option containing a reference to the ObjObjectMetadata or None if not found.
    pub fn get_object_metadata(&self, name: &str) -> Option<&ObjObjectMetadata> {
        self.objects_metadata.get(name)
    }

    /// Gets the metadata for an object by its index (order of appearance in the file).
    ///
    /// # Arguments
    ///
    /// * `index` - The zero-based index of the object.
    ///
    /// # Returns
    ///
    /// A Result containing a reference to the ObjObjectMetadata or ObjError if index is out of bounds.
    pub fn get_object_metadata_by_index(&self, index: usize) -> Result<&ObjObjectMetadata, FileSystemError> { // Return FileSystemError
         if index >= self.objects_order.len() {
              return Err(map_obj_error_to_fs_error(ObjError::InvalidObjectIndex(index)));
         }
         let name = &self.objects_order[index];
         // Since objects_order comes from objects_metadata keys, this lookup should not fail.
         let metadata = self.objects_metadata.get(name).ok_or_else(|| {
             #[cfg(not(feature = "std"))]
             crate::eprintln!("WARN: Internal error: Object name found in order list but not in metadata map: {}", name);
              #[cfg(feature = "std")]
              eprintln!("WARN: Internal error: Object name found in order list but not in metadata map: {}", name);
             map_obj_error_to_fs_error(ObjError::ObjectNotFound(name.clone())) // Map to ObjectNotFound
         })?;
        Ok(metadata)
    }


    /// Reads the data for a specific object by its name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the object.
    ///
    /// # Returns
    ///
    /// A Result containing the object's data as Vec<u8> or FileSystemError.
    pub fn read_object_data(&mut self, name: &str) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
        let metadata = self.objects_metadata.get(name).ok_or_else(|| {
             map_obj_error_to_fs_error(ObjError::ObjectNotFound(String::from(name))) // Requires alloc
        })?;

        // Seek to the start of the object's data
        self.reader.seek(SeekFrom::Start(metadata.data_offset)).map_err(|e| map_core_io_error_to_fs_error(e))?;

        // Read the object's data
        let mut data = vec![0u8; metadata.data_size as usize]; // Requires alloc
         if metadata.data_size > 0 { // Avoid reading 0 bytes and triggering read_exact on empty buffer
            self.reader.read_exact(&mut data).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use read_exact
         }


        Ok(data)
    }

    /// Reads the data for a specific object by its index.
    ///
    /// # Arguments
    ///
    /// * `index` - The zero-based index of the object.
    ///
    /// # Returns
    ///
    /// A Result containing the object's data as Vec<u8> or FileSystemError.
     pub fn read_object_data_by_index(&mut self, index: usize) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
         let metadata = self.get_object_metadata_by_index(index)?; // Use the existing method

         // Seek to the start of the object's data
         self.reader.seek(SeekFrom::Start(metadata.data_offset)).map_err(|e| map_core_io_error_to_fs_error(e))?;

         // Read the object's data
         let mut data = vec![0u8; metadata.data_size as usize]; // Requires alloc
         if metadata.data_size > 0 { // Avoid reading 0 bytes
            self.reader.read_exact(&mut data).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use read_exact
         }

         Ok(data)
     }

    /// Gets the total number of objects in the file.
    pub fn object_count(&self) -> usize {
        self.objects_order.len() // Or self.objects_metadata.len()
    }

     /// Provides a reference to the internal reader (use with caution).
     pub fn reader(&mut self) -> &mut R {
         &mut self.reader
     }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for ObjFile<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the ObjFile is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: ObjFile drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens an object file from the given path (std) or resource ID (no_std)
/// and creates an ObjFile instance by reading object metadata.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the ObjFile or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_obj_file<P: AsRef<Path>>(file_path: P) -> Result<ObjFile<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Create ObjFile by parsing metadata from the reader
    ObjFile::from_reader(reader, None, file_size) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_obj_file(file_path: &str) -> Result<ObjFile<SahneResourceReader>, FileSystemError> { // Return FileSystemError
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutunu al (SahneResourceReader için gerekli)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReader oluştur
    let reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek

    // Create ObjFile by parsing metadata from the reader
    ObjFile::from_reader(reader, Some(handle), file_size) // Pass the handle to the ObjFile
}


// Example main function (no_std)
#[cfg(feature = "example_obj")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("Object file parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy object file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/data.obj" exists.
      let obj_file_res = open_obj_file("sahne://files/data.obj");
      match obj_file_res {
          Ok(mut obj_file) => { // Need mut to read data
              crate::println!("Object file loaded with {} objects.", obj_file.object_count());
     //
     //         // Example: Read data of an object by name
              match obj_file.read_object_data("mesh_data") {
                  Ok(data) => {
                      crate::println!("Read {} bytes for object 'mesh_data'.", data.len());
     //                 // Process object data here
                  },
                  Err(e) => crate::eprintln!("Error reading object data: {:?}", e),
              }
     //
     //         // Example: Iterate through objects by index
              for i in 0..obj_file.object_count() {
                  match obj_file.get_object_metadata_by_index(i) {
                      Ok(metadata) => {
                          crate::println!("Object {}: Name: {}, Size: {}, Offset: {}",
                              i, metadata.name, metadata.data_size, metadata.data_offset);
                      },
                      Err(e) => crate::eprintln!("Error getting object metadata by index {}: {:?}", i, e),
                  }
              }
     
              // File is automatically closed when obj_file goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening object file: {:?}", e),
      }

     eprintln!("Object file parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_obj")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("Object file parser example (std) starting...");
     eprintln!("Object file parser example (std) using on-demand data reading.");

     // This example needs a dummy object file.
     use std::fs::remove_file;
     use std::io::Write;
     use byteorder::LittleEndian as StdLittleEndian;
     use byteorder::WriteBytesExt as StdWriteBytesExt;


     let obj_path = Path::new("example.obj");

     // Create a dummy object file structure: Magic, count, then name/data pairs.
      let mut dummy_data_cursor = Cursor::new(Vec::new());
       dummy_data_cursor.write_all(b"OBJ\0").unwrap(); // Magic number
       dummy_data_cursor.write_u32::<StdLittleEndian>(2).unwrap(); // 2 objects

       // Object 1: Name "mesh", Data "binary mesh data..."
       let name1 = "mesh";
       let data1 = b"binary mesh data for obj1...";
       dummy_data_cursor.write_u32::<StdLittleEndian>(name1.len() as u32).unwrap(); // Name length
       dummy_data_cursor.write_all(name1.as_bytes()).unwrap(); // Name bytes
       dummy_data_cursor.write_u32::<StdLittleEndian>(data1.len() as u32).unwrap(); // Data length
       dummy_data_cursor.write_all(data1).unwrap(); // Data bytes

       // Object 2: Name "texture", Data [0u8; 100]
       let name2 = "texture";
       let data2 = vec![0u8; 100];
       dummy_data_cursor.write_u32::<StdLittleEndian>(name2.len() as u32).unwrap(); // Name length
       dummy_data_cursor.write_all(name2.as_bytes()).unwrap(); // Name bytes
       dummy_data_cursor.write_u32::<StdLittleEndian>(data2.len() as u32).unwrap(); // Data length
       dummy_data_cursor.write_all(&data2).unwrap(); // Data bytes


       let dummy_data = dummy_data_cursor.into_inner();


       // Write dummy data to a temporary file for std test
        match File::create(obj_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_data) {
                       eprintln!("Error writing dummy OBJ file: {}", e);
                       return Err(map_std_io_error_to_fs_error(e));
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy OBJ file: {}", e);
                  return Err(map_std_io_error_to_fs_error(e));
             }
        }


     match open_obj_file(obj_path) { // Call the function that opens and creates the parser
         Ok(mut obj_file) => { // Need mut to read data
             println!("Object file loaded with {} objects.", obj_file.object_count());

             // Example: Read data of an object by name
             match obj_file.read_object_data("mesh") {
                 Ok(data) => {
                     println!("Read {} bytes for object 'mesh'.", data.len());
                      // Verify the data
                      assert_eq!(data, data1);
                 },
                 Err(e) => {
                      eprintln!("Error reading object data 'mesh': {}", e); // std error display
                      // Don't return error, let cleanup run
                 }
             }

              // Example: Read data of an object by index
               match obj_file.read_object_data_by_index(1) { // Index 1 is "texture"
                  Ok(data) => {
                      println!("Read {} bytes for object at index 1 ('texture').", data.len());
                       // Verify the data
                       assert_eq!(data, data2);
                  },
                  Err(e) => {
                       eprintln!("Error reading object data by index 1: {}", e); // std error display
                       // Don't return error, let cleanup run
                  }
              }

             // Example: Get metadata and print
             for i in 0..obj_file.object_count() {
                 match obj_file.get_object_metadata_by_index(i) {
                     Ok(metadata) => {
                         println!("Object {}: Name: {}, Size: {}, Offset: {}",
                             i, metadata.name, metadata.data_size, metadata.data_offset);
                     },
                      Err(e) => {
                          eprintln!("Error getting object metadata by index {}: {}", i, e);
                      }
                 }
             }


             // File is automatically closed when obj_file goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening object file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(obj_path) {
          eprintln!("Error removing dummy OBJ file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("Object file parser example (std) finished.");

     Ok(())
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
mod tests {
     // Needs std::io::Cursor for testing Read+Seek on dummy data
     #[cfg(feature = "std")]
     use std::io::Cursor;
     #[cfg(feature = "std")]
     use std::io::{Read, Seek, SeekFrom};
      #[cfg(feature = "std")]
      use std::fs::remove_file; // For cleanup
      #[cfg(feature = "std")]
      use std::path::Path;
      #[cfg(feature = "std")]
      use std::io::Write; // For creating dummy files
      #[cfg(feature = "std")]
      use byteorder::{LittleEndian as StdLittleEndian, WriteBytesExt as StdWriteBytesExt}; // Use std byteorder for writing test data


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests
     use alloc::vec::Vec as AllocVec; // Use AllocVec explicitly where needed


     // Helper function to create dummy OBJ file bytes in memory
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn create_dummy_obj_file_bytes(objects_data: &[(String, AllocVec<u8>)]) -> Result<AllocVec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(AllocVec::new());
           buffer.write_all(b"OBJ\0")?; // Magic number
           buffer.write_u32::<StdLittleEndian>(objects_data.len() as u32)?; // Object count

           for (name, data) in objects_data {
               let name_bytes = name.as_bytes();
               let name_len = name_bytes.len();
               let data_len = data.len();

               // Name length
               if name_len > MAX_NAME_LENGTH as usize { return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Object name length exceeds limit: {}", name_len)))); }
                buffer.write_u32::<StdLittleEndian>(name_len as u32)?;
               buffer.write_all(name_bytes)?;

               // Data length
                if data_len > MAX_DATA_LENGTH as usize { return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Object data length exceeds limit: {}", data_len)))); }
               buffer.write_u32::<StdLittleEndian>(data_len as u32)?;
               buffer.write_all(data)?; // Write data
           }

           Ok(buffer.into_inner())
       }


     // Test opening and parsing metadata from a valid OBJ file
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_open_obj_file_parse_metadata() -> Result<(), FileSystemError> { // Return FileSystemError

          // Create dummy OBJ file data with two objects
           let objects_data = vec![
               (String::from("obj1"), vec![1, 2, 3, 4]),
               (String::from("another_obj"), vec![5, 6, 7]),
           ];

          let dummy_obj_bytes = create_dummy_obj_file_bytes(&objects_data)
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;

          // Use Cursor as a reader
          let file_size = dummy_obj_bytes.len() as u64;
          let mut cursor = Cursor::new(dummy_obj_bytes.clone()); // Clone for potential re-reads in test

          // Create a dummy ObjFile by calling from_reader directly
          let obj_file = ObjFile::from_reader(cursor, None, file_size)?; // Pass None for handle

          // Assert metadata is parsed correctly
          assert_eq!(obj_file.object_count(), 2);

          // Check metadata for obj1
          let metadata1 = obj_file.get_object_metadata("obj1").expect("obj1 should exist");
          assert_eq!(metadata1.name, "obj1");
          assert_eq!(metadata1.data_size, 4);
          // Calculate expected offset: Magic (4) + Count (4) + obj1_name_len (4) + obj1_name (4) + obj1_data_len (4)
          assert_eq!(metadata1.data_offset, (4 + 4 + 4 + "obj1".as_bytes().len() + 4) as u64); // 4 + 4 + 4 + 4 + 4 = 20

          // Check metadata for another_obj
          let metadata2 = obj_file.get_object_metadata("another_obj").expect("another_obj should exist");
          assert_eq!(metadata2.name, "another_obj");
          assert_eq!(metadata2.data_size, 3);
          // Calculate expected offset: Previous offset + previous data size + obj2_name_len (4) + obj2_name (11) + obj2_data_len (4)
          assert_eq!(metadata2.data_offset, metadata1.data_offset + metadata1.data_size as u64 + 4 + "another_obj".as_bytes().len() as u64 + 4); // 20 + 4 + 4 + 11 + 4 = 43

          // Check object order and metadata by index
          assert_eq!(obj_file.get_object_metadata_by_index(0)?.name, "obj1");
          assert_eq!(obj_file.get_object_metadata_by_index(1)?.name, "another_obj");
          let result = obj_file.get_object_metadata_by_index(2);
          assert!(result.is_err()); // Index out of bounds


          Ok(())
     }

     // Test reading object data by name
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_read_object_data_by_name() -> Result<(), FileSystemError> { // Return FileSystemError

           // Create dummy OBJ file data with two objects
           let objects_data = vec![
               (String::from("obj1"), vec![1, 2, 3, 4]),
               (String::from("another_obj"), vec![5, 6, 7]),
           ];

          let dummy_obj_bytes = create_dummy_obj_file_bytes(&objects_data)
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;

          // Use Cursor as a reader
          let file_size = dummy_obj_bytes.len() as u64;
          let cursor = Cursor::new(dummy_obj_bytes.clone());

           // Create a dummy ObjFile
           let mut obj_file = ObjFile::from_reader(cursor, None, file_size)?; // Need mut to read data

           // Read data for obj1
           let data1 = obj_file.read_object_data("obj1")?;
           assert_eq!(data1, vec![1, 2, 3, 4]);

            // Read data for another_obj
            let data2 = obj_file.read_object_data("another_obj")?;
            assert_eq!(data2, vec![5, 6, 7]);

            // Attempt to read non-existent object
            let result = obj_file.read_object_data("non_existent");
            assert!(result.is_err());
            match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from ObjError::ObjectNotFound
                     assert!(msg.contains("Nesne bulunamadı: non_existent"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }


           Ok(())
      }


     // Test reading object data by index
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_read_object_data_by_index() -> Result<(), FileSystemError> { // Return FileSystemError

           // Create dummy OBJ file data with two objects
           let objects_data = vec![
               (String::from("obj1"), vec![1, 2, 3, 4]),
               (String::from("another_obj"), vec![5, 6, 7]),
           ];

          let dummy_obj_bytes = create_dummy_obj_file_bytes(&objects_data)
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;

          // Use Cursor as a reader
          let file_size = dummy_obj_bytes.len() as u64;
          let cursor = Cursor::new(dummy_obj_bytes.clone());

           // Create a dummy ObjFile
           let mut obj_file = ObjFile::from_reader(cursor, None, file_size)?; // Need mut to read data

           // Read data by index 0
           let data1 = obj_file.read_object_data_by_index(0)?;
           assert_eq!(data1, vec![1, 2, 3, 4]);

            // Read data by index 1
            let data2 = obj_file.read_object_data_by_index(1)?;
            assert_eq!(data2, vec![5, 6, 7]);

            // Attempt to read invalid index
            let result = obj_file.read_object_data_by_index(2);
            assert!(result.is_err());
            match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from ObjError::InvalidObjectIndex
                     assert!(msg.contains("Geçersiz nesne indeksi: 2"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }


           Ok(())
      }


     // Test handling of invalid magic number
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_open_obj_file_invalid_magic() {
           // Create dummy data with invalid magic number
           let mut dummy_data_cursor = Cursor::new(Vec::new());
           dummy_data_cursor.write_all(b"BAD\0").unwrap(); // Invalid magic
           let dummy_data = dummy_data_cursor.into_inner();

           let file_size = dummy_data.len() as u64;
           let cursor = Cursor::new(dummy_data);

           // Attempt to open/load, expect an error
           let result = ObjFile::from_reader(cursor, None, file_size);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from ObjError::InvalidMagicNumber
                   assert!(msg.contains("Geçersiz OBJ sihirli sayısı"));
                   assert!(msg.contains("42414400")); // Hex representation of "BAD\0"
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of truncated file during header reading (magic, count, lengths)
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_open_obj_file_truncated_header() {
            // Truncated magic (3 bytes)
            let dummy_data = b"OBJ".to_vec();
            let file_size = dummy_data.len() as u64;
            let cursor = Cursor::new(dummy_data);
            let result = ObjFile::from_reader(cursor, None, file_size);
            assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }


            // Valid magic, truncated count (2 bytes)
            let mut dummy_data_cursor = Cursor::new(Vec::new());
            dummy_data_cursor.write_all(b"OBJ\0").unwrap();
            dummy_data_cursor.write_u16::<StdLittleEndian>(5).unwrap(); // Only 2 bytes for count
            let dummy_data = dummy_data_cursor.into_inner(); // 4 + 2 = 6 bytes total
            let file_size = dummy_data.len() as u64;
            let cursor = Cursor::new(dummy_data);
            let result = ObjFile::from_reader(cursor, None, file_size);
            assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_u32)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }

            // Valid header, truncated name length (2 bytes)
            let mut dummy_data_cursor = Cursor::new(Vec::new());
             dummy_data_cursor.write_all(b"OBJ\0").unwrap();
             dummy_data_cursor.write_u32::<StdLittleEndian>(1).unwrap(); // 1 object
             dummy_data_cursor.write_u16::<StdLittleEndian>(10).unwrap(); // Only 2 bytes for name length (should be 4)
             let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 2 = 10 bytes total
             let file_size = dummy_data.len() as u64;
             let cursor = Cursor::new(dummy_data);
             let result = ObjFile::from_reader(cursor, None, file_size);
             assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_u32)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }

             // Valid header, valid name length, truncated name bytes
             let mut dummy_data_cursor = Cursor::new(Vec::new());
              dummy_data_cursor.write_all(b"OBJ\0").unwrap();
              dummy_data_cursor.write_u32::<StdLittleEndian>(1).unwrap(); // 1 object
              dummy_data_cursor.write_u32::<StdLittleEndian>(10).unwrap(); // Name length 10
              dummy_data_cursor.write_all(b"short").unwrap(); // Only 5 bytes for name (should be 10)
              let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 4 + 5 = 17 bytes total
              let file_size = dummy_data.len() as u64;
              let cursor = Cursor::new(dummy_data);
              let result = ObjFile::from_reader(cursor, None, file_size);
              assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }

       }

       // Test handling of name/data length exceeding limits
        #[test]
        #[cfg(feature = "std")] // Run this test only with std feature
        fn test_open_obj_file_length_limits() {
             // Name length exceeds limit
             let mut dummy_data_cursor = Cursor::new(Vec::new());
              dummy_data_cursor.write_all(b"OBJ\0").unwrap();
              dummy_data_cursor.write_u32::<StdLittleEndian>(1).unwrap(); // 1 object
              dummy_data_cursor.write_u32::<StdLittleEndian>(MAX_NAME_LENGTH + 1).unwrap(); // Name length > limit
              // Don't need to write name bytes, the check should happen after reading length.
              let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 4 = 12 bytes total
              let file_size = dummy_data.len() as u64;
              let cursor = Cursor::new(dummy_data);
              let result = ObjFile::from_reader(cursor, None, file_size);
              assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from ObjError::NameLengthExceeded
                     assert!(msg.contains("Nesne adı uzunluğu çok büyük"));
                      assert!(msg.contains(&format!("{}", MAX_NAME_LENGTH + 1)));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }

              // Data length exceeds limit
              let mut dummy_data_cursor = Cursor::new(Vec::new());
               dummy_data_cursor.write_all(b"OBJ\0").unwrap();
               dummy_data_cursor.write_u32::<StdLittleEndian>(1).unwrap(); // 1 object
               let name = "short_name";
               dummy_data_cursor.write_u32::<StdLittleEndian>(name.len() as u32).unwrap(); // Valid name length
               dummy_data_cursor.write_all(name.as_bytes()).unwrap();
               dummy_data_cursor.write_u32::<StdLittleEndian>(MAX_DATA_LENGTH + 1).unwrap(); // Data length > limit
               // Don't need to write data bytes.
              let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 4 + 10 + 4 = 26 bytes total
              let file_size = dummy_data.len() as u64;
              let cursor = Cursor::new(dummy_data);
              let result = ObjFile::from_reader(cursor, None, file_size);
              assert!(result.is_err());
              match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from ObjError::DataLengthExceeded
                     assert!(msg.contains("Nesne veri uzunluğu çok büyük"));
                      assert!(msg.contains(&format!("{}", MAX_DATA_LENGTH + 1)));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
              }
        }

      // Test handling of invalid UTF-8 name
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_open_obj_file_invalid_utf8_name() {
            // Create dummy data with valid header and lengths, but invalid UTF-8 in name bytes
            let mut dummy_data_cursor = Cursor::new(Vec::new());
             dummy_data_cursor.write_all(b"OBJ\0").unwrap();
             dummy_data_cursor.write_u32::<StdLittleEndian>(1).unwrap(); // 1 object
             dummy_data_cursor.write_u32::<StdLittleEndian>(3).unwrap(); // Name length 3
             dummy_data_cursor.write_all(&[0xFF, 0xFF, 0xFF]).unwrap(); // Invalid UTF-8 bytes
             dummy_data_cursor.write_u32::<StdLittleEndian>(0).unwrap(); // Data length 0 (doesn't matter)
             let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 4 + 3 + 4 = 19 bytes total
             let file_size = dummy_data.len() as u64;
             let cursor = Cursor::new(dummy_data);
             let result = ObjFile::from_reader(cursor, None, file_size);
             assert!(result.is_err());
             match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from ObjError::InvalidUtf8Name
                     assert!(msg.contains("Geçersiz UTF-8 nesne adı"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
       }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include opening valid/invalid files, handling IO errors,
     // and correctly parsing object metadata from mock data.
     // Test reading object data from mock data.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_obj", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
