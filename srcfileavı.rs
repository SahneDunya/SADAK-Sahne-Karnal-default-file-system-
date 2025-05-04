#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel tipleri içeri aktar
use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri ve Handle
// Sahne64 resource modülü (no_std implementasyonu için)
#[cfg(not(feature = "std"))]
use crate::resource;

// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Result as StdResult, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::string::String as StdString; // std String

// alloc crate for String, Vec, format! etc.
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

// core::io traits and types
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io
use core::cmp; // core::cmp

// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülden import edildiği varsayılır

// Sahne64 Resource Control Constants (Hypothetical - copied from srcfilealac.rs)
#[cfg(not(feature = "std"))]
mod sahne64_resource_controls {
    pub const CONTROL_SEEK: u64 = 1;
    pub const CONTROL_SEEK_FROM_START: u64 = 1;
    pub const CONTROL_SEEK_FROM_CURRENT: u64 = 2;
    pub const CONTROL_SEEK_FROM_END: u64 = 3;
    pub const CONTROL_GET_SIZE: u64 = 4;
}
#[cfg(not(feature = "std"))]
use sahne64_resource_controls::*; // Use the hypothetical constants

// Helper struct to implement core::io::Read and Seek for Sahne64 Handle (copied from srcfilealac.rs)
// This struct should ideally be in a common module if used across multiple files.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
}

#[cfg(not(feature = "std"))]
impl SahneResourceReader {
    pub fn new(handle: Handle) -> Self {
        SahneResourceReader { handle }
    }

    // Helper to map SahneError to core::io::Error
    fn map_sahne_error_to_io_error(e: SahneError) -> CoreIOError {
        // Map SahneError variants to appropriate CoreIOErrorKind
        CoreIOError::new(CoreIOErrorKind::Other, format!("SahneError: {:?}", e)) // Using Other and formatting Debug output
        // TODO: Implement a proper mapping based on SahneError variants
    }
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        resource::read(self.handle, buf).map_err(Self::map_sahne_error_to_io_error)
    }
    // read_exact has a default implementation based on read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        let (command, offset_arg) = match pos {
            SeekFrom::Start(o) => (CONTROL_SEEK_FROM_START, o),
            SeekFrom::End(o) => {
                 println!("WARN: SahneResourceReader::seek from End offset i64 ({}) being cast to u64 for syscall.", o);
                (CONTROL_SEEK_FROM_END, o as u64) // DİKKAT: Negatif i64 casting to u64 tehlikeli olabilir!
            },
            SeekFrom::Current(o) => {
                 println!("WARN: SahneResourceReader::seek from Current offset i64 ({}) being cast to u64 for syscall.", o);
                (CONTROL_SEEK_FROM_CURRENT, o as u64) // DİKKAT: Negatif i64 casting to u64 tehlikeli olabilir!
            },
        };

        let result = resource::control(self.handle, command, offset_arg, 0);

        match result {
            Ok(new_pos_i64) => {
                if new_pos_i64 < 0 {
                     println!("ERROR: SahneResourceReader::seek resource::control returned negative position: {}", new_pos_i64);
                    Err(CoreIOError::new(CoreIOErrorKind::Other, format!("Seek syscall returned negative position: {}", new_pos_i64)))
                } else {
                    Ok(new_pos_i64 as u64) // i64 -> u64 dönüşümü (yeni pozisyon)
                }
            },
            Err(e) => {
                 println!("ERROR: SahneResourceReader::seek resource::control hatası: {:?}", e);
                Err(Self::map_sahne_error_to_io_error(e))
            }
        }
    }
    // stream_position has a default implementation based on seek
}


// AVI Başlık Yapıları için Sabitler
const CKID_RIFF: u32 = 0x46464952; // "RIFF" (Little Endian)
const CKID_AVI : u32 = 0x20495641; // "AVI " (Little Endian)
const CKID_LIST: u32 = 0x5453494C; // "LIST" (Little Endian)
const CKID_hdrl: u32 = 0x6C726468; // "hdrl" (Little Endian)
const CKID_avih: u32 = 0x68697661; // "avih" (Little Endian)
const CKID_strl: u32 = 0x6C727473; // "strl" (Little Endian)
const CKID_strh: u32 = 0x68727473; // "strh" (Little Endian)
const CKID_strf: u32 = 0x66727473; // "strf" (Little Endian)
const CKID_movi: u32 = 0x69766F6D; // "movi" (Little Endian)
const CKID_00dc: u32 = 0x63643030; // "00dc" - Video Stream 0 (Little Endian)
const CKID_01wb: u32 = 0x62773130; // "01wb" - Audio Stream 1 (örnek) (Little Endian)
const CKID_idx1: u32 = 0x31786469; // "idx1" (Little Endian)

// Helper function to convert FourCC to String
fn fourcc_to_string(fourcc: u32) -> String {
    let bytes = fourcc.to_le_bytes();
    // core::str::from_utf8 kullanırız, format! için alloc gerekir.
    // Hata durumunda "???" döneriz.
    format!("{}", core::str::from_utf8(&bytes).unwrap_or("???"))
}

#[derive(Debug)]
struct AviMainHeader {
    dwMicroSecPerFrame: u32,
    dwMaxBytesPerSec: u32,
    dwPaddingGranularity: u32,
    dwFlags: u32,
    dwTotalFrames: u32,
    dwInitialFrames: u32,
    dwStreams: u32,
    dwSuggestedBufferSize: u32,
    dwWidth: u32,
    dwHeight: u32,
    dwSampleSize: u32,
    dwReserved: [u32; 4],
}

#[derive(Debug)]
struct AviStreamHeader {
    fccType: u32, // 'vids', 'auds', 'mids', 'txts'
    fccHandler: u32, // Codec FourCC
    dwFlags: u32,
    dwPriority: u16,
    dwLanguage: u16,
    dwInitialFrames: u32,
    dwScale: u32, // Samples per dwRate (e.g., audio sample count per second)
    dwRate: u32, // Time base (e.g., 1 second)
    dwStart: u32, // Starting time
    dwLength: u32, // Length in dwScale units
    dwSuggestedBufferSize: u32,
    dwSampleSize: u32, // Size of sample if fixed (audio), 0 if variable
    rcFrame: [i16; 4], // Video frame rectangle
}

#[derive(Debug)]
struct AviChunk {
    id: u32, // FourCC
    size: u32, // Size of the data part
    data: Vec<u8>, // Chunk data (can be large!)
}

#[derive(Debug)]
struct AviData {
    main_header: AviMainHeader,
    stream_headers: Vec<AviStreamHeader>,
    chunks: Vec<AviChunk>, // Only movie data chunks and maybe idx1
}

// Helper function to read a u32 from the reader (Little Endian)
#[cfg(feature = "std")] // std version uses std::io::Read
fn read_u32_le<R: StdRead>(reader: &mut R) -> Result<u32, StdIOError> {
    let mut buffer = [0u8; 4];
    reader.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))] // no_std version uses core::io::Read
fn read_u32_le<R: Read>(reader: &mut R) -> Result<u32, CoreIOError> {
    let mut buffer = [0u8; 4];
    reader.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

// Helper function to read a u16 from the reader (Little Endian)
#[cfg(feature = "std")] // std version uses std::io::Read
fn read_u16_le<R: StdRead>(reader: &mut R) -> Result<u16, StdIOError> {
    let mut buffer = [0u8; 2];
    reader.read_exact(&mut buffer)?;
    Ok(u16::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))] // no_std version uses core::io::Read
fn read_u16_le<R: Read>(reader: &mut R) -> Result<u16, CoreIOError> {
    let mut buffer = [0u8; 2];
    reader.read_exact(&mut buffer)?;
    Ok(u16::from_le_bytes(buffer))
}

// Helper function to read an i16 from the reader (Little Endian)
#[cfg(feature = "std")] // std version uses std::io::Read
fn read_i16_le<R: StdRead>(reader: &mut R) -> Result<i16, StdIOError> {
    let mut buffer = [0u8; 2];
    reader.read_exact(&mut buffer)?;
    Ok(i16::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))] // no_std version uses core::io::Read
fn read_i16_le<R: Read>(reader: &mut R) -> Result<i16, CoreIOError> {
    let mut buffer = [0u8; 2];
    reader.read_exact(&mut buffer)?;
    Ok(i16::from_le_bytes(buffer))
}

// Helper function to skip bytes in the reader using seek
#[cfg(feature = "std")] // std version uses std::io::Seek
fn skip_bytes<R: StdRead + StdSeek>(reader: &mut R, count: u64) -> Result<(), StdIOError> {
    // Using seek is much more efficient than reading into a buffer
    reader.seek(StdSeekFrom::Current(count as i64))?;
    Ok(())
}

#[cfg(not(feature = "std"))] // no_std version uses core::io::Seek
fn skip_bytes<R: Read + Seek>(reader: &mut R, count: u64) -> Result<(), CoreIOError> {
    // Using seek is much more efficient than reading into a buffer
    reader.seek(SeekFrom::Current(count as i64))?; // core::io::SeekFrom ve core::io::Seek kullanılır
    Ok(())
}


/// Parses an AVI file and extracts its structure and main headers.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - Path to the file (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A `Result` containing the parsed `AviData` or a `FileSystemError`.
pub fn parse_avi(file_path_or_resource_id: &str) -> Result<AviData, FileSystemError> {
    #[cfg(feature = "std")]
    {
        let file = File::open(file_path_or_resource_id).map_err(map_std_io_error_to_fs_error)?;
        let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

        // Parsing logic using reader (implements StdRead + StdSeek)
        let avi_data_result = parse_avi_internal(&mut reader).map_err(map_std_io_error_to_fs_error)?;
        Ok(avi_data_result)
    }
    #[cfg(not(feature = "std"))]
    {
        // Kaynağı edin
        let handle = resource::acquire(file_path_or_resource_id, resource::MODE_READ)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        // Sahne64 Handle'ı için core::io::Read + Seek implementasyonu sağlayan Reader struct'ı oluştur
        let mut reader = SahneResourceReader::new(handle);

        // Parsing logic using reader (implements core::io::Read + core::io::Seek)
        let avi_data_result = parse_avi_internal(&mut reader).map_err(map_core_io_error_to_fs_error)?;

        // Kaynağı serbest bırak
        let _ = resource::release(handle).map_err(|e| {
             eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e);
             map_sahne_error_to_fs_error(e)
         });

        Ok(avi_data_result)
    }
}

// Internal parsing logic independent of the underlying reader type (as long as it implements Read + Seek)
#[cfg(feature = "std")] // Needs to compile in std too, takes StdRead + StdSeek
fn parse_avi_internal<R: StdRead + StdSeek>(reader: &mut R) -> Result<AviData, StdIOError> { // Returns StdIOError
     use core::mem::size_of; // size_of needs to be available

     // RIFF Başlığını Okuma ve Doğrulama
     let riff_header = read_u32_le(reader)?;
     if riff_header != CKID_RIFF {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz RIFF başlığı: {:x}", riff_header)));
     }

     let file_size = read_u32_le(reader)?; // Dosya boyutunu okur (header'da)
     let avi_header_ckid = read_u32_le(reader)?;
     if avi_header_ckid != CKID_AVI {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz AVI CKID: {:x}", avi_header_ckid)));
     }

     // LIST 'hdrl' Başlığını Okuma ve Doğrulama
     let list_hdrl_ckid = read_u32_le(reader)?;
     if list_hdrl_ckid != CKID_LIST {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Beklenen LIST CKID bulunamadı: {:x}", list_hdrl_ckid)));
     }
     let list_hdrl_size = read_u32_le(reader)?; // LIST boyutu (hdrl dahil)
     let hdrl_ckid = read_u32_le(reader)?;
     if hdrl_ckid != CKID_hdrl {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Beklenen hdrl CKID bulunamadı: {:x}", hdrl_ckid)));
     }

     // 'avih' (AVI Main Header) Başlığını Okuma ve Ayrıştırma
     let avih_ckid = read_u32_le(reader)?;
     let avih_size = read_u32_le(reader)?;
     if avih_ckid != CKID_avih {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Beklenen avih CKID bulunamadı: {:x}", avih_ckid)));
     }
     // AviMainHeader boyutu 56 bayttır. avih boyutu da 56 olmalıdır.
     if avih_size as usize != size_of::<AviMainHeader>() {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz avih başlık boyutu: {}", avih_size)));
     }

     let main_header = parse_avi_main_header(reader)?;

     // hdrl LIST'indeki diğer atomları atla (strl, dmlh, vb. - avih'den sonraki kısım)
     // Güncel pozisyonu alalım
     let current_pos_after_avih = reader.stream_position()?;
     // hdrl LIST'inin sonu = hdrl LIST'inin başlangıcı + hdrl LIST'inin boyutu
     // RIFF başlığından sonra (8. byte) LIST 'hdrl' başlar.
     // LIST 'hdrl' başlangıcı = 12. byte (RIFF size + type + AVI type + LIST type = 4+4+4+4 = 16? No, 8 bytes size+type for each atom/list)
      RIFF (size + type) = 8 bytes
      AVI (type) = 4 bytes
     // LIST (size + type) = 8 bytes -> hdrl LIST starts at byte 12 (0-indexed) in the original file
     // The first LIST header (size+type) is at offset 8. The 'hdrl' type is at offset 12.
     // So the hdrl LIST starts at offset 8 + 4 (List size) + 4 (List type 'LIST'). RIFF size + type is 8. AVI type is 4.
     // RIFF Chunk: CKID(4) + Size(4) = 8 bytes.
     //   'AVI ' type: 4 bytes. Total RIFF header = 12 bytes.
     //   LIST 'hdrl' Chunk: CKID(4) + Size(4) = 8 bytes.
     //     'hdrl' type: 4 bytes.
     //     'avih' Chunk: CKID(4) + Size(4) = 8 bytes. + Data (56 bytes for AviMainHeader)
     // Total read so far: 8 (RIFF) + 4 (AVI) + 8 (LIST hdrl) + 4 (hdrl type) + 8 (avih header) + 56 (avih data) = 88 bytes.
     // hdrl LIST size starts from the LIST header (8 bytes before 'hdrl' type).
     // Current position is after reading avih data (56 bytes).
     // Position after avih data = Start of file + 8 (RIFF) + 4 (AVI) + 8 (LIST hdrl header) + 4 (hdrl type) + 8 (avih header) + 56 (avih data) = 88.
     // List hdrl starts at offset 8 + 4 (AVI type) = 12. So list_hdrl_offset = 12.
     // List hdrl total size = list_hdrl_offset + list_hdrl_size = 12 + list_hdrl_size.
     // We are currently at 88 bytes.
     // The data we need to skip in the hdrl LIST is from the end of avih header + data to the end of the hdrl LIST.
     // End of hdrl LIST = Start of hdrl LIST + list_hdrl_size.
     // Start of hdrl LIST = file_start + offset_to_hdrl_list_header = 0 + 12.
     // End of hdrl LIST = 12 + list_hdrl_size.
     // We are at current_pos_after_avih.
     // Bytes remaining in hdrl LIST = (12 + list_hdrl_size) - current_pos_after_avih.
     // This calculation seems complex and prone to errors with relative/absolute offsets.
     // A simpler approach: After parsing avih, the remaining size in the 'hdrl' LIST is list_hdrl_size - (avih_size + 8) - 4 (hdrl type) - 8 (avih header).
     // No, the LIST size includes its own header (8 bytes).
     // The content of the LIST 'hdrl' starts after the 'hdrl' type (4 bytes). The size of this content is list_hdrl_size - 4.
     // The content includes 'avih' (8 + avih_size), 'strl' (8 + strl_size) * dwStreams times, maybe 'dmlh'.
     // The offset after parsing avih data is reader.stream_position() after reading avih data.
     // The end of the 'hdrl' LIST is at the offset where the LIST 'hdrl' started + its size (list_hdrl_size).
     // Let's track the start of the 'hdrl' LIST. The first LIST header (size + type) is right after the 'AVI ' type.
     // RIFF(8) + AVI(4) = 12. The first LIST header is at offset 12.
     // The size of this LIST is list_hdrl_size.
     // The content of the LIST starts at 12 + 8 (LIST header) = 20.
     // The content is 'hdrl' (4) + avih (8 + avih_size) + strl (8 + strl_size) ...
     // The 'hdrl' type is at 20. The avih header is at 24. The avih data is at 32. End of avih data is 32 + avih_size.
     // current_pos_after_avih is 32 + avih_size.
     // End of the first LIST ('hdrl') is at 12 + 8 + list_hdrl_size = 20 + list_hdrl_size.
     // Bytes to skip = (20 + list_hdrl_size) - current_pos_after_avih.
     // This still seems overly complicated.

     // Let's assume a simpler structure for now and note the complexity.
     // The common pattern is: read atom header (size + type), if not desired, skip size - 8 bytes.
     // Inside a LIST, iterate atoms.

     // Re-parse strl atoms within the hdrl LIST
     let mut stream_headers = Vec::new();
     // Need to seek back to after the avih atom (offset 32 + avih_size) and parse the remaining atoms in hdrl.
     // The first atom after avih is typically the first strl LIST.
     let offset_after_avih_data = reader.stream_position()?;
     let end_of_hdrl_list = current_pos_after_avih + list_hdrl_size; // This is wrong, list_hdrl_size is the size of the LIST itself, not its content size after its header.
     // Let's use a loop to find and process strl lists within the hdrl LIST.
     // We are currently after the avih data. The next atom should be an strl LIST.
     // The hdrl LIST content size is list_hdrl_size - 4 (for 'hdrl' type). The actual list header is 8 bytes before 'hdrl' type.
     // So the actual size of the LIST header + content is 8 + (list_hdrl_size - 4) = list_hdrl_size + 4.
     // The position after reading the LIST 'hdrl' header (8 bytes) + 'hdrl' type (4 bytes) is 12 + 8 + 4 = 24.
     // The avih atom starts at 24. Its header is 8 bytes, data is avih_size. End of avih is 24 + 8 + avih_size.
     // current_pos_after_avih is this value.
     // The remaining size in the hdrl LIST after avih is (12 + 8 + list_hdrl_size) - current_pos_after_avih
     // = 20 + list_hdrl_size - current_pos_after_avih. No, it's from the start of the list header.

     // Let's simplify the parsing loop structure:
     // Loop through atoms in the file from the start.
     // Find RIFF, check type AVI.
     // Find LIST hdrl. Process its content.
     // Inside hdrl, find avih, parse it.
     // Inside hdrl, find strl, parse it multiple times (for each stream).
     // Find LIST movi. Process its content.
     // Inside movi, find data chunks. Store them.
     // Find idx1 (optional).

     // This requires a top-level atom parsing loop.

     reader.seek(StdSeekFrom::Start(12))?; // Skip RIFF header (8) and AVI type (4)

     loop {
         let current_atom_start_pos = reader.stream_position()?; // Start of the current atom header
         let mut header = [0; 8];
         let bytes_read = reader.read( &mut header)?;
         if bytes_read == 0 { break; } // End of file
         if bytes_read < 8 {
              // Partial header read at end of file
              return Err(StdIOError::new(StdIOErrorKind::UnexpectedEof, format!("Partial atom header read at {}", current_atom_start_pos)));
         }

         let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
         let atom_type = u32::from_le_bytes([header[4], header[5], header[6], header[7]]); // FourCC is Little Endian

         // println!("Parsing atom: {} at offset {} with size {}", fourcc_to_string(atom_type), current_atom_start_pos, atom_size); // Debugging output

         if atom_type == CKID_LIST {
             let list_type_ckid = read_u32_le(reader)?; // Read the LIST type
             let list_content_start = reader.stream_position()?; // Position after LIST type

             if list_type_ckid == CKID_hdrl {
                 // Inside hdrl LIST, look for avih and strl
                 let mut hdrl_content_pos = list_content_start;
                 let hdrl_list_end = current_atom_start_pos + 8 + atom_size; // Start of LIST header + LIST total size

                 while hdrl_content_pos < hdrl_list_end {
                      reader.seek(StdSeekFrom::Start(hdrl_content_pos))?; // Seek to the next atom in hdrl
                      let mut inner_header = [0; 8];
                       let bytes_read_inner = reader.read(&mut inner_header)?;
                       if bytes_read_inner == 0 { break; }
                       if bytes_read_inner < 8 {
                           return Err(StdIOError::new(StdIOErrorKind::UnexpectedEof, format!("Partial inner atom header read at {}", hdrl_content_pos)));
                       }
                      let inner_atom_size = u32::from_be_bytes([inner_header[0], inner_header[1], inner_header[2], inner_header[3]]) as u64;
                      let inner_atom_type = u32::from_le_bytes([inner_header[4], inner_header[5], inner_header[6], inner_header[7]]);

                      let inner_atom_data_start = reader.stream_position()?; // Position after inner atom header

                      if inner_atom_type == CKID_avih {
                          // Check avih size again
                          if inner_atom_size as usize != size_of::<AviMainHeader>() + 8 { // size + type + data = 8 + 56 = 64? No, size is size of data only.
                                                                                          // size_of::<AviMainHeader>() is 56. avih atom size is 56.
                               if inner_atom_size as usize != size_of::<AviMainHeader>() {
                                    return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz avih başlık boyutu: {}", inner_atom_size)));
                               }
                          }
                           reader.seek(StdSeekFrom::Start(inner_atom_data_start))?; // Go back to the start of avih data
                          main_header = parse_avi_main_header(reader)?; // Parse the main header
                      } else if inner_atom_type == CKID_LIST {
                           let inner_list_type_ckid = read_u32_le(reader)?; // Read the inner LIST type
                           if inner_list_type_ckid == CKID_strl {
                                // Process strl LIST
                                let strl_header = parse_stream_list(reader)?;
                                stream_headers.push(strl_header);
                           } else {
                                // Skip other inner LISTs
                                let inner_list_content_size = inner_atom_size.checked_sub(4) // Size of content after LIST type
                                    .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz iç LIST boyutu: {}", inner_atom_size)))?;
                                skip_bytes(reader, inner_list_content_size)?;
                           }
                      } else {
                           // Skip other atoms in hdrl
                            let size_to_skip_inner = inner_atom_size; // Atom size is size of data
                            skip_bytes(reader, size_to_skip_inner)?;
                      }

                      // Calculate the start of the next atom in hdrl
                      let next_inner_atom_start = inner_atom_data_start.checked_add(inner_atom_size)
                         .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki iç atom ofseti hesaplanırken taşma")))?;
                      hdrl_content_pos = next_inner_atom_start;
                 }

             } else if list_type_ckid == CKID_movi {
                 // Process movi LIST - This contains the actual video/audio data chunks
                 let movi_list_end = current_atom_start_pos + 8 + atom_size; // Start of LIST header + LIST total size
                 let mut movi_content_pos = list_content_start; // Position after LIST type ('movi')

                 while movi_content_pos < movi_list_end {
                      reader.seek(StdSeekFrom::Start(movi_content_pos))?; // Seek to the next chunk in movi
                      let mut chunk_header = [0; 8];
                       let bytes_read_chunk = reader.read(&mut chunk_header)?;
                       if bytes_read_chunk == 0 { break; }
                       if bytes_read_chunk < 8 {
                            return Err(StdIOError::new(StdIOErrorKind::UnexpectedEof, format!("Partial chunk header read at {}", movi_content_pos)));
                       }
                      let chunk_size = u32::from_be_bytes([chunk_header[0], chunk_header[1], chunk_header[2], chunk_header[3]]) as u64;
                      let chunk_id = u32::from_le_bytes([chunk_header[4], chunk_header[5], chunk_header[6], chunk_header[7]]);

                       // We are only interested in data chunks ('00dc', '01wb', etc.) and potentially 'idx1'
                       // The 'idx1' chunk is usually outside the 'movi' list, but some files might have it inside.
                       // For simplicity, let's process data chunks and skip others for now.

                       if chunk_id == CKID_00dc || chunk_id == CKID_01wb || chunk_id == CKID_idx1 { // Process data chunks and idx1
                             let mut data = vec![0; chunk_size as usize];
                             reader.read_exact(&mut data)?; // Read the chunk data
                             chunks.push(AviChunk { id: chunk_id, size: chunk_size as u32, data });

                             // Chunks are often padded to 16-bit boundaries. Size field is the data size.
                             // The actual size on disk is size + padding.
                             let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
                              let padding_bytes = padded_size as u64 - chunk_size;
                              if padding_bytes > 0 {
                                   skip_bytes(reader, padding_bytes)?; // Skip padding
                              }

                       } else {
                           // Skip other chunk types within movi
                            let size_to_skip_chunk = chunk_size; // Atom size is size of data
                            skip_bytes(reader, size_to_skip_chunk)?;
                             // Check for padding after skipping data
                              let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
                               let padding_bytes = padded_size as u64 - chunk_size;
                               if padding_bytes > 0 {
                                    skip_bytes(reader, padding_bytes)?; // Skip padding
                               }
                       }

                      // Calculate the start of the next chunk in movi
                      let next_chunk_start = movi_content_pos.checked_add(8 + chunk_size) // Position after chunk header + data
                         .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki öbek ofseti hesaplanırken taşma")))?;

                       // Add padding size to the next offset
                       let padded_size = (chunk_size as usize + 1) & !1;
                       let actual_chunk_size_on_disk = 8 + padded_size as u64; // Header (8) + data + padding
                       let next_chunk_start_with_padding = movi_content_pos.checked_add(actual_chunk_size_on_disk)
                           .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki öbek ofseti (padding dahil) hesaplanırken taşma")))?;


                      movi_content_pos = next_chunk_start_with_padding; // Sonraki öbeğin başlangıcı
                 }


             } else {
                 // Skip other LIST types
                 let list_content_size = atom_size.checked_sub(4) // Size of content after LIST type
                    .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz LIST boyutu: {}", atom_size)))?;
                 skip_bytes(reader, list_content_size)?;
             }

         } else {
             // Skip other top-level atoms (like 'idx1' if it's outside 'movi')
              // idx1 is usually a top-level chunk after the 'movi' list.
              // Let's handle it here.
              if atom_type == CKID_idx1 {
                   let mut data = vec![0; atom_size as usize];
                   reader.read_exact(&mut data)?; // Read the idx1 data
                   chunks.push(AviChunk { id: atom_type, size: atom_size as u32, data });

                   // Check for padding after idx1 chunk
                   let padded_size = (atom_size as usize + 1) & !1; // Pad to 2-byte boundary
                    let padding_bytes = padded_size as u64 - atom_size;
                    if padding_bytes > 0 {
                         skip_bytes(reader, padding_bytes)?; // Skip padding
                    }

              } else {
                   // Skip other unknown top-level atoms
                   let size_to_skip_atom = atom_size; // Atom size is size of data
                   skip_bytes(reader, size_to_skip_atom)?;
                   // Check for padding after skipping atom
                    let padded_size = (atom_size as usize + 1) & !1; // Pad to 2-byte boundary
                     let padding_bytes = padded_size as u64 - atom_size;
                     if padding_bytes > 0 {
                          skip_bytes(reader, padding_bytes)?; // Skip padding
                     }
              }
         }

         // Calculate the start of the next top-level atom
         // The position after processing the current atom/list (including its header and padding)
         // should be the start of the next atom header.
         // The loop already updates the reader's position via read/seek/skip_bytes.
         // The next iteration will read the header from the current position.
         // Need to be careful about where the next iteration *starts reading*.
         // The current_atom_start_pos + 8 (header) + atom_size + padding is the end of the current atom.
         // The next atom starts immediately after that.
         // The loop condition `while hdrl_content_pos < hdrl_list_end` etc. and updating the position
         // at the end of the loop seems correct for navigating within a LIST.
         // For top-level atoms, the implicit seek at the start of the loop's read call works if
         // the previous operation left the reader at the end of the previous atom + padding.

         // Let's verify the position after skipping/processing an atom.
         // A read_exact(header) advances position by 8.
         // skip_bytes(size) advances position by size.
         // Total advance = 8 + size + padding.
         // The loop will read the next header from the current position. This seems correct.

         // Need a condition to break the top-level loop if we are at the end of the file size
         // read from the RIFF header, or simply at the actual end of the file (read returns 0).
         // The read(header) returning 0 is the standard way to detect EOF.
     }

    // At the end of the parsing, check if necessary parts were found.
    if main_header.dwStreams == 0 || stream_headers.len() as u32 != main_header.dwStreams {
        println!("WARN: Stream header count mismatch. Expected: {}, Found: {}", main_header.dwStreams, stream_headers.len());
        // return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Stream header count mismatch")));
    }


    Ok(AviData { main_header, stream_headers, chunks })
}

// parse_avi_main_header helper
#[cfg(feature = "std")] // std version takes StdRead
fn parse_avi_main_header<R: StdRead>(reader: &mut R) -> Result<AviMainHeader, StdIOError> { // StdIOError döner
    Ok(AviMainHeader {
        dwMicroSecPerFrame: read_u32_le(reader)?,
        dwMaxBytesPerSec: read_u32_le(reader)?,
        dwPaddingGranularity: read_u32_le(reader)?,
        dwFlags: read_u32_le(reader)?,
        dwTotalFrames: read_u32_le(reader)?,
        dwInitialFrames: read_u32_le(reader)?,
        dwStreams: read_u32_le(reader)?,
        dwSuggestedBufferSize: read_u32_le(reader)?,
        dwWidth: read_u32_le(reader)?,
        dwHeight: read_u32_le(reader)?,
        dwSampleSize: read_u32_le(reader)?,
        dwReserved: [
            read_u32_le(reader)?,
            read_u32_le(reader)?,
            read_u32_le(reader)?,
            read_u32_le(reader)?,
        ],
    })
}

#[cfg(not(feature = "std"))] // no_std version takes core::io::Read
fn parse_avi_main_header<R: Read>(reader: &mut R) -> Result<AviMainHeader, CoreIOError> { // CoreIOError döner
    Ok(AviMainHeader {
        dwMicroSecPerFrame: read_u32_le(reader)?,
        dwMaxBytesPerSec: read_u32_le(reader)?,
        dwPaddingGranularity: read_u32_le(reader)?,
        dwFlags: read_u32_le(reader)?,
        dwTotalFrames: read_u32_le(reader)?,
        dwInitialFrames: read_u32_le(reader)?,
        dwStreams: read_u32_le(reader)?,
        dwSuggestedBufferSize: read_u32_le(reader)?,
        dwWidth: read_u32_le(reader)?,
        dwHeight: read_u32_le(reader)?,
        dwSampleSize: read_u32_le(reader)?,
        dwReserved: [
            read_u32_le(reader)?,
            read_u32_le(reader)?,
            read_u32_le(reader)?,
            read_u32_le(reader)?,
        ],
    })
}

// parse_stream_list helper (processes an 'strl' LIST)
#[cfg(feature = "std")] // std version takes StdRead + StdSeek
fn parse_stream_list<R: StdRead + StdSeek>(reader: &mut R) -> Result<AviStreamHeader, StdIOError> { // StdIOError döner
    use core::mem::size_of; // size_of needs to be available

    let strh_ckid = read_u32_le(reader)?; // Should be CKID_strh
    let strh_size = read_u32_le(reader)?; // Size of strh data
    if strh_ckid != CKID_strh {
        return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Beklenen strh CKID bulunamadı: {:x}", strh_ckid)));
    }
    // AviStreamHeader boyutu 56 bayttır. strh boyutu da 56 olmalıdır.
    if strh_size as usize != size_of::<AviStreamHeader>() {
         return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz strh başlık boyutu: {}", strh_size)));
    }

    let stream_header = AviStreamHeader {
        fccType: read_u32_le(reader)?,
        fccHandler: read_u32_le(reader)?,
        dwFlags: read_u32_le(reader)?,
        dwPriority: read_u16_le(reader)?,
        dwLanguage: read_u16_le(reader)?,
        dwInitialFrames: read_u32_le(reader)?,
        dwScale: read_u32_le(reader)?,
        dwRate: read_u32_le(reader)?,
        dwStart: read_u32_le(reader)?,
        dwLength: read_u32_le(reader)?,
        dwSuggestedBufferSize: read_u32_le(reader)?,
        dwSampleSize: read_u32_le(reader)?,
        rcFrame: [
            read_i16_le(reader)?,
            read_i16_le(reader)?,
            read_i16_le(reader)?,
            read_i16_le(reader)?,
        ],
    };

    // Skip the remaining atoms in the strl LIST after strh (e.g., 'strf', 'strd', 'strn')
    // The size of the 'strl' LIST content is the LIST size (from the parent loop) minus its type (4 bytes).
    // The size of the atoms parsed so far within 'strl' is 8 ('strh' header) + strh_size.
    // Need to skip (strl_list_content_size) - (8 + strh_size) bytes.
    // This requires the strl list size, which is available in the calling function (parse_avi_internal).
    // So, skip the remaining content in the strl list from the calling function.
    // This function only parses the strh and expects the reader to be positioned at the start of strh.
    // After reading strh data (strh_size), the reader is positioned correctly to skip the rest of the strl list from the caller.


    Ok(stream_header)
}

#[cfg(not(feature = "std"))] // no_std version takes core::io::Read + core::io::Seek
fn parse_stream_list<R: Read + Seek>(reader: &mut R) -> Result<AviStreamHeader, CoreIOError> { // CoreIOError döner
    use core::mem::size_of; // size_of needs to be available

    let strh_ckid = read_u32_le(reader)?; // Should be CKID_strh
    let strh_size = read_u32_le(reader)?; // Size of strh data
    if strh_ckid != CKID_strh {
        return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Beklenen strh CKID bulunamadı: {:x}", strh_ckid)));
    }
    // AviStreamHeader boyutu 56 bayttır. strh boyutu da 56 olmalıdır.
    if strh_size as usize != size_of::<AviStreamHeader>() {
         return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz strh başlık boyutu: {}", strh_size)));
    }

    let stream_header = AviStreamHeader {
        fccType: read_u32_le(reader)?,
        fccHandler: read_u32_le(reader)?,
        dwFlags: read_u32_le(reader)?,
        dwPriority: read_u16_le(reader)?,
        dwLanguage: read_u16_le(reader)?,
        dwInitialFrames: read_u32_le(reader)?,
        dwScale: read_u32_le(reader)?,
        dwRate: read_u32_le(reader)?,
        dwStart: read_u32_le(reader)?,
        dwLength: read_u32_le(reader)?,
        dwSuggestedBufferSize: read_u32_le(reader)?,
        dwSampleSize: read_u32_le(reader)?,
        rcFrame: [
            read_i16_le(reader)?,
            read_i16_le(reader)?,
            read_i16_le(reader)?,
            read_i16_le(reader)?,
        ],
    };

    // After reading strh data, the reader is positioned correctly to skip the rest of the strl list from the caller.
    Ok(stream_header)
}

// parse_movie_data helper (processes a 'movi' LIST) - collects data chunks
#[cfg(feature = "std")] // std version takes StdRead + StdSeek
fn parse_movie_data<R: StdRead + StdSeek>(reader: &mut R, chunks: &mut Vec<AviChunk>, list_size: u64) -> Result<(), StdIOError> { // StdIOError döner
    // list_size is the size of the 'movi' LIST including its type ('movi'), but not its header (CKID+Size).
    // The LIST header (CKID+Size) is 8 bytes. The LIST type is 4 bytes.
    // The content of the LIST starts after the type.
    // The size of the LIST content is list_size.
    // The end of the movi LIST is at the position where the LIST header started + 8 + list_size.
    // Need the start position of the movi LIST header from the caller.
    // Let's assume the reader is positioned at the start of the movi LIST content (after 'movi' type).
    // The total size of the LIST (including header) is the size from the parent atom.
    // Let's pass the actual size of the LIST chunk (size field from its header)
    // which includes the type ('movi') and the chunks within it.
    // The provided code passed list_size as u32, which was likely the size field from the header.

    // Refactor: Pass the total size of the LIST chunk (size from header) to this function.
    // The first 4 bytes of this size is the LIST type ('movi').
    // The remaining bytes are the chunks.
    // Let list_chunk_size be the size from the header (u32).
    // The size of the content (chunks) is list_chunk_size - 4 (for 'movi' type).
    // We are positioned after reading the 'movi' type.

    // Let's assume the caller passes the size field from the LIST header.
    // This size field is the size of the LIST content, including the LIST type.
    // So list_size includes the 4 bytes for the LIST type ('movi').
    // The actual chunks start after the LIST type.
    // Total size of LIST header + content = 8 + list_size.
    // We are positioned after reading the LIST type (4 bytes).
    // Remaining bytes in the LIST content = list_size - 4.

    let mut bytes_read_in_list_content = 0;
    let list_content_size = list_size.checked_sub(4).unwrap_or(0); // Size after 'movi' type

     while bytes_read_in_list_content < list_content_size as u64 {
         let current_chunk_start_in_list = reader.stream_position()?; // Start of the current chunk header

          // Check if enough bytes remain for a chunk header (8 bytes)
         let bytes_remaining_in_list = list_content_size as u64 - bytes_read_in_list_content;
         if bytes_remaining_in_list < 8 {
             if bytes_remaining_in_list > 0 {
                 // Partial chunk header at the end of the LIST
                  return Err(StdIOError::new(StdIOErrorKind::UnexpectedEof, format!("Partial chunk header at {} in movi LIST", current_chunk_start_in_list)));
             }
              // No more bytes for a full header, end of LIST content
             break;
         }


         let mut chunk_header = [0; 8];
         reader.read_exact(&mut chunk_header)?;
         let chunk_size = u32::from_be_bytes([chunk_header[0], chunk_header[1], chunk_header[2], chunk_header[3]]) as u64;
         let chunk_id = u32::from_le_bytes([chunk_header[4], chunk_header[5], chunk_header[6], chunk_header[7]]);

         let chunk_data_start = reader.stream_position()?; // Position after chunk header

         // Check if chunk size makes sense within remaining list content
         if bytes_read_in_list_content + 8 + chunk_size > list_content_size as u64 {
              return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Chunk size ({}) exceeds remaining LIST content size at {}", chunk_size, current_chunk_start_in_list)));
         }

         // We are typically interested in data chunks ('00dc', '01wb', etc.) and potentially 'idx1' if it's inside movi.
         // Let's process data chunks and skip others for now.

         if chunk_id >= CKID_00dc && chunk_id < CKID_00dc + main_header.dwStreams { // Data chunk (e.g., 00dc, 01wb, ...)
             let mut data = vec![0; chunk_size as usize];
             reader.read_exact(&mut data)?; // Read the chunk data
             chunks.push(AviChunk { id: chunk_id, size: chunk_size as u32, data });

             // Chunks are often padded to 16-bit boundaries. Size field is the data size.
             // The actual size on disk is size + padding.
             let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
              let padding_bytes = padded_size as u64 - chunk_size;
              if padding_bytes > 0 {
                   skip_bytes(reader, padding_bytes)?; // Skip padding
              }

         } else if chunk_id == CKID_LIST {
              // Nested LIST within movi? This is less common but possible.
              // Need to handle nested LISTs here recursively or with a loop.
              // For simplicity, let's skip nested LISTs for now.
              let list_type_ckid = read_u32_le(reader)?; // Read the inner LIST type
               let list_content_size_inner = chunk_size.checked_sub(4).unwrap_or(0); // Size after inner LIST type
               skip_bytes(reader, list_content_size_inner as u64)?; // Skip the content of the inner LIST

               // LIST chunks are also padded
               let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
               let padding_bytes = padded_size as u64 - chunk_size;
               if padding_bytes > 0 {
                    skip_bytes(reader, padding_bytes)?; // Skip padding
               }

         } else {
             // Skip other chunk types within movi
             let size_to_skip_chunk = chunk_size; // Atom size is size of data
             skip_bytes(reader, size_to_skip_chunk)?;
              // Check for padding after skipping data
               let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
                let padding_bytes = padded_size as u64 - chunk_size;
                if padding_bytes > 0 {
                     skip_bytes(reader, padding_bytes)?; // Skip padding
                }
         }

         // Update bytes_read_in_list_content by the total size of the chunk on disk (header + data + padding)
         let padded_size = (chunk_size as usize + 1) & !1;
         let actual_chunk_size_on_disk = 8 + padded_size as u64; // Header (8) + data + padding
         bytes_read_in_list_content += actual_chunk_size_on_disk;

         // Seek to the start of the next chunk is handled by the next iteration's read(header) if we skipped correctly.
         // The current position after processing the chunk (including padding) should be the start of the next chunk.
         // This seems correct if skip_bytes advances the stream position.
     }

    Ok(())
}

#[cfg(not(feature = "std"))] // no_std version takes core::io::Read + core::io::Seek
fn parse_movie_data<R: Read + Seek>(reader: &mut R, chunks: &mut Vec<AviChunk>, list_size: u32) -> Result<(), CoreIOError> { // CoreIOError döner
    let mut bytes_read_in_list_content = 0;
    let list_content_size = list_size.checked_sub(4).unwrap_or(0);

     while bytes_read_in_list_content < list_content_size as u64 {
         let current_chunk_start_in_list = reader.stream_position()?;

          let bytes_remaining_in_list = list_content_size as u64 - bytes_read_in_list_content;
          if bytes_remaining_in_list < 8 {
              if bytes_remaining_in_list > 0 {
                   return Err(CoreIOError::new(CoreIOErrorKind::UnexpectedEof, format!("Partial chunk header at {} in movi LIST", current_chunk_start_in_list)));
              }
             break;
          }


         let mut chunk_header = [0; 8];
         reader.read_exact(&mut chunk_header)?;
         let chunk_size = u32::from_be_bytes([chunk_header[0], chunk_header[1], chunk_header[2], chunk_header[3]]) as u64;
         let chunk_id = u32::from_le_bytes([chunk_header[4], chunk_header[5], chunk_header[6], chunk_header[7]]);

         let chunk_data_start = reader.stream_position()?; // Position after chunk header

         if bytes_read_in_list_content + 8 + chunk_size > list_content_size as u64 {
              return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Chunk size ({}) exceeds remaining LIST content size at {}", chunk_size, current_chunk_start_in_list)));
         }


         if chunk_id >= CKID_00dc && chunk_id < CKID_00dc + main_header.dwStreams { // Data chunk (e.g., 00dc, 01wb, ...)
             let mut data = vec![0; chunk_size as usize]; // Vec requires alloc
             reader.read_exact(&mut data)?; // Read the chunk data
             chunks.push(AviChunk { id: chunk_id, size: chunk_size as u32, data });

             let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
              let padding_bytes = padded_size as u64 - chunk_size;
              if padding_bytes > 0 {
                   skip_bytes(reader, padding_bytes)?; // Skip padding
              }

         } else if chunk_id == CKID_LIST {
              let list_type_ckid = read_u32_le(reader)?; // Read the inner LIST type
               let list_content_size_inner = chunk_size.checked_sub(4).unwrap_or(0); // Size after inner LIST type
               skip_bytes(reader, list_content_size_inner as u64)?; // Skip the content of the inner LIST

               let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
               let padding_bytes = padded_size as u64 - chunk_size;
               if padding_bytes > 0 {
                    skip_bytes(reader, padding_bytes)?; // Skip padding
               }

         } else {
             let size_to_skip_chunk = chunk_size; // Atom size is size of data
             skip_bytes(reader, size_to_skip_chunk)?;
              let padded_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
               let padding_bytes = padded_size as u64 - chunk_size;
               if padding_bytes > 0 {
                    skip_bytes(reader, padding_bytes)?; // Skip padding
               }
         }

         let padded_size = (chunk_size as usize + 1) & !1;
         let actual_chunk_size_on_disk = 8 + padded_size as u64;
         bytes_read_in_list_content += actual_chunk_size_on_disk;
     }

    Ok(())
}


// Helper function to map std::io::Error to FileSystemError (defined earlier, copy here for clarity)
#[cfg(feature = "std")]
fn map_std_io_error_to_fs_error(e: StdIOError) -> FileSystemError {
    FileSystemError::IOError(format!("IO Error: {}", e))
}

// Helper function to map CoreIOError to FileSystemError (defined earlier, copy here for clarity)
#[cfg(not(feature = "std"))]
fn map_core_io_error_to_fs_error(e: CoreIOError) -> FileSystemError {
     FileSystemError::IOError(format!("CoreIOError: {:?}", e))
     // TODO: Implement a proper mapping based on CoreIOErrorKind
}

// Helper function to map SahneError to FileSystemError (defined earlier, copy here for clarity)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e))
}


// Example main functions
#[cfg(feature = "example_avi")] // Different feature flag
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     #[cfg(not(feature = "std"))]
     {
          eprintln!("AVI parsing example (no_std) starting...");
          // TODO: Call init_console(crate::Handle(3)); if needed
     }
     #[cfg(feature = "std")]
     {
          eprintln!("AVI parsing example (std) starting...");
     }

     // Test with a hypothetical file/resource ID
     let file_path_or_resource_id = "sahne://files/video.avi";

     match parse_avi(file_path_or_resource_id) {
         Ok(avi_data) => {
             println!("AVI File Parsed Successfully!\n");
             println!("AVI Main Header: {:?}", avi_data.main_header);
             println!("\nStream Headers: {:?}", avi_data.stream_headers);
             println!("\nTotal Chunk Count: {}", avi_data.chunks.len());
              for chunk in &avi_data.chunks {
                  println!("\nChunk ID: {}, Size: {} bytes, First 16 bytes: {:?}...",
                           fourcc_to_string(chunk.id), chunk.size, &chunk.data[..cmp::min(16, chunk.data.len())]);
              }
         }
         Err(e) => {
             eprintln!("AVI Parsing Error for '{}': {}", file_path_or_resource_id, e);
         }
     }

     #[cfg(not(feature = "std"))]
     eprintln!("AVI parsing example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("AVI parsing example (std) finished.");

     Ok(())
}

// Test module
#[cfg(test)]
#[cfg(feature = "std")] // std feature and test attribute
mod tests {
    use super::*;
    use std::io::Cursor; // In-memory reader/seeker
    use alloc::vec; // vec! macro
    use alloc::string::ToString; // to_string()

     // Helper to create a basic RIFF AVI header in memory
     fn create_basic_avi_header() -> Vec<u8> {
         let mut data = vec![];
         data.extend_from_slice(&CKID_RIFF.to_le_bytes()); // RIFF
         data.extend_from_slice(&0u32.to_le_bytes()); // File size (placeholder)
         data.extend_from_slice(&CKID_AVI.to_le_bytes()); // AVI
         data.extend_from_slice(&CKID_LIST.to_le_bytes()); // LIST
         data.extend_from_slice(&0u32.to_le_bytes()); // LIST size (placeholder)
         data.extend_from_slice(&CKID_hdrl.to_le_bytes()); // hdrl
         data.extend_from_slice(&CKID_avih.to_le_bytes()); // avih
         data.extend_from_slice(&56u32.to_le_bytes()); // avih size (56 bytes for AviMainHeader)
         // Dummy AviMainHeader data (56 bytes)
         for _ in 0..56 { data.push(0); }
         data
     }

     #[test]
     fn test_parse_avi_basic_header() {
         let mut data = create_basic_avi_header();
         // Add some dummy content after hdrl LIST for parse_avi_internal to iterate
         data.extend_from_slice(&CKID_movi.to_le_bytes()); // Add movi list placeholder
         data.extend_from_slice(&8u32.to_le_bytes()); // movi list size (dummy)
         data.extend_from_slice(&CKID_movi.to_le_bytes()); // movi list type (dummy)
         // Correct the LIST hdrl size
         let list_hdrl_content_size = data.len() - 12 - 8; // Total size - RIFF(8) - AVI(4) - LIST header(8)
         let list_hdrl_total_size = list_hdrl_content_size + 4; // Content size + LIST type size
         let list_hdrl_header_size_offset = 8 + 4; // After RIFF header and AVI type
          let list_hdrl_size_bytes = (list_hdrl_total_size as u32).to_le_bytes();
          data[list_hdrl_header_size_offset..list_hdrl_header_size_offset+4].copy_from_slice(&list_hdrl_size_bytes);

          // Correct the overall file size in the RIFF header
           let total_file_size = data.len() - 8; // Total size - RIFF header size (8)
           let riff_size_bytes = (total_file_size as u32).to_le_bytes();
            data[4..8].copy_from_slice(&riff_size_bytes);


         let mut reader = Cursor::new(data);
         let result = parse_avi_internal(&mut reader); // Call the internal parser
         assert!(result.is_ok(), "Parsing failed: {:?}", result.err());
         let avi_data = result.unwrap();

         assert_eq!(avi_data.stream_headers.len(), 0); // Basic header has no streams
         assert_eq!(avi_data.chunks.len(), 0); // Basic header has no movie chunks
         // Can check main_header fields if dummy data was specific.
     }

     // TODO: Add more comprehensive tests with actual AVI structure simulation (ftyp, movi, strl, chunks)
     // TODO: Add tests for error conditions (invalid headers, truncated file, etc.)
     // TODO: Add tests specifically for the no_std implementation using a mock SahneResourceReader
}

// Redundant no_std print module and panic handler removed.
