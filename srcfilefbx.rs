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
// Sahne64 fs modülü (fs::read_at, fs::fstat için varsayım - used by SahneResourceReader)
#[cfg(not(feature = "std"))]
use crate::fs;

// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::{File, self};
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

// core::io traits and types
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io
use core::fmt; // core::fmt
use core::cmp; // core::cmp
use core::convert::TryInto; // core::convert::TryInto

// byteorder crate (no_std compatible)
use byteorder::{LittleEndian, ReadBytesExt, ByteOrder}; // LittleEndian, ReadBytesExt, ByteOrder trait/types

// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülden import edildiği varsayılır


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


/// Custom error type for FBX parsing issues.
#[derive(Debug)]
pub enum FbxError {
    InvalidMagicNumber(Vec<u8>),
    UnknownPropertyType(u8),
    Utf8Error(core::str::Utf8Error),
    OffsetOverflow,
    UnexpectedEof, // For read_exact failures in core::io (mapped from CoreIOError)
    // Add other FBX specific parsing errors here
}

// Implement Display for FbxError
impl fmt::Display for FbxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FbxError::InvalidMagicNumber(magic) => write!(f, "Geçersiz FBX sihirli sayısı: {:x?}", magic),
            FbxError::UnknownPropertyType(type_code) => write!(f, "Bilinmeyen özellik tipi: {}", type_code),
            FbxError::Utf8Error(e) => write!(f, "UTF8 hatası: {}", e),
            FbxError::OffsetOverflow => write!(f, "Ofset hesaplanırken taşma"),
            FbxError::UnexpectedEof => write!(f, "Beklenenden erken dosya sonu"),
        }
    }
}

// Helper function to map FbxError to FileSystemError
fn map_fbx_error_to_fs_error(e: FbxError) -> FileSystemError {
    match e {
        FbxError::InvalidMagicNumber(magic) => FileSystemError::InvalidData(format!("Geçersiz FBX sihirli sayısı: {:x?}", magic)),
        FbxError::UnknownPropertyType(type_code) => FileSystemError::InvalidData(format!("Bilinmeyen özellik tipi: {}", type_code)),
        FbxError::Utf8Error(e) => FileSystemError::InvalidData(format!("UTF8 hatası: {}", e)),
        FbxError::OffsetOverflow => FileSystemError::InvalidData(format!("Ofset hesaplanırken taşma")),
        FbxError::UnexpectedEof => FileSystemError::IOError(format!("Beklenenden erken dosya sonu")), // Mapping parsing EOF to IO Error
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilebmp.rs'den kopyalandı)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
// fstat ile dosya boyutunu alarak seek(End) desteği sağlar.
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
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        if self.position >= self.file_size {
            return Ok(0); // EOF
        }
        let bytes_available = (self.file_size - self.position) as usize;
        let bytes_to_read = cmp::min(buf.len(), bytes_available);

        if bytes_to_read == 0 {
             return Ok(0);
        }

        // Assuming fs::read_at(handle, offset, buf) Result<usize, SahneError>
        let bytes_read = fs::read_at(self.handle, self.position, &mut buf[..bytes_to_read])
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::read_at error: {:?}", e)))?;

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read that uses read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize,
            SeekFrom::End(offset) => {
                file_size_isize.checked_add(offset)
                    .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from end)")))?
            },
            SeekFrom::Current(offset) => {
                (self.position as isize).checked_add(offset)
                     .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from current)")))?
            },
        };

        if new_pos < 0 {
            return Err(CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Invalid seek position (result is negative)")));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
    // stream_position has a default implementation in core::io::Seek that uses seek(Current(0))
}


// FBX dosya formatının temel yapıları

// FBX Başlığı yapısı
#[derive(Debug)]
pub struct FbxHeader {
    pub magic_number: [u8; 21], // "Kaydaz FBX " + \x00
    pub unknown: [u8; 2],      // [0x1A, 0x00]
    pub version: u32,         // Versiyon numarası
}

// FBX Düğüm Özelliği (Property) enum'ı
#[derive(Debug, PartialEq)] // Add PartialEq for testing
pub enum FbxProperty {
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    RawBytes(Vec<u8>), // Ham veri için
    Bool(bool),
    // Note: Other FBX types like arrays (f, d, l, i, b), S, R, etc. might need to be added.
    // Arrays have type code + u32 count + u32 encoding (0=raw, 1=LZW?) + u32 data_len + data
}

// FBX Düğümü yapısı
#[derive(Debug, PartialEq)] // Add PartialEq for testing
pub struct FbxNode {
    pub end_offset: u64,          // Düğüm sonu offset'i (dosya başından itibaren) - Should be u64 for consistency with seek
    pub num_properties: u64,      // Özellik sayısı - Should be u64
    pub property_list_len: u64,   // Özellik listesinin uzunluğu - Should be u64
    pub name_len: u8,             // İsim uzunluğu
    pub name: String,             // Düğüm ismi
    pub properties: Vec<FbxProperty>, // Özellikler listesi
    pub nested_nodes: Vec<FbxNode>, // İç içe düğümler
}


/// Reads and parses the FBX header from the provided reader.
/// Assumes the reader is positioned at the start of the header (offset 0).
/// Uses Little Endian byte order.
fn read_fbx_header<R: Read + Seek>(reader: &mut R) -> Result<FbxHeader, FileSystemError> { // FileSystemError döner
    let mut header = FbxHeader {
        magic_number: [0; 21],
        unknown: [0; 2],
        version: 0,
    };

    reader.seek(SeekFrom::Start(0)).map_err(map_core_io_error_to_fs_error)?; // Ensure at start

    reader.read_exact(&mut header.magic_number).map_err(|e| match e.kind() { // core::io::Error -> FbxError -> FileSystemError
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;
    reader.read_exact(&mut header.unknown).map_err(|e| match e.kind() {
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;

     // Check magic number "Kaydaz FBX " + \x00
     if &header.magic_number != b"Kaydaz FBX \0" {
          // The original code had "Kaydaz FBX \x00" in comment, using that as the magic
          // The actual FBX magic is "FBX" followed by a different sequence depending on version.
          // Let's use the common binary magic number found in many parsers: "FBX\x00\x46\x42\x58\x20\x20\x00" (FBX  \0)
          // Or maybe the original magic number was specific to a variant?
          // Let's stick to the provided "Kaydaz FBX \0" magic for now but note it's unusual.
          // Actual magic is usually "Kaydaz FBX " + 0x1A + 0x00 + version
          // Let's check the provided magic number including the unknown bytes
          let mut full_magic = [0u8; 23]; // 21 + 2
          full_magic[..21].copy_from_slice(&header.magic_number);
          full_magic[21..].copy_from_slice(&header.unknown);

           if &full_magic[..23] != b"Kaydaz FBX \x00\x1A\x00" {
               return Err(map_fbx_error_to_fs_error(FbxError::InvalidMagicNumber(full_magic.to_vec()))); // FbxError -> FileSystemError
           }


    header.version = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() { // Use byteorder::ReadBytesExt
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;


    Ok(header)
}

/// Reads and parses a single FBX property from the provided reader.
/// Uses Little Endian byte order.
fn read_fbx_property<R: Read + Seek>(reader: &mut R) -> Result<FbxProperty, FileSystemError> { // FileSystemError döner
    let type_code = reader.read_u8().map_err(|e| match e.kind() { // core::io::ReadBytesExt
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;

    match type_code as char {
        'C' => { // Boolean (1 byte)
             let val = reader.read_u8().map_err(|e| match e.kind() {
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             Ok(FbxProperty::Bool(val != 0))
        },
        'Y' => { // 2-byte signed Integer
             let val = reader.read_i16::<LittleEndian>().map_err(|e| match e.kind() { // byteorder
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             Ok(FbxProperty::Integer(val as i32)) // Store as i32
        },
        'I' => { // 4-byte signed Integer
             let val = reader.read_i32::<LittleEndian>().map_err(|e| match e.kind() { // byteorder
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             Ok(FbxProperty::Integer(val))
        },
        'L' => { // 8-byte signed Integer (Long)
             let val = reader.read_i64::<LittleEndian>().map_err(|e| match e.kind() { // byteorder
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             Ok(FbxProperty::Long(val))
        },
        'F' => { // 4-byte single-precision Float
             let val = reader.read_f32::<LittleEndian>().map_err(|e| match e.kind() { // byteorder
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             Ok(FbxProperty::Float(val))
        },
        'D' => { // 8-byte double-precision Float
             let val = reader.read_f64::<LittleEndian>().map_err(|e| match e.kind() { // byteorder
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             Ok(FbxProperty::Double(val))
        },
        'S' | 'R' => { // String or Raw data
             // Length is u32 (4 bytes)
             let len = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() { // byteorder
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             let mut buffer = vec![0; len as usize]; // Requires alloc
             reader.read_exact(&mut buffer).map_err(|e| match e.kind() { // core::io::Read
                  CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
                  _ => map_core_io_error_to_fs_error(e),
              })?;
             if type_code as char == 'S' {
                 match String::from_utf8(buffer) { // Requires alloc and utf8 conversion
                     Ok(s) => Ok(FbxProperty::String(s)),
                     Err(e) => Err(map_fbx_error_to_fs_error(FbxError::Utf8Error(e.utf8_error()))), // FbxError -> FileSystemError
                 }
             } else {
                 Ok(FbxProperty::RawBytes(buffer))
             }
        },
         // TODO: Add support for array types (f, d, l, i, b)
         'f' | 'd' | 'l' | 'i' | 'b' => {
             // Format: u32 count, u32 encoding, u32 data_len, data
              eprintln!("WARN: FBX array property type '{}' parsing not fully implemented.", type_code as char); // no_std print

              let count = reader.read_u32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;
              let encoding = reader.read_u32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?; // 0 = raw, 1 = LZW
              let data_len = reader.read_u32::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;

              // Skip the data for now
              reader.seek(SeekFrom::Current(data_len as i64)).map_err(map_core_io_error_to_fs_error)?;

              // Return a placeholder or error
              Err(FileSystemError::NotSupported) // Or a specific FbxError for unsupported type
         }
        _ => Err(map_fbx_error_to_fs_error(FbxError::UnknownPropertyType(type_code))), // FbxError -> FileSystemError
    }
}

/// Reads and parses a single FBX node from the provided reader.
/// Assumes the reader provides seek functionality and uses Little Endian byte order.
/// Returns Some(FbxNode) if a node is found, or None if an empty node marker is found.
fn read_fbx_node<R: Read + Seek>(reader: &mut R) -> Result<Option<FbxNode>, FileSystemError> { // FileSystemError döner
    // Node Record Format (header + properties + nested nodes + null marker):
    // u32 EndOffset
    // u32 NumProperties
    // u32 PropertyListLen
    // u8 NameLen
    // char Name[NameLen]
    // Property[NumProperties]
    // NestedNode[]
    // u13 NullMarker (if version >= 7500, otherwise 0x00) - Note: the original code uses u32, check spec.
    // The end_offset points to the byte AFTER the node's null marker.

    let current_pos_before_header = reader.stream_position().map_err(map_core_io_error_to_fs_error)?; // Get current position

    // Read node header fields
    let end_offset = reader.read_u64::<LittleEndian>().map_err(|e| match e.kind() { // FBX SDK 7.x uses u64 for offsets/lengths
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;
    let num_properties = reader.read_u64::<LittleEndian>().map_err(|e| match e.kind() {
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;
    let property_list_len = reader.read_u64::<LittleEndian>().map_err(|e| match e.kind() {
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;
    let name_len = reader.read_u8().map_err(|e| match e.kind() {
         CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
         _ => map_core_io_error_to_fs_error(e),
     })?;


    // Empty node marker check (EndOffset == 0)
    // According to FBX SDK 7.x, the null record is 13 zero bytes.
    // Check if the first 13 bytes are zero (end_offset, num_properties, property_list_len, name_len, and the first 4 bytes of name).
    // The original code checked if end_offset, num_properties, property_list_len, and name_len were zero.
    // A more robust check is to read the first 13 bytes and check if they are all zero.
    // If we read the fields above, we can check if end_offset == 0.
    // If end_offset is 0, it's supposed to be followed by 12 more zero bytes for the null marker (total 13).

    if end_offset == 0 {
         // It's likely the null marker. Read the remaining 12 bytes and verify they are zero.
         let mut null_marker_remaining = [0u8; 12];
         reader.read_exact(&mut null_marker_remaining).map_err(|e| match e.kind() {
              CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
              _ => map_core_io_error_to_fs_error(e),
          })?;
         if null_marker_remaining.iter().all(|&b| b == 0) {
             return Ok(None); // Successfully read null marker
         } else {
             // Unexpected non-zero bytes after a zero EndOffset
             return Err(FileSystemError::InvalidData(format!("Beklenmeyen null marker formatı: Sıfır EndOffset sonrası sıfır olmayan baytlar.")));
         }
         // Note: For FBX versions < 7500, the null marker is just one zero byte (the EndOffset itself is 0).
         // Need to check FBX version from the header to handle this correctly.
         // Assuming version >= 7500 for the 13-byte null marker based on the original code structure implies reading more than 4 bytes for the header.
         // The original code structure for read_fbx_node reads 4 u32s and 1 u8, totaling 17 bytes before the name_len.
         // This doesn't match the standard FBX node record format (u32, u32, u32, u8 = 13 bytes + name).
         // Let's adjust to the standard format (u64, u64, u64, u8 = 25 bytes + name for SDK 7.x).
         // The original code's field sizes seem incorrect for modern FBX. Let's use u64 for counts/offsets.
         // Corrected fields above in FbxNode struct.

         // Re-reading node header fields as u64:
         // u64 EndOffset (8 bytes)
         // u64 NumProperties (8 bytes)
         // u64 PropertyListLen (8 bytes)
         // u8 NameLen (1 byte)
         // Total header size before name: 8 + 8 + 8 + 1 = 25 bytes.

         // Null marker check using u64 EndOffset: If EndOffset is 0, it's the null marker (13 zero bytes total).
         // We already read 8 bytes for end_offset. If it's 0, read the next 5 zero bytes (total 13).
         // Let's re-read from the start of the potential node record to check for the 13 zero bytes.
         reader.seek(SeekFrom::Start(current_pos_before_header)).map_err(map_core_io_error_to_fs_error)?; // Go back to the start of the potential node

         let mut null_marker_buffer = [0u8; 13];
         reader.read_exact(&mut null_marker_buffer).map_err(|e| match e.kind() {
              CoreIOErrorKind::UnexpectedEof => map_fbx_error_to_fs_error(FbxError::UnexpectedEof),
              _ => map_core_io_error_to_fs_error(e),
          })?;

         if null_marker_buffer.iter().all(|&b| b == 0) {
              // Successfully read the 13-byte null marker
              return Ok(None);
         } else {
              // It's not a null marker, rewind and parse the actual node header
              reader.seek(SeekFrom::Start(current_pos_before_header)).map_err(map_core_io_error_to_fs_error)?;

              // Re-read node header fields as u64
              let end_offset = reader.read_u64::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;
              let num_properties = reader.read_u64::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;
              let property_list_len = reader.read_u64::<LittleEndian>().map_err(|e| map_core_io_error_to_fs_error(e))?;
              let name_len = reader.read_u8().map_err(|e| map_core_io_error_to_fs_error(e))?;

              // Check if EndOffset is valid (must be > 0 for a non-null node)
               if end_offset == 0 {
                    // This should not happen if the 13-byte null check passed, but as a safeguard:
                    return Err(FileSystemError::InvalidData(format!("Sıfır EndOffset ile geçerli olmayan düğüm formatı.")));
               }


              // Read name bytes
              let mut name_bytes = vec![0; name_len as usize]; // Requires alloc
              reader.read_exact(&mut name_bytes).map_err(|e| map_core_io_error_to_fs_error(e))?;
              let name = String::from_utf8(name_bytes).map_err(|e| map_fbx_error_to_fs_error(FbxError::Utf8Error(e.utf8_error())))?; // UTF8 conversion -> FbxError -> FileSystemError


              // Read properties
              let mut properties = Vec::with_capacity(num_properties as usize); // Requires alloc
              let properties_start_pos = reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
              let expected_properties_end_pos = properties_start_pos.checked_add(property_list_len).ok_or(map_fbx_error_to_fs_error(FbxError::OffsetOverflow))?;

               for _ in 0..num_properties {
                   // Ensure we don't read properties beyond the declared property_list_len
                    if reader.stream_position().map_err(map_core_io_error_to_fs_error)? >= expected_properties_end_pos {
                        eprintln!("WARN: property_list_len ({})'den sonra hala okunacak özellik var.", property_list_len); // no_std print
                         break; // Stop reading properties
                    }
                   properties.push(read_fbx_property(reader)?); // This returns FileSystemError
               }

               // After reading properties, the reader should be at properties_start_pos + total size of properties read.
               // We need to seek to the expected end of the property list to start reading nested nodes.
               reader.seek(SeekFrom::Start(expected_properties_end_pos)).map_err(map_core_io_error_to_fs_error)?;


              // Read nested nodes
              let mut nested_nodes = Vec::new(); // Requires alloc
               let expected_nested_nodes_end_pos = end_offset.checked_sub(13) // EndOffset points AFTER the 13-byte null marker of the node
                   .ok_or_else(|| map_fbx_error_to_fs_error(FbxError::OffsetOverflow))?;

              // Read nested nodes until the current position reaches the expected end of nested nodes
              while reader.stream_position().map_err(map_core_io_error_to_fs_error)? < expected_nested_nodes_end_pos {
                   // Check if there are enough bytes left for a potential node header (at least 13 for null marker or 25 for header)
                   let current_pos = reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
                   let bytes_remaining_in_node = expected_nested_nodes_end_pos.checked_sub(current_pos)
                       .ok_or_else(|| map_fbx_error_to_fs_error(FbxError::OffsetOverflow))?;

                    if bytes_remaining_in_node < 13 {
                         // Not enough bytes for even a null marker, something is wrong
                         eprintln!("WARN: Kalan {} bayt, en az 13 bayt bekleniyor.", bytes_remaining_in_node); // no_std print
                         break; // Stop reading nested nodes
                    }

                  match read_fbx_node(reader)? { // Recursive call
                      Some(nested_node) => nested_nodes.push(nested_node),
                      None => {
                           // Found a null marker within nested nodes, this is the end of this node's nested list
                           // The read_fbx_node(reader)? call for None already consumed the 13 null bytes.
                           break; // Stop reading nested nodes for this parent
                      }
                  }
              }

               // After reading nested nodes, the reader's position should be at expected_nested_nodes_end_pos
               // or at the position after the null marker if one was encountered.
               // The next 13 bytes should be the null marker for the current node if it's not the root level.
               // If it's a root level node, there is no null marker after it, unless it's the very last node.
               // The end_offset points AFTER the current node's null marker.
               // Let's ensure the reader is at the position indicated by end_offset before returning.
               // This implicitly consumes the current node's null marker if it exists.
               reader.seek(SeekFrom::Start(end_offset)).map_err(map_core_io_error_to_fs_error)?;


              Ok(Some(FbxNode {
                  end_offset,
                  num_properties,
                  property_list_len,
                  name_len,
                  name,
                  properties,
                  nested_nodes,
              }))
         }
    }
}


/// Reads and parses an FBX file from the given path (std) or resource ID (no_std).
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing a Vec of root FbxNode or a FileSystemError.
#[cfg(feature = "std")]
pub fn read_fbx_file<P: AsRef<Path>>(file_path: P) -> Result<Vec<FbxNode>, FileSystemError> { // FileSystemError döner
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    let header = read_fbx_header(&mut reader)?; // This now returns FileSystemError
    println!("FBX Başlığı: {:?}", header); // std print

    let mut nodes = Vec::new(); // Use alloc::vec::Vec
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?; // Get file size
    reader.seek(SeekFrom::Start(reader.stream_position().map_err(map_std_io_error_to_fs_error)?)).map_err(map_std_io_error_to_fs_error)?; // Go back to current position after getting size

    // Read nodes until the end of the file.
    // Root level nodes don't have a surrounding node record.
    // They are read sequentially until the end of the file is reached,
    // or a 13-byte null marker is found at the very end of the file (optional).

    // The logic in read_fbx_node handles reading one node record including nested nodes and the trailing null marker.
    // We need to call read_fbx_node repeatedly until we hit EOF.
    // read_fbx_node returns None if it reads the 13-byte null marker.
    // The file might end with a 13-byte null marker or just end abruptly after the last node's data.
    // Let's read nodes until read_fbx_node returns None or an error, or we reach EOF.

    let current_pos_after_header = reader.stream_position().map_err(map_std_io_error_to_fs_error)?;

    while reader.stream_position().map_err(map_std_io_error_to_fs_error)? < file_size {
        // Check if there are enough bytes left for a potential node header (at least 13 for null marker or 25 for header)
        let current_pos = reader.stream_position().map_err(map_std_io_error_to_fs_error)?;
        let bytes_remaining_in_file = file_size.checked_sub(current_pos)
            .ok_or_else(|| FileSystemError::IOError(format!("Dosya sonu hesaplanırken hata")))?;

        if bytes_remaining_in_file < 13 {
             // Not enough bytes for a node header or null marker, likely unexpected EOF
             if bytes_remaining_in_file > 0 { // If some bytes are left but less than 13
                  eprintln!("WARN: Dosya sonuna yakın beklenmeyen {} bayt kaldı. En az 13 bayt bekleniyor.", bytes_remaining_in_file); // std print
             }
             break; // Stop reading
        }

        match read_fbx_node(&mut reader)? { // This returns Result<Option<FbxNode>, FileSystemError>
            Some(node) => nodes.push(node),
            None => {
                 // Found a null marker at the root level. This should typically be at the very end.
                 // If there are bytes left after this null marker, something is wrong.
                 let pos_after_null = reader.stream_position().map_err(map_std_io_error_to_fs_error)?;
                  if pos_after_null < file_size {
                       eprintln!("WARN: Null marker sonrası hala dosya verisi var. Kalan {} bayt.", file_size - pos_after_null); // std print
                       // Depending on strictness, this could be an error
                       // return Err(FileSystemError::InvalidData(format!("Null marker sonrası beklenmeyen veri.")));
                  }
                 break; // Stop reading after hitting the root level null marker
            }
        }
    }


    Ok(nodes)
}

#[cfg(not(feature = "std"))]
pub fn read_fbx_file(file_path: &str) -> Result<Vec<FbxNode>, FileSystemError> { // FileSystemError döner
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
    let mut reader = SahneResourceReader::new(handle, file_size);

    let header = read_fbx_header(&mut reader) // This now returns FileSystemError
        .map_err(|e| {
             let _ = resource::release(handle).map_err(|release_e| eprintln!("WARN: Kaynak serbest bırakma hatası after header read error: {:?}", release_e));
             e // Pass the original parsing error
         })?;

    crate::println!("FBX Başlığı: {:?}", header); // no_std print

    let mut nodes = Vec::new(); // Use alloc::vec::Vec
    let current_pos_after_header = reader.stream_position().map_err(map_core_io_error_to_fs_error)?;

    // Read nodes until the end of the file
    while reader.stream_position().map_err(map_core_io_error_to_fs_error)? < file_size {
        // Check if there are enough bytes left for a potential node header (at least 13 for null marker or 25 for header)
        let current_pos = reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
        let bytes_remaining_in_file = file_size.checked_sub(current_pos)
            .ok_or_else(|| FileSystemError::IOError(format!("Dosya sonu hesaplanırken hata")))?;

         if bytes_remaining_in_file < 13 {
              if bytes_remaining_in_file > 0 {
                   eprintln!("WARN: Dosya sonuna yakın beklenmeyen {} bayt kaldı. En az 13 bayt bekleniyor.", bytes_remaining_in_file); // no_std print
              }
             break; // Stop reading
         }


        match read_fbx_node(&mut reader)? { // This returns Result<Option<FbxNode>, FileSystemError>
            Some(node) => nodes.push(node),
            None => {
                 // Found a null marker at the root level.
                  let pos_after_null = reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
                   if pos_after_null < file_size {
                        eprintln!("WARN: Null marker sonrası hala dosya verisi var. Kalan {} bayt.", file_size - pos_after_null); // no_std print
                   }
                 break; // Stop reading after hitting the root level null marker
            }
        }
    }


    // Kaynağı serbest bırak
    let _ = resource::release(handle).map_err(|e| {
         eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e);
         map_sahne_error_to_fs_error(e)
     });

    Ok(nodes)
}


// Example main functions
#[cfg(feature = "example_fbx")] // Different feature flag
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     #[cfg(not(feature = "std"))]
     {
          eprintln!("FBX file example (no_std) starting...");
          // TODO: Call init_console(crate::Handle(3)); if needed
     }
     #[cfg(feature = "std")]
     {
          eprintln!("FBX file example (std) starting...");
     }

     // Test with a hypothetical file/resource ID
     let file_path_or_resource_id = "sahne://files/my_model.fbx";

     match read_fbx_file(file_path_or_resource_id) { // This function is now #[cfg(feature = "std")] or #[cfg(not(feature = "std"))]
         Ok(nodes) => {
              #[cfg(not(feature = "std"))]
              crate::println!("FBX Dosyası Başarıyla Okundu ve Ayrıştırıldı.\n");
              #[cfg(feature = "std")]
              println!("FBX Dosyası Başarıyla Okundu ve Ayrıştırıldı.\n");

             // Kök düğümleri işle
              #[cfg(not(feature = "std"))]
             for node in &nodes {
                 crate::println!("Kök Düğüm: {:?}", node.name);
             }
             #[cfg(feature = "std")]
             for node in &nodes {
                 println!("Kök Düğüm: {:?}", node.name);
             }

             // Example of printing full node structure (if needed)
             // #[cfg(not(feature = "std"))]
             // for node in &nodes {
             //      print_node_recursive(node, 0);
             // }
             // #[cfg(feature = "std")]
             // for node in &nodes {
             //      print_node_recursive(node, 0);
             // }

         }
         Err(err) => {
              #[cfg(not(feature = "std"))]
              crate::eprintln!("FBX dosyası okuma hatası: {}", err);
              #[cfg(feature = "std")]
              eprintln!("FBX dosyası okuma hatası: {}", err);
              return Err(err);
         }
     }

     #[cfg(not(feature = "std"))]
     eprintln!("FBX file example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("FBX file example (std) finished.");

     Ok(())
}


// (Opsiyonel) Düğüm yapısını recursive olarak yazdırma fonksiyonu (detaylı inceleme için)
#[allow(dead_code)] // Şimdilik kullanılmadığı için uyarıyı kapat
fn print_node_recursive(node: &FbxNode, indent_level: usize) {
    let indent = alloc::string::String::from("  ").repeat(indent_level); // Use alloc::string::String
    #[cfg(feature = "std")]
    println!("{}{}: {:?}", indent, node.name, node.properties);
    #[cfg(not(feature = "std"))]
    crate::println!("{}{}: {:?}", indent, node.name, node.properties);
    for nested_node in &node.nested_nodes {
        print_node_recursive(nested_node, indent_level + 1);
    }
}


// Test module (requires mock Sahne64 or std FS for testing)
#[cfg(test)]
mod tests {
     // Needs byteorder for creating dummy data and std::io::Cursor for testing Read+Seek
     #[cfg(feature = "std")]
     use std::io::Cursor;
     #[cfg(feature = "std")]
     use byteorder::{LittleEndian as StdLittleEndian, WriteBytesExt as StdWriteBytesExt};
     #[cfg(feature = "std")]
     use std::io::{Read, Seek, SeekFrom, Write};

     use super::*; // Import items from the parent module
     use alloc::vec; // vec! macro
     use alloc::string::ToString; // to_string() for string conversion in tests

     // Helper to create a dummy FBX file header + a few nodes in memory
     #[cfg(feature = "std")] // This helper uses std::io and byteorder write
     fn create_test_fbx_data(nodes_data: &[u8]) -> Vec<u8> {
          let mut buffer = Vec::new();

          // FBX Header
          buffer.extend_from_slice(b"Kaydaz FBX \0\x1A\x00"); // Magic + Unknown
          buffer.write_u32::<StdLittleEndian>(7500).unwrap(); // Version (example)

          // Nodes Data + Null Marker
          buffer.extend_from_slice(nodes_data);

           // Add a final 13-byte null marker at the very end (optional in some FBX versions)
           // If nodes_data doesn't end with a null marker, add one.
           if nodes_data.len() < 13 || !nodes_data[(nodes_data.len() - 13)..].iter().all(|&b| b == 0) {
               buffer.extend_from_slice(&[0u8; 13]);
           }


          buffer
     }

     // Helper to write a single node record (excluding the final 13-byte null marker)
     #[cfg(feature = "std")] // This helper uses std::io and byteorder write
     fn write_node_record<W: Write + Seek>(writer: &mut W, node: &FbxNode) -> Result<(), StdIOError> {
          let start_pos = writer.stream_position()?;

          // Placeholder for EndOffset, NumProperties, PropertyListLen (will fill later)
          writer.write_u64::<StdLittleEndian>(0)?; // EndOffset
          writer.write_u64::<StdLittleEndian>(0)?; // NumProperties
          writer.write_u64::<StdLittleEndian>(0)?; // PropertyListLen
          writer.write_u8(node.name_len)?;
          writer.write_all(node.name.as_bytes())?;

          let properties_start_pos = writer.stream_position()?;

          // Write properties
          for prop in &node.properties {
               match prop {
                    FbxProperty::Bool(v) => { writer.write_u8(b'C')?; writer.write_u8(*v as u8)?; },
                    FbxProperty::Integer(v) => { writer.write_u8(b'I')?; writer.write_i32::<StdLittleEndian>(*v)?; },
                    FbxProperty::Long(v) => { writer.write_u8(b'L')?; writer.write_i64::<StdLittleEndian>(*v)?; },
                    FbxProperty::Float(v) => { writer.write_u8(b'F')?; writer.write_f32::<StdLittleEndian>(*v)?; },
                    FbxProperty::Double(v) => { writer.write_u8(b'D')?; writer.write_f64::<StdLittleEndian>(*v)?; },
                    FbxProperty::String(s) => { writer.write_u8(b'S')?; writer.write_u32::<StdLittleEndian>(s.len() as u32)?; writer.write_all(s.as_bytes())?; },
                    FbxProperty::RawBytes(b) => { writer.write_u8(b'R')?; writer.write_u32::<StdLittleEndian>(b.len() as u32)?; writer.write_all(b)?; },
                    // TODO: Handle arrays
               }
          }

          let properties_end_pos = writer.stream_position()?;
          let property_list_len = properties_end_pos - properties_start_pos;

          // Write nested nodes
          for nested_node in &node.nested_nodes {
               write_node_record(writer, nested_node)?; // Recursive call
          }

           // Write the 13-byte null marker for this node's nested list
           writer.write_all(&[0u8; 13])?;

          let node_end_pos = writer.stream_position()?;
          let end_offset = node_end_pos; // EndOffset is the position AFTER this node's null marker.

          // Go back and fill in the header fields
          writer.seek(SeekFrom::Start(start_pos))?;
          writer.write_u64::<StdLittleEndian>(end_offset)?;
          writer.write_u64::<StdLittleEndian>(node.properties.len() as u64)?; // Use actual count
          writer.write_u64::<StdLittleEndian>(property_list_len)?;
          writer.seek(SeekFrom::Start(node_end_pos))?; // Go back to the end

          Ok(())
     }


     // Helper function to read FBX from a generic reader (similar to internal logic)
     // This needs to be adapted to return FileSystemError
      fn read_fbx_from_reader<R: Read + Seek>(reader: &mut R) -> Result<Vec<FbxNode>, FileSystemError> {
          let header = read_fbx_header(reader)?; // Returns FileSystemError
          // println!("FBX Header (Test): {:?}", header); // Use println! in test

          let mut nodes = Vec::new();
          let file_size = reader.seek(SeekFrom::End(0)).map_err(|e| FileSystemError::IOError(format!("Seek End Error: {:?}", e)))?;
          reader.seek(SeekFrom::Start(reader.stream_position().map_err(|e| FileSystemError::IOError(format!("Stream Position Error: {:?}", e)))?)).map_err(|e| FileSystemError::IOError(format!("Seek Start Error: {:?}", e)))?;

          let current_pos_after_header = reader.stream_position().map_err(|e| FileSystemError::IOError(format!("Stream Position Error: {:?}", e)))?;

           while reader.stream_position().map_err(|e| FileSystemError::IOError(format!("Stream Position Error: {:?}", e)))? < file_size {
                let current_pos = reader.stream_position().map_err(|e| FileSystemError::IOError(format!("Stream Position Error: {:?}", e)))?;
                let bytes_remaining_in_file = file_size.checked_sub(current_pos)
                    .ok_or_else(|| FileSystemError::IOError(format!("Dosya sonu hesaplanırken hata")))?;

                 if bytes_remaining_in_file < 13 {
                     break; // Stop reading
                 }

               match read_fbx_node(reader)? { // This returns Result<Option<FbxNode>, FileSystemError>
                   Some(node) => nodes.push(node),
                   None => {
                        let pos_after_null = reader.stream_position().map_err(|e| FileSystemError::IOError(format!("Stream Position Error: {:?}", e)))?;
                         if pos_after_null < file_size {
                              eprintln!("WARN: Null marker sonrası hala dosya verisi var. Kalan {} bayt.", file_size - pos_after_null);
                         }
                        break;
                   }
               }
           }

          Ok(nodes)
      }


     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_read_write_fbx_in_memory() -> Result<(), FileSystemError> {
          // Create dummy nodes
          let prop1 = FbxProperty::Integer(123);
          let prop2 = FbxProperty::String(String::from("TestString"));
          let prop3 = FbxProperty::Double(45.67);

          let nested_node = FbxNode {
               end_offset: 0, num_properties: 0, property_list_len: 0, name_len: 5, name: String::from("Child"), properties: vec![FbxProperty::Bool(true)], nested_nodes: vec![], // Placeholder lengths/offsets
          };

          let root_node = FbxNode {
               end_offset: 0, num_properties: 0, property_list_len: 0, name_len: 4, name: String::from("Root"), properties: vec![prop1, prop2, prop3], nested_nodes: vec![nested_node], // Placeholder lengths/offsets
          };

          // Write nodes to an in-memory cursor to simulate file content
          let mut cursor = Cursor::new(Vec::new());
          // write_node_record(&mut cursor, &root_node)?; // Write the root node record
          // The root level in FBX binary doesn't have a surrounding node record like nested nodes.
          // Instead, the root level nodes are written sequentially followed by an optional null marker at the end.
          // Let's write the nodes directly without the outer write_node_record structure.
          // This requires manually writing the node header for root nodes too.

          // Simplified structure for testing: Write header, then write one root node record, then the final null marker.
          // This doesn't fully simulate multiple root nodes but tests the node parsing logic.

          // Let's create the full FBX data including header and one root node + its children and the final null marker
          let mut nodes_cursor = Cursor::new(Vec::new());
           write_node_record(&mut nodes_cursor, &root_node).unwrap(); // Write the root node structure
          let nodes_data = nodes_cursor.into_inner();

          let fbx_data = create_test_fbx_data(&nodes_data); // Add FBX header and final null marker


          // Read the FBX data back from the in-memory cursor
          let mut read_cursor = Cursor::new(fbx_data.clone()); // Clone to keep original data for comparison

          let loaded_nodes = read_fbx_from_reader(&mut read_cursor)?; // This calls read_fbx_header and then reads nodes

          // Assert the loaded nodes match the original structure
          assert_eq!(loaded_nodes.len(), 1);
          let loaded_root_node = &loaded_nodes[0];

          assert_eq!(loaded_root_node.name, "Root");
          assert_eq!(loaded_root_node.properties.len(), 3);
          assert_eq!(loaded_root_node.properties[0], FbxProperty::Integer(123));
          assert_eq!(loaded_root_node.properties[1], FbxProperty::String(String::from("TestString")));
          assert_eq!(loaded_root_node.properties[2], FbxProperty::Double(45.67));

          assert_eq!(loaded_root_node.nested_nodes.len(), 1);
          let loaded_nested_node = &loaded_root_node.nested_nodes[0];
          assert_eq!(loaded_nested_node.name, "Child");
          assert_eq!(loaded_nested_node.properties.len(), 1);
          assert_eq!(loaded_nested_node.properties[0], FbxProperty::Bool(true));
          assert_eq!(loaded_nested_node.nested_nodes.len(), 0);

          // Check calculated lengths and offsets in the loaded nodes (should match what was written)
          // Need to verify the end_offset, num_properties, property_list_len in the loaded nodes
          // These were placeholders when creating root_node and nested_node.
          // The write_node_record fills them in during writing.
          // Let's re-parse the written data manually or use the loaded_root_node to check the values.

          // Re-read the written data using cursor to check field values
          let mut check_cursor = Cursor::new(fbx_data.clone());
          let _ = read_fbx_header(&mut check_cursor).unwrap(); // Skip header

           // Read the first root node's header fields directly
           let root_node_start_pos = check_cursor.stream_position().unwrap();
           let loaded_root_end_offset = check_cursor.read_u64::<StdLittleEndian>().unwrap();
           let loaded_root_num_properties = check_cursor.read_u64::<StdLittleEndian>().unwrap();
           let loaded_root_property_list_len = check_cursor.read_u64::<StdLittleEndian>().unwrap();
           let loaded_root_name_len = check_cursor.read_u8().unwrap();
           let mut loaded_root_name_bytes = vec![0; loaded_root_name_len as usize];
           check_cursor.read_exact(&mut loaded_root_name_bytes).unwrap();
           let loaded_root_name = String::from_utf8(loaded_root_name_bytes).unwrap();

           assert_eq!(loaded_root_name, "Root");
           assert_eq!(loaded_root_num_properties, root_node.properties.len() as u64);

           // Check the calculated property_list_len
           let expected_property_list_len: u64 = loaded_root_node.properties.iter().map(|p| {
               match p {
                   FbxProperty::Bool(_) => 1 + 1, // type_code + data
                   FbxProperty::Integer(_) => 1 + 4,
                   FbxProperty::Long(_) => 1 + 8,
                   FbxProperty::Float(_) => 1 + 4,
                   FbxProperty::Double(_) => 1 + 8,
                   FbxProperty::String(s) => 1 + 4 + s.len() as u64, // type_code + len (u32) + data
                   FbxProperty::RawBytes(b) => 1 + 4 + b.len() as u64, // type_code + len (u32) + data
                    // TODO: Handle arrays
                   _ => panic!("Unsupported property type in test"),
               }
           }).sum();
           assert_eq!(loaded_root_property_list_len, expected_property_list_len);


           // Check the calculated end_offset for the root node
           let root_node_header_size = 25u64; // 8+8+8+1
           let root_node_content_size = loaded_root_property_list_len + // properties
                                       // Size of nested nodes + their null markers
                                       loaded_root_node.nested_nodes.iter().map(|n| {
                                            // Size of nested node header + properties + nested nodes + 13-byte null marker
                                            // This is complex to calculate manually.
                                            // Let's rely on the loaded_nested_node's end_offset relative to its start pos.
                                            // The loaded_nested_node.end_offset is the position after its null marker.
                                            // The size of the nested node record including its null marker is loaded_nested_node.end_offset - nested_node_start_pos.
                                            let nested_node_start_pos_in_file = root_node_start_pos + root_node_header_size + loaded_root_property_list_len; // Start of nested nodes after root properties
                                            let nested_node_relative_start_pos = check_cursor.stream_position().unwrap(); // Current pos after root properties
                                            let nested_node_total_size = loaded_nested_node.end_offset - nested_node_relative_start_pos; // This should be the size of the nested node record including its null marker

                                            nested_node_total_size
                                       }).sum::<u64>();

           let expected_root_end_offset = root_node_start_pos + root_node_header_size + loaded_root_property_list_len + root_node_content_size + 13; // + 13 bytes for the root node's null marker
           // Note: The root node record in a binary FBX file does NOT have a leading node record header (end_offset, etc.).
           // Only nested nodes have the node record header.
           // The file structure is Header | NodeRecord1 | NodeRecord2 | ... | OptionalNullMarker (13 bytes)
           // So the parsing logic for root nodes in read_fbx_file needs to be different from nested nodes.
           // read_fbx_node assumes it's reading a node record header.
           // The while loop in read_fbx_file should call read_fbx_node.
           // If read_fbx_node parses a node, it consumes the node record + its nested nodes + its null marker, and the cursor is positioned AFTER that null marker.
           // The next call to read_fbx_node will read the next node record header.
           // If read_fbx_node returns None, it means it read the 13-byte null marker.

           // Let's assume the test setup with write_node_record simulates writing a root node record for testing read_fbx_node.
           // In a real scenario, read_fbx_file needs to handle the root level structure correctly.
           // The current read_fbx_file logic seems to be designed to read root nodes by repeatedly calling read_fbx_node,
           // which implies the root level also uses node records, which is contrary to the binary FBX specification.

           // Let's adjust the test to create data that *mimics* the structure read_fbx_node expects at the root level,
           // i.e., a node record header + content + null marker.

           // Re-evaluate create_test_fbx_data and write_node_record:
           // create_test_fbx_data should create the full file: Header | RootNodeRecord | OptionalFinalNullMarker
           // write_node_record should write a node record (Header + Properties + NestedNodes + NullMarker)

           // Let's try writing the root node using write_node_record into the buffer that follows the FBX header.
           let mut nodes_cursor = Cursor::new(Vec::new());
           write_node_record(&mut nodes_cursor, &root_node).unwrap(); // This writes the root node record including its null marker
           let nodes_data = nodes_cursor.into_inner(); // This contains the byte representation of the root node record

           // The root level of a binary FBX is a sequence of node records.
           // The file structure is Header | NodeRecord1 | NodeRecord2 | ... | NodeRecordN | OptionalFinalNullMarker (13 zero bytes)

           // Let's create a simple FBX file with Header and ONE root node record followed by the final null marker.
           let mut file_buffer = Cursor::new(Vec::new());
           // Write Header
           file_buffer.extend_from_slice(b"Kaydaz FBX \0\x1A\x00"); // Magic + Unknown
           file_buffer.write_u32::<StdLittleEndian>(7500).unwrap(); // Version

           // Write the root node record
           write_node_record(&mut file_buffer, &root_node).unwrap();

           // Add the final 13-byte null marker
            file_buffer.extend_from_slice(&[0u8; 13]).unwrap();

           let fbx_data_correct_structure = file_buffer.into_inner();


           // Read the data back using the corrected structure test data
           let mut read_cursor_correct = Cursor::new(fbx_data_correct_structure.clone());
            let loaded_nodes_correct = read_fbx_from_reader(&mut read_cursor_correct)?;

            // Assert the loaded nodes match the original structure (same assertion logic as before)
            assert_eq!(loaded_nodes_correct.len(), 1);
            let loaded_root_node_correct = &loaded_nodes_correct[0];

            assert_eq!(loaded_root_node_correct.name, "Root");
            assert_eq!(loaded_root_node_correct.properties.len(), 3);
            assert_eq!(loaded_root_node_correct.properties[0], FbxProperty::Integer(123));
            assert_eq!(loaded_root_node_correct.properties[1], FbxProperty::String(String::from("TestString")));
            assert_eq!(loaded_root_node_correct.properties[2], FbxProperty::Double(45.67));

            assert_eq!(loaded_root_node_correct.nested_nodes.len(), 1);
            let loaded_nested_node_correct = &loaded_root_node_correct.nested_nodes[0];
            assert_eq!(loaded_nested_node_correct.name, "Child");
            assert_eq!(loaded_nested_node_correct.properties.len(), 1);
            assert_eq!(loaded_nested_node_correct.properties[0], FbxProperty::Bool(true));
            assert_eq!(loaded_nested_node_correct.nested_nodes.len(), 0);

            // Check the end_offset of the loaded root node
            let loaded_root_node_start_pos_in_file = 23u64; // After the 23-byte header
            let expected_root_node_size_including_null = loaded_root_node_correct.end_offset - loaded_root_node_start_pos_in_file;

             // Calculate the size of the root node record manually based on the data we wrote
             let root_node_header_size = 25u64;
             let nested_node_header_size = 25u64;

             let nested_node_properties_size: u64 = loaded_nested_node_correct.properties.iter().map(|p| {
                 match p {
                     FbxProperty::Bool(_) => 1 + 1,
                     _ => panic!("Unsupported nested property type in test"),
                 }
             }).sum();
             let nested_node_size_including_null = nested_node_header_size + nested_node_properties_size + 13; // Nested node has a 13-byte null marker

             let root_node_properties_size: u64 = loaded_root_node_correct.properties.iter().map(|p| {
                  match p {
                       FbxProperty::Integer(_) => 1 + 4,
                       FbxProperty::String(s) => 1 + 4 + s.len() as u64,
                       FbxProperty::Double(_) => 1 + 8,
                       _ => panic!("Unsupported root property type in test"),
                  }
             }).sum();

             let root_node_content_size = root_node_properties_size + nested_node_size_including_null;
             let expected_root_node_size = root_node_header_size + root_node_content_size + 13; // Root node record size including its null marker


            // The EndOffset points AFTER the node's own 13-byte null marker.
            // So the size of the node record is EndOffset - StartPos.
            // The root node starts at offset 23. Its end_offset is loaded_root_node_correct.end_offset.
            // The size of the root node record should be loaded_root_node_correct.end_offset - 23.

            let calculated_root_node_size = loaded_root_node_correct.end_offset - loaded_root_node_start_pos_in_file;
            // Need to calculate the expected size based on the written data manually.

            // Let's check the final position of the read cursor after parsing.
            // It should be just before the final 13-byte null marker of the file.
             let final_null_marker_start_pos = file_size - 13; // Assuming file_size is known in test
             let final_file_size = fbx_data_correct_structure.len();
             let expected_cursor_pos_after_nodes = final_file_size - 13;

             assert_eq!(read_cursor_correct.stream_position().unwrap(), expected_cursor_pos_after_nodes as u64);


           Ok(())
     }

     // TODO: Add tests for error conditions (invalid magic, truncated file, unknown property types, invalid offsets/lengths)
     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
}

// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_fbx", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
