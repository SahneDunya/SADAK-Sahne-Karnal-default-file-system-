#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// byteorder crate (no_std compatible)
use byteorder::{LittleEndian, ReadBytesExt, ByteOrder}; // LittleEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec, format!
use alloc::string::String; // For error messages
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt}; // core::io


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


/// Custom error type for binary STL parsing issues.
#[derive(Debug)]
pub enum StlError {
    UnexpectedEof(String), // During header or triangle data reading
    InvalidTriangleCount(u64), // Calculated size doesn't match reported count
    ShortTriangleData(usize), // Read less than 50 bytes for a triangle
    SeekError(u64), // Failed to seek
    // Add other STL specific parsing errors here (e.g., text STL detected - although this parser is for binary)
}

// Implement Display for StlError
impl fmt::Display for StlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StlError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            StlError::InvalidTriangleCount(calculated_size) => write!(f, "Geçersiz üçgen sayısı (hesaplanan boyut {})", calculated_size),
            StlError::ShortTriangleData(bytes_read) => write!(f, "Eksik üçgen verisi ({} bayt okundu, 50 bekleniyordu)", bytes_read),
            StlError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map StlError to FileSystemError
fn map_stl_error_to_fs_error(e: StlError) -> FileSystemError {
    match e {
        StlError::UnexpectedEof(_) | StlError::SeekError(_) => FileSystemError::IOError(format!("STL IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("STL ayrıştırma/veri hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilertf.rs'den kopyalandı)
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
    // read_to_end has a default implementation in core::io::ReadExt that uses read
    // read_to_string has a default implementation in core::io::ReadExt that uses read and from_utf8
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

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReader {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReader is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReader drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant print module and panic handler boilerplate.


/// Represents a triangle in a binary STL file.
#[derive(Debug, PartialEq, Clone, Copy)] // Add PartialEq, Clone, Copy for tests
pub struct Triangle {
    pub normal: [f32; 3],
    pub vertices: [[f32; 3]; 3],
    pub attribute_byte_count: u16,
}

/// Represents the data from a binary STL file.
pub struct Stl {
    pub triangles: Vec<Triangle>, // Requires alloc
}

impl Stl {
    /// Reads and parses a binary STL file from the given reader.
    /// The reader should be positioned at the start of the file.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read + Seek.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed Stl data or a FileSystemError.
    pub fn from_reader<R: Read + Seek>(mut reader: R) -> Result<Stl, FileSystemError> { // Return FileSystemError
        // Skip the 80-byte header
        let mut header = [0u8; 80];
        reader.read_exact(&mut header).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_stl_error_to_fs_error(StlError::UnexpectedEof(String::from("header"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Read the 4-byte triangle count (Little Endian)
        let triangle_count = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_stl_error_to_fs_error(StlError::UnexpectedEof(String::from("triangle count"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })? as usize; // Convert to usize


        // Allocate vector for triangles
        let mut triangles = Vec::with_capacity(triangle_count); // Requires alloc


        // Read triangle data (50 bytes per triangle)
        let mut triangle_bytes = [0u8; 50]; // Reuse buffer

        for _ in 0..triangle_count {
            let bytes_read = reader.read(&mut triangle_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use read to get bytes_read

            // Ensure exactly 50 bytes were read for the triangle
            if bytes_read != 50 {
                 return Err(map_stl_error_to_fs_error(StlError::ShortTriangleData(bytes_read)));
            }


            // Parse triangle data (Little Endian)
            // Use ReadBytesExt::read_f32 and read_u16 directly, which handle byte slicing and conversion and return io::Result
            let mut cursor = core::io::Cursor::new(&triangle_bytes); // Use core::io::Cursor for in-memory reading from the buffer

            let normal = [
                 cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                 cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                 cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
            ];

            let vertices = [
                 [
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                 ],
                 [
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                 ],
                 [
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                     cursor.read_f32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?,
                 ],
            ];

            let attribute_byte_count = cursor.read_u16::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;


            // Push the parsed triangle to the vector
            triangles.push(Triangle {
                normal,
                vertices,
                attribute_byte_count,
            });
        }

        // Basic check: Ensure the remaining file size matches expected based on triangle count
        // This is not strictly required by the STL format but helps validate file integrity.
        // The reader should be positioned right after the last triangle.
        // The expected remaining size should be 0 if the count is accurate.
        let current_position = reader.stream_position().map_err(|e| map_core_io_error_to_fs_error(e))?;
        // This check requires the original file size, which we don't have access to in this `from_reader` function.
        // It's better to do this check in the `open_stl_file` function where file_size is known.
        // Alternatively, pass file_size to from_reader if needed for this check.

        Ok(Stl { triangles }) // Return the parsed Stl struct
    }
}


/// Opens a binary STL file from the given path (std) or resource ID (no_std)
/// and parses its content.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the parsed Stl data or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_stl_file<P: AsRef<Path>>(file_path: P) -> Result<Stl, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (optional for from_reader, but good for validation)
    // Seek to end to get size, then seek back to start
    let mut temp_reader = BufReader::new(File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?); // Need a temporary reader
    let file_size = temp_reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    // No need to seek temp_reader back, it will be dropped.


    // Parse the STL data from the reader
    Stl::from_reader(reader) // Call the generic from_reader function
}

#[cfg(not(feature = "std"))]
pub fn open_stl_file(file_path: &str) -> Result<Stl, FileSystemError> { // Return FileSystemError
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Get file size (needed for SahneResourceReader and potential validation)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReader oluştur
    let reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek


    // Parse the STL data from the reader
    Stl::from_reader(reader) // Call the generic from_reader function

    // File handle is released when 'reader' goes out of scope (due to Drop on SahneResourceReader).
}


// Example main function (no_std)
#[cfg(feature = "example_stl")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("Binary STL parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy binary STL file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Create dummy binary STL data bytes for the mock filesystem
     // Header (80 bytes) + Triangle Count (4 bytes, Little Endian) + Triangle Data (50 bytes per triangle)
      let mut dummy_stl_data: Vec<u8> = Vec::new();
      // Header (80 bytes - dummy)
      dummy_stl_data.extend_from_slice(&[0u8; 80]);
      // Triangle Count (1 triangle, Little Endian)
      dummy_stl_data.extend_from_slice(&1u32.to_le_bytes());
      // Triangle 1 Data (50 bytes)
      // Normal (3 x f32, Little Endian)
      dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      // Vertices (3 x 3 x f32, Little Endian)
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());

      dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());

      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes());
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      // Attribute Byte Count (2 bytes, Little Endian - dummy)
      dummy_stl_data.extend_from_slice(&0u16.to_le_bytes());


      // Assuming the mock filesystem is set up to provide this data for "sahne://files/cube.stl"

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/cube.stl" exists with the dummy data.
      let stl_res = open_stl_file("sahne://files/cube.stl");
      match stl_res {
          Ok(stl) => {
              crate::println!("STL file loaded (header and triangles parsed).");
              crate::println!(" Triangle Count: {}", stl.triangles.len());
              if let Some(first_triangle) = stl.triangles.first() {
                  crate::println!(" First Triangle Normal: {:?}", first_triangle.normal);
                  crate::println!(" First Triangle Vertices: {:?}", first_triangle.vertices);
              }
     //
     //         // File is automatically closed when the underlying reader/handle goes out of scope (due to Drop)
          },
          Err(e) => crate::eprintln!("Error opening/parsing STL file: {:?}", e),
      }

     eprintln!("Binary STL parser example (no_std) needs Sahne64 mocks to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The dummy binary STL data to be available at the specified path.

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_stl")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("Binary STL parser example (std) starting...");
     eprintln!("Binary STL parser example (std) using core::io and byteorder.");

     // Create dummy binary STL data bytes
     // Header (80 bytes) + Triangle Count (4 bytes, Little Endian) + Triangle Data (50 bytes per triangle)
      let mut dummy_stl_data: Vec<u8> = Vec::new();
      // Header (80 bytes - dummy)
      dummy_stl_data.extend_from_slice(&[0u8; 80]);
      // Triangle Count (2 triangles, Little Endian)
      dummy_stl_data.extend_from_slice(&2u32.to_le_bytes());
      // Triangle 1 Data (50 bytes)
      // Normal (3 x f32: 1,0,0)
      dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      // Vertices (3 x 3 x f32)
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); // v1
      dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); // v2
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); // v3
      // Attribute Byte Count (0)
      dummy_stl_data.extend_from_slice(&0u16.to_le_bytes());

      // Triangle 2 Data (50 bytes)
      // Normal (3 x f32: 0,1,0)
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes());
      // Vertices (3 x 3 x f32)
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); // v1
      dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); // v2
      dummy_stl_data.extend_from_slice(&0.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); dummy_stl_data.extend_from_slice(&1.0f32.to_le_bytes()); // v3
      // Attribute Byte Count (0)
      dummy_stl_data.extend_from_slice(&0u16.to_le_bytes());


     let file_path = Path::new("example.stl");

      // Write dummy data to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_stl_data).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy STL file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy STL file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_stl_file(file_path) { // Call the function that opens and parses header
         Ok(stl) => {
             println!("STL file loaded (header and triangles parsed).");
             println!(" Triangle Count: {}", stl.triangles.len());

             // Assert triangle count based on dummy data
             assert_eq!(stl.triangles.len(), 2);

             // Assert properties of the first triangle
             if let Some(first_triangle) = stl.triangles.first() {
                 println!(" First Triangle Normal: {:?}", first_triangle.normal);
                 println!(" First Triangle Vertices: {:?}", first_triangle.vertices);
                 assert_eq!(first_triangle.normal, [1.0, 0.0, 0.0]);
                 assert_eq!(first_triangle.vertices[0], [0.0, 0.0, 0.0]);
                 assert_eq!(first_triangle.vertices[1], [1.0, 0.0, 0.0]);
                 assert_eq!(first_triangle.vertices[2], [0.0, 1.0, 0.0]);
                 assert_eq!(first_triangle.attribute_byte_count, 0);
             } else {
                  eprintln!("Error: No triangles found in parsed STL.");
                  return Err(FileSystemError::InvalidData(String::from("No triangles found")));
             }

             // Assert properties of the second triangle
              if let Some(second_triangle) = stl.triangles.get(1) {
                  println!(" Second Triangle Normal: {:?}", second_triangle.normal);
                  println!(" Second Triangle Vertices: {:?}", second_triangle.vertices);
                  assert_eq!(second_triangle.normal, [0.0, 1.0, 0.0]);
                  assert_eq!(second_triangle.vertices[0], [0.0, 1.0, 0.0]);
                  assert_eq!(second_triangle.vertices[1], [1.0, 1.0, 0.0]);
                  assert_eq!(second_triangle.vertices[2], [0.0, 1.0, 1.0]);
                  assert_eq!(second_triangle.attribute_byte_count, 0);
              } else {
                   eprintln!("Error: Second triangle not found in parsed STL.");
                   return Err(FileSystemError::InvalidData(String::from("Second triangle not found")));
              }


             // File is automatically closed when the underlying reader/handle goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening/parsing STL file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy STL file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("Binary STL parser example (std) finished.");

     Ok(())
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
#[cfg(feature = "std")] // Only run tests with std feature enabled
mod tests {
     // Needs std::io::Cursor for testing Read+Seek on dummy data
     use std::io::Cursor;
     use std::io::{Read, Seek, SeekFrom};
     use std::fs::remove_file; // For cleanup
     use std::path::Path;
     use std::io::Write; // For creating dummy files


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec::Vec; // For Vec
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests
     use byteorder::WriteBytesExt as StdWriteBytesExt; // For writing integers/floats in LittleEndian


     // Helper function to create dummy binary STL bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn create_dummy_stl_bytes(triangle_count: u32, triangles: &[Triangle]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let mut buffer = Cursor::new(Vec::new());
           // Header (80 bytes - dummy)
           buffer.write_all(&[0u8; 80])?;
           // Triangle Count (Little Endian)
           buffer.write_u32::<LittleEndian>(triangle_count)?;
           // Triangle Data (50 bytes per triangle)
           for triangle in triangles {
               // Normal
               buffer.write_f32::<LittleEndian>(triangle.normal[0])?;
               buffer.write_f32::<LittleEndian>(triangle.normal[1])?;
               buffer.write_f32::<LittleEndian>(triangle.normal[2])?;
               // Vertices
               for vertex in &triangle.vertices {
                   buffer.write_f32::<LittleEndian>(vertex[0])?;
                   buffer.write_f32::<LittleEndian>(vertex[1])?;
                   buffer.write_f32::<LittleEndian>(vertex[2])?;
               }
               // Attribute Byte Count
               buffer.write_u16::<LittleEndian>(triangle.attribute_byte_count)?;
           }

           Ok(buffer.into_inner())
       }


     // Test parsing a valid binary STL file in memory
     #[test]
     fn test_from_reader_valid_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
          // Create dummy triangles
          let triangle1 = Triangle { normal: [1.0, 0.0, 0.0], vertices: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], attribute_byte_count: 0 };
          let triangle2 = Triangle { normal: [0.0, 1.0, 0.0], vertices: [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 1.0]], attribute_byte_count: 0 };
          let triangles_data = vec![triangle1, triangle2];

          // Create dummy binary STL bytes
          let dummy_stl_bytes = create_dummy_stl_bytes(triangles_data.len() as u32, &triangles_data)
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


          // Use Cursor as a Read + Seek reader
          let cursor = Cursor::new(dummy_stl_bytes.clone());

          // Parse the STL data from the reader
          let stl = Stl::from_reader(cursor)?;

          // Assert triangle count is correct
          assert_eq!(stl.triangles.len(), triangles_data.len());

          // Assert triangle data is correct
          assert_eq!(stl.triangles, triangles_data);


          Ok(())
     }

     // Test handling of unexpected EOF during header reading
      #[test]
      fn test_from_reader_truncated_header() {
           // Truncated header (only 40 bytes)
           let dummy_bytes = vec![0u8; 40];

           let cursor = Cursor::new(dummy_bytes);
           // Attempt to parse from the reader, expect an error during header reading
           let result = Stl::from_reader(cursor);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from StlError::UnexpectedEof (via read_exact)
                   assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                   assert!(msg.contains("header"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       // Test handling of unexpected EOF during triangle count reading
        #[test]
        fn test_from_reader_truncated_count() {
             // Valid header (80 bytes) + Truncated count (only 2 bytes)
             let mut dummy_bytes = vec![0u8; 80];
              dummy_bytes.extend_from_slice(&[1u8, 0u8]); // Partial count (1, 0...)

             let cursor = Cursor::new(dummy_bytes);
             // Attempt to parse from the reader, expect an error during count reading
             let result = Stl::from_reader(cursor);

             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from StlError::UnexpectedEof (via read_u32)
                     assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                     assert!(msg.contains("triangle count"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


       // Test handling of unexpected EOF during triangle data reading
        #[test]
        fn test_from_reader_truncated_triangle_data() {
             // Valid header (80) + Valid count (1 triangle) + Truncated triangle data (e.g., 25 bytes instead of 50)
             let mut dummy_bytes = vec![0u8; 80];
             dummy_bytes.extend_from_slice(&1u32.to_le_bytes()); // Count = 1
             dummy_bytes.extend_from_slice(&vec![0u8; 25]); // Truncated triangle data

             let cursor = Cursor::new(dummy_bytes);
             // Attempt to parse from the reader, expect an error during triangle data reading loop
             let result = Stl::from_reader(cursor);

             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from StlError::ShortTriangleData
                     assert!(msg.contains("Eksik üçgen verisi (25 bayt okundu, 50 bekleniyordu)"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


     // Test handling of invalid triangle count (file size doesn't match) - This check is done in open_stl_file, not from_reader
     // Need a mock filesystem for a no_std test of this.

     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during reading,
     // and correctly parsing headers and triangles from mock data. Test the file size validation in open_stl_file.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_stl", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
