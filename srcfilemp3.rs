// srcfilemp3.rs
// Basic MP3 header parser for Sahne64

#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

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


/// Custom error type for MP3 parsing issues.
#[derive(Debug)]
pub enum Mp3Error {
    UnexpectedEof, // During header reading
    InvalidSyncWord, // First 11 bits are not 0xFFE
    InvalidVersion(u8), // Invalid MPEG version bits
    InvalidLayer(u8), // Invalid Layer bits
    InvalidBitrateIndex(u8), // Invalid bitrate index
    InvalidSampleRateIndex(u8), // Invalid sample rate index
    ReservedBitsSet, // Reserved bits are not zero
    // Add other MP3 specific parsing errors here
}

// Implement Display for Mp3Error
impl fmt::Display for Mp3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mp3Error::UnexpectedEof => write!(f, "Beklenmedik dosya sonu (başlık okurken)"),
            Mp3Error::InvalidSyncWord => write!(f, "Geçersiz MP3 senkronizasyon kelimesi"),
            Mp3Error::InvalidVersion(v) => write!(f, "Geçersiz MP3 Versiyonu (bits: {})", v),
            Mp3Error::InvalidLayer(l) => write!(f, "Geçersiz MP3 Katmanı (bits: {})", l),
            Mp3Error::InvalidBitrateIndex(idx) => write!(f, "Geçersiz Bit Hızı İndeksi: {}", idx),
            Mp3Error::InvalidSampleRateIndex(idx) => write!(f, "Geçersiz Örnekleme Hızı İndeksi: {}", idx),
            Mp3Error::ReservedBitsSet => write!(f, "Ayrılmış bitler ayarlı"),
        }
    }
}

// Helper function to map Mp3Error to FileSystemError
fn map_mp3_error_to_fs_error(e: Mp3Error) -> FileSystemError {
    match e {
        Mp3Error::UnexpectedEof => FileSystemError::IOError(format!("Beklenmedik dosya sonu (başlık okurken)")), // Map parsing EOF to IO Error
        _ => FileSystemError::InvalidData(format!("MP3 ayrıştırma hatası: {}", e)), // Map other parsing errors to InvalidData
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


// Removed custom Read/Seek traits and Sahne64File struct.
// These are replaced by core::io traits and SahneResourceReader.


/// Represents an MP3 file and its header information.
pub struct Mp3File<R: Read + Seek> {
    reader: R, // Reader implementing Read + Seek
    // Store Handle separately if Drop is needed for resource release
    handle: Option<Handle>, // Use Option<Handle> for resource management
    file_size: u64, // Store file size for checks

    pub header: Mp3Header, // Parsed MP3 header
    // Add fields for ID3 tags if parsed
}

impl<R: Read + Seek> Mp3File<R> {
    /// Creates a new `Mp3File` instance from a reader and parses the header.
    /// This is used internally after opening the file/resource.
    fn from_reader(reader: R, handle: Option<Handle>, file_size: u64, header: Mp3Header) -> Self {
        Self { reader, handle, file_size, header }
    }

    /// Parses the 4-byte MP3 frame header from the reader.
    /// Assumes the reader is positioned at the start of an MP3 frame (or sync word).
    fn parse_header(reader: &mut R) -> Result<Mp3Header, FileSystemError> { // Return FileSystemError
        let mut buffer = [0u8; 4];
        // Use read_exact from core::io::Read
        reader.read_exact(&mut buffer).map_err(|e| match e.kind() {
            core::io::ErrorKind::UnexpectedEof => map_mp3_error_to_fs_error(Mp3Error::UnexpectedEof), // Map UnexpectedEof to Mp3Error
            _ => map_core_io_error_to_fs_error(e), // Map other IO errors
        })?;


        // MP3 Header parsing logic (copied from original code)
        // Byte 0: all 1s (syncword high) - 0xFF
        // Byte 1: first 3 bits all 1s (syncword low) - E0 mask
        if buffer[0] != 0xFF || (buffer[1] & 0xE0) != 0xE0 {
             return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidSyncWord));
        }

        let version_bits = (buffer[1] >> 3) & 0x03;
        let layer_bits = (buffer[1] >> 1) & 0x03;
        let protection_bit_bit = (buffer[1] & 0x01) == 0x00; // 0=protected (CRC), 1=not protected
        let bitrate_index = (buffer[2] >> 4) & 0x0F;
        let sample_rate_index = (buffer[2] >> 2) & 0x03;
        let padding_bit_bit = (buffer[2] >> 1) & 0x01 == 0x01;
        let private_bit_bit = (buffer[2] & 0x01) == 0x01;
        let channel_mode_bits = (buffer[3] >> 6) & 0x03;
        let mode_extension_bits = (buffer[3] >> 4) & 0x03;
        let copyright_bit = (buffer[3] >> 3) & 0x01 == 0x01;
        let original_home_bit = (buffer[3] >> 2) & 0x01 == 0x01;


        // Validate version and layer bits (reserved values)
        let version = match version_bits {
             0x00 => 2, // MPEG 2.5 (Unofficial)
             0x01 => { #[cfg(not(feature = "std"))] crate::eprintln!("WARN: Reserved MPEG Version bits: 0x01"); return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidVersion(version_bits))); } // Reserved
             0x02 => 2, // MPEG 2
             0x03 => 1, // MPEG 1
             _ => { #[cfg(not(feature = "std"))] crate::eprintln!("WARN: Unexpected MPEG Version bits: {}", version_bits); return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidVersion(version_bits))); } // Should not happen based on mask
        };

        let layer = match layer_bits {
             0x00 => { #[cfg(not(feature = "std"))] crate::eprintln!("WARN: Reserved Layer bits: 0x00"); return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidLayer(layer_bits))); } // Reserved
             0x01 => 3, // Layer III
             0x02 => 2, // Layer II
             0x03 => 1, // Layer I
             _ => { #[cfg(not(feature = "std"))] crate::eprintln!("WARN: Unexpected Layer bits: {}", layer_bits); return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidLayer(layer_bits))); } // Should not happen based on mask
        };

        // Check for invalid combinations or reserved indices in tables
        let bitrate = match Self::get_bitrate(version, layer, bitrate_index) {
             Some(bitrate) => bitrate,
             None => return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidBitrateIndex(bitrate_index))),
        };

        let sample_rate = match Self::get_sample_rate(version, sample_rate_index) {
             Some(sample_rate) => sample_rate,
             None => return Err(map_mp3_error_to_fs_error(Mp3Error::InvalidSampleRateIndex(sample_rate_index))),
        };

        // Although padding_bit and private_bit are parsed, their validity depends on context/usage.
        // Channel mode and mode extension bits are also parsed as raw values.

        Ok(Mp3Header {
            version,
            layer,
            protection_bit: protection_bit_bit,
            bitrate,
            sample_rate,
            padding_bit: padding_bit_bit,
            private_bit: private_bit_bit,
            channel_mode: channel_mode_bits,
            mode_extension: mode_extension_bits,
            copyright: copyright_bit,
            original_home: original_home_bit,
        })
    }

    // Bit hızı tablosu (bps) - Use bps for internal storage
    fn get_bitrate(version: u8, layer: u8, index: u8) -> Option<u32> {
        let bitrate_table_mpeg1_layer1: [Option<u32>; 16] = [
             None, Some(32), Some(64), Some(96), Some(128), Some(160), Some(192), Some(224),
             Some(256), Some(288), Some(320), Some(352), Some(384), Some(416), Some(448), None,
        ];
        let bitrate_table_mpeg1_layer2: [Option<u32>; 16] = [
             None, Some(32), Some(48), Some(56), Some(64), Some(80), Some(96), Some(112),
             Some(128), Some(160), Some(192), Some(224), Some(256), Some(320), Some(384), None,
        ];
        let bitrate_table_mpeg1_layer3: [Option<u32>; 16] = [
             None, Some(32), Some(40), Some(48), Some(56), Some(64), Some(80), Some(96),
             Some(112), Some(128), Some(160), Some(192), Some(224), Some(256), Some(320), None,
        ];

        // MPEG 2/2.5 Layer 1/2/3 bitrates are the same table
        let bitrate_table_mpeg2_25: [Option<u32>; 16] = [
             None, Some(8), Some(16), Some(24), Some(32), Some(40), Some(48), Some(56),
             Some(64), Some(80), Some(96), Some(112), Some(128), Some(144), Some(160), None,
        ];


        let bitrate_table = match (version, layer) {
             (1, 1) => &bitrate_table_mpeg1_layer1,
             (1, 2) => &bitrate_table_mpeg1_layer2,
             (1, 3) => &bitrate_table_mpeg1_layer3,
             (2, 1) | (2, 2) | (2, 3) | // MPEG 2 Layer 1, 2, 3
             (25, 1) | (25, 2) | (25, 3) // MPEG 2.5 Layer 1, 2, 3
                 => &bitrate_table_mpeg2_25,
             _ => return None, // Invalid version or layer combination
        };

        if index as usize >= bitrate_table.len() {
             return None; // Index out of bounds
        }
        bitrate_table[index as usize].map(|br_kbps| br_kbps * 1000) // kbps -> bps
    }

    // Örnekleme hızı tablosu (Hz)
    fn get_sample_rate(version: u8, index: u8) -> Option<u32> {
        let sample_rate_table_mpeg1: [Option<u32>; 4] = [
             Some(44100), Some(48000), Some(32000), None, // 0=44.1 kHz, 1=48 kHz, 2=32 kHz, 3=reserved
        ];
        let sample_rate_table_mpeg2: [Option<u32>; 4] = [
             Some(22050), Some(24000), Some(16000), None, // 0=22.05 kHz, 1=24 kHz, 2=16 kHz, 3=reserved
        ];
        let sample_rate_table_mpeg25: [Option<u32>; 4] = [
             Some(11025), Some(12000), Some(8000),  None, // 0=11.025 kHz, 1=12 kHz, 2=8 kHz,  3=reserved
        ];

        let sample_rate_table = match version {
             1 => &sample_rate_table_mpeg1,
             2 => &sample_rate_table_mpeg2,
             25 => &sample_rate_table_mpeg25, // MPEG 2.5
             _ => return None, // Invalid version
        };

        if index as usize >= sample_rate_table.len() {
             return None; // Index out of bounds
        }
        sample_rate_table[index as usize]
    }


    /// Reads and processes MP3 frames (placeholder).
    pub fn read_frames(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
        #[cfg(feature = "std")]
        println!("Çerçeveler okunuyor... (Henüz implemente edilmedi)");
        #[cfg(not(feature = "std"))]
        crate::println!("Çerçeveler okunuyor... (Henüz implemente edilmedi)");
        Ok(())
    }

    /// Reads ID3 tags (placeholder).
    pub fn read_id3_tags(&mut self) -> Result<(), FileSystemError> { // Return FileSystemError
        #[cfg(feature = "std")]
        println!("ID3 Tagları okunuyor... (Henüz implemente edilmedi)");
        #[cfg(not(feature = "std"))]
        crate::println!("ID3 Tagları okunuyor... (Henüz implemente edilmedi)");
        Ok(())
    }

    /// Prints the parsed MP3 header information.
    pub fn print_header_info(&self) {
        #[cfg(feature = "std")]
        {
            println!("MP3 Başlık Bilgileri:");
            println!("  Versiyon: MPEG {}", if self.header.version == 25 { "2.5" } else { self.header.version.to_string().as_str() }); // Handle 2.5 for display
            println!("  Katman: Layer {}", self.header.layer);
            println!("  Koruma biti: {}", if self.header.protection_bit { "Yok (CRC)" } else { "Var (CRC)" });
            println!("  Bit Hızı: {} kbps", self.header.bitrate / 1000); // kbps cinsinden gösteriyoruz
            println!("  Örnekleme Hızı: {} Hz", self.header.sample_rate);
            println!("  Dolgu Biti: {}", if self.header.padding_bit { "Var" } else { "Yok" });
            println!("  Özel Bit: {}", if self.header.private_bit { "Var" } else { "Yok" });
            println!("  Kanal Modu: {}", self.get_channel_mode_str());
            println!("  Mod Uzantısı: {}", self.header.mode_extension);
            println!("  Telif Hakkı: {}", if self.header.copyright { "Var" } else { "Yok" });
            println!("  Orijinal/Ev Yapımı: {}", if self.header.original_home { "Orijinal" } else { "Ev Yapımı" });
        }
        #[cfg(not(feature = "std"))]
        {
            crate::println!("MP3 Başlık Bilgileri:");
            crate::println!("  Versiyon: MPEG {}", if self.header.version == 25 { "2.5" } else { alloc::string::ToString::to_string(&self.header.version).as_str() }); // Handle 2.5 for display, requires alloc
            crate::println!("  Katman: Layer {}", self.header.layer);
            crate::println!("  Koruma biti: {}", if self.header.protection_bit { "Yok (CRC)" } else { "Var (CRC)" });
            crate::println!("  Bit Hızı: {} kbps", self.header.bitrate / 1000); // kbps cinsinden gösteriyoruz
            crate::println!("  Örnekleme Hızı: {} Hz", self.header.sample_rate);
            crate::println!("  Dolgu Biti: {}", if self.header.padding_bit { "Var" } else { "Yok" });
            crate::println!("  Özel Bit: {}", if self.header.private_bit { "Var" } else { "Yok" });
            crate::println!("  Kanal Modu: {}", self.get_channel_mode_str());
            crate::println!("  Mod Uzantısı: {}", self.header.mode_extension);
            crate::println!("  Telif Hakkı: {}", if self.header.copyright { "Var" } else { "Yok" });
            crate::println!("  Orijinal/Ev Yapımı: {}", if self.header.original_home { "Orijinal" } else { "Ev Yapımı" });
        }
    }

    fn get_channel_mode_str(&self) -> &'static str {
        match self.header.channel_mode {
            0x00 => "Stereo",
            0x01 => "Joint Stereo", // (Stereo) redundant part removed
            0x02 => "Dual Channel", // (İki Kanal) redundant part removed
            0x03 => "Mono",
            _ => "Bilinmiyor", // Should not happen based on mask
        }
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read + Seek> Drop for Mp3File<R> {
     fn drop(&mut self) {
         // Release the resource Handle when the parser is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: Mp3File drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


/// Opens an MP3 file from the given path (std) or resource ID (no_std)
/// and creates an Mp3File instance by parsing the header.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the Mp3File or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_mp3_file<P: AsRef<Path>>(file_path: P) -> Result<Mp3File<BufReader<File>>, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (required by MkvParser constructor)
    let file_size = reader.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    reader.seek(SeekFrom::Start(0)).map_err(map_std_io_error_to_fs_error)?; // Seek back to start

    // Parse the header using the reader
    let header = Mp3File::<BufReader<File>>::parse_header(&mut reader)?;

    Ok(Mp3File::from_reader(reader, None, file_size, header)) // Pass None for handle in std version
}

#[cfg(not(feature = "std"))]
pub fn open_mp3_file(file_path: &str) -> Result<Mp3File<SahneResourceReader>, FileSystemError> { // Return FileSystemError
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
    let mut reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek

    // Parse the header using the reader
    let header = Mp3File::<SahneResourceReader>::parse_header(&mut reader)?; // Pass the handle to the parser

    Ok(Mp3File::from_reader(reader, Some(handle), file_size, header))
}


// Example main functions
#[cfg(feature = "example_mp3")] // Different feature flag
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     #[cfg(not(feature = "std"))]
     {
          eprintln!("MP3 parser example (no_std) starting...");
          // TODO: Call init_console(crate::Handle(3)); if needed
     }
     #[cfg(feature = "std")]
     {
          eprintln!("MP3 parser example (std) starting...");
     }

     // Test with a hypothetical file path (std) or resource ID (no_std)
     #[cfg(feature = "std")]
     let mp3_path = Path::new("example.mp3"); // This file needs to exist and be a valid MP3 for the std example
     #[cfg(not(feature = "std"))]
     let mp3_path = "sahne://files/example.mp3"; // This resource needs to exist and be a valid MP3 for the no_std example


     match open_mp3_file(mp3_path) { // Call the function that opens and creates the parser
         Ok(mut mp3_file) => { // Need mut for read_frames/read_id3_tags
             mp3_file.print_header_info(); // Print parsed header info

             // Call placeholder functions
              if let Err(e) = mp3_file.read_frames() {
                  #[cfg(not(feature = "std"))]
                  crate::eprintln!("Çerçeve okuma hatası: {:?}", e);
                   #[cfg(feature = "std"))]
                   eprintln!("Çerçeve okuma hatası: {}", e);
                  return Err(e); // Or log and continue
              }
              if let Err(e) = mp3_file.read_id3_tags() {
                   #[cfg(not(feature = "std"))]
                   crate::eprintln!("ID3 tag okuma hatası: {:?}", e);
                   #[cfg(feature = "std"))]
                   eprintln!("ID3 tag okuma hatası: {}", e);
                   return Err(e); // Or log and continue
               }

             // The Handle is automatically released when 'mp3_file' goes out of scope (due to Drop)
         }
         Err(e) => {
              #[cfg(not(feature = "std"))]
              crate::eprintln!("MP3 dosyası açma hatası: {:?}", e);
              #[cfg(feature = "std"))]
              eprintln!("MP3 dosyası açma hatası: {}", e); // std error display
              return Err(e);
         }
     }

     #[cfg(not(feature = "std"))]
     eprintln!("MP3 parser example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("MP3 parser example (std) finished.");

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


     // Helper function to create dummy MP3 header bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor and byteorder Write
      fn create_dummy_mp3_header_bytes(version_bits: u8, layer_bits: u8, protection_bit: bool, bitrate_index: u8, sample_rate_index: u8, padding_bit: bool, private_bit: bool, channel_mode_bits: u8, mode_extension_bits: u8, copyright: bool, original_home: bool) -> Vec<u8> {
          let mut buffer = Cursor::new(Vec::new());
           // Byte 0: 0xFF (syncword high)
           buffer.write_u8(0xFF).unwrap();
           // Byte 1: syncword low (3 bits 1s) | version | layer | protection_bit
           let byte1 = 0xE0 | (version_bits << 3) | (layer_bits << 1) | (if protection_bit { 0 } else { 1 }); // Protection bit logic inverted
           buffer.write_u8(byte1).unwrap();
           // Byte 2: bitrate_index | sample_rate_index | padding_bit | private_bit
            let byte2 = (bitrate_index << 4) | (sample_rate_index << 2) | (if padding_bit { 1 } else { 0 } << 1) | (if private_bit { 1 } else { 0 });
           buffer.write_u8(byte2).unwrap();
           // Byte 3: channel_mode | mode_extension | copyright | original_home
            let byte3 = (channel_mode_bits << 6) | (mode_extension_bits << 4) | (if copyright { 1 } else { 0 } << 3) | (if original_home { 1 } else { 0 } << 2);
           buffer.write_u8(byte3).unwrap();

          buffer.into_inner() // Return the bytes
      }

     // Test the MP3 header parsing logic with various inputs (using the helper)
     #[test]
     #[cfg(feature = "std")] // Run this test only with std feature
     fn test_parse_mp3_header() -> Result<(), FileSystemError> { // Return FileSystemError
          // Test a valid MPEG 1 Layer III header (e.g., 128kbps, 44.1kHz, Stereo)
          let header_bytes = create_dummy_mp3_header_bytes(
              0x03, // MPEG 1 (0x03)
              0x01, // Layer III (0x01)
              true, // Not protected (CRC)
              0x09, // 128 kbps for MPEG 1 Layer III
              0x00, // 44.1 kHz for MPEG 1
              false, // No padding
              false, // No private bit
              0x00, // Stereo
              0x00, // Mode extension
              false, // No copyright
              false, // Not original
          );

          // Use Cursor as a reader for the in-memory data
          let mut cursor = Cursor::new(header_bytes.clone()); // Clone for potential re-reads in test

          // Create a dummy Mp3File instance with the cursor reader (only header is needed for this test)
          let file_size = header_bytes.len() as u64; // Only the header bytes
          let mut dummy_parser = Mp3File::from_reader(cursor, None, file_size, Mp3Header { // Dummy header initially
              version: 0, layer: 0, protection_bit: false, bitrate: 0, sample_rate: 0,
              padding_bit: false, private_bit: false, channel_mode: 0, mode_extension: 0,
              copyright: false, original_home: false,
          });


          // Call the parsing function directly
          let parsed_header = Mp3File::<Cursor<Vec<u8>>>::parse_header(&mut dummy_parser.reader)?;

          // Assert the parsed header fields match the expected values
          assert_eq!(parsed_header.version, 1);
          assert_eq!(parsed_header.layer, 3);
          assert_eq!(parsed_header.protection_bit, true);
          assert_eq!(parsed_header.bitrate, 128000); // bps
          assert_eq!(parsed_header.sample_rate, 44100); // Hz
          assert_eq!(parsed_header.padding_bit, false);
          assert_eq!(parsed_header.private_bit, false);
          assert_eq!(parsed_header.channel_mode, 0x00);
          assert_eq!(parsed_header.mode_extension, 0x00);
          assert_eq!(parsed_header.copyright, false);
          assert_eq!(parsed_header.original_home, false);

          Ok(())
     }

      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_parse_mp3_header_invalid_sync_word() {
           // Create dummy bytes with invalid sync word
           let invalid_header_bytes = b"ABCD\x00\x00\x00\x00".to_vec(); // Not 0xFFE...

           let mut cursor = Cursor::new(invalid_header_bytes);
           let file_size = cursor.get_ref().len() as u64;
            let mut dummy_parser = Mp3File::from_reader(cursor, None, file_size, Mp3Header {
                version: 0, layer: 0, protection_bit: false, bitrate: 0, sample_rate: 0,
                padding_bit: false, private_bit: false, channel_mode: 0, mode_extension: 0,
                copyright: false, original_home: false,
            });

           // Attempt to parse, expect an error
           let result = Mp3File::<Cursor<Vec<u8>>>::parse_header(&mut dummy_parser.reader);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from Mp3Error::InvalidSyncWord
                   assert!(msg.contains("Geçersiz MP3 senkronizasyon kelimesi"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       #[test]
       #[cfg(feature = "std")] // Run this test only with std feature
       fn test_parse_mp3_header_truncated() {
            // Create dummy bytes that are too short for the header (e.g., 3 bytes)
            let truncated_header_bytes = b"\xFF\xFA\x00".to_vec(); // Only 3 bytes

            let mut cursor = Cursor::new(truncated_header_bytes);
            let file_size = cursor.get_ref().len() as u64;
             let mut dummy_parser = Mp3File::from_reader(cursor, None, file_size, Mp3Header {
                 version: 0, layer: 0, protection_bit: false, bitrate: 0, sample_rate: 0,
                 padding_bit: false, private_bit: false, channel_mode: 0, mode_extension: 0,
                 copyright: false, original_home: false,
             });

            // Attempt to parse, expect an error
            let result = Mp3File::<Cursor<Vec<u8>>>::parse_header(&mut dummy_parser.reader);

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from CoreIOError::UnexpectedEof (via read_exact)
                    assert!(msg.contains("Beklenmeyen dosya sonu") || msg.contains("UnexpectedEof") || msg.contains("end of file"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }

     // Test bitrate lookup
      #[test]
      fn test_get_bitrate() {
          // MPEG 1 Layer III
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(1, 3, 0x09), Some(128000)); // 128 kbps
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(1, 3, 0x01), Some(32000)); // 32 kbps
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(1, 3, 0x00), None); // Bad index
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(1, 3, 0x0F), None); // Bad index

          // MPEG 2 Layer II
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(2, 2, 0x08), Some(128000)); // 128 kbps
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(2, 2, 0x01), Some(8000)); // 8 kbps
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(2, 2, 0x00), None); // Bad index

          // Invalid version/layer combo
          assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_bitrate(99, 99, 0x01), None);
      }

      // Test sample rate lookup
       #[test]
       fn test_get_sample_rate() {
           // MPEG 1
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(1, 0x00), Some(44100)); // 44.1 kHz
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(1, 0x01), Some(48000)); // 48 kHz
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(1, 0x03), None); // Reserved index

           // MPEG 2
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(2, 0x00), Some(22050)); // 22.05 kHz
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(2, 0x02), Some(16000)); // 16 kHz

           // MPEG 2.5
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(25, 0x00), Some(11025)); // 11.025 kHz
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(25, 0x02), Some(8000)); // 8 kHz

           // Invalid version
           assert_eq!(Mp3File::<&mut core::io::Cursor<Vec<u8>>>::get_sample_rate(99, 0x00), None);
       }


     // Test for open_mp3_file in std environment (uses actual file I/O)
      #[test]
      #[cfg(feature = "std")] // Run this test only with std feature
      fn test_open_mp3_file_std() -> Result<(), FileSystemError> { // Return FileSystemError
          let dir = tempfile::tempdir().map_err(|e| FileSystemError::IOError(format!("Tempdir error: {}", e)))?;
          let file_path = dir.path().join("test_std.mp3");

          // Create a dummy file using std FS with a valid MP3 header at the start
           let header_bytes = create_dummy_mp3_header_bytes(
              0x03, 0x01, true, 0x09, 0x00, false, false, 0x00, 0x00, false, false
           ); // MPEG 1 Layer III, 128kbps, 44.1kHz
          let mut file = File::create(&file_path).map_err(|e| map_std_io_error_to_fs_error(e))?;
          file.write_all(&header_bytes).map_err(|e| map_std_io_error_to_fs_error(e))?;
           // Add some dummy data after the header
           file.write_all(&[0u8; 100]).map_err(|e| map_std_io_error_to_fs_error(e))?;


          // Call open_mp3_file with the file path
          let mp3_file = open_mp3_file(&file_path).map_err(|e| {
               // Clean up the file on error before returning
               let _ = remove_file(&file_path);
               e
          })?;

          // Assert the header was parsed correctly
           assert_eq!(mp3_file.header.version, 1);
           assert_eq!(mp3_file.header.layer, 3);
           assert_eq!(mp3_file.header.bitrate, 128000);
           assert_eq!(mp3_file.header.sample_rate, 44100);

           // Assert file size is correct
           assert_eq!(mp3_file.file_size, header_bytes.len() as u64 + 100);

           // Assert the reader is positioned after the header
           assert_eq!(mp3_file.reader.stream_position().unwrap(), header_bytes.len() as u64);


          // Clean up the dummy file
          let _ = remove_file(&file_path); // Ignore result, best effort cleanup

          Ok(())
      }

     // TODO: Add tests for open_mp3_file in no_std environment using a mock Sahne64 filesystem.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors, and
     // correctly parsing headers from mock data.
}


// Standart kütüphane olmayan ortam için panic handler (removed redundant, keep one in lib.rs or common module)
// Redundant print module also removed.


// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_mp3", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
