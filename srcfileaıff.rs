#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel Sahne64 tipleri ve modülleri
use crate::{SahneError, Handle}; // Hata tipleri ve Handle
#[cfg(not(feature = "std"))]
use crate::resource; // Sahne64 resource modülü (no_std implementasyonu için)
#[cfg(not(feature = "std"))]
use crate::fs; // Sahne64 fs modülü (fs::read_at, fs::fstat için varsayım)

// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind};
#[cfg(feature = "std")]
use std::error::Error as StdError; // std::error::Error
#[cfg(feature = "std")]
use std::fmt as StdFmt; // std::fmt

// core kütüphanesinden gerekli modüller
use core::fmt;
use core::mem::size_of; // core::mem::size_of
use core::convert::TryInto; // core::convert::TryInto
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io traits and types

// byteorder crate (no_std uyumlu)
use byteorder::{BigEndian, ReadBytesExt, ByteOrder}; // BigEndian, ReadBytesExt, ByteOrder trait/types from byteorder crate

// alloc crate for String, format!
use alloc::string::String;
use alloc::format;

// Floating point support (required for sample rate calculation)
// Ensure the target environment supports f64.
use core::f64;


// Özel hata türü tanımla
#[derive(Debug)]
pub enum AiffError {
    /// Temel I/O işlemlerinden kaynaklanan hatalar (Sahne64 veya std::io).
    IoError(String), // SahneError veya std::io::Error mesajını tutalım
    /// AIFF dosya formatı ile ilgili geçerli olmayan veriler.
    InvalidData(String),
    /// Sayısal dönüşüm veya hesaplama hatası (örneğin, sample rate).
    ConversionError(String),
    // SahneError'ı doğrudan sarmak da bir seçenek olabilir:
    // UnderlyingSahneError(SahneError),
}

// Hata mesajlarını formatlamak için fmt::Display implementasyonu
impl fmt::Display for AiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiffError::IoError(msg) => write!(f, "Dosya okuma/yazma hatası: {}", msg),
            AiffError::InvalidData(msg) => write!(f, "Geçersiz AIFF verisi: {}", msg),
            AiffError::ConversionError(msg) => write!(f, "Dönüşüm hatası: {}", msg),
            // AiffError::UnderlyingSahneError(e) => write!(f, "Sahne64 hatası: {:?}", e),
        }
    }
}

// std ortamında Error trait'ini implement etme
#[cfg(feature = "std")]
impl StdError for AiffError {}

// std::io::Error -> AiffError dönüşümü
#[cfg(feature = "std")]
impl From<std::io::Error> for AiffError {
    fn from(err: std::io::Error) -> Self {
        // std::io::Error'dan string mesajı alıp IoError'a sarmalayalım.
        AiffError::IoError(format!("std::io::Error: {}", err))
    }
}

// SahneError -> AiffError dönüşümü (no_std)
// Doğrudan SahneError'ı tutmak yerine string mesajını alalım.
#[cfg(not(feature = "std"))]
impl From<SahneError> for AiffError {
    fn from(err: SahneError) -> Self {
        // SahneError'dan Debug formatıyla string mesajı alalım.
        AiffError::IoError(format!("SahneError: {:?}", err))
    }
}

// core::io::Error -> AiffError dönüşümü (no_std Reader'dan gelen hatalar için)
#[cfg(not(feature = "std"))]
impl From<CoreIOError> for AiffError {
    fn from(err: CoreIOError) -> Self {
         // CoreIOError'dan Debug formatıyla string mesajı alalım.
         AiffError::IoError(format!("CoreIOError: {:?}", err))
         // TODO: CoreIOErrorKind'e göre daha spesifik AiffError varyantlarına map edilebilir.
    }
}


// AIFF metadata yapısı
#[derive(Debug)]
pub struct AiffMetadata {
    pub num_channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub num_frames: u32,
}

/// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfileaccdb.rs'den adapte)
/// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
/// fstat ile dosya boyutunu alarak seek(End) desteği sağlar.
/// Sahne64 API'sının bu syscall'ları Handle üzerinde sağladığı varsayılır.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
}

#[cfg(not(feature = "std"))]
impl SahneResourceReader {
    pub fn new(handle: Handle) -> Self {
        SahneResourceReader { handle, position: 0 }
    }

    // Helper to map SahneError to CoreIOError (for core::io traits) - already done via From<SahneError> for CoreIOError if needed, but AiffError is the return type.
    // Let's keep the direct SahneError -> AiffError mapping.
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        // fs::read_at(fd, offset, buf) Result<usize, SahneError> döner.
        // SahneError'ı CoreIOError'a çevirmeliyiz.
        // Assuming fs::read_at takes Handle.
        let bytes_read = fs::read_at(self.handle, self.position, buf)
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::read_at error: {:?}", e)))?; // Map SahneError to CoreIOError

        self.position += bytes_read as u64; // Pozisyonu güncelle
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
         // Need fs::fstat(handle) -> Result<FileStat, SahneError> where FileStat has size: u64
         // Assuming fs::fstat takes Handle and returns size.
        let file_size = fs::fstat(self.handle)
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::fstat error: {:?}", e)))? // Map SahneError to CoreIOError
            .size as u64; // Assuming size is u64

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize, // isize for calculations
            SeekFrom::End(offset) => {
                (file_size as isize).checked_add(offset)
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

        self.position = new_pos as u64; // Update user-space position
        Ok(self.position) // Return new position
    }
    // stream_position has a default implementation in core::io::Seek
}


/// Belirtilen dosya yolundaki (veya kaynak ID'sindeki) AIFF dosyasının
/// meta verilerini (kanallar, sample rate, bit derinliği, frame sayısı) okur.
#[cfg(feature = "std")]
pub fn read_aiff_metadata(file_path: &str) -> Result<AiffMetadata, AiffError> {
    let file = File::open(file_path)?; // std::io::Error -> AiffError via From
    let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek
    read_metadata_from_reader(&mut reader) // AiffError döner
}

#[cfg(not(feature = "std"))]
pub fn read_aiff_metadata(file_path: &str) -> Result<AiffMetadata, AiffError> { // AiffError döner
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)?; // SahneError -> AiffError via From

    // Sahne64 Handle'ı için core::io::Read + Seek implementasyonu sağlayan Reader struct'ı oluştur
    // Bu struct fs::read_at ve fs::fstat kullanır (varsayım)
    let mut reader = SahneResourceReader::new(handle);

    // Meta veriyi oku
    let metadata_result = read_metadata_from_reader(&mut reader); // AiffError döner

    // Kaynağı serbest bırak
    let _ = resource::release(handle).map_err(|e| {
         eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print makrosu
         AiffError::IoError(format!("Resource release error: {:?}", e)) // SahneError -> AiffError
     });

    metadata_result
}


// Atom/Chunk işleme fonksiyonları (core::io::Read + Seek trait'leri üzerinden çalışır)
// std ve no_std implementasyonları aynı temel mantığı kullanacak.
// Bu fonksiyonlar AiffError döner.

#[cfg(feature = "std")] // std versiyonu StdRead + StdSeek alır
fn read_metadata_from_reader<R: StdRead + StdSeek>(reader: &mut R) -> Result<AiffMetadata, AiffError> { // AiffError döner
     use byteorder::{BigEndian as ReaderBigEndian, ReadBytesExt as ReaderReadBytesExt}; // Use byteorder traits/types with std::io
     use std::string::ToString; // to_string() for error messages

     // AIFF dosya başlığını (FORM chunk) kontrol et. Hata mesajlarını iyileştir.
     let mut form_chunk_id = [0; 4];
     reader.read_exact(&mut form_chunk_id)?; // StdRead::read_exact

     if &form_chunk_id != b"FORM" {
         return Err(AiffError::InvalidData(
             "Geçersiz AIFF dosyası: FORM chunk ID bulunamadı veya hatalı.".to_string(),
         ));
     }

     // Chunk boyutu (Big Endian)
     let form_chunk_size = reader.read_u32::<ReaderBigEndian>()?; // ReadBytesExt::read_u32

     let mut aiff_type = [0; 4];
     reader.read_exact(&mut aiff_type)?; // StdRead::read_exact

     if &aiff_type != b"AIFF" {
         return Err(AiffError::InvalidData(
             "Geçersiz AIFF dosyası: AIFF türü bulunamadı veya hatalı.".to_string(),
         ));
     }

     // Common chunk'u bul ve meta verileri oku
     // COMM chunk boyutu sabittir (18 byte) + padding
     loop {
         let mut chunk_id = [0; 4];
         // reader.read_exact() returns Result<(), IoError> which converts to AiffError
         let bytes_read_res = reader.read_exact(&mut chunk_id);

         match bytes_read_res {
             Ok(_) => {
                 let chunk_size = reader.read_u32::<ReaderBigEndian>()?; // ReadBytesExt::read_u32

                 if &chunk_id == b"COMM" {
                     // COMM chunk bulundu, verilerini oku
                     let num_channels = reader.read_u16::<ReaderBigEndian>()?; // ReadBytesExt::read_u16
                     let num_frames = reader.read_u32::<ReaderBigEndian>()?; // ReadBytesExt::read_u32
                     let bits_per_sample = reader.read_u16::<ReaderBigEndian>()?; // ReadBytesExt::read_u16

                     // Sample rate için genişletilmiş formatı oku (10 byte)
                     let mut sample_rate_extended_bytes = [0; 10];
                     reader.read_exact(&mut sample_rate_extended_bytes)?; // StdRead::read_exact

                     // Sample rate'i genişletilmiş formattan çöz (IEEE 754 Extended Double)
                     // AIFF spesifikasyonu 80-bit (10 byte) extended format kullanır.
                     // Bu formatı f64'e doğru bir şekilde çevirmek gerekir.
                     // ByteOrder::read_f64 could read 8 bytes, but AIFF uses 10.
                     // Need a helper to read 10 bytes and convert.

                     let sample_rate = match convert_aiff_sample_rate(&sample_rate_extended_bytes) {
                         Ok(rate) => rate,
                         Err(e) => return Err(AiffError::ConversionError(format!("Sample rate dönüşüm hatası: {}", e))),
                     };


                     // COMM chunk'ın kalan verisini atla (eğer 18 bayttan büyükse, örneğin padding)
                     // COMM chunk boyutu 18 bayt olmalıdır. Eğer daha büyükse spec dışıdır veya padding vardır?
                     // AIFF spesifikasyonuna göre COMM chunk boyutu tam 18 olmalıdır.
                     if chunk_size != 18 {
                          // Geçersiz COMM chunk boyutu
                          return Err(AiffError::InvalidData(format!("Geçersiz COMM chunk boyutu: Beklenen 18, bulunan {}", chunk_size)));
                     }


                     return Ok(AiffMetadata {
                         num_channels,
                         sample_rate,
                         bits_per_sample,
                         num_frames,
                     });
                 } else {
                     // Common chunk değilse, sonraki chunk'a atla.
                     // Chunk boyutu (chunk_size) okunur.
                     // Eğer boyut tek ise 1 bayt padding eklenir.
                     let padded_chunk_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
                     reader.seek(StdSeekFrom::Current(padded_chunk_size as i64))?; // StdSeek::seek
                 }
             }
             Err(e) => {
                 // read_exact hata döndürdüyse (örn. dosya sonu)
                 return Err(e.into()); // std::io::Error -> AiffError dönüşümü
             }
         }
     }

     // Eğer döngü bitmeden COMM chunk bulunamazsa (dosya sonu okunursa döngü biter)
     // Bu noktaya sadece eğer dosya sonuna gelmeden döngüden çıkılırsa gelinir?
     // Loop condition is implicitly handled by reader.read_exact.
     // If read_exact returns Err(UnexpectedEof), it's caught in the match.
     // So if we exit the loop, it means read_exact hit EOF *after* reading a chunk header,
     // or there was a different IO error, or the file was structured oddly.
     // If read_exact returned an error, it's already handled.
     // If read_exact was Ok but then the chunk size caused seek error, that's an error.
     // If we parse all chunks in the FORM chunk and don't find COMM, we should exit the loop.
     // The loop continues indefinitely until read_exact fails or COMM is found.
     // This structure is okay; the final Implicit return after the loop is unreachable if chunks exist.
     // If the file is just "FORM<size>AIFF", the loop will hit EOF and read_exact will fail.

     // However, if the FORM chunk size is smaller than the actual file content, we might read past the FORM chunk.
     // A better approach is to track the end of the FORM chunk and stay within it.
     // Let's refactor to process chunks within the FORM chunk's bounds.

    // --- Refactored parsing loop to respect FORM chunk bounds ---
    reader.seek(StdSeekFrom::Start(12))?; // Position after "FORM" + size + "AIFF" (4+4+4=12 bytes)
    let mut current_pos_in_form_content = 0u64;
    // The total size of the FORM chunk content is form_chunk_size.
    // The content starts after the "AIFF" type (at offset 12).
    // The end of the FORM chunk is at the position where "FORM" started + 8 + form_chunk_size.
    // Let's find the start of the FORM chunk header (offset 0).
    reader.seek(StdSeekFrom::Start(0))?;
    let form_chunk_header_pos = reader.stream_position()?; // Should be 0.
    reader.seek(StdSeekFrom::Start(12))?; // Go back to after "AIFF"

    let form_content_start_pos = reader.stream_position()?; // Should be 12.
    let form_content_end_pos = form_chunk_header_pos.checked_add(8 + form_chunk_size)
        .ok_or_else(|| AiffError::InvalidData(format!("FORM chunk sonu hesaplanırken taşma")))?;

     while reader.stream_position()? < form_content_end_pos {
         let current_chunk_start_pos = reader.stream_position()?;

         let mut chunk_id = [0; 4];
         // Read exactly 4 bytes for chunk ID, if not enough, it's EOF before end of FORM chunk
         if reader.read_exact(&mut chunk_id).is_err() {
             // Hit EOF while reading chunk ID, before end of FORM chunk.
             return Err(AiffError::InvalidData(format!("Beklenenden erken dosya sonu: Chunk ID okunurken.")));
         }


         let chunk_size = reader.read_u32::<ReaderBigEndian>()?; // Read chunk size (Big Endian)


         let chunk_data_end_pos = current_chunk_start_pos.checked_add(8 + chunk_size as u64)
             .ok_or_else(|| AiffError::InvalidData(format!("Chunk veri sonu hesaplanırken taşma")))?;

         // Check if chunk goes beyond FORM chunk end
         if chunk_data_end_pos > form_content_end_pos {
              return Err(AiffError::InvalidData(format!("Chunk {} (boyut {}) FORM chunk sınırını aşıyor.", format!("{}", core::str::from_utf8(&chunk_id).unwrap_or("???")), chunk_size)));
         }


         if &chunk_id == b"COMM" {
             // COMM chunk bulundu, verilerini oku
             let num_channels = reader.read_u16::<ReaderBigEndian>()?;
             let num_frames = reader.read_u32::<ReaderBigEndian>()?;
             let bits_per_sample = reader.read_u16::<ReaderBigEndian>()?;

             // Sample rate için genişletilmiş formatı oku (10 byte)
             let mut sample_rate_extended_bytes = [0; 10];
             reader.read_exact(&mut sample_rate_extended_bytes)?;

             let sample_rate = match convert_aiff_sample_rate(&sample_rate_extended_bytes) {
                 Ok(rate) => rate,
                 Err(e) => return Err(AiffError::ConversionError(format!("Sample rate dönüşüm hatası: {}", e))),
             };

             // COMM chunk boyutu kontrolü
             if chunk_size != 18 {
                  return Err(AiffError::InvalidData(format!("Geçersiz COMM chunk boyutu: Beklenen 18, bulunan {}", chunk_size)));
             }


             return Ok(AiffMetadata {
                 num_channels,
                 sample_rate,
                 bits_per_sample,
                 num_frames,
             });
         } else {
             // Common chunk değilse, sonraki chunk'a atla.
             // Eğer chunk boyutu tek ise 1 bayt padding eklenir.
             let padded_chunk_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
             reader.seek(StdSeekFrom::Current(padded_chunk_size as i64))?; // Atla
         }
         // After skipping, reader is positioned at the start of the next chunk.
         // The loop condition will check if we are still within the FORM chunk bounds.
     }

     // Eğer döngü biterse (tüm FORM chunk işlendi veya hata oldu) ve COMM chunk bulunamadıysa.
     Err(AiffError::InvalidData("COMM chunk bulunamadı: FORM chunk içinde arandı.".to_string()))
     // --- End of refactored parsing loop ---
}

#[cfg(not(feature = "std"))] // no_std versiyonu Read + Seek alır
fn read_metadata_from_reader<R: Read + Seek>(reader: &mut R) -> Result<AiffMetadata, AiffError> { // AiffError döner
    // byteorder::BigEndian ve ReadBytesExt traitlerini kullanıyoruz.
    // core::io::Read ve Seek implement eden R tipi gerekiyor.

     // AIFF dosya başlığını (FORM chunk) kontrol et.
     let mut form_chunk_id = [0; 4];
     reader.read_exact(&mut form_chunk_id)?; // ReadBytesExt::read_exact (core::io::Read üzerine implement edilmiş)

     if &form_chunk_id != b"FORM" {
         return Err(AiffError::InvalidData(
             "Geçersiz AIFF dosyası: FORM chunk ID bulunamadı veya hatalı.".to_string(),
         ));
     }

     let form_chunk_size = reader.read_u32::<BigEndian>()?; // ReadBytesExt::read_u32 (BigEndian)

     let mut aiff_type = [0; 4];
     reader.read_exact(&mut aiff_type)?; // ReadBytesExt::read_exact

     if &aiff_type != b"AIFF" {
         return Err(AiffError::InvalidData(
             "Geçersiz AIFF dosyası: AIFF türü bulunamadı veya hatalı.".to_string(),
         ));
     }

     // Common chunk'u bul ve meta verileri oku (FORM chunk sınırları içinde)
    reader.seek(SeekFrom::Start(12))?; // Position after "FORM" + size + "AIFF" (4+4+4=12 bytes)
    let mut current_pos_in_form_content = 0u64;
    // Need to get the start position of the FORM chunk header (offset 0)
    // This requires seeking to 0 and getting the position.
    reader.seek(SeekFrom::Start(0))?;
    let form_chunk_header_pos = reader.stream_position()?; // Should be 0.
    reader.seek(SeekFrom::Start(12))?; // Go back to after "AIFF"

    let form_content_start_pos = reader.stream_position()?; // Should be 12.
    let form_content_end_pos = form_chunk_header_pos.checked_add(8 + form_chunk_size as u64)
        .ok_or_else(|| AiffError::InvalidData(format!("FORM chunk sonu hesaplanırken taşma")))?;


     while reader.stream_position()? < form_content_end_pos {
         let current_chunk_start_pos = reader.stream_position()?;

         let mut chunk_id = [0; 4];
         // Read exactly 4 bytes for chunk ID, if not enough, it's EOF before end of FORM chunk
         if reader.read_exact(&mut chunk_id).is_err() {
              return Err(AiffError::InvalidData(format!("Beklenenden erken dosya sonu: Chunk ID okunurken.")));
         }


         let chunk_size = reader.read_u32::<BigEndian>()?; // Read chunk size (Big Endian)

         let chunk_data_end_pos = current_chunk_start_pos.checked_add(8 + chunk_size as u64)
             .ok_or_else(|| AiffError::InvalidData(format!("Chunk veri sonu hesaplanırken taşma")))?;

         // Check if chunk goes beyond FORM chunk end
         if chunk_data_end_pos > form_content_end_pos {
              return Err(AiffError::InvalidData(format!("Chunk {} (boyut {}) FORM chunk sınırını aşıyor.", format!("{}", core::str::from_utf8(&chunk_id).unwrap_or("???")), chunk_size)));
         }


         if &chunk_id == b"COMM" {
             // COMM chunk bulundu, verilerini oku
             let num_channels = reader.read_u16::<BigEndian>()?; // ReadBytesExt::read_u16
             let num_frames = reader.read_u32::<BigEndian>()?; // ReadBytesExt::read_u32
             let bits_per_sample = reader.read_u16::<BigEndian>()?; // ReadBytesExt::read_u16

             // Sample rate için genişletilmiş formatı oku (10 byte)
             let mut sample_rate_extended_bytes = [0; 10];
             reader.read_exact(&mut sample_rate_extended_bytes)?; // ReadBytesExt::read_exact

             let sample_rate = match convert_aiff_sample_rate(&sample_rate_extended_bytes) {
                 Ok(rate) => rate,
                 Err(e) => return Err(AiffError::ConversionError(format!("Sample rate dönüşüm hatası: {}", e))),
             };

             // COMM chunk boyutu kontrolü
             if chunk_size != 18 {
                  return Err(AiffError::InvalidData(format!("Geçersiz COMM chunk boyutu: Beklenen 18, bulunan {}", chunk_size)));
             }

             return Ok(AiffMetadata {
                 num_channels,
                 sample_rate,
                 bits_per_sample,
                 num_frames,
             });
         } else {
             // Common chunk değilse, sonraki chunk'a atla.
             let padded_chunk_size = (chunk_size as usize + 1) & !1; // Pad to 2-byte boundary
             reader.seek(SeekFrom::Current(padded_chunk_size as i64))?; // Atla
         }
     }

     // Eğer döngü biterse (tüm FORM chunk işlendi) ve COMM chunk bulunamadıysa.
     Err(AiffError::InvalidData("COMM chunk bulunamadı: FORM chunk içinde arandı.".to_string()))
}

// Helper to convert AIFF 80-bit extended float to f64 (copied from std version's logic)
// This requires floating-point support in the target no_std environment.
fn convert_aiff_sample_rate(bytes: &[u8]) -> Result<u32, String> {
    if bytes.len() != 10 {
        return Err(format!("Beklenen 10 bayt yerine {} bayt", bytes.len()));
    }

    // AIFF 80-bit extended float format:
    // 1 bit sign
    // 15 bits biased exponent (bias 16383)
    // 1 bit integer part (always 1 for normalized numbers, 0 for denormalized)
    // 63 bits fractional part
    // Total 80 bits (10 bytes)

    let mut exponent_bytes = [0; 2];
    exponent_bytes.copy_from_slice(&bytes[0..2]);
    let biased_exponent = u16::from_be_bytes(exponent_bytes);

    let sign = if (biased_exponent & 0x8000) != 0 { -1.0 } else { 1.0 };
    let exponent = (biased_exponent & 0x7FFF) as i32 - 16383; // Unbias exponent

    // Mantissa (integer part + fractional part)
    // Integer part is implied 1 unless exponent is 0 (denormalized)
    // fractional part is 63 bits in the remaining 8 bytes (bytes[2..10])
    let mut mantissa_bytes = [0; 8];
    mantissa_bytes.copy_from_slice(&bytes[2..10]);
    let mantissa_bits = u64::from_be_bytes(mantissa_bytes);

    // The 1 bit integer part is implicit UNLESS the biased exponent is 0.
    let mantissa: f64 = if biased_exponent == 0 {
        // Denormalized number. Integer part is 0.
        mantissa_bits as f64 / 2u64.pow(63) as f64 // fractional part / 2^63
    } else {
        // Normalized number. Integer part is 1.
        1.0 + (mantissa_bits as f64 / 2u64.pow(63) as f64) // 1 + fractional part / 2^63
    };

    let sample_rate_f64 = sign * mantissa * 2.0f64.powi(exponent);

    if sample_rate_f64 < 0.0 {
         // Sample rate cannot be negative.
         return Err(format!("Negatif sample rate değeri: {}", sample_rate_f64));
    }

    // Convert f64 to u32. Handle potential overflow/out of range.
    if sample_rate_f64 > u32::MAX as f64 {
         return Err(format!("Sample rate değeri çok büyük: {}", sample_rate_f64));
    }
    if sample_rate_f64 < 0.0 { // Already checked above, but for safety
        return Err(format!("Sample rate değeri negatif: {}", sample_rate_f64));
    }


    Ok(sample_rate_f64 as u32)
}


// Test module (mostly std implementation)
#[cfg(test)]
#[cfg(feature = "std")] // std feature and test attribute
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use byteorder::{WriteBytesExt, BigEndian as StdBigEndian}; // Need for writing test AIFF data
    use std::fs;
    use core::f64; // For f64::to_exponent etc.

    // Helper function: Creates a simple AIFF file in memory for testing.
    fn create_test_aiff_file() -> Vec<u8> {
        let mut buffer = Vec::new();

        // FORM chunk header (ID + Size + Type)
        buffer.extend_from_slice(b"FORM");
        // Placeholder for FORM chunk size (size of AIFF type + COMM chunk)
        // Size of AIFF type = 4 bytes. Size of COMM chunk header (ID+Size) = 8 bytes. Size of COMM data = 18 bytes. Total = 4 + 8 + 18 = 30 bytes.
        // FORM chunk size field = size of AIFF type + size of COMM chunk (including its header and data)
        // COMM chunk size is 18. COMM chunk total size on disk is 8 (header) + 18 (data) = 26.
        // FORM chunk size field = 4 ("AIFF") + 26 (COMM chunk) = 30.
        buffer.write_u32::<StdBigEndian>(30).unwrap(); // FORM chunk size (size of content after FORM + its size field)
        buffer.extend_from_slice(b"AIFF"); // AIFF type

        // COMM chunk (ID + Size + Data)
        buffer.extend_from_slice(b"COMM");
        buffer.write_u32::<StdBigEndian>(18).unwrap(); // COMM chunk data size (fixed 18 bytes)
        buffer.write_u16::<StdBigEndian>(2).unwrap();   // num_channels = 2
        buffer.write_u32::<StdBigEndian>(44100 * 5).unwrap(); // num_frames = 5 seconds @ 44100Hz
        buffer.write_u16::<StdBigEndian>(16).unwrap();  // bits_per_sample = 16

        // Sample Rate (80-bit extended format - 10 bytes)
        let sample_rate_f64: f64 = 44100.0;
        // Convert f64 to AIFF 80-bit extended format
        // This is complex, using a simplified helper or library is better.
        // Manually constructing for a known value (44100.0):
        // 44100.0 = 4.41e4 = 4.41 * 10^4. In binary?
        // 44100 = 1010110010010100_2. Normalized: 1.010110010010100 * 2^15
        // Exponent = 15. Biased exponent = 15 + 16383 = 16398 = 0100000000001110_2 = 0x400E
        // Mantissa = 010110010010100... (63 bits total after leading 1)
        // For 44100.0, the exact 80-bit representation is known: 0x400e 0xac44 0x0000 0x0000 0x0000
        buffer.write_u16::<StdBigEndian>(0x400E).unwrap(); // Biased Exponent
        buffer.write_u64::<StdBigEndian>(0xac44000000000000).unwrap(); // Mantissa (most significant 64 bits)
                                                                     // The exact mantissa for 44100.0 is 0xac44000000000000.

        // Optional: Add other chunks like SSND (Sound Data)
         buffer.extend_from_slice(b"SSND");
         buffer.write_u32::<StdBigEndian>(data_size).unwrap(); // SSND chunk size
         buffer.write_u32::<StdBigEndian>(0).unwrap(); // offset (for block align)
         buffer.write_u32::<StdBigEndian>(0).unwrap(); // block size
         buffer.extend_from_slice(&audio_data); // Actual audio data

        // Need to pad chunks if their size is odd
        // COMM size is 18 (even), no padding needed after COMM data.
        // Total size of FORM chunk = 4 ("AIFF") + COMM chunk total size (26) + SSND chunk total size (if added) ...

        // Re-calculate FORM chunk size based on total buffer size
        let total_file_size = buffer.len();
        let form_chunk_size = total_file_size - 8; // Total size - FORM header (ID + Size = 8)
        buffer[4..8].copy_from_slice(&form_chunk_size.to_be_bytes()); // Update FORM size field (Big Endian)

        buffer
    }

    // Helper to convert AIFF 80-bit extended float to f64 (copied from super)
    fn convert_aiff_sample_rate_helper(bytes: &[u8]) -> Result<u32, String> {
        if bytes.len() != 10 {
            return Err(format!("Expected 10 bytes, found {}", bytes.len()));
        }

        let biased_exponent = u16::from_be_bytes([bytes[0], bytes[1]]);
        let sign = if (biased_exponent & 0x8000) != 0 { -1.0 } else { 1.0 };
        let exponent = (biased_exponent & 0x7FFF) as i32 - 16383;

        let mut mantissa_bytes = [0; 8];
        mantissa_bytes.copy_from_slice(&bytes[2..10]);
        let mantissa_bits = u64::from_be_bytes(mantissa_bytes);

        let mantissa: f64 = if biased_exponent == 0 {
            mantissa_bits as f64 / 2u64.pow(63) as f64
        } else {
            1.0 + (mantissa_bits as f64 / 2u64.pow(63) as f64)
        };

        let sample_rate_f64 = sign * mantissa * 2.0f64.powi(exponent);

        if sample_rate_f64 < 0.0 {
             return Err(format!("Negative sample rate: {}", sample_rate_f64));
        }
        if sample_rate_f64 > u32::MAX as f64 {
             return Err(format!("Sample rate too large: {}", sample_rate_f64));
        }

        Ok(sample_rate_f64 as u32)
    }


    #[test]
    fn test_read_aiff_metadata() {
        // Test AIFF file data
        let aiff_data = create_test_aiff_file();

        // Use Cursor as a reader for the in-memory data
        let mut cursor = Cursor::new(aiff_data.clone());

        // Call the metadata reading function with the Cursor
        let metadata = read_metadata_from_reader(&mut cursor).unwrap();

        // Assert the extracted metadata matches the test file data
        assert_eq!(metadata.num_channels, 2);
        assert_eq!(metadata.sample_rate, 44100); // Check conversion is correct
        assert_eq!(metadata.bits_per_sample, 16);
        assert_eq!(metadata.num_frames, 44100 * 5);
    }

    #[test]
    fn test_read_aiff_metadata_invalid_form_chunk() {
        let invalid_data = b"FORX\x00\x00\x00\x08AIFFCOMM...".to_vec(); // Invalid FORM ID
        let mut cursor = Cursor::new(invalid_data);
        let result = read_metadata_from_reader(&mut cursor);
        assert!(result.is_err());
        match result.err().unwrap() {
            AiffError::InvalidData(msg) => assert_eq!(msg, "Geçersiz AIFF dosyası: FORM chunk ID bulunamadı veya hatalı."),
            _ => panic!("Wrong error type"),
        }
    }

     #[test]
     fn test_read_aiff_metadata_wrong_aiff_type() {
         let mut data = b"FORM\x00\x00\x00\x08AIFXCOMM...".to_vec(); // Wrong AIFF type
         let mut cursor = Cursor::new(data);
         let result = read_metadata_from_reader(&mut cursor);
         assert!(result.is_err());
         match result.err().unwrap() {
             AiffError::InvalidData(msg) => assert_eq!(msg, "Geçersiz AIFF dosyası: AIFF türü bulunamadı veya hatalı."),
             _ => panic!("Wrong error type"),
         }
     }

    #[test]
    fn test_read_aiff_metadata_no_comm_chunk() {
        // Only FORM and AIFF type, no COMM chunk
        let mut data = b"FORM\x00\x00\x00\x04AIFF".to_vec();
        let mut cursor = Cursor::new(data);
        let result = read_metadata_from_reader(&mut cursor);
        assert!(result.is_err());
        match result.err().unwrap() {
            AiffError::InvalidData(msg) => assert_eq!(msg, "COMM chunk bulunamadı: FORM chunk içinde arandı."),
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_read_aiff_metadata_comm_chunk_too_short() {
        // FORM, AIFF, COMM header, but not enough data for COMM content
        let mut data = b"FORM\x00\x00\x00\x0cAIFFCOMM\x00\x00\x00\x12".to_vec(); // COMM size 18, but only header+size exists
        // Total size = 4 (FORM) + 4 (size) + 4 (AIFF) + 4 (COMM) + 4 (size) = 20.
        // FORM size = 20 - 8 = 12. FORM\x00\x00\x00\x0cAIFFCOMM\x00\x00\x00\x12
         data[4..8].copy_from_slice(&(12u32.to_be_bytes())); // Correct FORM size

        let mut cursor = Cursor::new(data);
        let result = read_metadata_from_reader(&mut cursor);
        assert!(result.is_err());
         // read_exact inside reading COMM content should fail
        match result.err().unwrap() {
             AiffError::IoError(msg) => assert!(msg.contains("read_exact error")), // core::io::Read::read_exact default implementation
             _ => panic!("Wrong error type: {:?}", result.err()),
         }
    }

     #[test]
     fn test_aiff_sample_rate_conversion() {
         // Test known sample rate conversions from 80-bit extended format
         // 44100.0 = 0x400eac44000000000000
         let bytes_44100: [u8; 10] = [0x40, 0x0e, 0xac, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
         assert_eq!(convert_aiff_sample_rate_helper(&bytes_44100).unwrap(), 44100);

         // 48000.0 = 0x400fbb80000000000000
         let bytes_48000: [u8; 10] = [0x40, 0x0f, 0xbb, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
         assert_eq!(convert_aiff_sample_rate_helper(&bytes_48000).unwrap(), 48000);

         // 96000.0 = 0x40107700000000000000
         let bytes_96000: [u8; 10] = [0x40, 0x10, 0x77, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
         assert_eq!(convert_aiff_sample_rate_helper(&bytes_96000).unwrap(), 96000);

         // Test edge cases or invalid inputs
         let too_short_bytes: [u8; 5] = [0; 5];
         assert!(convert_aiff_sample_rate_helper(&too_short_bytes).is_err());

         // Test negative sample rate representation (hypothetical)
         let negative_bytes: [u8; 10] = [0xC0, 0x0e, 0xac, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]; // Sign bit set
         assert!(convert_aiff_sample_rate_helper(&negative_bytes).is_err()); // Should return error due to negative value

          // Test a value that overflows u32 (hypothetical)
          // A very large exponent
          let mut large_bytes: [u8; 10] = [0; 10];
           large_bytes[0..2].copy_from_slice(&0x7FFFu16.to_be_bytes()); // Max biased exponent
           large_bytes[2] = 0x80; // Leading 1 in mantissa (implicit)
           assert!(convert_aiff_sample_rate_helper(&large_bytes).is_err()); // Should overflow u32

     }
}

// Redundant no_std print module and panic handler removed.
