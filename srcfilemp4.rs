#![allow(unused_imports)] // Henüz kullanılmayan importlar için uyarı vermesin
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


// Sahne64 fonksiyonlarını kullanmak için bu modülü içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle

// byteorder crate (no_std compatible)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec; // For read buffer
use alloc::format;


// core::result, core::option, core::str, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::str; // For from_utf8_lossy or from_utf8
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


/// Custom error type for MP4 atom parsing issues.
#[derive(Debug)]
pub enum Mp4Error {
    UnexpectedEof, // During atom header reading
    InvalidAtomSize(u32), // Atom size is zero or too small (for 32-bit size)
    Invalid64BitSize, // 64-bit size is zero
    SeekError(u64), // Failed to seek to a specific position
    InvalidAtomTypeEncoding(core::str::Utf8Error), // Atom type bytes are not valid UTF8 (if strict)
    // Add other MP4 specific parsing errors here (e.g., unknown atom type, invalid structure)
}

// Implement Display for Mp4Error
impl fmt::Display for Mp4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mp4Error::UnexpectedEof => write!(f, "Beklenmedik dosya sonu (atom başlığı okurken)"),
            Mp4Error::InvalidAtomSize(size) => write!(f, "Geçersiz 32-bit atom boyutu: {}", size),
            Mp4Error::Invalid64BitSize => write!(f, "Geçersiz 64-bit atom boyutu"),
            Mp4Error::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
             Mp4Error::InvalidAtomTypeEncoding(e) => write!(f, "Atom tipi UTF8 hatası: {}", e),
        }
    }
}

// Helper function to map Mp4Error to FileSystemError
fn map_mp4_error_to_fs_error(e: Mp4Error) -> FileSystemError {
    match e {
        Mp4Error::UnexpectedEof => FileSystemError::IOError(format!("Beklenmedik dosya sonu (atom başlığı okurken)")), // Map parsing EOF to IO Error
        Mp4Error::SeekError(pos) => FileSystemError::IOError(format!("Seek hatası pozisyon: {}", pos)), // Map seek errors to IO Error
        _ => FileSystemError::InvalidData(format!("MP4 ayrıştırma hatası: {}", e)), // Map other parsing errors to InvalidData
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilemov.rs'den kopyalandı)
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


/// Represents a basic MP4 atom header. Does NOT store the atom's data.
#[derive(Debug)]
pub struct Mp4AtomHeader {
    pub size: u64, // Use u64 to support 64-bit sizes
    pub atom_type: [u8; 4], // Atom type is represented as [u8; 4]
    // Add the file offset where this atom starts if needed for navigation
     pub offset: u64,
}


/// Basic MP4 (ISO Base Media File Format) parser.
/// Focuses on iterating through top-level atoms.
/// Does NOT parse nested atoms or atom data content.
pub struct Mp4Parser<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks
}

impl<R: Read + Seek> Mp4Parser<R> {
    /// Creates a new `Mp4Parser` instance from a reader.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64) -> Self {
        Self { reader, handle, file_size }
    }

    /// Parses and yields the next top-level atom header.
    /// Returns None if EOF is reached.
    /// Returns a FileSystemError on parsing errors.
    fn parse_next_atom_header(&mut self) -> Result<Option<Mp4AtomHeader>, FileSystemError> { // Return FileSystemError

        let current_pos = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?;

        // Check if we are at or beyond the file size
        if current_pos >= self.file_size {
             return Ok(None); // Reached end of file
        }

        // Read atom size (4 bytes, Big Endian)
        let atom_size_32_result = self.reader.read_u32::<BigEndian>(); // Use byteorder::ReadBytesExt
        let atom_size_32 = match atom_size_32_result {
             Ok(size) => size,
             Err(e) => {
                  // If EOF during size reading, check if it's exactly 0 bytes left
                  if e.kind() == core::io::ErrorKind::UnexpectedEof && self.file_size == current_pos {
                      return Ok(None); // Clean EOF
                  }
                  return Err(map_core_io_error_to_fs_error(e)); // Other IO errors
             }
        };

        let atom_size: u64; // Use u64 for the final atom size

        // Check for 64-bit size (size = 1)
        if atom_size_32 == 1 {
             // Read 64-bit extended size (8 bytes, Big Endian)
             let atom_size_64_result = self.reader.read_u64::<BigEndian>(); // Use byteorder::ReadBytesExt
             let atom_size_64 = match atom_size_64_result {
                  Ok(size) => size,
                  Err(e) => {
                       // If EOF during 64-bit size reading
                       return Err(map_core_io_error_to_fs_error(e)); // Map IO errors
                  }
             };
             if atom_size_64 < 16 { // Minimum size for 64-bit sized atom is 16 (8 size + 8 type)
                  return Err(map_mp4_error_to_fs_error(Mp4Error::Invalid64BitSize));
             }
             atom_size = atom_size_64;

        } else if atom_size_32 == 0 {
            // Size 0 typically means the atom extends to the end of the file,
            // but this is usually only for the last atom ('mdat').
            // Handling this requires knowing the total file size.
            // For a basic parser, this could be treated as an error or the size
            // calculated based on remaining file size. Let's calculate remaining size.
            let remaining_size = self.file_size.checked_sub(current_pos).unwrap_or(0);
            if remaining_size < 8 { // Must be at least 8 bytes for size + type
                 return Err(map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(0))); // Invalid size 0 if not enough remaining bytes
            }
            atom_size = remaining_size;

        }
        else {
            // 32-bit size
            if atom_size_32 < 8 { // Minimum size for 32-bit sized atom is 8 (4 size + 4 type)
                 return Err(map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(atom_size_32)));
            }
            atom_size = atom_size_32 as u64;
        }


        // Read atom type (4 bytes)
        let mut atom_type_bytes = [0u8; 4];
        let bytes_read = self.reader.read(&mut atom_type_bytes).map_err(map_core_io_error_to_fs_error)?; // Use read
         if bytes_read != 4 {
              return Err(map_mp4_error_to_fs_error(Mp4Error::UnexpectedEof)); // Should have read 4 bytes
         }


        // Check if atom extends beyond file bounds (important before seeking)
        let atom_end_pos = current_pos.checked_add(atom_size).ok_or_else(|| map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(atom_size as u32)))?; // Use u64 size
         if atom_end_pos > self.file_size {
              #[cfg(not(feature = "std"))]
              crate::eprintln!("WARN: Atom (type: {:?}, size: {}) extends beyond file size ({}).", atom_type_bytes, atom_size, self.file_size);
              #[cfg(feature = "std")]
              eprintln!("WARN: Atom (type: {:?}, size: {}) extends beyond file size ({}).", atom_type_bytes, atom_size, self.file_size);
             // Depending on strictness, this could be an error
             // return Err(map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(atom_size as u32)));
         }


        // The parser should now be positioned at the start of the atom's data.
        // The data size is atom_size - 8 (for 32-bit size) or atom_size - 16 (for 64-bit size).
        let header_size = if atom_size_32 == 1 { 16 } else { 8 }; // Size of the size+type header
        let data_size = atom_size.checked_sub(header_size).ok_or_else(|| map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(atom_size as u32)))?; // Use u64 size calculation


        Ok(Some(Mp4AtomHeader {
            size: atom_size,
            atom_type: atom_type_bytes,
            // offset: current_pos, // Add offset if needed
        }))
    }

    /// Iterates through the top-level atom headers in the file.
    /// Parses the header and skips the atom data.
    /// Returns an iterator-like structure or processes atoms in a loop.
    /// For simplicity, let's provide a method that processes atoms in a loop.
    pub fn process_top_level_atoms<F>(&mut self, mut callback: F) -> Result<(), FileSystemError> // Return FileSystemError
        where F: FnMut(&Mp4AtomHeader, &mut R) -> Result<(), FileSystemError> // Callback takes header and reader (positioned at data start)
    {
        loop {
            let current_pos_before_header = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
            let atom_header_option = self.parse_next_atom_header()?; // Parses header, positions reader at data start

            match atom_header_option {
                Some(header) => {
                    // Atom header parsed, reader is now at the start of the atom's data.
                    // Callback can process the header and read/skip data using the reader.

                    let header_size = if header.size <= u32::MAX as u64 && header.size != 1 { 8 } else { 16 };
                    let data_size = header.size.checked_sub(header_size).ok_or_else(|| map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(header.size as u32)))?; // Use u64 size calculation


                    #[cfg(not(feature = "std"))]
                    crate::println!("Top-level Atom: {} (0x{:X}), Size: {} bayt, Data Size: {} bayt",
                         str::from_utf8_lossy(&header.atom_type), header.atom_type.as_u32::<BigEndian>().unwrap_or(0), header.size, data_size); // Use lossy and byteorder for printing
                    #[cfg(feature = "std")]
                    println!("Top-level Atom: {} (0x{:X}), Size: {} bayt, Data Size: {} bayt",
                         str::from_utf8_lossy(&header.atom_type), header.atom_type.as_u32::<BigEndian>().unwrap_or(0), header.size, data_size);


                    // Call the callback function to process the atom
                    callback(&header, &mut self.reader)?; // Callback handles data reading/skipping


                    // After the callback, ensure the reader is positioned at the end of the atom's data.
                    // If the callback didn't read/skip the exact data_size, we must seek to the start of the next atom.
                    let expected_next_atom_pos = current_pos_before_header.checked_add(header.size).ok_or_else(|| map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(header.size as u32)))?; // Use u64 size calculation
                    let current_pos_after_callback = self.reader.stream_position().map_err(map_core_io_error_to_fs_error)?;

                    if current_pos_after_callback != expected_next_atom_pos {
                        // If callback didn't read/skip correctly, seek to the start of the next atom.
                        #[cfg(not(feature = "std"))]
                        crate::eprintln!("WARN: Atom data işleme sonrası beklenmeyen pozisyon. Beklenen sonraki atom başlangıcı: {}, Gerçek pozisyon: {}. İlerleniyor.",
                             expected_next_atom_pos, current_pos_after_callback);
                         #[cfg(feature = "std")]
                         eprintln!("WARN: Atom data işleme sonrası beklenmeyen pozisyon. Beklenen sonraki atom başlangıcı: {}, Gerçek pozisyon: {}. İlerleniyor.",
                             expected_next_atom_pos, current_pos_after_callback);

                         self.reader.seek(SeekFrom::Start(expected_next_atom_pos as u64)).map_err(|e| map_core_io_error_to_fs_error(e))?; // Seek to the start of the next atom
                    }


                }
                None => {
                    // End of file reached during header parsing
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for Mp4Parser<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the parser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: Mp4Parser drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens an MP4 file from the given path (std) or resource ID (no_std)
/// and creates a basic Mp4Parser.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the Mp4Parser or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_mp4_file<P: AsRef<Path>>(file_path: P) -> Result<Mp4Parser<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    Ok(Mp4Parser::from_reader(reader, None, file_size)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_mp4_file(file_path: &str) -> Result<Mp4Parser<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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

    Ok(Mp4Parser::from_reader(reader, Some(handle), file_size)) // Pass the handle to the parser
}


// Example main function (no_std)
#[cfg(feature = "example_mp4")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("MP4 parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy MP4 file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/small.mp4" exists.
      let mp4_file_res = open_mp4_file("sahne://files/small.mp4");
      match mp4_file_res {
          Ok(mut parser) => { // Need mut to call process_top_level_atoms
              crate::println!("Attempting to process top-level MP4 atoms...");
              let result = parser.process_top_level_atoms(|header, reader| {
     //             // This callback is executed for each top-level atom.
     //             // 'header' contains size and type.
     //             // 'reader' is positioned at the start of the atom's data.
     //             // We must read or seek exactly 'header.size - header_size' bytes from the reader.
     //             // For this example, we just skip the data.
     //
                   let header_size = if header.size <= u32::MAX as u64 && header.size != 1 { 8 } else { 16 };
                   let data_size = header.size.checked_sub(header_size).ok_or_else(|| map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(header.size as u32)))?;
     //
                  if data_size > 0 {
                       reader.seek(SeekFrom::Current(data_size as i64))
                           .map_err(|e| map_core_io_error_to_fs_error(e))?;
                  }
     
                  Ok(()) // Callback succeeded
              });
     
              if let Err(e) = result {
                  crate::eprintln!("MP4 atom processing error: {:?}", e);
                  return Err(e);
              }
              crate::println!("MP4 atom processing complete.");
          },
          Err(e) => crate::eprintln!("Error opening MP4 file: {:?}", e),
      }

     eprintln!("MP4 parser example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_mp4")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("MP4 parser example (std) starting...");
     eprintln!("MP4 parser example (std) using simple top-level atom processor.");

     // This example needs a dummy MP4 file. Creating a valid MP4 file from scratch is complex.
     // For testing, you might need a pre-existing minimal MP4 file or use a library
     // like `mp4` or `isobmff` to create one if they support no_std (unlikely).

     // Let's try to open a hypothetical dummy file for testing the parser logic flow.
     // This file must actually exist in the test environment or be created using std FS.

     let mp4_path = Path::new("example.mp4"); // This file needs to exist for the std example

      // Create a very minimal dummy MP4 file: ftyp + moov + mdat (headers only, minimal data)
      // Use the write_mov_atom helper (from srcfilemov.rs test)
      #[cfg(test)] // Use the helper defined in the test module for std builds
       fn write_mov_atom<W: Write>(writer: &mut W, atom_type: &[u8; 4], data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
           let size = (8 + data.len()) as u32; // Size includes header (4 size + 4 type)
            // Handle 64-bit size for completeness in test data creation, though the parser handles it.
             if size == 1 { return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Cannot create atom with size 1 using this helper"))); } // Avoid size 1 issue

             if size < 8 && size != 0 { return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Atom size must be >= 8 or 0 or 1"))); }


            if size > u32::MAX {
                // Write 64-bit size format
                 writer.write_u32::<BigEndian>(1).unwrap(); // Size is 1
                 writer.write_all(atom_type).unwrap();
                 writer.write_u64::<BigEndian>(8 + 8 + data.len() as u64).unwrap(); // Actual size (16 bytes header + data)
                 writer.write_all(data).unwrap();
            } else {
                 // Write 32-bit size format
                 writer.write_u32::<BigEndian>(size).unwrap();
                 writer.write_all(atom_type).unwrap();
                 writer.write_all(data).unwrap();
            }
           Ok(())
       }


      let mut dummy_data_cursor = Cursor::new(Vec::new());
       // ftyp atom: size 20, type 'ftyp', 12 bytes dummy data
      write_mov_atom(&mut dummy_data_cursor, b"ftyp", &[0u8; 12]).unwrap(); // size 20

      // moov atom: size 24, type 'moov', 16 bytes dummy data (should contain nested atoms, but simplified)
       // Let's create a moov atom that contains nested atoms, even if simplified, to test skipping nested data.
       let mut moov_nested_cursor = Cursor::new(Vec::new());
        write_mov_atom(&mut moov_nested_cursor, b"mvhd", &[0u8; 4]).unwrap(); // mvhd: size 12
        write_mov_atom(&mut moov_nested_cursor, b"trak", &[1u8; 12]).unwrap(); // trak: size 20
       let moov_nested_data = moov_nested_cursor.into_inner(); // 12 + 20 = 32 bytes

       write_mov_atom(&mut dummy_data_cursor, b"moov", &moov_nested_data).unwrap(); // moov: size 8 + 32 = 40


      // mdat atom: size 16, type 'mdat', 8 bytes dummy data
      write_mov_atom(&mut dummy_data_cursor, b"mdat", &[2u8; 8]).unwrap(); // size 16

       // Add an atom with 64-bit size at the end
        let large_data = vec![0u8; 1024 * 10]; // 10KB data
        write_mov_atom(&mut dummy_data_cursor, b"free", &large_data).unwrap(); // Large free atom

       let dummy_data = dummy_data_cursor.into_inner();


       // Write dummy data to a temporary file for std test
        match File::create(mp4_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(&dummy_data) {
                       eprintln!("Error writing dummy MP4 file: {}", e);
                       return Err(map_std_io_error_to_fs_error(e));
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy MP4 file: {}", e);
                  return Err(map_std_io_error_to_fs_error(e));
             }
        }


     match open_mp4_file(mp4_path) { // Call the function that opens and creates the parser
         Ok(mut parser) => { // Need mut to call process_top_level_atoms
             println!("Attempting to process top-level MP4 atoms...");
             let result = parser.process_top_level_atoms(|header, reader| {
                 // This callback is executed for each top-level atom.
                 // 'header' contains size and type.
                 // 'reader' is positioned at the start of the atom's data.
                 // We must read or seek exactly 'header.size - header_size' bytes from the reader.
                 // For this example, we just skip the data.

                  let header_size = if header.size <= u32::MAX as u64 && header.size != 1 { 8 } else { 16 };
                  let data_size = header.size.checked_sub(header_size).ok_or_else(|| map_mp4_error_to_fs_error(Mp4Error::InvalidAtomSize(header.size as u32)))?;

                 #[cfg(test)] // Use test-specific println for clarity in test output
                 println!("  Processing data for atom: {} (size: {}), data size: {}",
                      str::from_utf8_lossy(&header.atom_type), header.size, data_size);


                 if data_size > 0 {
                      // Skip the atom data
                      reader.seek(SeekFrom::Current(data_size as i64))
                          .map_err(|e| map_core_io_error_to_fs_error(e))?;
                 }

                 Ok(()) // Callback succeeded
             });

             if let Err(e) = result {
                 eprintln!("MP4 atom processing error: {}", e); // std error display
                 // Don't return error, let cleanup run
             } else {
                 println!("MP4 atom processing complete.");
             }
         }
         Err(e) => {
              eprintln!("Error opening MP4 file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
     if let Err(e) = remove_file(mp4_path) {
          eprintln!("Error removing dummy MP4 file: {}", e);
          // Don't return error, cleanup is best effort
     }


     eprintln!("MP4 parser example (std) finished.");

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


     // Helper function to create dummy MP4 data bytes in memory (atoms)
     // This helper includes logic to write atoms with 32-bit or 64-bit size based on data length.
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
       fn write_mp4_atom_bytes(atom_type: &[u8; 4], data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
           let data_len = data.len();
           let total_size_if_32bit = (8 + data_len) as u32;
           let total_size_if_64bit = 8 + 8 + data_len as u64; // 16 bytes header + data

           let mut buffer = Cursor::new(Vec::new());

           if total_size_if_32bit == 1 || total_size_if_32bit == 0 || total_size_if_32bit < 8 && total_size_if_32bit != 0 {
               // Force 64-bit size if 32-bit size would be invalid or 1.
               // Write 64-bit size format
                buffer.write_u32::<BigEndian>(1).unwrap(); // Size is 1
                buffer.write_all(atom_type).unwrap();
                buffer.write_u64::<BigEndian>(total_size_if_64bit).unwrap(); // Actual size
                buffer.write_all(data).unwrap();

           } else if total_size_if_32bit > u32::MAX {
                // Size requires 64-bit format
                 buffer.write_u32::<BigEndian>(1).unwrap(); // Size is 1
                 buffer.write_all(atom_type).unwrap();
                 buffer.write_u64::<BigEndian>(total_size_if_64bit).unwrap(); // Actual size
                 buffer.write_all(data).unwrap();
           }
           else {
                // Use 32-bit size format
                buffer.write_u32::<BigEndian>(total_size_if_32bit).unwrap();
                buffer.write_all(atom_type).unwrap();
                buffer.write_all(data).unwrap();
           }

           Ok(buffer.into_inner())
       }


     // Test the basic atom parsing (top-level iteration)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_parse_top_level_atoms() -> Result<(), FileSystemError> { // Return FileSystemError

         // Create dummy MP4 data with ftyp, moov, mdat atoms using the helper
          let ftyp_bytes = write_mp4_atom_bytes(b"ftyp", &[0u8; 12])
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?; // size 20
          let moov_bytes = write_mp4_atom_bytes(b"moov", &[1u8; 16])
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?; // size 24
          let mdat_bytes = write_mp4_atom_bytes(b"mdat", &[2u8; 8])
               .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?; // size 16

          let mut dummy_data_vec = Vec::new();
          dummy_data_vec.extend_from_slice(&ftyp_bytes);
          dummy_data_vec.extend_from_slice(&moov_bytes);
          dummy_data_vec.extend_from_slice(&mdat_bytes);

         // Use Cursor as a reader
         let file_size = dummy_data_vec.len() as u64;
         let mut cursor = Cursor::new(dummy_data_vec.clone());

         // Create a dummy Mp4Parser with the cursor reader
         let mut parser = Mp4Parser::from_reader(cursor, None, file_size);

         // Process top-level atoms and collect their headers
         let mut collected_headers = Vec::new();
         let result = parser.process_top_level_atoms(|header, reader| {
              #[cfg(test)] // Use test-specific println for clarity in test output
              println!("Test: Processing atom type: {} (size: {})",
                   str::from_utf8_lossy(&header.atom_type), header.size);

              collected_headers.push(Mp4AtomHeader { size: header.size, atom_type: header.atom_type });

              // The callback must consume/skip the data. The process_top_level_atoms
              // will seek if the position is incorrect after the callback.
              // We don't need to read/seek here unless we want to test callback logic.
              // For this test, just collecting headers is enough.

              Ok(()) // Callback succeeded
         });

         // Assert the parsing was successful
         result?;

         // Assert the collected headers are correct
          assert_eq!(collected_headers.len(), 3);
          assert_eq!(collected_headers[0].atom_type, *b"ftyp");
          assert_eq!(collected_headers[0].size, 20); // 8 + 12
          assert_eq!(collected_headers[1].atom_type, *b"moov");
          assert_eq!(collected_headers[1].size, 24); // 8 + 16
          assert_eq!(collected_headers[2].atom_type, *b"mdat");
          assert_eq!(collected_headers[2].size, 16); // 8 + 8


          // Verify the cursor is at the end of the data after parsing
          assert_eq!(parser.reader.stream_position().unwrap(), file_size);


         Ok(())
     }

     // Test handling of 64-bit sized atoms
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_64bit_atom() -> Result<(), FileSystemError> { // Return FileSystemError

           // Create dummy data with a large atom using 64-bit size format
           let large_data = vec![0u8; 1024 * 1024 * 5]; // 5MB data
           let large_atom_bytes = write_mp4_atom_bytes(b"wide", &large_data)
                .map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?; // Type 'wide'


            // Create a dummy file with just this large atom
           let mut dummy_data_vec = Vec::new();
           dummy_data_vec.extend_from_slice(&large_atom_bytes);

           let file_size = dummy_data_vec.len() as u64;
           let mut cursor = Cursor::new(dummy_data_vec.clone());

           // Create a dummy Mp4Parser
           let mut parser = Mp4Parser::from_reader(cursor, None, file_size);

            // Process top-level atoms and collect headers
            let mut collected_headers = Vec::new();
            let result = parser.process_top_level_atoms(|header, reader| {
                 #[cfg(test)]
                 println!("Test: Processing atom type: {} (size: {})",
                      str::from_utf8_lossy(&header.atom_type), header.size);
                 collected_headers.push(Mp4AtomHeader { size: header.size, atom_type: header.atom_type });
                 Ok(()) // Callback succeeded (implicit skip by process_top_level_atoms)
            });

            // Assert the parsing was successful
            result?;

            // Assert the collected header is correct
            assert_eq!(collected_headers.len(), 1);
            assert_eq!(collected_headers[0].atom_type, *b"wide");
             // Expected size is 16 (header) + data_len
             assert_eq!(collected_headers[0].size, 16 + large_data.len() as u64);


           // Verify the cursor is at the end of the data after parsing
           assert_eq!(parser.reader.stream_position().unwrap(), file_size);


           Ok(())
      }

     // Test handling of unexpected EOF during atom header reading (size)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_truncated_size() {
           // Create dummy data that is too short for atom size (e.g., 3 bytes)
           let dummy_data = b"\x00\x00\x00".to_vec(); // Only 3 bytes

           // Use Cursor as a reader
           let file_size = dummy_data.len() as u64;
           let mut cursor = Cursor::new(dummy_data);
           let mut parser = Mp4Parser::from_reader(cursor, None, file_size);

           // Attempt to parse, expect an error during size reading (EOF)
           let mut collected_headers = Vec::new();
           let result = parser.process_top_level_atoms(|header, reader| {
                collected_headers.push(Mp4AtomHeader { size: header.size, atom_type: header.atom_type });
                Ok(())
           });

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_u32::<BigEndian>)
                   assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
           // No headers should have been collected
           assert_eq!(collected_headers.len(), 0);
      }


      // Test handling of unexpected EOF during atom type reading
       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_parse_truncated_type() {
            // Create dummy data with valid size, but truncated type (e.g., size 8, type only 2 bytes)
            let mut dummy_data_cursor = Cursor::new(Vec::new());
            dummy_data_cursor.write_u32::<BigEndian>(8).unwrap(); // Size 8
            dummy_data_cursor.write_all(b"ty").unwrap(); // Truncated type (2 bytes instead of 4)
            let dummy_data = dummy_data_cursor.into_inner(); // 4 + 2 = 6 bytes total

            let file_size = dummy_data.len() as u64;
            let mut cursor = Cursor::new(dummy_data);
            let mut parser = Mp4Parser::from_reader(cursor, None, file_size);

            // Attempt to parse, expect an error during type reading (EOF)
            let mut collected_headers = Vec::new();
            let result = parser.process_top_level_atoms(|header, reader| {
                 collected_headers.push(Mp4AtomHeader { size: header.size, atom_type: header.atom_type });
                 Ok(())
            });


            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read)
                    assert!(msg.contains("Beklenmeden erken dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
            // No headers should have been collected
            assert_eq!(collected_headers.len(), 0);
       }


       // Test handling of invalid atom size (size < 8)
        #[test]
        #[cfg(feature = "std")] // Run this test only with std feature
        fn test_parse_invalid_32bit_atom_size() {
             // Create dummy data with an atom header having size < 8
             let mut dummy_data_cursor = Cursor::new(Vec::new());
             dummy_data_cursor.write_u32::<BigEndian>(4).unwrap(); // Invalid size 4
             dummy_data_cursor.write_all(b"test").unwrap(); // Type
             let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 = 8 bytes total

             let file_size = dummy_data.len() as u64;
             let mut cursor = Cursor::new(dummy_data);
             let mut parser = Mp4Parser::from_reader(cursor, None, file_size);

             // Attempt to parse, expect an error due to invalid size
             let mut collected_headers = Vec::new();
             let result = parser.process_top_level_atoms(|header, reader| {
                  collected_headers.push(Mp4AtomHeader { size: header.size, atom_type: header.atom_type });
                  Ok(())
             });


             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from Mp4Error::InvalidAtomSize
                     assert!(msg.contains("Geçersiz 32-bit atom boyutu: 4"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
             // No headers should have been collected
             assert_eq!(collected_headers.len(), 0);
        }

       // Test handling of invalid 64-bit atom size (size < 16)
        #[test]
        #[cfg(feature = "std")] // Run this test only with std feature
        fn test_parse_invalid_64bit_atom_size() {
             // Create dummy data with a size of 1 (indicating 64-bit size) but the 64-bit size is < 16
             let mut dummy_data_cursor = Cursor::new(Vec::new());
             dummy_data_cursor.write_u32::<BigEndian>(1).unwrap(); // Size is 1 (indicates 64-bit size)
             dummy_data_cursor.write_all(b"test").unwrap(); // Type
             dummy_data_cursor.write_u64::<BigEndian>(10).unwrap(); // Invalid 64-bit size 10 (should be >= 16)
             let dummy_data = dummy_data_cursor.into_inner(); // 4 + 4 + 8 = 16 bytes total

             let file_size = dummy_data.len() as u64;
             let mut cursor = Cursor::new(dummy_data);
             let mut parser = Mp4Parser::from_reader(cursor, None, file_size);

             // Attempt to parse, expect an error due to invalid 64-bit size
             let mut collected_headers = Vec::new();
             let result = parser.process_top_level_atoms(|header, reader| {
                  collected_headers.push(Mp4AtomHeader { size: header.size, atom_type: header.atom_type });
                  Ok(())
             });


             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from Mp4Error::Invalid64BitSize
                     assert!(msg.contains("Geçersiz 64-bit atom boyutu"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
             // No headers should have been collected
             assert_eq!(collected_headers.len(), 0);
        }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This involves simulating resource acquire/release, fs::fstat, fs::read_at.
     // Test cases should include opening valid/invalid files, handling IO errors,
     // and correctly parsing atom headers (including 64-bit size and size 0).
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_mp4", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
