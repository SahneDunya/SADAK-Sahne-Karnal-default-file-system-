#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

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


// Sahne64 fonksiyonlarını kullanmak için bu modülü içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec; // For read buffer
use alloc::format;


// core::result, core::option, core::str, core::fmt, core::cmp, core::ops::Drop
use core::result::Result;
use core::option::Option;
use core::str; // For from_utf8_lossy or from_utf8
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait


// core::io traits and types needed for SahneResourceReader (if used)
#[cfg(not(feature = "std"))]
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


/// Custom error type for MOV atom parsing issues.
#[derive(Debug)]
pub enum MovError {
    UnexpectedEof, // During atom header reading
    InvalidAtomSize(u32), // Atom size is zero or too small
    InvalidAtomTypeEncoding(core::str::Utf8Error), // Atom type bytes are not valid UTF8
    SeekError(u64), // Failed to seek to a specific position
    // Add other MOV specific parsing errors here (e.g., unknown atom type, invalid structure)
}

// Implement Display for MovError
impl fmt::Display for MovError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MovError::UnexpectedEof => write!(f, "Beklenmedik dosya sonu"),
            MovError::InvalidAtomSize(size) => write!(f, "Geçersiz atom boyutu: {}", size),
            MovError::InvalidAtomTypeEncoding(e) => write!(f, "Atom tipi UTF8 hatası: {}", e),
            MovError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
        }
    }
}

// Helper function to map MovError to FileSystemError
fn map_mov_error_to_fs_error(e: MovError) -> FileSystemError {
    match e {
        MovError::UnexpectedEof => FileSystemError::IOError(format!("Beklenmedik dosya sonu")), // Map parsing EOF to IO Error
        MovError::InvalidAtomSize(size) => FileSystemError::InvalidData(format!("Geçersiz atom boyutu: {}", size)),
        MovError::InvalidAtomTypeEncoding(e) => FileSystemError::InvalidData(format!("Atom tipi UTF8 hatası: {}", e)),
        MovError::SeekError(pos) => FileSystemError::IOError(format!("Seek hatası pozisyon: {}", pos)), // Map seek errors to IO Error
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilemkv.rs'den kopyalandı)
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


/// Basic MOV (QuickTime) file parser.
/// Focuses on iterating through top-level atoms.
/// Does NOT fully parse the entire MOV structure or media data.
pub struct MovParser<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks
}

impl<R: Read + Seek> MovParser<R> {
    /// Creates a new `MovParser` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        Self { reader, handle, file_size }
    }

    /// Parses the top-level atoms in the MOV file.
    /// Iterates through atoms (size + type) and skips data.
    /// Does NOT parse nested atoms (except for a basic loop in parse_moov_atom).
    pub fn parse(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
        // Atoms have a standard structure: 4-byte size (Big Endian) + 4-byte type + data
        // File starts with the first atom header.

        let mut current_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?; // Get initial position

        loop {
            // Check if we are at or beyond the file size
            if current_pos >= self.file_size {
                 #[cfg(not(feature = "std"))]
                 crate::println!("Dosya sonuna ulaşıldı.");
                 #[cfg(feature = "std")]
                 println!("Dosya sonuna ulaşıldı.");
                break; // Reached end of file
            }

            // Read atom size (4 bytes, Big Endian)
            let atom_size_result = self.reader.read_u32::<BigEndian>(); // Use byteorder::ReadBytesExt
            let atom_size = match atom_size_result {
                 Ok(size) => size as u64, // Convert to u64 for consistency with Seek
                 Err(e) => {
                      // If EOF during size reading, check if it's exactly 0 bytes left
                      if e.kind() == core::io::ErrorKind::UnexpectedEof && self.file_size == current_pos {
                          #[cfg(not(feature = "std"))]
                          crate::println!("Dosya sonuna ulaşıldı (size okurken).");
                          #[cfg(feature = "std")]
                          println!("Dosya sonuna ulaşıldı (size okurken).");
                          break; // Clean EOF
                      }
                      return Err(map_core_io_error_to_fs_error(e)); // Other IO errors
                 }
            };

            // Minimum atom size is 8 bytes (4 for size, 4 for type)
            if atom_size < 8 {
                 // If size is 0, it might be padding or invalid. If > 0 and < 8, it's invalid.
                  if atom_size == 0 {
                      // Treat size 0 as invalid or padding, depending on context.
                      // Assuming invalid for now based on QuickTime spec.
                       return Err(map_mov_error_to_fs_error(MovError::InvalidAtomSize(0)));
                  } else {
                       return Err(map_mov_error_to_fs_error(MovError::InvalidAtomSize(atom_size as u32)));
                  }
            }

            // Read atom type (4 bytes)
            let mut atom_type_bytes = [0u8; 4];
            self.reader.read_exact(&mut atom_type_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use read_exact

            // Convert atom type bytes to String for printing (lossy conversion for potentially non-UTF8 types)
            let atom_type_string = str::from_utf8_lossy(&atom_type_bytes); // Requires alloc

            #[cfg(not(feature = "std"))]
            crate::println!("Atom Type: {}, Size: {} bayt", atom_type_string, atom_size);
            #[cfg(feature = "std")]
            println!("Atom Type: {}, Size: {} bayt", atom_type_string, atom_size);

            // Process known top-level atom types
            // The data for the atom follows the 8-byte header (size + type).
            // The size includes the header itself. So data size is atom_size - 8.
            let data_size = atom_size.checked_sub(8).ok_or_else(|| map_mov_error_to_fs_error(MovError::InvalidAtomSize(atom_size as u32)))?;

            match &atom_type_bytes { // Compare directly with byte slices
                b"moov" => self.parse_moov_atom(data_size)?,
                b"mdat" => self.parse_mdat_atom(data_size)?,
                // Add other top-level atoms if needed (e.g., 'ftyp')
                _ => {
                    // Skip unknown atom data
                    self.reader.seek(SeekFrom::Current(data_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Use reader.seek
                }
            }

            // Move to the start of the next atom
            current_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?; // Update current position
        }
        Ok(())
    }

    /// Parses the content of a 'moov' atom (Movie Atom).
    /// Recursively iterates through nested atoms within the 'moov' atom's data.
    /// `moov_data_size` is the size of the data part of the moov atom (excluding its 8-byte header).
    fn parse_moov_atom(&mut self, moov_data_size: u64) -> Result<(), FileSystemError> { // Return FileSystemError
        let moov_end_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?
                            .checked_add(moov_data_size)
                            .ok_or_else(|| map_mov_error_to_fs_error(MovError::SeekError(self.reader.stream_position().unwrap_or(0))))?; // Calculate end position of moov data

        // Iterate through atoms within the moov atom's data
        while self.reader.stream_position().map_err(map_core_io_error_to_fs_error)? < moov_end_pos {
            let current_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?;

            // Read nested atom size (4 bytes, Big Endian)
            let nested_atom_size_result = self.reader.read_u32::<BigEndian>(); // Use byteorder
             let nested_atom_size = match nested_atom_size_result {
                 Ok(size) => size as u64, // Convert to u64
                 Err(e) => {
                     // If EOF during size reading, check if we are exactly at moov_end_pos
                      if e.kind() == core::io::ErrorKind::UnexpectedEof && current_pos == moov_end_pos {
                          break; // Clean EOF within moov atom
                      }
                      return Err(map_core_io_error_to_fs_error(e)); // Other IO errors
                 }
             };

            // Minimum nested atom size is 8 bytes
            if nested_atom_size < 8 {
                 if nested_atom_size == 0 {
                       // Treat size 0 as invalid within moov
                        return Err(map_mov_error_to_fs_error(MovError::InvalidAtomSize(0)));
                   } else {
                       return Err(map_mov_error_to_fs_error(MovError::InvalidAtomSize(nested_atom_size as u32)));
                   }
            }

            // Read nested atom type (4 bytes)
            let mut nested_atom_type_bytes = [0u8; 4];
            self.reader.read_exact(&mut nested_atom_type_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?;

            // Convert type to String for printing
            let nested_atom_type_string = str::from_utf8_lossy(&nested_atom_type_bytes); // Requires alloc

            #[cfg(not(feature = "std"))]
            crate::println!("    Atom: {}, Size: {} bayt", nested_atom_type_string, nested_atom_size);
            #[cfg(feature = "std"))]
            println!("    Atom: {}, Size: {} bayt", nested_atom_type_string, nested_atom_size);


            // Process known nested atom types or skip data
            let nested_atom_data_size = nested_atom_size.checked_sub(8).ok_or_else(|| map_mov_error_to_fs_error(MovError::InvalidAtomSize(nested_atom_size as u32)))?;

            match &nested_atom_type_bytes {
                b"trak" => {
                     // Recursively parse 'trak' atom content if needed, or skip data
                     // For now, just skip the data
                     self.reader.seek(SeekFrom::Current(nested_atom_data_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Use reader.seek
                },
                // Add other nested atom types if needed (e.g., 'mdia', 'minf', 'stbl')
                _ => {
                     // Skip unknown nested atom data
                     self.reader.seek(SeekFrom::Current(nested_atom_data_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Use reader.seek
                }
            }
            // After processing a nested atom, the reader should be at the start of the next nested atom.
            // The loop condition checks if we are still within the moov_end_pos.
        }

        // After the loop, the reader should be exactly at moov_end_pos.
        // If not, there's a parsing issue or truncated moov data.
        let final_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
         if final_pos != moov_end_pos {
              eprintln!("WARN: MOOV atomu ayrıştırma sonrası beklenmeyen pozisyon. Beklenen: {}, Gerçek: {}", moov_end_pos, final_pos);
              // Depending on strictness, this could be an error
              // return Err(map_mov_error_to_fs_error(MovError::InvalidAtomSize(moov_data_size as u32))); // Or a specific MkvError for structure mismatch
         }

        Ok(())
    }

    /// Handles skipping the data of an 'mdat' atom (Media Data Atom).
    /// `mdat_data_size` is the size of the data part of the mdat atom.
    fn parse_mdat_atom(&mut self, mdat_data_size: u64) -> Result<(), FileSystemError> { // Return FileSystemError
        // 'mdat' atom contains the actual media data. Parsing this requires codecs.
        // For this basic parser, we just skip the data.
         #[cfg(not(feature = "std"))]
         crate::println!("  MDAT atomu bulundu (boyut: {} bayt). Atlaniyor.", mdat_data_size);
         #[cfg(feature = "std"))]
         println!("  MDAT atomu bulundu (boyut: {} bayt). Atlaniyor.", mdat_data_size);

        self.reader.seek(SeekFrom::Current(mdat_data_size as i64)).map_err(map_core_io_error_to_fs_error)?; // Use reader.seek
        Ok(())
    }

    // Removed custom read_exact and seek methods.
    // These functionalities are provided by the R: Read + Seek trait on self.reader.
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for MovParser<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the parser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: MovParser drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens a MOV file from the given path (std) or resource ID (no_std)
/// and creates a basic MovParser.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the MovParser or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_mov_file<P: AsRef<Path>>(file_path: P) -> Result<MovParser<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (required by MkvParser constructor)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    Ok(MovParser::from_reader(reader, None, file_size)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_mov_file(file_path: &str) -> Result<MovParser<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    Ok(MovParser::from_reader(reader, Some(handle), file_size)) // Pass the handle to the parser
}


// Example main function (no_std)
#[cfg(feature = "example_mov")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("MOV parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy MOV file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/small.mov" exists.
     // let mov_file_res = open_mov_file("sahne://files/small.mov");
     // match mov_file_res {
     //     Ok(mut parser) => { // Need mut to call parse
     //         crate::println!("Attempting to parse MOV file...");
     //         if let Err(e) = parser.parse() {
     //             crate::eprintln!("MOV file parsing error: {:?}", e);
     //             return Err(e);
     //         }
     //         crate::println!("MOV file parsing complete.");
     //     },
     //     Err(e) => crate::eprintln!("Error opening MOV file: {:?}", e),
     // }

     eprintln!("MOV parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_mov")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("MOV parser example (std) starting...");
     eprintln!("MOV parser example (std) using simple atom parser.");

     // This example needs a dummy MOV file. Creating a valid MOV file from scratch is complex.
     // For testing, you might need a pre-existing minimal MOV file or use a library
     // like `mp4` or `isobmff` to create one if they support no_std (unlikely).

     // Let's try to open a hypothetical dummy file for testing the parser logic flow.
     // This file must actually exist in the test environment or be created using std FS.

     let mov_path = Path::new("example.mov"); // This file needs to exist for the std example

      // Create a very minimal dummy MOV file: ftyp + moov + mdat (headers only, no real data)
      // This is highly simplified and might not be a valid MOV file, but could test atom parsing logic.
      // ftyp atom: size (12) + type ('ftyp') + major_brand (4 bytes) + minor_version (4 bytes) + compatible_brands (list of 4-byte brands)
      // moov atom: size + type ('moov') + nested atoms (mvhd, trak, etc.)
      // mdat atom: size + type ('mdat') + media data

      // Simplified dummy data for testing atom iteration:
      // ftyp atom: 12 bytes total. Size: 0x0000000C. Type: 'ftyp'. Data: 4 bytes major_brand, 4 bytes minor_version.
      // moov atom: 8 bytes total. Size: 0x00000008. Type: 'moov'. Data: None (just header). This is invalid, moov must contain other atoms.
      // mdat atom: 8 bytes total. Size: 0x00000008. Type: 'mdat'. Data: None (just header). This is invalid, mdat must contain data.

      // Let's create dummy data with valid header structure (size + type) and some minimal data to pass size checks.
       let mut dummy_data_cursor = Cursor::new(Vec::new());
        // ftyp atom: size 20, type 'ftyp', 12 bytes dummy data
       dummy_data_cursor.write_u32::<BigEndian>(20).unwrap(); // Size
       dummy_data_cursor.write_all(b"ftyp").unwrap(); // Type
       dummy_data_cursor.write_all(&[0u8; 12]).unwrap(); // Dummy data

       // moov atom: size 24, type 'moov', 16 bytes dummy data (should contain nested atoms, but simplified)
       dummy_data_cursor.write_u32::<BigEndian>(24).unwrap(); // Size
       dummy_data_cursor.write_all(b"moov").unwrap(); // Type
       dummy_data_cursor.write_all(&[1u8; 16]).unwrap(); // Dummy data

       // mdat atom: size 16, type 'mdat', 8 bytes dummy data
       dummy_data_cursor.write_u32::<BigEndian>(16).unwrap(); // Size
       dummy_data_cursor.write_all(b"mdat").unwrap(); // Type
       dummy_data_cursor.write_all(&[2u8; 8]).unwrap(); // Dummy data

       let dummy_data = dummy_data_cursor.into_inner();

       // Write dummy data to a temporary file for std test
        match File::create(mov_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_data) {
                       eprintln!("Error writing dummy MOV file: {}", e);
                       return Err(map_std_io_error_to_fs_error(e));
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy MOV file: {}", e);
                  return Err(map_std_io_error_to_fs_error(e));
             }
        }


     match open_mov_file(mov_path) { // Call the function that opens and creates the parser
         Ok(mut parser) => { // Need mut to call parse
             println!("Attempting to parse MOV file...");
             if let Err(e) = parser.parse() {
                 eprintln!("MOV file parsing error: {}", e); // std error display
                 // Don't return error, let cleanup run
             } else {
                 println!("MOV file parsing complete.");
             }
         }
         Err(e) => {
              eprintln!("Error opening MOV file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(mov_path) {
          eprintln!("Error removing dummy MOV file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("MOV parser example (std) finished.");

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
      use byteorder::{BigEndian as StdBigEndian, WriteBytesExt as StdWriteBytesExt}; // Use std byteorder for writing test data


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec; // For vec! (implicitly used by String/Vec)
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests


     // Helper function to create dummy MOV data bytes in memory (atoms)
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
      fn create_dummy_mov_data_bytes(atoms_data: &[u8]) -> Vec<u8> {
          let mut buffer = Cursor::new(Vec::new());
           buffer.write_all(atoms_data).unwrap();
          buffer.into_inner()
      }

      // Helper to write a single MOV atom (Size + Type + Data)
       #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn write_mov_atom<W: Write>(writer: &mut W, atom_type: &[u8; 4], data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
           let size = (8 + data.len()) as u32; // Size includes header (4 size + 4 type)
           writer.write_u32::<BigEndian>(size).unwrap();
           writer.write_all(atom_type).unwrap();
           writer.write_all(data).unwrap();
           Ok(())
       }


     // Test the basic atom parsing (top-level iteration)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_parse_top_level_atoms() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error> for easier test error handling

         // Create dummy MOV data with ftyp, moov, mdat atoms
         let mut atoms_cursor = Cursor::new(Vec::new());
          write_mov_atom(&mut atoms_cursor, b"ftyp", &[0u8; 12])?; // ftyp: size 20
          write_mov_atom(&mut atoms_cursor, b"moov", &[1u8; 16])?; // moov: size 24
          write_mov_atom(&mut atoms_cursor, b"mdat", &[2u8; 8])?; // mdat: size 16
         let dummy_mov_data = atoms_cursor.into_inner();

         // Use Cursor as a reader
         let file_size = dummy_mov_data.len() as u64;
         let mut cursor = Cursor::new(dummy_mov_data.clone()); // Clone for potential re-reads in test

         // Create a dummy MovParser with the cursor reader
         let mut parser = MovParser::from_reader(cursor, None, file_size);

         // Call the parse function
         parser.parse()?; // Should complete without error

          // Verify the cursor is at the end of the data after parsing
          assert_eq!(parser.reader.stream_position().unwrap(), file_size);


         Ok(())
     }

     // Test parsing moov atom with nested atoms (simplified)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_moov_atom_nested() -> Result<(), Box<dyn std::error::Error>> { // Return Box<dyn Error>

           // Create dummy data for a moov atom with nested atoms
           // moov atom size 40. Contains:
           //   mvhd atom: size 12, type 'mvhd', 4 bytes data
           //   trak atom: size 20, type 'trak', 12 bytes data
           let mut moov_content_cursor = Cursor::new(Vec::new());
            write_mov_atom(&mut moov_content_cursor, b"mvhd", &[0u8; 4])?; // mvhd: size 12
            write_mov_atom(&mut moov_content_cursor, b"trak", &[1u8; 12])?; // trak: size 20
           let moov_content_data = moov_content_cursor.into_inner(); // Total size 12 + 20 = 32 bytes

           // Create the moov atom itself
           let mut atoms_cursor = Cursor::new(Vec::new());
            write_mov_atom(&mut atoms_cursor, b"moov", &moov_content_data)?; // moov: size 8 + 32 = 40
           let dummy_mov_data = atoms_cursor.into_inner(); // Just the moov atom bytes

           // Use Cursor as a reader
           let file_size = dummy_mov_data.len() as u64;
           let mut cursor = Cursor::new(dummy_mov_data.clone());

           // Create a dummy MovParser starting positioned at the beginning of the moov atom data
           // Simulate the parser having just read the moov header and being positioned at the start of its data.
           let moov_atom_size_in_test = 40;
           let moov_atom_data_size_in_test = moov_atom_size_in_test - 8; // 32

            // We need to simulate the `parse` function calling `parse_moov_atom` after reading the moov header.
            // Let's just test the `parse_moov_atom` function directly with a reader positioned correctly.
            let mut reader: &mut dyn Read + Seek = &mut cursor; // Treat cursor as Read+Seek trait

            // Manually read the moov header first to position the reader correctly
            let read_moov_size = reader.read_u32::<BigEndian>()?;
            let mut read_moov_type_bytes = [0u8; 4];
            reader.read_exact(&mut read_moov_type_bytes)?;
            assert_eq!(read_moov_type_bytes, *b"moov");
            assert_eq!(read_moov_size, moov_atom_size_in_test as u32);


           // Now call parse_moov_atom with the size of the moov data
           let mut parser = MovParser::from_reader(reader, None, file_size); // Create parser with the positioned reader

           parser.parse_moov_atom(moov_atom_data_size_in_test as u64)?; // Should parse nested atoms

            // Verify the cursor is at the end of the moov atom data
            assert_eq!(parser.reader.stream_position().unwrap(), file_size); // Should be at the end of the dummy data

          Ok(())
      }


     // Test handling of unexpected EOF during atom header reading
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_truncated_atom_header() {
           // Create dummy data that is too short for an atom header (e.g., only 4 bytes)
           let dummy_data = b"\x00\x00\x00\x10abcd".to_vec(); // Size 16, type 'abcd', but truncated
           let truncated_data = dummy_data[..4].to_vec(); // Only 4 bytes

           // Use Cursor as a reader
           let file_size = truncated_data.len() as u64;
           let mut cursor = Cursor::new(truncated_data);
           let mut parser = MovParser::from_reader(cursor, None, file_size);

           // Attempt to parse top-level atoms, expect an error during size reading (EOF)
           let result = parser.parse();

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_u32::<BigEndian>)
                   assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       // Test handling of invalid atom size (size < 8)
        #[test]
        #[cfg(feature = "std")] // Run this test only with std feature
        fn test_parse_invalid_atom_size() {
             // Create dummy data with an atom header having size < 8
             let mut dummy_data_cursor = Cursor::new(Vec::new());
             dummy_data_cursor.write_u32::<BigEndian>(4).unwrap(); // Invalid size 4
             dummy_data_cursor.write_all(b"test").unwrap(); // Type
             let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 = 8 bytes total

             let file_size = dummy_data.len() as u64;
             let mut cursor = Cursor::new(dummy_data);
             let mut parser = MovParser::from_reader(cursor, None, file_size);

             // Attempt to parse top-level atoms, expect an error due to invalid size
             let result = parser.parse();

             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from MovError::InvalidAtomSize
                     assert!(msg.contains("Geçersiz atom boyutu: 4"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This involves simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include opening valid/invalid files, handling IO errors,
     // and correctly parsing atom headers.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_mov", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
