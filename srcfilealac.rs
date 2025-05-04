use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri ve Handle
#[cfg(not(feature = "std"))]
use crate::resource; // Sahne64 resource modülü (no_std implementasyonu için)

// no_std environment imports
#[cfg(not(feature = "std"))]
use core::result::Result; // Redundant
#[cfg(not(feature = "std"))]
use core::fmt::Write as CoreWrite; // Used in redundant print, removed

// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Result as StdResult, Error as StdIOError, ErrorKind as StdIOErrorKind}; // std::io::Error etc.
#[cfg(feature = "std")]
use std::str as StdStr; // std::str

// alloc crate for Vec, String, format! etc.
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;
use alloc::string::String;
use alloc::format;

// core::io traits and types
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io

// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // Assuming these are pub from crate root/common module

// Sahne64 Resource Control Constants (Hypothetical)
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

// Helper struct to implement core::io::Read and Seek for Sahne64 Handle
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
    // Current position is managed by the underlying resource via syscalls,
    // but we might need to store it here if the API doesn't return it consistently
    // from read/write/control. Let's assume seek returns the new position.
}

#[cfg(not(feature = "std"))]
impl SahneResourceReader {
    pub fn new(handle: Handle) -> Self {
        SahneResourceReader { handle }
    }

    // Helper to map SahneError to core::io::Error
    fn map_sahne_error_to_io_error(e: SahneError) -> CoreIOError {
        // Map SahneError variants to appropriate CoreIOErrorKind
        // This requires knowledge of SahneError variants.
        // For now, map to a generic error kind or Other.
        CoreIOError::new(CoreIOErrorKind::Other, format!("SahneError: {:?}", e)) // Using Other and formatting Debug output
        // A better mapping would be specific:
         match e {
             SahneError::ResourceNotFound => CoreIOError::new(CoreIOErrorKind::NotFound, format!("Resource not found")),
             SahneError::InvalidParameter => CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Invalid parameter")),
             SahneError::PermissionDenied => CoreIOError::new(CoreIOErrorKind::PermissionDenied, format!("Permission denied")),
             SahneError::EndOfFile => CoreIOError::new(CoreIOErrorKind::UnexpectedEof, format!("End of file")), // Or just return Ok(0) from read
             _ => CoreIOError::new(CoreIOErrorKind::Other, format!("SahneError: {:?}", e)),
         }
    }
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        // resource::read returns Result<usize, SahneError>
        resource::read(self.handle, buf).map_err(Self::map_sahne_error_to_io_error)
    }
    // read_exact has a default implementation based on read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        // Use resource::control for seeking.
        // Need to map core::io::SeekFrom to Sahne64 control command and argument.
        let (command, offset_arg) = match pos {
            SeekFrom::Start(o) => (CONTROL_SEEK_FROM_START, o),
            SeekFrom::End(o) => {
                // Handle i64 offset for End/Current
                // Assuming resource::control takes u64 as argument, need to pass i64 offset correctly.
                // This is an ABI detail that needs clarification in Sahne64 API.
                // For now, pass as u64, casting i64. This is UNSAFE if offset is negative.
                // A better API would handle i64 directly or have separate commands/args.
                 println!("WARN: SahneResourceReader::seek from End offset i64 ({}) being cast to u64 for syscall.", o); // no_std print makrosu
                (CONTROL_SEEK_FROM_END, o as u64) // DİKKAT: Negatif i64 casting to u64 tehlikeli olabilir!
            },
            SeekFrom::Current(o) => {
                 println!("WARN: SahneResourceReader::seek from Current offset i64 ({}) being cast to u64 for syscall.", o); // no_std print makrosu
                (CONTROL_SEEK_FROM_CURRENT, o as u64) // DİKKAT: Negatif i64 casting to u64 tehlikeli olabilir!
            },
        };

        // Assuming resource::control returns Result<i64, SahneError> where i64 is the new position.
        let result = resource::control(self.handle, command, offset_arg, 0); // resource::control çağrılır

        match result {
            Ok(new_pos_i64) => {
                if new_pos_i64 < 0 {
                    // Negative result from syscall indicates an error.
                     println!("ERROR: SahneResourceReader::seek resource::control returned negative position: {}", new_pos_i64); // no_std print makrosu
                    Err(CoreIOError::new(CoreIOErrorKind::Other, format!("Seek syscall returned negative position: {}", new_pos_i64))) // Or InvalidInput
                } else {
                    Ok(new_pos_i64 as u64) // i64 -> u64 dönüşümü (yeni pozisyon)
                }
            },
            Err(e) => {
                 println!("ERROR: SahneResourceReader::seek resource::control hatası: {:?}", e); // no_std print makrosu
                Err(Self::map_sahne_error_to_io_error(e)) // SahneError -> CoreIOError
            }
        }
    }

    // stream_position has a default implementation based on seek
}


// Helper to map std::io::Error to FileSystemError
#[cfg(feature = "std")]
fn map_io_error_to_fs_error(e: StdIOError) -> FileSystemError {
    FileSystemError::IOError(format!("IO Error: {}", e))
}

// Helper to map SahneError to FileSystemError
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
}

// Helper to map CoreIOError to FileSystemError (for no_std Read/Seek on SahneResourceReader)
#[cfg(not(feature = "std"))]
fn map_core_io_error_to_fs_error(e: CoreIOError) -> FileSystemError {
     FileSystemError::IOError(format!("CoreIOError: {:?}", e)) // Using Debug format for CoreIOError
     // Specific mapping based on e.kind() could be done here
}


#[derive(Debug)]
pub struct AlacMetadata {
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    // Daha fazla meta veri alanı eklenebilir
}

/// Belirtilen dosya yolundaki (veya kaynak ID'sindeki) ALAC dosyasının
/// meta verilerini (sample rate, channels, bits per sample) okur.
#[cfg(feature = "std")]
pub fn read_alac_metadata(file_path: &str) -> Result<AlacMetadata, FileSystemError> { // FileSystemError döner
    let file = File::open(file_path).map_err(map_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file); // BufReader implement StdRead + StdSeek

    // ftyp atomunu kontrol et
    check_ftyp_atom(&mut reader).map_err(map_io_error_to_fs_error)?;

    // moov atomunu bul ve işle
    let moov_atom_offset = find_moov_atom(&mut reader).map_err(map_io_error_to_fs_error)?;
    let metadata = process_moov_atom(&mut reader, moov_atom_offset).map_err(map_io_error_to_fs_error)?;

    // Meta veri bulunduysa dön, yoksa hata ver
    metadata.ok_or_else(|| FileSystemError::Other(format!("ALAC meta verileri bulunamadı")))
}

/// Belirtilen Sahne64 kaynak ID'sindeki ALAC dosyasının
/// meta verilerini okur (no_std).
#[cfg(not(feature = "std"))]
pub fn read_alac_metadata(resource_id: &str) -> Result<AlacMetadata, FileSystemError> { // FileSystemError döner
    // Kaynağı edin
    let handle = resource::acquire(resource_id, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Sahne64 Handle'ı için core::io::Read + Seek implementasyonu sağlayan Reader struct'ı oluştur
    let mut reader = SahneResourceReader::new(handle);

    // ftyp atomunu kontrol et
    check_ftyp_atom(&mut reader).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError

    // moov atomunu bul ve işle
    let moov_atom_offset = find_moov_atom(&mut reader).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError
    let metadata = process_moov_atom(&mut reader, moov_atom_offset).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError

    // Kaynağı serbest bırak
    let _ = resource::release(handle).map_err(|e| {
         eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print makrosu
         map_sahne_error_to_fs_error(e) // SahneError -> FileSystemError
     });

    // Meta veri bulunduysa dön, yoksa hata ver
    metadata.ok_or_else(|| FileSystemError::Other(format!("ALAC meta verileri bulunamadı")))
}


// Atom işleme fonksiyonları (core::io::Read + Seek trait'leri üzerinden çalışır)
// std ve no_std implementasyonları aynı temel mantığı kullanacak.

// ftyp atomunu kontrol et
#[cfg(feature = "std")] // std versiyonu std::io::Error döner
fn check_ftyp_atom<R: StdRead + StdSeek>(reader: &mut R) -> Result<(), StdIOError> { // StdIOError döner
    // seek ve read_exact çağrıları R trait'leri üzerinden yapılır.
    reader.seek(StdSeekFrom::Start(0))?; // Dosyanın başına git (reader StdSeek implement etmeli)

    let mut ftyp_header = [0; 8];
    // read_exact() varsayılan olarak Read trait'inde implement edilmiştir.
    // core::io::Read trait'inde de read_exact default implementasyonu vardır.
    // StdRead de read_exact implement eder.
    reader.read_exact(&mut ftyp_header)?;

    let ftyp_size = u32::from_be_bytes([ftyp_header[0], ftyp_header[1], ftyp_header[2], ftyp_header[3]]);
    let ftyp_type = &ftyp_header[4..8];

    if ftyp_type != b"ftyp" {
        return Err(StdIOError::new(
            StdIOErrorKind::InvalidData,
            "Geçersiz MP4 dosyası: ftyp atomu bulunamadı",
        ));
    }

    if ftyp_size < 12 { // ftyp atomu en az 12 bayt olmalı (size + type + major_brand + ...)
        return Err(StdIOError::new(
            StdIOErrorKind::InvalidData,
            "Geçersiz ftyp atom boyutu",
        ));
    }

    let mut major_brand = [0; 4];
    reader.read_exact(&mut major_brand)?;
     // std dalında std::str::from_utf8 kullanılır.
    if &major_brand != b"M4A " && &major_brand != b"mp42" && &major_brand != b"isom" { // Yaygın major brandler
        return Err(StdIOError::new(
            StdIOErrorKind::InvalidData,
            format!("Beklenmeyen major brand: {:?}", StdStr::from_utf8(&major_brand)),
        ));
    }
    Ok(())
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn check_ftyp_atom<R: Read + Seek>(reader: &mut R) -> Result<(), CoreIOError> { // CoreIOError döner
    // seek ve read_exact çağrıları R trait'leri üzerinden yapılır.
    reader.seek(SeekFrom::Start(0))?; // Kaynağın başına git (reader Seek implement etmeli)

    let mut ftyp_header = [0; 8];
    // core::io::Read trait'inde read_exact default implementasyonu vardır.
    reader.read_exact(&mut ftyp_header)?;

    let ftyp_size = u32::from_be_bytes([ftyp_header[0], ftyp_header[1], ftyp_header[2], ftyp_header[3]]);
    let ftyp_type = &ftyp_header[4..8];

    if ftyp_type != b"ftyp" {
        return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz MP4 dosyası: ftyp atomu bulunamadı")));
    }

    if ftyp_size < 12 {
         return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz ftyp atom boyutu")));
    }

    let mut major_brand = [0; 4];
    reader.read_exact(&mut major_brand)?;
     // no_std dalında core::str::from_utf8 kullanılır. format! için alloc gerekir.
    if &major_brand != b"M4A " && &major_brand != b"mp42" && &major_brand != b"isom" {
        return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Beklenmeyen major brand: {:?}", core::str::from_utf8(&major_brand).unwrap_or(""))));
    }
    Ok(())
}

// moov atomunu bulur ve başlangıç ofsetini döner
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn find_moov_atom<R: StdRead + StdSeek>(reader: &mut R) -> Result<u64, StdIOError> { // StdIOError döner
    reader.seek(StdSeekFrom::Start(0))?; // Dosyanın başına git
    loop {
        let current_pos = reader.stream_position()?; // stream_position default implementasyona sahip

        let mut header = [0; 8];
        let bytes_read = reader.read(&mut header)?;
        if bytes_read == 0 {
            return Err(StdIOError::new(
                StdIOErrorKind::NotFound,
                "moov atomu bulunamadı (dosya sonu)", // Daha açıklayıcı mesaj
            ));
        }
        if bytes_read < 8 {
             // Yeterli veri yok, ancak dosya sonuna ulaştıysak hata vermeli (read == 0 kontrol edildi).
             // Kısmi okuma olduysa, kalanını okumaya çalışmak yerine hata vermek daha güvenli olabilir.
             return Err(StdIOError::new(
                 StdIOErrorKind::UnexpectedEof,
                 "Dosya sonuna beklenenden erken ulaşıldı atom başlığı okunurken",
             ));
        }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let atom_type = &header[4..8];

        if atom_type == b"moov" {
             // moov atomunun başlangıç pozisyonu = current_pos (okuma öncesi)
            return Ok(current_pos); // 'moov' atomunun başlangıç pozisyonunu döndür
        }

        // Bir sonraki atomun başına atla
        // Eğer atom_size 8'den küçükse (imkansız ama kontrol edelim) veya 0 ise
         if atom_size == 0 {
             // Atom boyutu 0 ise, dosyanın sonuna kadar oku veya hata ver. MP4'te boyut 0 genellikle geçerli değil.
             return Err(StdIOError::new(
                 StdIOErrorKind::InvalidData,
                 "Geçersiz atom boyutu (0)",
             ));
         }
         // Atom boyutu 1 ise, 64-bit boyut takip eder
         if atom_size == 1 {
              // 64-bit boyut için 8 bayt daha oku
              let mut large_size_bytes = [0; 8];
              reader.read_exact(&mut large_size_bytes)?;
              let large_atom_size = u64::from_be_bytes(large_size_bytes);
              let size_to_skip = large_atom_size.checked_sub(16) // size(8) + type(4) + extended_size(8) = 20, 16'yı okuduk.
                .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, "Geçersiz 64-bit atom boyutu"))?;

              let next_offset = current_pos.checked_add(large_atom_size) // current_pos + large_atom_size
                 .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, "Sonraki ofset hesaplanırken taşma"))?;

               reader.seek(StdSeekFrom::Start(next_offset))?; // Tam olarak bir sonraki atomun başına git
         } else {
             // Normal 32-bit boyut
              let size_to_skip = atom_size.checked_sub(8) // size(4) + type(4) = 8'i okuduk.
                 .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, "Geçersiz atom boyutu"))?;

              let next_offset = current_pos.checked_add(atom_size as u64) // current_pos + atom_size
                 .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, "Sonraki ofset hesaplanırken taşma"))?;

              reader.seek(StdSeekFrom::Start(next_offset))?; // Tam olarak bir sonraki atomun başına git

         }
    }
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn find_moov_atom<R: Read + Seek>(reader: &mut R) -> Result<u64, CoreIOError> { // CoreIOError döner
    reader.seek(SeekFrom::Start(0))?; // Kaynağın başına git
    loop {
        let current_pos = reader.stream_position()?; // stream_position default implementasyona sahip

        let mut header = [0; 8];
        let bytes_read = reader.read(&mut header)?;
        if bytes_read == 0 {
            return Err(CoreIOError::new(CoreIOErrorKind::NotFound, format!("moov atomu bulunamadı (kaynak sonu)")));
        }
        if bytes_read < 8 {
            return Err(CoreIOError::new(CoreIOErrorKind::UnexpectedEof, format!("Kaynağın sonuna beklenenden erken ulaşıldı atom başlığı okunurken")));
        }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let atom_type = &header[4..8];

        if atom_type == b"moov" {
            return Ok(current_pos); // 'moov' atomunun başlangıç pozisyonunu döndür
        }

        // Bir sonraki atomun başına atla
         if atom_size == 0 {
             return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu (0)")));
         }
         if atom_size == 1 {
              // 64-bit boyut için 8 bayt daha oku
              let mut large_size_bytes = [0; 8];
              reader.read_exact(&mut large_size_bytes)?;
              let large_atom_size = u64::from_be_bytes(large_size_bytes);
              let size_to_skip = large_atom_size.checked_sub(16) // size(8) + type(4) + extended_size(8) = 20, 16'yı okuduk.
                .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz 64-bit atom boyutu")))?;

              let next_offset = current_pos.checked_add(large_atom_size) // current_pos + large_atom_size
                 .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

               reader.seek(SeekFrom::Start(next_offset))?; // Tam olarak bir sonraki atomun başına git
         } else {
             // Normal 32-bit boyut
              let size_to_skip = atom_size.checked_sub(8) // size(4) + type(4) = 8'i okuduk.
                 .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu")))?;

              let next_offset = current_pos.checked_add(atom_size as u64) // current_pos + atom_size
                 .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

              reader.seek(SeekFrom::Start(next_offset))?; // Tam olarak bir sonraki atomun başına git
         }
    }
}

// moov atomunu işler, trak atomlarını arar
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn process_moov_atom<R: StdRead + StdSeek>(reader: &mut R, moov_atom_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    reader.seek(StdSeekFrom::Start(moov_atom_offset))?;
    let mut moov_header = [0; 8];
    reader.read_exact(&mut moov_header)?; // moov başlığını tekrar oku (boyut ve tip)
    let moov_size = u32::from_be_bytes([moov_header[0], moov_header[1], moov_header[2], moov_header[3]]) as u64;

    let mut current_offset = moov_atom_offset + 8; // moov başlığından sonraki pozisyon

    while current_offset < moov_atom_offset + moov_size {
         reader.seek(StdSeekFrom::Start(current_offset))?;
         let mut header = [0; 8];
         // read() yerine read_exact() kullanmak daha güvenli, tam 8 byte bekleriz
         if reader.read_exact(&mut header).is_err() { // Dosya sonuna ulaşılmış olabilir (veya yeterli veri yok)
             break; // Döngüden çık, hata yoksa moov atomu bitti
         }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"trak" {
            let metadata = process_trak_atom(reader, current_offset)?;
            if metadata.is_some() {
                 // ALAC metadata bulundu, döndür
                return Ok(metadata);
            }
        }

        // Bir sonraki atomun başına atla
         if atom_size == 0 {
              // Boyut 0 ise bu atom geçerli değil veya dosya sonu, döngüyü kır
             break;
         }
         let next_offset = current_offset.checked_add(atom_size) // current_offset + atom_size
            .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

         if next_offset <= current_offset { // Taşma veya geçersiz boyut (next_offset = current_offset ise atom_size 0 demektir)
              // next_offset current_offset'tan küçük olamaz (u64). Eşitlik size 0 anlamına gelir.
              // Size 0 durumu yukarıda ele alındı. Buraya geliyorsa mantıksal hata var.
             return Err(StdIOError::new(
                 StdIOErrorKind::InvalidData,
                 format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size),
             ));
         }
         current_offset = next_offset; // Sonraki atomun başlangıcı
         // 64-bit boyutlu atomlar find_moov_atom'da ele alınmalıydı, burada sadece 32-bit atomların içindeyiz varsayılıyor.
         // Eğer 64-bit atom moov içinde olursa ve boyutu buradan geçerse, seek yanlış olur.
         // find_moov_atom ve diğer parent atom işleme fonksiyonlarının 64-bit boyutu doğru işlemesi kritik.
    }

    Ok(None) // ALAC meta verileri bulunamadı (trak atomu içinde)
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn process_moov_atom<R: Read + Seek>(reader: &mut R, moov_atom_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    reader.seek(SeekFrom::Start(moov_atom_offset))?;
    let mut moov_header = [0; 8];
    reader.read_exact(&mut moov_header)?; // moov başlığını tekrar oku (boyut ve tip)
    let moov_size = u32::from_be_bytes([moov_header[0], moov_header[1], moov_header[2], moov_header[3]]) as u64;

    let mut current_offset = moov_atom_offset + 8; // moov başlığından sonraki pozisyon

    while current_offset < moov_atom_offset + moov_size {
        reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
         if reader.read_exact(&mut header).is_err() {
             break; // Döngüden çık
         }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"trak" {
            let metadata = process_trak_atom(reader, current_offset)?;
            if metadata.is_some() {
                 // ALAC metadata bulundu, döndür
                return Ok(metadata);
            }
        }

        // Bir sonraki atomun başına atla
         if atom_size == 0 {
             break;
         }
         let next_offset = current_offset.checked_add(atom_size)
            .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

         if next_offset <= current_offset { // Taşma veya geçersiz boyut
             return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
         }
         current_offset = next_offset;
    }

    Ok(None) // ALAC meta verileri bulunamadı (trak atomu içinde)
}

// trak atomunu işler, mdia atomlarını arar
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn process_trak_atom<R: StdRead + StdSeek>(reader: &mut R, trak_atom_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    let mut trak_header = [0; 8];
    reader.seek(StdSeekFrom::Start(trak_atom_offset))?;
    reader.read_exact(&mut trak_header)?;
    let trak_size = u32::from_be_bytes([trak_header[0], trak_header[1], trak_header[2], trak_header[3]]) as u64;


    let mut current_offset = trak_atom_offset + 8;

    while current_offset < trak_atom_offset + trak_size {
        reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
         if reader.read_exact(&mut header).is_err() {
             break;
         }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"mdia" {
            let metadata = process_mdia_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset.checked_add(atom_size)
           .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

        if next_offset <= current_offset {
             return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
        }
        current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (mdia atomu içinde)
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn process_trak_atom<R: Read + Seek>(reader: &mut R, trak_atom_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    let mut trak_header = [0; 8];
    reader.seek(SeekFrom::Start(trak_atom_offset))?;
    reader.read_exact(&mut trak_header)?;
    let trak_size = u32::from_be_bytes([trak_header[0], trak_header[1], trak_header[2], trak_header[3]]) as u64;


    let mut current_offset = trak_atom_offset + 8;

    while current_offset < trak_atom_offset + trak_size {
        reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
         if reader.read_exact(&mut header).is_err() {
             break;
         }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"mdia" {
            let metadata = process_mdia_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset.checked_add(atom_size)
           .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

        if next_offset <= current_offset {
             return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
        }
        current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (mdia atomu içinde)
}


// mdia atomunu işler, minf atomunu arar
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn process_mdia_atom<R: StdRead + StdSeek>(reader: &mut R, mdia_atom_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    let mut mdia_header = [0; 8];
    reader.seek(StdSeekFrom::Start(mdia_atom_offset))?;
    reader.read_exact(&mut mdia_header)?;
    let mdia_size = u32::from_be_bytes([mdia_header[0], mdia_header[1], mdia_header[2], mdia_header[3]]) as u64;

    let mut current_offset = mdia_atom_offset + 8;

    while current_offset < mdia_atom_offset + mdia_size {
        reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"minf" {
            let metadata = process_minf_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset.checked_add(atom_size)
           .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

        if next_offset <= current_offset {
             return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
        }
        current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (minf atomu içinde)
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn process_mdia_atom<R: Read + Seek>(reader: &mut R, mdia_atom_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    let mut mdia_header = [0; 8];
    reader.seek(SeekFrom::Start(mdia_atom_offset))?;
    reader.read_exact(&mut mdia_header)?;
    let mdia_size = u32::from_be_bytes([mdia_header[0], mdia_header[1], mdia_header[2], mdia_header[3]]) as u64;

    let mut current_offset = mdia_atom_offset + 8;

    while current_offset < mdia_atom_offset + mdia_size {
        reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"minf" {
            let metadata = process_minf_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset.checked_add(atom_size)
           .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

        if next_offset <= current_offset {
             return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
        }
        current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (minf atomu içinde)
}

// minf atomunu işler, stbl atomunu arar
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn process_minf_atom<R: StdRead + StdSeek>(reader: &mut R, minf_atom_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    let mut minf_header = [0; 8];
    reader.seek(StdSeekFrom::Start(minf_atom_offset))?;
    reader.read_exact(&mut minf_header)?;
    let minf_size = u32::from_be_bytes([minf_header[0], minf_header[1], minf_header[2], minf_header[3]]) as u64;

    let mut current_offset = minf_atom_offset + 8;

    while current_offset < minf_atom_offset + minf_size {
        reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stbl" {
            let metadata = process_stbl_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset.checked_add(atom_size)
           .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

        if next_offset <= current_offset {
             return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
        }
        current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (stbl atomu içinde)
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn process_minf_atom<R: Read + Seek>(reader: &mut R, minf_atom_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    let mut minf_header = [0; 8];
    reader.seek(SeekFrom::Start(minf_atom_offset))?;
    reader.read_exact(&mut minf_header)?;
    let minf_size = u32::from_be_bytes([minf_header[0], minf_header[1], minf_header[2], minf_header[3]]) as u64;

    let mut current_offset = minf_atom_offset + 8;

    while current_offset < minf_atom_offset + minf_size {
        reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stbl" {
            let metadata = process_stbl_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset.checked_add(atom_size)
           .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

        if next_offset <= current_offset {
             return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
        }
        current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (stbl atomu içinde)
}


// stbl atomunu işler, stsd atomunu arar
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn process_stbl_atom<R: StdRead + StdSeek>(reader: &mut R, stbl_atom_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    let mut stbl_header = [0; 8];
    reader.seek(StdSeekFrom::Start(stbl_atom_offset))?;
    reader.read_exact(&mut stbl_header)?;
    let stbl_size = u32::from_be_bytes([stbl_header[0], stbl_header[1], stbl_header[2], stbl_header[3]]) as u64;

    let mut current_offset = stbl_atom_offset + 8;

    while current_offset < stbl_atom_offset + stbl_size {
        reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stsd" {
            // stsd atomu içinde ALAC descriptor'ını arayacağız
            return process_stsd_atom(reader, current_offset); // process_stsd_atom Result<Option<AlacMetadata>, IoError> döner
        }
         let next_offset = current_offset.checked_add(atom_size)
            .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

         if next_offset <= current_offset {
              return Err(StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
         }
         current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (stsd atomu içinde)
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn process_stbl_atom<R: Read + Seek>(reader: &mut R, stbl_atom_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    let mut stbl_header = [0; 8];
    reader.seek(SeekFrom::Start(stbl_atom_offset))?;
    reader.read_exact(&mut stbl_header)?;
    let stbl_size = u32::from_be_bytes([stbl_header[0], stbl_header[1], stbl_header[2], stbl_header[3]]) as u64;

    let mut current_offset = stbl_atom_offset + 8;

    while current_offset < stbl_atom_offset + stbl_size {
        reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stsd" {
            return process_stsd_atom(reader, current_offset); // process_stsd_atom Result<Option<AlacMetadata>, CoreIOError> döner
        }
         let next_offset = current_offset.checked_add(atom_size)
            .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki ofset hesaplanırken taşma")))?;

         if next_offset <= current_offset {
              return Err(CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz atom boyutu veya taşma (current: {}, size: {})", current_offset, atom_size)));
         }
         current_offset = next_offset;
    }
    Ok(None) // ALAC meta verileri bulunamadı (stsd atomu içinde)
}


// stsd atomunu işler, alac veya enca descriptor'ını arar
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn process_stsd_atom<R: StdRead + StdSeek>(reader: &mut R, stsd_atom_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    reader.seek(StdSeekFrom::Start(stsd_atom_offset + 8))?; // stsd başlığını atla (boyut+tip)

    // version(1 byte) + flags(3 bytes) (şimdilik atla)
    let mut version_flags = [0; 4];
    reader.read_exact(&mut version_flags)?;

    let mut entry_count_bytes = [0; 4];
    reader.read_exact(&mut entry_count_bytes)?;
    let entry_count = u32::from_be_bytes(entry_count_bytes);

    let mut current_offset = stsd_atom_offset + 8 + 4 + 4; // stsd başlığı + version/flags + entry_count'ı atladık

    for _ in 0..entry_count {
         reader.seek(StdSeekFrom::Start(current_offset))?; // Her girişin başına git
        let mut entry_header = [0; 8];
        if reader.read_exact(&mut entry_header).is_err() {
            break; // Yeterli veri yok veya hata
        }
        let entry_size = u32::from_be_bytes([entry_header[0], entry_header[1], entry_header[2], entry_header[3]]) as u64;
        let entry_type = &entry_header[4..8];

        if entry_type == b"alac" || entry_type == b"enca" { // 'enca' şifrelenmiş ALAC için olabilir
             // ALAC descriptor bulundu, oku ve metadatayı dön
             return read_alac_desc_data(reader, current_offset); // read_alac_desc_data Result<Option<AlacMetadata>, IoError> döner
        } else {
             // Diğer descriptor'ı atla
             let size_to_skip = entry_size.checked_sub(8) // size(4) + type(4) = 8'i okuduk
                .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Geçersiz giriş boyutu")))?;

             let next_offset = current_offset.checked_add(entry_size)
                .ok_or_else(|| StdIOError::new(StdIOErrorKind::InvalidData, format!("Sonraki giriş ofseti hesaplanırken taşma")))?;

             current_offset = next_offset; // Sonraki girişin başlangıcı
              reader.seek(StdSeekFrom::Current(size_to_skip))?; // Bu yaklaşım yerine doğrudan next_offset'e seek daha güvenli
        }
    }

    Ok(None) // ALAC descriptor bulunamadı
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn process_stsd_atom<R: Read + Seek>(reader: &mut R, stsd_atom_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    reader.seek(SeekFrom::Start(stsd_atom_offset + 8))?; // stsd başlığını atla (boyut+tip)

    // version(1 byte) + flags(3 bytes) (şimdilik atla)
    let mut version_flags = [0; 4];
    reader.read_exact(&mut version_flags)?;

    let mut entry_count_bytes = [0; 4];
    reader.read_exact(&mut entry_count_bytes)?;
    let entry_count = u32::from_be_bytes(entry_count_bytes);

    let mut current_offset = stsd_atom_offset + 8 + 4 + 4; // stsd başlığı + version/flags + entry_count'ı atladık

    for _ in 0..entry_count {
        reader.seek(SeekFrom::Start(current_offset))?; // Her girişin başına git
        let mut entry_header = [0; 8];
        if reader.read_exact(&mut entry_header).is_err() {
            break; // Yeterli veri yok veya hata
        }
        let entry_size = u32::from_be_bytes([entry_header[0], entry_header[1], entry_header[2], entry_header[3]]) as u64;
        let entry_type = &entry_header[4..8];

        if entry_type == b"alac" || entry_type == b"enca" { // 'enca' şifrelenmiş ALAC için olabilir
             // ALAC descriptor bulundu, oku ve metadatayı dön
             return read_alac_desc_data(reader, current_offset); // read_alac_desc_data Result<Option<AlacMetadata>, CoreIOError> döner
        } else {
             // Diğer descriptor'ı atla
             let size_to_skip = entry_size.checked_sub(8) // size(4) + type(4) = 8'i okuduk
                .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Geçersiz giriş boyutu")))?;

             let next_offset = current_offset.checked_add(entry_size)
                .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidData, format!("Sonraki giriş ofseti hesaplanırken taşma")))?;

             current_offset = next_offset; // Sonraki girişin başlangıcı
        }
    }

    Ok(None) // ALAC descriptor bulunamadı
}


// alac descriptor verisini okur ve AlacMetadata oluşturur
#[cfg(feature = "std")] // std versiyonu io::Error döner
fn read_alac_desc_data<R: StdRead + StdSeek>(reader: &mut R, alac_desc_offset: u64) -> Result<Option<AlacMetadata>, StdIOError> { // StdIOError döner
    reader.seek(StdSeekFrom::Start(alac_desc_offset + 8))?; // alac atom başlığını atla (size+type)

    // Descriptor versiyon ve revizyon numaralarını atla (ilk 4 bayt)
    reader.seek(StdSeekFrom::Current(4))?;

    // ALAC descriptor formatına göre bilgileri oku
    // https://developer.apple.com/library/archive/documentation/QuickTime/QTFF/QTFFChap3/qtff3.html#//apple_ref/doc/uid/TP40000939-CH205-SW81
    // veya https://wiki.multimedia.cx/index.php/Apple_Lossless_Audio_Codec#Codec_Private_Data

    let mut descriptor_bytes = [0; 20]; // ALAC Descriptor'ının ilk 20 baytı sample rate, channels vb. içerir.
                                       // Tam boyut 36 bayt veya daha fazla olabilir, ihtiyacımız olan kısımları okuyalım.
    reader.read_exact(&mut descriptor_bytes)?;


    // Descriptor'dan gerekli bilgileri çıkar
    // Bu offsetler descriptor başından itibaren (atladığımız ilk 8 byte sonrası)
    // Tam ALAC specific information structure (CodecPrivateData) layout:
    // 4 bytes: max frame bytes
    // 4 bytes: max packet bytes
    // 1 byte: comportable version
    // 1 byte: bits per sample (BPS)
    // 2 bytes: pb
    // 1 byte: mb
    // 1 byte: kb
    // 1 byte: channels
    // 2 bytes: max history
    // 4 bytes: initial history
    // 4 bytes: sample rate (BE)

    let bits_per_sample = descriptor_bytes[8]; // BPS
    let channels = descriptor_bytes[11]; // channels
    let sample_rate = u32::from_be_bytes([descriptor_bytes[16], descriptor_bytes[17], descriptor_bytes[18], descriptor_bytes[19]]); // Sample Rate (BE)


    Ok(Some(AlacMetadata {
        sample_rate,
        channels,
        bits_per_sample,
    }))
}

#[cfg(not(feature = "std"))] // no_std versiyonu CoreIOError döner
fn read_alac_desc_data<R: Read + Seek>(reader: &mut R, alac_desc_offset: u64) -> Result<Option<AlacMetadata>, CoreIOError> { // CoreIOError döner
    reader.seek(SeekFrom::Start(alac_desc_offset + 8))?; // alac atom başlığını atla (size+type)

    // Descriptor versiyon ve revizyon numaralarını atla (ilk 4 bayt)
    reader.seek(SeekFrom::Current(4))?;

    // ALAC descriptor formatına göre bilgileri oku (core::io::Read + Seek üzerinden)
    let mut descriptor_bytes = [0; 20]; // ALAC Descriptor'ının ilk 20 baytı.
    reader.read_exact(&mut descriptor_bytes)?;


    // Descriptor'dan gerekli bilgileri çıkar (aynı mantık)
    let bits_per_sample = descriptor_bytes[8]; // BPS
    let channels = descriptor_bytes[11]; // channels
    let sample_rate = u32::from_be_bytes([descriptor_bytes[16], descriptor_bytes[17], descriptor_bytes[18], descriptor_bytes[19]]); // Sample Rate (BE)


    Ok(Some(AlacMetadata {
        sample_rate,
        channels,
        bits_per_sample,
    }))
}


// Örnek main fonksiyonları (test amaçlı)
#[cfg(feature = "example_alac")] // Farklı bir özellik bayrağı kullanıldı
fn main() { // main fonksiyonu Result dönmeyebilir, hataları kendisi handle etmeli
    #[cfg(not(feature = "std"))]
    { // no_std println!/eprintln! makrolarının scope'u
         eprintln!("ALAC metadata example (no_std) starting...");
         // Varsayımsal bir konsol handle'ı ayarlayalım.
          crate::init_console(crate::Handle(3)); // init_console'ı çağırabilmek için Handle tipi ve init_console fonksiyonu pub olmalı.
    }
    #[cfg(feature = "std")]
    { // std println!/eprintln! makrolarının scope'u
         eprintln!("ALAC metadata example (std) starting...");
    }


    // Test amaçlı varsayımsal dosya yolu/kaynak ID'si
    // Gerçek bir ALAC (.m4a) dosyası gereklidir.
    let file_path_or_resource_id = "sahne://files/music.m4a"; // Sahne64 kaynak ID'si

    match read_alac_metadata(file_path_or_resource_id) {
        Ok(metadata) => {
            println!("ALAC Meta Verileri:");
            println!("  Sample Rate: {}", metadata.sample_rate);
            println!("  Channels: {}", metadata.channels);
            println!("  Bits per Sample: {}", metadata.bits_per_sample);
        }
        Err(e) => {
            eprintln!("'{}' ALAC meta veri okuma hatası: {}", file_path_or_resource_id, e);
        }
    }

     #[cfg(not(feature = "std"))]
     eprintln!("ALAC metadata example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("ALAC metadata example (std) finished.");
}

// Test modülü (çoğunlukla std implementasyonunu test eder)
#[cfg(test)]
#[cfg(feature = "std")] // std feature'ı ve test özelliği varsa derle
mod tests {
    use super::*;
    use std::fs::write; // std::fs::write kullanımı için
     use std::io::Cursor; // Bellek içi Reader/Seeker için
     use alloc::string::ToString; // test içinde to_string() kullanımı için

     // Basit ftyp atomu (boyut 16, tip ftyp, major_brand M4A )
     const FTYP_ATOM_M4A: &[u8] = &[
         0x00, 0x00, 0x00, 0x10, // size (16)
         b'f', b't', b'y', b'p', // type (ftyp)
         b'M', b'4', b'A', b' ', // major_brand (M4A )
         0x00, 0x00, 0x00, 0x00, // minor_version
     ];

     // Basit moov atomu (boyut 8, tip moov)
     const MOOV_ATOM_BASIC: &[u8] = &[
         0x00, 0x00, 0x00, 0x08, // size (8)
         b'm', b'o', b'o', b'v', // type (moov)
     ];

     // Çok basit bir ALAC MP4 dosyası simülasyonu (sadece ftyp ve moov başlıkları)
     const SIMPLE_ALAC_FILE_SIM: &[u8] = &[
          0x00, 0x00, 0x00, 0x18, // size (24) - ftyp atomu ve sonrası
          b'f', b't', b'y', b'p', // type (ftyp)
          b'M', b'4', b'A', b' ', // major_brand (M4A )
          0x00, 0x00, 0x00, 0x00, // minor_version
          b'i', b's', b'o', b'm', // compatible_brands (isom)
          b'm', b'p', b'4', b'2', // compatible_brands (mp42)
          // moov atomu (daha sonra gelecek)
          0x00, 0x00, 0x00, 0x08, // size (8)
          b'm', b'o', b'o', b'v', // type (moov)
          // Diğer atomlar...
     ];


     // check_ftyp_atom testleri (bellek içi Cursor kullanılarak)
     #[test]
     fn test_check_ftyp_atom_valid() {
         let mut reader = Cursor::new(FTYP_ATOM_M4A);
         let result = check_ftyp_atom(&mut reader);
         assert!(result.is_ok());
     }

      #[test]
      fn test_check_ftyp_atom_invalid_type() {
          let mut ftyp_header_invalid = [
              0x00, 0x00, 0x00, 0x10, // size
              b'x', b'x', b'x', b'x', // type (invalid)
              b'M', b'4', b'A', b' ', // major_brand
              0x00, 0x00, 0x00, 0x00, // minor_version
          ];
          let mut reader = Cursor::new(&ftyp_header_invalid);
          let result = check_ftyp_atom(&mut reader);
          assert!(result.is_err());
           // Hata türünü kontrol edebiliriz (std::io::ErrorKind)
           assert_eq!(result.unwrap_err().kind(), StdIOErrorKind::InvalidData);
      }

      #[test]
      fn test_check_ftyp_atom_too_short() {
          let mut short_header = [0; 7]; // 8 bayttan az
          let mut reader = Cursor::new(&short_header);
          let result = check_ftyp_atom(&mut reader);
          assert!(result.is_err());
           // Hata türünü kontrol edebiliriz (std::io::ErrorKind)
           assert_eq!(result.unwrap_err().kind(), StdIOErrorKind::UnexpectedEof); // read_exact hata verecektir
      }

     // find_moov_atom testleri (bellek içi Cursor kullanılarak)
     #[test]
     fn test_find_moov_atom_found() {
         let mut reader = Cursor::new(SIMPLE_ALAC_FILE_SIM);
         let result = find_moov_atom(&mut reader);
         assert!(result.is_ok());
         // moov atomunun ofseti ftyp atomundan hemen sonra, ftyp boyutu 24
         assert_eq!(result.unwrap(), 24); // moov atomunun başlangıç ofseti
     }

      #[test]
      fn test_find_moov_atom_not_found() {
          let data_without_moov = &[
              0x00, 0x00, 0x00, 0x10, // size
              b'o', b't', b'h', b'r', // type (other)
              0x01, 0x02, 0x03, 0x04, // data
          ];
          let mut reader = Cursor::new(data_without_moov);
          let result = find_moov_atom(&mut reader);
          assert!(result.is_err());
           // Hata türünü kontrol edebiliriz (std::io::ErrorKind)
           assert_eq!(result.unwrap_err().kind(), StdIOErrorKind::NotFound);
      }

     // read_alac_metadata testleri
     // Bu testler std ortamında gerçek veya simule edilmiş ALAC dosyaları gerektirir.
     // Gerçek dosya kullanmak için include_bytes! veya std::fs gerekir.
     // Simule edilmiş veri kullanmak için karmaşık MP4 yapısını oluşturmak gerekir.

     #[test]
     #[ignore = "Requires a valid ALAC file and std file system access"] // Bu testi varsayılan olarak atla
     fn test_read_alac_metadata_with_file() {
          // Lütfen geçerli bir ALAC (.m4a) dosya yolu sağlayın
         let file_path = "path/to/your/test/file.m4a"; // BURAYI DÜZENLEYİN
         match read_alac_metadata(file_path) {
             Ok(metadata) => {
                 println!("Test dosyası metadata: {:?}", metadata);
                 // Burada beklenen değerlere göre assert yapın
                  assert_eq!(metadata.sample_rate, ...);
                  assert_eq!(metadata.channels, ...);
                  assert_eq!(metadata.bits_per_sample, ...);
             }
             Err(e) => {
                 panic!("read_alac_metadata hata döndürdü: {:?}", e);
             }
         }
     }

    // TODO: no_std implementasyonu için testler yazılmalı.
    // Bu testler Sahne64 ortamında veya bir emülatörde çalıştırılmalıdır.
    // SahneResourceReader struct'ı resource::acquire/read/release/control
    // çağrılarını doğru şekilde simule eden bir altyapı gerektirir.
}

// Tekrarlanan no_std print modülü ve panic handler kaldırıldı.
