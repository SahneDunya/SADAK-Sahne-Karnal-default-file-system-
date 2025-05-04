#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader as StdBufReader, BufWriter as StdBufWriter, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Write as StdWrite, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt, WriteExt as StdWriteExt}; // Added BufWriter, Error, ErrorKind
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::{O_RDONLY, O_WRONLY, O_CREAT, O_TRUNC, O_RDWR}; // Import necessary fs flags


// byteorder crate (no_std compatible)
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt, ByteOrder}; // LittleEndian, ReadBytesExt, WriteBytesExt, ByteOrder trait/types

// alloc crate for String, Vec, format!
use alloc::string::{String, ToString}; // Import ToString trait for to_string()
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Write, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt, WriteExt as CoreWriteExt}; // core::io


// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden or common module


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


/// Custom error type for WAV handling issues.
#[derive(Debug)]
pub enum WavError {
    UnexpectedEof(String), // During header or data reading
    InvalidRiffWaveHeader, // Missing "RIFF" or "WAVE" identifiers
    InvalidFmtChunkHeader, // Missing "fmt " identifier
    UnsupportedAudioFormat(u16), // Only PCM (1) is supported
    InvalidDataChunkHeader, // Missing "data" identifier
    DataSizeMismatch(u32, usize), // Header data_size doesn't match actual bytes read
    SeekError(u64), // Failed to seek
    WriteError(String), // Generic write error message
    // Add other WAV specific errors here
}

// Implement Display for WavError
impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WavError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            WavError::InvalidRiffWaveHeader => write!(f, "Geçersiz RIFF/WAVE başlığı"),
            WavError::InvalidFmtChunkHeader => write!(f, "Geçersiz fmt alt bölüm başlığı"),
            WavError::UnsupportedAudioFormat(format) => write!(f, "Desteklenmeyen ses formatı: Sadece PCM (1) destekleniyor, bulunan {}", format),
            WavError::InvalidDataChunkHeader => write!(f, "Geçersiz data alt bölüm başlığı"),
            WavError::DataSizeMismatch(header_size, actual_size) => write!(f, "Veri boyutu uyuşmazlığı: Başlık {} bayt, okunan {} bayt", header_size, actual_size),
            WavError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
            WavError::WriteError(msg) => write!(f, "Yazma hatası: {}", msg),
        }
    }
}

// Helper function to map WavError to FileSystemError
fn map_wav_error_to_fs_error(e: WavError) -> FileSystemError {
    match e {
        WavError::UnexpectedEof(_) | WavError::SeekError(_) | WavError::WriteError(_) => FileSystemError::IOError(format!("WAV IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("WAV format/veri hatası: {}", e)), // Map format/parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read, Write ve Seek implementasyonu (copied from srcfiletxt.rs)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at/fs::write_at ile okuma/yazma yapar.
// fstat ile dosya boyutını alarak seek(End) desteği sağlar.
// Sahne64 API'sının bu syscall'ları Handle üzerinde sağladığı varsayılır.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReadWriteSeek { // Renamed to reflect Read+Write+Seek
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu (read/write için güncellenmeli)
}

#[cfg(not(feature = "std"))]
impl SahneResourceReadWriteSeek {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReadWriteSeek { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl core::io::Read for SahneResourceReadWriteSeek { // Use core::io::Read trait
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
impl core::io::Write for SahneResourceReadWriteSeek { // Use core::io::Write trait (for write_at)
    fn write(&mut self, buf: &[u8]) -> Result<usize, core::io::Error> { // Return core::io::Error
         // Assuming fs::write_at(handle, offset, buf) Result<usize, SahneError>
         // This write implementation writes at the current position and updates it.
         let bytes_to_write = buf.len();
         if bytes_to_write == 0 { return Ok(0); }

         let bytes_written = fs::write_at(self.handle, self.position, buf)
             .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::write_at error: {:?}", e)))?; // Map SahneError to core::io::Error

         self.position += bytes_written as u64;

         // Update file_size if writing extends beyond current size
         if self.position > self.file_size {
              self.file_size = self.position;
              // Note: In a real filesystem, updating file size might require a separate syscall (e.g., ftruncate)
              // or might be handled implicitly by write_at at the end of the file.
              // Assuming for this model that writing past file_size implicitly extends it and updates fstat.
         }


         Ok(bytes_written)
    }

     fn flush(&mut self) -> Result<(), core::io::Error> {
         // Assuming fs::flush(handle) or sync() is available for durability.
         // If not, this is a no-op or needs a different syscall.
         // For this model, assume no explicit flush syscall is needed for basic durability after write.
         Ok(())
     }
}


#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReadWriteSeek { // Use core::io::Seek trait
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
impl Drop for SahneResourceReadWriteSeek {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReadWriteSeek is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReadWriteSeek drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant fs, SahneError definitions.
// Removed custom Read, Write, Seek traits and FileReader, FileWriter structs.
// Removed redundant print module and panic handler boilerplate.


/// Represents the header of a WAV audio file.
#[derive(Debug, PartialEq, Clone, Copy)] // Add PartialEq, Clone, Copy for tests
pub struct WavHeader {
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub channel_count: u16,
    pub data_size: u32, // Size of the audio data in bytes
}

impl WavHeader {
    /// Reads and parses the WAV header from a reader.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read + Seek.
    ///             The reader should be positioned at the start of the file.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed WavHeader or a FileSystemError.
    pub fn read<R: Read + Seek>(mut reader: R) -> Result<Self, FileSystemError> { // Return FileSystemError
        // Use a buffered reader for efficiency
        #[cfg(feature = "std")]
        let mut reader = StdBufReader::new(reader);
        #[cfg(not(feature = "std"))]
        // Assuming a custom no_std BufReader implementation exists and is in scope (e.g., crate::BufReader)
        let mut reader = crate::BufReader::new(reader);


        // RIFF header (12 bytes)
        let mut riff_header = [0u8; 12];
        reader.read_exact(&mut riff_header).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("RIFF header"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;


        if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
             return Err(map_wav_error_to_fs_error(WavError::InvalidRiffWaveHeader));
        }

        // Chunk Size (u32 Little Endian, bytes 4-7 in RIFF header, not used by this header struct)
        // Let's read it to advance the reader position correctly, but ignore the value here.
        let _chunk_size = reader.get_ref().seek(SeekFrom::Current(4)) // Seek 4 bytes past "RIFF"
            .map_err(|e| map_core_io_error_to_fs_error(e))?;


        // Format subchunk (8 bytes)
        let mut fmt_header_id = [0u8; 4];
        reader.read_exact(&mut fmt_header_id).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("fmt header id"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        if &fmt_header_id != b"fmt " {
             return Err(map_wav_error_to_fs_error(WavError::InvalidFmtChunkHeader));
        }

        let fmt_size = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("fmt size"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
         // We should read the rest of the fmt chunk based on fmt_size if it's > 16,
         // but for basic header only, we assume fmt_size is 16 for PCM and skip extra bytes if any.
         let bytes_to_skip_in_fmt = fmt_size as i64 - 16;
         if bytes_to_skip_in_fmt < 0 {
             // fmt_size less than expected 16 bytes for basic PCM header
             return Err(map_wav_error_to_fs_error(WavError::InvalidFmtChunkHeader)); // Or a more specific error
         }
         if bytes_to_skip_in_fmt > 0 {
              reader.get_ref().seek(SeekFrom::Current(bytes_to_skip_in_fmt)).map_err(|e| map_core_io_error_to_fs_error(e))?;
         }


        let audio_format = reader.read_u16::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("audio format"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        if audio_format != 1 { // 1 = PCM
             return Err(map_wav_error_to_fs_error(WavError::UnsupportedAudioFormat(audio_format)));
        }

        let channel_count = reader.read_u16::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("channel count"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        let sample_rate = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("sample rate"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;
        let _byte_rate = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("byte rate"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?; // Byte rate (unused)
        let _block_align = reader.read_u16::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("block align"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?; // Block align (unused)
        let bits_per_sample = reader.read_u16::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("bits per sample"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        // Data subchunk (8 bytes)
        let mut data_header_id = [0u8; 4];
        reader.read_exact(&mut data_header_id).map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("data header id"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;

        if &data_header_id != b"data" {
             // Handle optional chunks before 'data', e.g., LIST, INFO.
             // For this basic parser, if it's not 'data', skip the chunk based on its size and look for 'data'.
             // This requires reading the chunk size and seeking.
             // A proper WAV parser would iterate chunks.
             // For now, return error if not 'data' right after fmt.
             return Err(map_wav_error_to_fs_error(WavError::InvalidDataChunkHeader));
        }

        let data_size = reader.read_u32::<LittleEndian>().map_err(|e| match e.kind() {
             core::io::ErrorKind::UnexpectedEof => map_wav_error_to_fs_error(WavError::UnexpectedEof(String::from("data size"))), // Requires alloc
             _ => map_core_io_error_to_fs_error(e),
        })?;


        Ok(WavHeader {
            sample_rate,
            bits_per_sample,
            channel_count,
            data_size,
        })
    }

    /// Writes the WAV header to a writer.
    ///
    /// # Arguments
    ///
    /// * `writer`: A mutable reference to the writer implementing Write + Seek.
    ///             The writer should be positioned at the start of the file.
    ///
    /// # Returns
    ///
    /// A Result indicating success or a FileSystemError.
    pub fn write<W: Write + Seek>(&self, mut writer: W) -> Result<(), FileSystemError> { // Return FileSystemError
         // Use a buffered writer for efficiency
        #[cfg(feature = "std")]
        let mut writer = StdBufWriter::new(writer);
        #[cfg(not(feature = "std"))]
        // Assuming a custom no_std BufWriter implementation exists and is in scope (e.g., crate::BufWriter)
        let mut writer = crate::BufWriter::new(writer);


        // RIFF header (12 bytes)
        writer.write_all(b"RIFF").map_err(|e| map_core_io_error_to_fs_error(e))?;
        writer.write_u32::<LittleEndian>(36 + self.data_size).map_err(|e| map_core_io_error_to_fs_error(e))?; // File size (36 + data_size)
        writer.write_all(b"WAVE").map_err(|e| map_core_io_error_to_fs_error(e))?;

        // Format subchunk (24 bytes)
        writer.write_all(b"fmt ").map_err(|e| map_core_io_error_to_fs_error(e))?;
        writer.write_u32::<LittleEndian>(16).map_err(|e| map_core_io_error_to_fs_error(e))?; // Subchunk size (16 for PCM)
        writer.write_u16::<LittleEndian>(1).map_err(|e| map_core_io_error_to_fs_error(e))?; // Audio format (1 = PCM)
        writer.write_u16::<LittleEndian>(self.channel_count).map_err(|e| map_core_io_error_to_fs_error(e))?;
        writer.write_u32::<LittleEndian>(self.sample_rate).map_err(|e| map_core_io_error_to_fs_error(e))?;
        // Byte rate = SampleRate * NumChannels * BitsPerSample/8
        writer.write_u32::<LittleEndian>(self.sample_rate * self.channel_count as u32 * self.bits_per_sample as u32 / 8).map_err(|e| map_core_io_error_to_fs_error(e))?;
        // Block align = NumChannels * BitsPerSample/8
        writer.write_u16::<LittleEndian>(self.channel_count * self.bits_per_sample / 8).map_err(|e| map_core_io_error_to_fs_error(e))?;
        writer.write_u16::<LittleEndian>(self.bits_per_sample).map_err(|e| map_core_io_error_to_fs_error(e))?;

        // Data subchunk header (8 bytes) - The actual data will be written separately.
        writer.write_all(b"data").map_err(|e| map_core_io_error_to_fs_error(e))?;
        writer.write_u32::<LittleEndian>(self.data_size).map_err(|e| map_core_io_error_to_fs_error(e))?; // Size of the data


        // Flush the buffered writer
        writer.flush().map_err(|e| map_core_io_error_to_fs_error(e))?;


        Ok(())
    }
}

/// Reads the raw audio data from the WAV file.
/// The reader should be positioned at the start of the data subchunk.
///
/// # Arguments
///
/// * `reader`: A mutable reference to the reader implementing Read + Seek.
/// * `header`: The parsed WavHeader, containing the data size.
///
/// # Returns
///
/// A Result containing the audio data as Vec<u8> or a FileSystemError.
pub fn read_wav_data<R: Read + Seek>(mut reader: R, header: &WavHeader) -> Result<Vec<u8>, FileSystemError> { // Return FileSystemError
    // Use a buffered reader for efficiency
    #[cfg(feature = "std")]
    let mut reader = StdBufReader::new(reader);
    #[cfg(not(feature = "std"))]
    let mut reader = crate::BufReader::new(reader); // Use Sahne64 BufReader


    let mut data = Vec::with_capacity(header.data_size as usize); // Requires alloc

     // Read exactly data_size bytes
     let bytes_read = reader.take(header.data_size as u64).read_to_end(&mut data).map_err(|e| map_core_io_error_to_fs_error(e))?; // Use take for safety


    // Verify that the expected number of bytes was read
    if bytes_read != header.data_size as usize {
         return Err(map_wav_error_to_fs_error(WavError::DataSizeMismatch(header.data_size, bytes_read)));
    }

    Ok(data) // Return the audio data bytes
}

/// Writes raw audio data to the WAV file.
/// The writer should be positioned at the start of the data subchunk.
///
/// # Arguments
///
/// * `writer`: A mutable reference to the writer implementing Write + Seek.
/// * `data`: The raw audio data bytes to write.
///
/// # Returns
///
/// A Result indicating success or a FileSystemError.
pub fn write_wav_data<W: Write + Seek>(mut writer: W, data: &[u8]) -> Result<(), FileSystemError> { // Return FileSystemError
    // Use a buffered writer for efficiency
    #[cfg(feature = "std")]
    let mut writer = StdBufWriter::new(writer);
    #[cfg(not(feature = "std"))]
    let mut writer = crate::BufWriter::new(writer); // Use Sahne64 BufWriter


    // Write all the data bytes
    writer.write_all(data).map_err(|e| map_core_io_error_to_fs_error(e))?;


    // Flush the buffered writer
    writer.flush().map_err(|e| map_core_io_error_to_fs_error(e))?;


    Ok(()) // Return success
}


/// Opens a WAV file from the given path (std) or resource ID (no_std)
/// for reading and returns a reader wrapping the file handle.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing a reader (implementing Read + Seek + Drop) or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_wav_reader<P: AsRef<Path>>(file_path: P) -> Result<File, FileSystemError> { // Return std::fs::File (implements Read+Seek+Drop)
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    Ok(file)
}

#[cfg(not(feature = "std"))]
pub fn open_wav_reader(file_path: &str) -> Result<SahneResourceReadWriteSeek, FileSystemError> { // Return SahneResourceReadWriteSeek (implements Read+Seek+Drop)
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutını al (needed for SahneResourceReadWriteSeek)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReadWriteSeek oluştur
    let reader = SahneResourceReadWriteSeek::new(handle, file_size); // Implements core::io::Read + Seek + Drop

    Ok(reader) // Return the reader
}


/// Opens a WAV file from the given path (std) or resource ID (no_std)
/// for writing (creates/truncates) and returns a writer wrapping the file handle.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing a writer (implementing Write + Seek + Drop) or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_wav_writer<P: AsRef<Path>>(file_path: P) -> Result<File, FileSystemError> { // Return std::fs::File (implements Write+Seek+Drop)
    let file = File::open(file_path.as_ref()).map_err(|e| map_std_io_error_to_fs_error(e)).or_else(|e| {
        // If file not found in std, try to create it.
        if let FileSystemError::IOError(msg) = &e {
             #[cfg(feature = "std")]
             if msg.contains("No such file or directory") || msg.contains("not found") {
                 return File::create(file_path.as_ref()).map_err(map_std_io_error_to_fs_error);
             }
        }
        Err(e)
    })?;
     // Truncate the file if it exists (assuming open does not implicitly truncate)
     // std::fs::File::create truncates automatically. open() might not.
     // If using open with O_WRONLY | O_CREAT, we need to explicitly truncate if file exists.
     // Let's use File::create for simplicity in std example/tests, which truncates.
     // If open is used, ftruncate syscall might be needed.
     // For now, assume File::create or open+O_TRUNC handles this.
    Ok(file)
}

#[cfg(not(feature = "std"))]
pub fn open_wav_writer(file_path: &str) -> Result<SahneResourceReadWriteSeek, FileSystemError> { // Return SahneResourceReadWriteSeek (implements Write+Seek+Drop)
    // Kaynağı edin for writing, create, and truncate
    let handle = resource::acquire(file_path, resource::MODE_WRITE | resource::MODE_CREATE | resource::MODE_TRUNCATE)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Get file size (initially 0 for a new/truncated file)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;


    // SahneResourceReadWriteSeek oluştur
    let writer = SahneResourceReadWriteSeek::new(handle, file_size); // Implements core::io::Write + Seek + Drop

    Ok(writer) // Return the writer
}



// Example main function (std)
#[cfg(feature = "example_wav")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> io::Result<()> { // Return std::io::Result for std example
     eprintln!("WAV handler example (std) starting...");
     eprintln!("WAV handler example (std) using std::io and byteorder.");

     // Example WAV header (1 second of 16-bit stereo PCM at 44100 Hz)
     let header = WavHeader {
         sample_rate: 44100,
         bits_per_sample: 16,
         channel_count: 2,
         data_size: 44100 * 2 * 2, // Sample Rate * Channels * (Bits Per Sample / 8)
     };

     let file_path = Path::new("example.wav");

      // Open file for writing and get the writer
      match open_wav_writer(file_path) {
           Ok(mut writer) => { // Use mut because write methods take &mut self
                // Write the WAV header
                if let Err(e) = header.write(&mut writer).map_err(|e| {
                     eprintln!("Error writing WAV header: {}", e);
                      // Map FileSystemError back to std::io::Error for std main
                     match e {
                         FileSystemError::IOError(msg) => io::Error::new(io::ErrorKind::Other, msg),
                         FileSystemError::InvalidData(msg) => io::Error::new(io::ErrorKind::InvalidData, msg),
                         FileSystemError::NotFound(msg) => io::Error::new(io::ErrorKind::NotFound, msg),
                         FileSystemError::PermissionDenied(msg) => io::Error::new(io::ErrorKind::PermissionDenied, msg),
                         FileSystemError::NotSupported(msg) => io::Error::new(io::ErrorKind::Unsupported, msg),
                         FileSystemError::Other(msg) => io::Error::new(io::ErrorKind::Other, msg),
                     }
                }) { return Err(e); }

                // Create dummy audio data (all zeros for silence)
                let data = vec![0u8; header.data_size as usize]; // Requires alloc

                // Write the WAV data
                 if let Err(e) = write_wav_data(&mut writer, &data).map_err(|e| {
                      eprintln!("Error writing WAV data: {}", e);
                       // Map FileSystemError back to std::io::Error for std main
                      match e {
                          FileSystemError::IOError(msg) => io::Error::new(io::ErrorKind::Other, msg),
                          FileSystemError::InvalidData(msg) => io::Error::new(io::ErrorKind::InvalidData, msg),
                          FileSystemError::NotFound(msg) => io::Error::new(io::ErrorKind::NotFound, msg),
                          FileSystemError::PermissionDenied(msg) => io::Error::new(io::ErrorKind::PermissionDenied, msg),
                          FileSystemError::NotSupported(msg) => io::Error::new(io::ErrorKind::Unsupported, msg),
                          FileSystemError::Other(msg) => io::Error::new(io::ErrorKind::Other, msg),
                      }
                 }) { return Err(e); }

                // File is automatically closed/flushed when writer goes out of scope (due to Drop on File/BufWriter).
                 println!("Dummy WAV file created: {}", file_path.display());
           },
           Err(e) => {
               eprintln!("Error opening WAV file for writing: {}", e); // std error display
               // Map FileSystemError back to std::io::Error for std main
               match e {
                   FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                   FileSystemError::InvalidData(msg) => return Err(io::Error::new(io::ErrorKind::InvalidData, msg)),
                   FileSystemError::NotFound(msg) => return Err(io::Error::new(io::ErrorKind::NotFound, msg)),
                   FileSystemError::PermissionDenied(msg) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, msg)),
                   FileSystemError::NotSupported(msg) => return Err(io::Error::new(io::ErrorKind::Unsupported, msg)),
                   FileSystemError::Other(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
               }
           }
      }


     // Open the created WAV file for reading and get the reader
      match open_wav_reader(file_path) {
           Ok(mut reader) => { // Use mut because read methods take &mut self
               // Read the WAV header
               match WavHeader::read(&mut reader) {
                    Ok(read_header) => {
                         println!("Read WAV Header:");
                         println!("  Sample Rate: {}", read_header.sample_rate);
                         println!("  Bits Per Sample: {}", read_header.bits_per_sample);
                         println!("  Channel Count: {}", read_header.channel_count);
                         println!("  Data Size: {}", read_header.data_size);

                         // Verify read header matches written header
                         assert_eq!(read_header, header);

                         // Read the WAV data
                          match read_wav_data(&mut reader, &read_header) {
                              Ok(read_data) => {
                                  println!("Read WAV Data Length: {}", read_data.len());
                                   // Verify read data size matches header data size
                                  assert_eq!(read_data.len(), read_header.data_size as usize);
                                   // For zero data, verify content is all zeros
                                  assert!(read_data.iter().all(|&b| b == 0));
                              },
                              Err(e) => {
                                  eprintln!("Error reading WAV data: {}", e); // std error display
                                   // Map FileSystemError back to std::io::Error for std main
                                  match e {
                                      FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                                      FileSystemError::InvalidData(msg) => return Err(io::Error::new(io::ErrorKind::InvalidData, msg)),
                                      FileSystemError::NotFound(msg) => return Err(io::Error::new(io::ErrorKind::NotFound, msg)),
                                      FileSystemError::PermissionDenied(msg) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, msg)),
                                      FileSystemError::NotSupported(msg) => return Err(io::Error::new(io::ErrorKind::Unsupported, msg)),
                                      FileSystemError::Other(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                                  }
                              }
                          }
                    },
                    Err(e) => {
                         eprintln!("Error reading WAV header: {}", e); // std error display
                          // Map FileSystemError back to std::io::Error for std main
                         match e {
                             FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                             FileSystemError::InvalidData(msg) => return Err(io::Error::new(io::ErrorKind::InvalidData, msg)),
                             FileSystemError::NotFound(msg) => return Err(io::Error::new(io::ErrorKind::NotFound, msg)),
                             FileSystemError::PermissionDenied(msg) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, msg)),
                             FileSystemError::NotSupported(msg) => return Err(io::Error::new(io::ErrorKind::Unsupported, msg)),
                             FileSystemError::Other(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                         }
                    }
               }

              // File is automatically closed when reader goes out of scope (due to Drop on File).
           },
           Err(e) => {
              eprintln!("Error opening WAV file for reading: {}", e); // std error display
               // Map FileSystemError back to std::io::Error for std main
              match e {
                  FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                  FileSystemError::InvalidData(msg) => return Err(io::Error::new(io::ErrorKind::InvalidData, msg)),
                  FileSystemError::NotFound(msg) => return Err(io::Error::new(io::ErrorKind::NotFound, msg)),
                  FileSystemError::PermissionDenied(msg) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, msg)),
                  FileSystemError::NotSupported(msg) => return Err(io::Error::new(io::ErrorKind::Unsupported, msg)),
                  FileSystemError::Other(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
              }
           }
      }


     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy WAV file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("WAV handler example (std) finished.");

     Ok(()) // Return Ok from std main
}

// Example main function (no_std)
#[cfg(feature = "example_wav")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for no_std example
     eprintln!("WAV handler example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // and simulate fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Example WAV header (1 second of 16-bit stereo PCM at 44100 Hz)
     let header = WavHeader {
         sample_rate: 44100,
         bits_per_sample: 16,
         channel_count: 2,
         data_size: 44100 * 2 * 2, // Sample Rate * Channels * (Bits Per Sample / 8)
     };

     let filename = "sahne://files/example.wav";

      // Hypothetical usage with Sahne64 mocks:
      // // Assume a mock filesystem is set up and "sahne://files/example.wav" can be created.
       match open_wav_writer(filename) {
            Ok(mut writer) => { // Use mut because write methods take &mut self
                 crate::println!("Opened {} for writing.", filename);
      //
      //           // Write the WAV header
                 match header.write(&mut writer) {
                      Ok(_) => crate::println!("WAV header written."),
                      Err(e) => crate::eprintln!("Error writing WAV header: {:?}", e),
                 }
      //
      //           // Create dummy audio data (all zeros for silence)
                 let data = vec![0u8; header.data_size as usize]; // Requires alloc
      //
      //           // Write the WAV data
                  match write_wav_data(&mut writer, &data) {
                      Ok(_) => crate::println!("WAV data written."),
                      Err(e) => crate::eprintln!("Error writing WAV data: {:?}", e),
                  }
      //
      //           // File is automatically closed/flushed when writer goes out of scope (due to Drop on SahneResourceReadWriteSeek).
                  crate::println!("Closed {} (after writing).", filename);
            },
            Err(e) => crate::eprintln!("Error opening {} for writing: {:?}", e, filename),
       }
      //
      // // Open the created WAV file for reading
       match open_wav_reader(filename) {
            Ok(mut reader) => { // Use mut because read methods take &mut self
                 crate::println!("Opened {} for reading.", filename);
      //
      //           // Read the WAV header
                 match WavHeader::read(&mut reader) {
                      Ok(read_header) => {
                           crate::println!("Read WAV Header:");
                           crate::println!("  Sample Rate: {}", read_header.sample_rate);
                           crate::println!("  Bits Per Sample: {}", read_header.bits_per_sample);
                           crate::println!("  Channel Count: {}", read_header.channel_count);
                           crate::println!("  Data Size: {}", read_header.data_size);
      //
      //                     // Read the WAV data
                            match read_wav_data(&mut reader, &read_header) {
                                Ok(read_data) => {
                                    crate::println!("Read WAV Data Length: {}", read_data.len());
                                 },
                                 Err(e) => {
                                     crate::eprintln!("Error reading WAV data: {:?}", e);
                                 }
                             }
                      },
                      Err(e) => {
                           crate::eprintln!("Error reading WAV header: {:?}", e);
                      }
                 }
      //
      //           // File is automatically closed when reader goes out of scope (due to Drop on SahneResourceReadWriteSeek).
                  crate::println!("Closed {} (after reading).", filename);
            },
            Err(e) => crate::eprintln!("Error opening {} for reading: {:?}", e, filename),
       }


     eprintln!("WAV handler example (no_std) needs Sahne64 mocks and byteorder crate to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The byteorder crate compiled with no_std and alloc features.

     Ok(()) // Dummy return
}


// Test module (std feature active)
#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Write, Seek, SeekFrom};
    use std::io::Cursor as StdCursor; // For in-memory testing
    use byteorder::{WriteBytesExt as StdWriteBytesExt, ReadBytesExt as StdReadBytesExt}; // For byteorder traits on Cursor
    use std::error::Error; // For Box<dyn Error>


    // Helper to create dummy WAV bytes in memory
    fn create_dummy_wav_bytes(header: &WavHeader, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buffer = StdCursor::new(Vec::new()); // Use std::io::Cursor for in-memory buffer

        // Write RIFF header
        buffer.write_all(b"RIFF")?;
        buffer.write_u32::<LittleEndian>(36 + header.data_size)?;
        buffer.write_all(b"WAVE")?;

        // Write fmt subchunk
        buffer.write_all(b"fmt ")?;
        buffer.write_u32::<LittleEndian>(16)?; // fmt chunk size (16 for PCM)
        buffer.write_u16::<LittleEndian>(1)?; // Audio format (1 = PCM)
        buffer.write_u16::<LittleEndian>(header.channel_count)?;
        buffer.write_u32::<LittleEndian>(header.sample_rate)?;
        buffer.write_u32::<LittleEndian>(header.sample_rate * header.channel_count as u32 * header.bits_per_sample as u32 / 8)?; // Byte rate
        buffer.write_u16::<LittleEndian>(header.channel_count * header.bits_per_sample / 8)?; // Block align
        buffer.write_u16::<LittleEndian>(header.bits_per_sample)?;

        // Write data subchunk header
        buffer.write_all(b"data")?;
        buffer.write_u32::<LittleEndian>(header.data_size)?;

        // Write audio data
        buffer.write_all(data)?;

        Ok(buffer.into_inner()) // Return the underlying Vec<u8>
    }


    #[test]
    fn test_wav_header_read_write_std_cursor() -> Result<(), FileSystemError> { // Return FileSystemError for std test
        let original_header = WavHeader {
            sample_rate: 48000,
            bits_per_sample: 24,
            channel_count: 1,
            data_size: 48000 * 1 * 3, // Example data size
        };

         // Write the header to an in-memory buffer
         let mut buffer = StdCursor::new(Vec::new());
         original_header.write(&mut buffer)?; // Write method takes Write + Seek

         // Get the written bytes
         let written_bytes = buffer.into_inner();
         // println!("Written Header Bytes: {:?}", written_bytes); // Debug print

         // Create a new cursor to read from the written bytes
         let mut reader_buffer = StdCursor::new(written_bytes);

         // Read the header from the buffer
         let read_header = WavHeader::read(&mut reader_buffer)?; // Read method takes Read + Seek

         // Assert that the read header matches the original header
         assert_eq!(read_header, original_header);


        Ok(()) // Return Ok from test function
    }

     #[test]
      fn test_read_wav_data_std_cursor() -> Result<(), FileSystemError> {
           let header = WavHeader { sample_rate: 8000, bits_per_sample: 8, channel_count: 1, data_size: 100 };
           let dummy_data = vec![0xAA; header.data_size as usize]; // Dummy data

           // Create dummy WAV bytes with header and data
            let wav_bytes = create_dummy_wav_bytes(&header, &dummy_data)
                .map_err(|e| FileSystemError::Other(format!("Test data creation error: {}", e)))?;

           // Create a cursor to read from the bytes
           let mut cursor = StdCursor::new(wav_bytes.clone());

           // Skip the header to position the cursor at the start of the data chunk
           // Header size is 12 (RIFF) + 24 (fmt) + 8 (data header) = 44 bytes
           cursor.seek(SeekFrom::Start(44)).map_err(map_std_io_error_to_fs_error)?;

           // Read the WAV data using the read_wav_data function
           let read_data = read_wav_data(&mut cursor, &header)?;

           // Assert that the read data matches the original dummy data
           assert_eq!(read_data, dummy_data);


           Ok(())
      }


      #[test]
      fn test_write_wav_data_std_cursor() -> Result<(), FileSystemError> {
           let header = WavHeader { sample_rate: 16000, bits_per_sample: 16, channel_count: 1, data_size: 50 };
           let dummy_data = vec![0x55; header.data_size as usize]; // Dummy data

           // Create a cursor to write to
           let mut cursor = StdCursor::new(Vec::new()); // Start with empty buffer

           // Write the WAV data using the write_wav_data function
           write_wav_data(&mut cursor, &dummy_data)?; // Write method takes Write + Seek


           // Get the written bytes
           let written_bytes = cursor.into_inner();

           // For this test, we only wrote the data, not the header.
           // We can't easily verify just the data without creating a full WAV structure.
           // A better test would involve writing the header first, then data, and reading back.
           // The test_wav_header_read_write_std_cursor already covers writing/reading header.
           // Let's verify the size of the written data.
           assert_eq!(written_bytes.len(), header.data_size as usize);
            // Assert content is the dummy data bytes
           assert_eq!(written_bytes, dummy_data); // write_wav_data only writes the data part


           Ok(())
      }

       #[test]
       fn test_read_wav_header_invalid_data_cursor() {
            // Create invalid WAV bytes (e.g., truncated header)
            let dummy_bytes = vec![ b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'W' ]; // Too short for RIFF + WAVE + fmt ID

            let mut cursor = StdCursor::new(dummy_bytes);

            // Attempt to read the header, expect an error
            let result = WavHeader::read(&mut cursor);

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::IOError(msg) => { // Mapped from core::io::ErrorKind::UnexpectedEof
                    assert!(msg.contains("WAV IO hatası"));
                    assert!(msg.contains("Beklenmedik dosya sonu"));
                    assert!(msg.contains("RIFF header") || msg.contains("fmt header id")); // Could fail at different read_exact calls
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }

       #[test]
        fn test_read_wav_header_invalid_format_cursor() {
             // Create WAV bytes with invalid WAVE identifier
             let mut dummy_bytes = vec![ b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'N', b'O', b'T', b'W', b'A', b'V', b'E' ]; // Invalid identifier

             let mut cursor = StdCursor::new(dummy_bytes);

             // Attempt to read the header, expect an InvalidRiffWaveHeader error
             let result = WavHeader::read(&mut cursor);

             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::InvalidData(msg) => { // Mapped from WavError::InvalidRiffWaveHeader
                     assert!(msg.contains("WAV format/veri hatası"));
                     assert!(msg.contains("Geçersiz RIFF/WAVE başlığı"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::write_at, fs::lseek.
    // Test cases should include opening valid/invalid files, handling IO errors during reading/writing,
    // and verifying header and data content with mock data.
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_wav", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
