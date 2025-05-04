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
// Sahne64 fs modülü (fs::open, fs::read_at, fs::fstat için varsayım - used by SahneResourceReader)
#[cfg(not(feature = "std"))]
use crate::fs;


// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::{File, self};
#[cfg(feature = "std")]
use std::io::{BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, Write as StdWrite};
#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::vec::Vec as StdVec; // std::vec::Vec
#[cfg(feature = "std")]
use std::string::ToString as StdToString; // for format! in std

// alloc crate for String, Vec, format!
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

// core::io traits and types
#[cfg(not(feature = "std"))] // core::io traits are needed in no_std for SahneResourceReader
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io
use core::cmp; // core::cmp
use core::mem::size_of; // core::mem::size_of
use core::convert::TryInto; // core::convert::TryInto

// Need no_std println!/eprintln! macros (if used in error reporting or examples)
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

// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfileblend.rs'den kopyalandı)
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
    // read_exact has a default implementation in core::io::Read
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
    // stream_position has a default implementation in core::io::Seek
}


// BMP Structures
#[derive(Debug)]
pub struct BmpHeader {
    /// BMP Dosya Başlığı (14 bayt) ve DIB Başlığı (Genellikle BITMAPINFOHEADER - 40 bayt) alanları.
    /// Bu alanlar dosyanın başından itibaren okunur.
    /// File Header:
    /// 0-1: Signature (BM)
    /// 2-5: File size (u32)
    /// 6-9: Reserved (u32)
    /// 10-13: Image data offset (u32)
    /// DIB Header (BITMAPINFOHEADER):
    /// 14-17: DIB header size (u32, should be 40 for BITMAPINFOHEADER)
    /// 18-21: Image width (u32)
    /// 22-25: Image height (u32)
    /// 26-27: Color planes (u16, should be 1)
    /// 28-29: Bits per pixel (u16)
    /// ... other DIB fields (compression, image size, etc. - 24 bytes for BITMAPINFOHEADER)
    /// Total header size before data: 14 (File Header) + 40 (DIB Header) = 54 bytes.
    /// Image data offset should typically be >= 54.

    pub file_size: u32, // Dosya boyutu
    pub image_data_offset: u32, // Piksel verisinin başlangıç ofseti
    pub width: u32, // Görüntü genişliği
    pub height: u32, // Görüntü yüksekliği
    pub bits_per_pixel: u16, // Piksel başına bit sayısı
    pub dib_header_size: u32, // DIB başlığının boyutu (genellikle 40)
    // Diğer önemli DIB alanları eklenebilir:
     pub compression: u32,
     pub image_size: u32, // Sıkıştırılmamış görüntü verisinin boyutu (byte cinsinden)
     pub x_resolution: u32,
     pub y_resolution: u32,
     pub colors_used: u32,
     pub colors_important: u32,
}

#[derive(Debug)]
pub struct BmpImage {
    pub header: BmpHeader,
    pub data: Vec<u8>, // Piksel verisi
}

impl BmpImage {
    /// Belirtilen dosya yolundan (veya kaynak ID'sinden) bir BMP görüntüsünü okur.
    /// Başlık bilgilerini ve piksel verisini ayrıştırır.
    ///
    /// # Arguments
    ///
    /// * `filename_or_resource_id` - Dosya yolu (std) veya Sahne64 kaynak ID'si (no_std).
    ///
    /// # Returns
    ///
    /// Başarılı olursa `BmpImage` yapısı veya bir `FileSystemError`.
    #[cfg(feature = "std")]
    pub fn read_from_file<P: AsRef<Path>>(filename: P) -> Result<Self, FileSystemError> { // FileSystemError döner
        let file = File::open(filename.as_ref()).map_err(map_std_io_error_to_fs_error)?;
        let mut reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

        // BMP Dosya Başlığını oku (14 bayt)
        let mut file_header_bytes = [0u8; 14];
        reader.read_exact(&mut file_header_bytes).map_err(map_std_io_error_to_fs_error)?;

        // Sihirli sayıyı kontrol et "BM"
        if &file_header_bytes[0..2] != b"BM" {
             return Err(FileSystemError::InvalidData(format!("Geçersiz BMP sihirli sayısı: {:x?}", &file_header_bytes[0..2]))); // FileSystemError
        }

        let file_size = u32::from_le_bytes(file_header_bytes[2..6].try_into().map_err(|_| FileSystemError::InvalidData(format!("Dosya boyutu baytları geçersiz")))?);
        // Reserved (6-9) - atla
        let image_data_offset = u32::from_le_bytes(file_header_bytes[10..14].try_into().map_err(|_| FileSystemError::InvalidData(format!("Piksel verisi ofset baytları geçersiz")))?);

        // DIB Başlığını oku (genellikle BITMAPINFOHEADER - 40 bayt)
        // DIB başlığının boyutu ilk 4 bayttır. Bu boyutu okuyup buna göre okumalıyız.
        let mut dib_header_size_bytes = [0u8; 4];
        reader.read_exact(&mut dib_header_size_bytes).map_err(map_std_io_error_to_fs_error)?;
        let dib_header_size = u32::from_le_bytes(dib_header_size_bytes.try_into().map_err(|_| FileSystemError::InvalidData(format!("DIB başlık boyutu baytları geçersiz")))?);

        if dib_header_size < 40 { // Minimum BITMAPINFOHEADER boyutu
             return Err(FileSystemError::InvalidData(format!("Geçersiz DIB başlık boyutu: {}", dib_header_size)));
        }

        let mut dib_header_remaining_bytes = vec![0u8; (dib_header_size - 4) as usize]; // Okunması gereken kalan DIB başlığı
        reader.read_exact(&mut dib_header_remaining_bytes).map_err(map_std_io_error_to_fs_error)?;


        // DIB başlığından gerekli alanları ayrıştır
        // BITMAPINFOHEADER (40 bayt) formatına göre ofsetler:
        // 4-7: Width (u32) - offset 18 in original file (14 + 4)
        // 8-11: Height (u32) - offset 22 in original file (14 + 8)
        // 14-15: Bits per pixel (u16) - offset 28 in original file (14 + 14)
        // Index within dib_header_remaining_bytes (after reading the first 4 bytes of DIB size):
        let width = u32::from_le_bytes(dib_header_remaining_bytes[0..4].try_into().map_err(|_| FileSystemError::InvalidData(format!("Genişlik baytları geçersiz")))?);
        let height = u32::from_le_bytes(dib_header_remaining_bytes[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("Yükseklik baytları geçersiz")))?);
        let bits_per_pixel = u16::from_le_bytes(dib_header_remaining_bytes[10..12].try_into().map_err(|_| FileSystemError::InvalidData(format!("Bit derinliği baytları geçersiz")))?);


        let header = BmpHeader {
            file_size,
            image_data_offset,
            width,
            height,
            bits_per_pixel,
            dib_header_size,
            // Diğer DIB alanları burada ayrıştırılabilir ve BmpHeader yapısına eklenebilir.
             compression: u32::from_le_bytes(dib_header_remaining_bytes[12..16].try_into().unwrap()),
             image_size: u32::from_le_bytes(dib_header_remaining_bytes[16..20].try_into().unwrap()),
        };

        // Görüntü verisine atla
        reader.seek(SeekFrom::Start(header.image_data_offset as u64)).map_err(map_std_io_error_to_fs_error)?;

        // Görüntü verisinin boyutunu hesapla
        // File size - image data offset.
        // Ya da sıkıştırılmamış veri için: ((((width * bits_per_pixel) + 31) / 32) * 4) * height
        // Basitçe file_size - image_data_offset kullanalım.
        let image_data_size = file_size.checked_sub(image_data_offset)
             .ok_or_else(|| FileSystemError::InvalidData(format!("Geçersiz görüntü verisi boyutu hesaplaması (file_size {} < data_offset {})", file_size, image_data_offset)))? as usize;


        // Görüntü verisini oku
        let mut image_data = Vec::new(); // Use alloc::vec::Vec
        image_data.resize(image_data_size, 0); // resize the vector to the correct size
        reader.read_exact(&mut image_data).map_err(map_std_io_error_to_fs_error)?; // read_exact will read exactly image_data_size bytes or error

        Ok(BmpImage {
            header,
            data: image_data,
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn read_from_file(filename: &str) -> Result<Self, FileSystemError> { // FileSystemError döner
        // Kaynağı edin
        let handle = resource::acquire(filename, resource::MODE_READ)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        // Dosyanın boyutunu al (SahneResourceReader için gerekli veya seek(End) için)
         let file_stat = fs::fstat(handle)
             .map_err(|e| {
                  let _ = resource::release(handle);
                  map_sahne_error_to_fs_error(e)
              })?;
         let file_size_u64 = file_stat.size as u64;


        // Sahne64 Handle'ı için core::io::Read + Seek implementasyonu sağlayan Reader struct'ı oluştur
        let mut reader = SahneResourceReader::new(handle, file_size_u64); // Pass file_size to reader


        // BMP Dosya Başlığını oku (14 bayt)
        let mut file_header_bytes = [0u8; 14];
        reader.read_exact(&mut file_header_bytes).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError

        // Sihirli sayıyı kontrol et "BM"
        if &file_header_bytes[0..2] != b"BM" {
             // Kaynağı serbest bırakmadan önce hata dön
             let _ = resource::release(handle).map_err(|e| {
                  eprintln!("WARN: Kaynak serbest bırakma hatası: {:?}", e);
                  map_sahne_error_to_fs_error(e)
              });
             return Err(FileSystemError::InvalidData(format!("Geçersiz BMP sihirli sayısı: {:x?}", &file_header_bytes[0..2]))); // FileSystemError
        }

        let file_size = u32::from_le_bytes(file_header_bytes[2..6].try_into().map_err(|_| FileSystemError::InvalidData(format!("Dosya boyutu baytları geçersiz")))?);
        // Reserved (6-9) - atla
        let image_data_offset = u32::from_le_bytes(file_header_bytes[10..14].try_into().map_err(|_| FileSystemError::InvalidData(format!("Piksel verisi ofset baytları geçersiz")))?);

        // DIB Başlığını oku (genellikle BITMAPINFOHEADER - 40 bayt)
        let mut dib_header_size_bytes = [0u8; 4];
        reader.read_exact(&mut dib_header_size_bytes).map_err(map_core_io_error_to_fs_error)?;
        let dib_header_size = u32::from_le_bytes(dib_header_size_bytes.try_into().map_err(|_| FileSystemError::InvalidData(format!("DIB başlık boyutu baytları geçersiz")))?);

        if dib_header_size < 40 { // Minimum BITMAPINFOHEADER boyutu
             let _ = resource::release(handle).map_err(|e| { eprintln!("WARN: Kaynak serbest bırakma hatası: {:?}", e); map_sahne_error_to_fs_error(e) });
             return Err(FileSystemError::InvalidData(format!("Geçersiz DIB başlık boyutu: {}", dib_header_size)));
        }

        // Okunması gereken kalan DIB başlığı
        let mut dib_header_remaining_bytes = vec![0u8; (dib_header_size - 4) as usize]; // Requires alloc
        reader.read_exact(&mut dib_header_remaining_bytes).map_err(map_core_io_error_to_fs_error)?;


        // DIB başlığından gerekli alanları ayrıştır
        let width = u32::from_le_bytes(dib_header_remaining_bytes[0..4].try_into().map_err(|_| FileSystemError::InvalidData(format!("Genişlik baytları geçersiz")))?);
        let height = u32::from_le_bytes(dib_header_remaining_bytes[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("Yükseklik baytları geçersiz")))?);
        let bits_per_pixel = u16::from_le_bytes(dib_header_remaining_bytes[10..12].try_into().map_err(|_| FileSystemError::InvalidData(format!("Bit derinliği baytları geçersiz")))?);

        let header = BmpHeader {
            file_size,
            image_data_offset,
            width,
            height,
            bits_per_pixel,
            dib_header_size,
        };

        // Görüntü verisine atla - SEEK to image_data_offset
        reader.seek(SeekFrom::Start(header.image_data_offset as u64)).map_err(map_core_io_error_to_fs_error)?; // CoreIOError -> FileSystemError

        // Görüntü verisinin boyutunu hesapla (Basitçe file_size - image_data_offset)
        let image_data_size = file_size.checked_sub(image_data_offset)
             .ok_or_else(|| FileSystemError::InvalidData(format!("Geçersiz görüntü verisi boyutu hesaplaması (file_size {} < data_offset {})", file_size, image_data_offset)))? as usize;


        // Görüntü verisini oku
        let mut image_data = Vec::new(); // Use alloc::vec::Vec
        image_data.resize(image_data_size, 0); // Resize vector
        reader.read_exact(&mut image_data).map_err(map_core_io_error_to_fs_error)?; // read_exact will read exactly image_data_size bytes or error

        // Kaynağı serbest bırak
        let _ = resource::release(handle).map_err(|e| {
             eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e);
             map_sahne_error_to_fs_error(e)
         });


        Ok(BmpImage {
            header,
            data: image_data,
        })
    }


    /// Bir BMP görüntüsünü belirtilen dosya yoluna (veya kaynak ID'sine) yazar.
    ///
    /// # Arguments
    ///
    /// * `filename_or_resource_id` - Dosya yolu (std) veya Sahne64 kaynak ID'si (no_std).
    ///
    /// # Returns
    ///
    /// Başarılı olursa Ok(()) veya bir `FileSystemError`.
    #[cfg(feature = "std")]
    pub fn write_to_file<P: AsRef<Path>>(&self, filename: P) -> Result<(), FileSystemError> { // FileSystemError döner
        let mut file = File::create(filename.as_ref()).map_err(map_std_io_error_to_fs_error)?; // Create/Overwrite file

        // BMP File Header (14 bytes)
        file.write_all(b"BM").map_err(map_std_io_error_to_fs_error)?;
        file.write_all(&self.header.file_size.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;
        file.write_all(&[0u8; 4]).map_err(map_std_io_error_to_fs_error)?; // Reserved
        file.write_all(&self.header.image_data_offset.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;

        // DIB Header (40 bytes for BITMAPINFOHEADER)
        file.write_all(&self.header.dib_header_size.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?; // DIB header size
        file.write_all(&self.header.width.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;
        file.write_all(&self.header.height.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;
        file.write_all(&[1, 0]).map_err(map_std_io_error_to_fs_error)?; // Color planes
        file.write_all(&self.header.bits_per_pixel.to_le_bytes()).map_err(map_std_io_error_to_fs_error)?;

        // Write remaining DIB header fields (assuming BITMAPINFOHEADER size 40)
        // This is writing dummy zeros based on the original code.
        // A more robust implementation would store/write all DIB header fields.
        // If dib_header_size > 40, we'd need to write more bytes here.
        // Let's calculate bytes written so far in DIB header: 4 (size) + 4 (width) + 4 (height) + 2 (planes) + 2 (bpp) = 16 bytes.
        // Remaining bytes for BITMAPINFOHEADER (40) = 40 - 16 = 24 bytes.
        let bytes_written_in_dib_header_so_far = 4 + 4 + 4 + 2 + 2;
        let remaining_dib_bytes = self.header.dib_header_size.checked_sub(bytes_written_in_dib_header_so_far as u32)
             .ok_or_else(|| FileSystemError::InvalidData(format!("Geçersiz DIB başlık boyutu veya yazma hatası")))? as usize;
         file.write_all(&vec![0u8; remaining_dib_bytes]).map_err(map_std_io_error_to_fs_error)?; // Write dummy bytes


        // Pad between DIB header and image data if image_data_offset > (14 + dib_header_size)
        let header_end_offset = 14u32.checked_add(self.header.dib_header_size)
             .ok_or_else(|| FileSystemError::InvalidData(format!("Başlık sonu ofseti hesaplanırken taşma")))?;

        if self.header.image_data_offset > header_end_offset {
             let padding_size = self.header.image_data_offset.checked_sub(header_end_offset)
                  .ok_or_else(|| FileSystemError::InvalidData(format!("Padding boyutu hesaplanırken taşma")))? as usize;
             file.write_all(&vec![0u8; padding_size]).map_err(map_std_io_error_to_fs_error)?; // Write padding
        } else if self.header.image_data_offset < header_end_offset {
             // Image data offset should not be less than header end offset
              return Err(FileSystemError::InvalidData(format!("Görüntü verisi ofseti ({}) başlık sonundan ({}) küçük.", self.header.image_data_offset, header_end_offset)));
        }


        // Image Data
        file.write_all(&self.data).map_err(map_std_io_error_to_fs_error)?;

        Ok(())
    }

    #[cfg(not(feature = "std"))]
    pub fn write_to_file(&self, filename: &str) -> Result<(), FileSystemError> { // FileSystemError döner
        // Kaynağı yazma modunda edin (O_CREAT | O_WRONLY Sahne64 karşılığı varsayım)
        let handle = resource::acquire(filename, resource::MODE_WRITE | resource::FLAG_CREATE)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        // BMP File Header (14 bytes)
        // resource::write(handle, data) Result<usize, SahneError> döner (varsayım)
        resource::write(handle, b"BM")
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        resource::write(handle, &self.header.file_size.to_le_bytes())
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        resource::write(handle, &[0u8; 4]) // Reserved
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        resource::write(handle, &self.header.image_data_offset.to_le_bytes())
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;

        // DIB Header (40 bytes for BITMAPINFOHEADER)
        resource::write(handle, &self.header.dib_header_size.to_le_bytes())
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?; // DIB header size
        resource::write(handle, &self.header.width.to_le_bytes())
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        resource::write(handle, &self.header.height.to_le_bytes())
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        resource::write(handle, &[1, 0]) // Color planes
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        resource::write(handle, &self.header.bits_per_pixel.to_le_bytes())
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;

        // Write remaining DIB header fields (assuming BITMAPINFOHEADER size 40)
        let bytes_written_in_dib_header_so_far = 4 + 4 + 4 + 2 + 2;
        let remaining_dib_bytes = self.header.dib_header_size.checked_sub(bytes_written_in_dib_header_so_far as u32)
             .ok_or_else(|| FileSystemError::InvalidData(format!("Geçersiz DIB başlık boyutu veya yazma hatası")))? as usize;
         resource::write(handle, &vec![0u8; remaining_dib_bytes]) // Requires alloc
              .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;


        // Pad between DIB header and image data if image_data_offset > (14 + dib_header_size)
        let header_end_offset = 14u32.checked_add(self.header.dib_header_size)
             .ok_or_else(|| FileSystemError::InvalidData(format!("Başlık sonu ofseti hesaplanırken taşma")))?;

        if self.header.image_data_offset > header_end_offset {
             let padding_size = self.header.image_data_offset.checked_sub(header_end_offset)
                  .ok_or_else(|| FileSystemError::InvalidData(format!("Padding boyutu hesaplanırken taşma")))? as usize;
             resource::write(handle, &vec![0u8; padding_size]) // Requires alloc
                  .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;
        } else if self.header.image_data_offset < header_end_offset {
              let _ = resource::release(handle).map_err(|e| { eprintln!("WARN: Kaynak serbest bırakma hatası: {:?}", e); map_sahne_error_to_fs_error(e) });
              return Err(FileSystemError::InvalidData(format!("Görüntü verisi ofseti ({}) başlık sonundan ({}) küçük.", self.header.image_data_offset, header_end_offset)));
        }


        // Image Data
        resource::write(handle, &self.data)
             .map_err(|e| { let _ = resource::release(handle); map_sahne_error_to_fs_error(e) })?;


        // Kaynağı serbest bırak
        resource::release(handle)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        Ok(())
    }


    // pub fn write_to_file_optimized is removed as it's identical
}

// Example main function (no_std)
#[cfg(feature = "example_bmp")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("BMP example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // Test with a hypothetical file/resource ID
     let filename_or_resource_id = "sahne://files/test_image.bmp";

     // Basit bir dummy BMP görüntüsü oluşturma
     let dummy_width = 4;
     let dummy_height = 2;
     let dummy_bpp = 24; // 24 bpp (BGR)
     let row_size_bytes = ((dummy_width * dummy_bpp + 31) / 32) * 4; // Her satır 4 baytın katı olmalı
     let dummy_data_size = row_size_bytes * dummy_height;
     let dummy_data: Vec<u8> = vec![0x00, 0x00, 0xFF; dummy_data_size / 3]; // Kırmızı pikseller (BGR)
     let file_header_size = 14;
     let dib_header_size = 40; // BITMAPINFOHEADER
     let image_data_offset = file_header_size + dib_header_size; // 54

     let header = BmpHeader {
          file_size: (image_data_offset as u32).checked_add(dummy_data_size as u32)
               .ok_or_else(|| FileSystemError::InvalidData(format!("Dosya boyutu hesaplanırken taşma"))).unwrap(), // Example, handle error properly
          image_data_offset: image_data_offset as u32,
          width: dummy_width,
          height: dummy_height,
          bits_per_pixel: dummy_bpp as u16,
          dib_header_size: dib_header_size as u32,
     };

     let bmp_image_to_write = BmpImage {
         header,
         data: dummy_data, // Requires alloc
     };

     // Dosyaya yazma
     match bmp_image_to_write.write_to_file(filename_or_resource_id) {
         Ok(_) => println!("BMP dosyası başarıyla yazıldı: {}", filename_or_resource_id),
         Err(e) => eprintln!("BMP dosyası yazılırken hata oluştu: {}", e),
     }

     // Dosyadan okuma
     match BmpImage::read_from_file(filename_or_resource_id) {
         Ok(loaded_bmp_image) => {
             println!("BMP dosyası başarıyla okundu.");
             println!("Başlık: {:?}", loaded_bmp_image.header);
             println!("Görüntü verisi boyutu: {} bayt", loaded_bmp_image.data.len());
             // Optionally check some pixel data
              if loaded_bmp_image.data.len() >= 3 {
                  println!("İlk piksel (BGR): {:?}", &loaded_bmp_image.data[0..3]);
              }
         }
         Err(e) => eprintln!("BMP dosyası okunurken hata oluştu: {}", e),
     }

     // TODO: Sahne64 ortamında dosya silme API'sını kullan


     eprintln!("BMP example (no_std) finished.");

     Ok(())
}

// Example main function (std)
#[cfg(feature = "example_bmp")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("BMP example (std) starting...");

     // Test with a hypothetical file path
     let filename = Path::new("test_image_std.bmp");

     // Basit bir dummy BMP görüntüsü oluşturma (std version)
     let dummy_width = 4;
     let dummy_height = 2;
     let dummy_bpp = 24; // 24 bpp (BGR)
     let row_size_bytes = ((dummy_width * dummy_bpp + 31) / 32) * 4;
     let dummy_data_size = row_size_bytes * dummy_height;
     let dummy_data: StdVec<u8> = vec![0x00, 0x00, 0xFF; dummy_data_size / 3]; // Kırmızı pikseller (BGR)
     let file_header_size = 14;
     let dib_header_size = 40; // BITMAPINFOHEADER
     let image_data_offset = file_header_size + dib_header_size; // 54

     let header = BmpHeader {
          file_size: (image_data_offset as u32).checked_add(dummy_data_size as u32)
               .ok_or_else(|| FileSystemError::InvalidData(format!("Dosya boyutu hesaplanırken taşma"))).unwrap(), // Example, handle error properly
          image_data_offset: image_data_offset as u32,
          width: dummy_width,
          height: dummy_height,
          bits_per_pixel: dummy_bpp as u16,
          dib_header_size: dib_header_size as u32,
     };

     let bmp_image_to_write = BmpImage {
         header,
         data: dummy_data, // Use std::vec::Vec
     };


     // Dosyaya yazma
     match bmp_image_to_write.write_to_file(filename) {
         Ok(_) => println!("BMP dosyası başarıyla yazıldı: {}", filename.display()),
         Err(e) => eprintln!("BMP dosyası yazılırken hata oluştu: {}", e),
     }

     // Dosyadan okuma
     match BmpImage::read_from_file(filename) {
         Ok(loaded_bmp_image) => {
             println!("BMP dosyası başarıyla okundu.");
             println!("Başlık: {:?}", loaded_bmp_image.header);
             println!("Görüntü verisi boyutu: {} bayt", loaded_bmp_image.data.len());
             // Optionally check some pixel data
              if loaded_bmp_image.data.len() >= 3 {
                  println!("İlk piksel (BGR): {:?}", &loaded_bmp_image.data[0..3]);
              }
         }
         Err(e) => eprintln!("BMP dosyası okunurken hata oluştu: {}", e),
     }


     // Dosyayı temizle
     if let Err(e) = fs::remove_file(filename) {
          eprintln!("Test dosyası silinirken hata oluştu: {}", e);
     }


     eprintln!("BMP example (std) finished.");

     Ok(())
}


// Test module (primarily for std implementation using mocks or sim)
#[cfg(test)]
#[cfg(feature = "std")] // Only compile tests for std
mod tests {
    use super::*;
    use std::io::Cursor; // In-memory reader/seeker
    use std::io::{Write, Read, Seek, SeekFrom}; // For Cursor traits
    use std::error::Error as StdError; // For error handling
    use alloc::vec; // vec! macro
    use alloc::string::ToString as AllocToString; // to_string() for error messages in alloc

     // Helper to create a basic BMP file header + DIB header + data in memory
    fn create_test_bmp_file(width: u32, height: u32, bits_per_pixel: u16, image_data_offset: u32, data_content: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::new();

        let file_header_size = 14u32;
        let dib_header_size = 40u32; // Assuming BITMAPINFOHEADER

        let calculated_image_data_size = ((width * bits_per_pixel as u32 + 31) / 32) * 4 * height;
         let calculated_file_size = image_data_offset.checked_add(calculated_image_data_size)
             .ok_or_else(|| format!("Calculated file size overflow")).unwrap(); // Simple error handling in helper

        // BMP File Header (14 bytes)
        buffer.extend_from_slice(b"BM");
        buffer.extend_from_slice(&calculated_file_size.to_le_bytes()); // File size
        buffer.extend_from_slice(&[0u8; 4]); // Reserved
        buffer.extend_from_slice(&image_data_offset.to_le_bytes()); // Image data offset

        // DIB Header (BITMAPINFOHEADER - 40 bytes)
        buffer.extend_from_slice(&dib_header_size.to_le_bytes()); // DIB header size (40)
        buffer.extend_from_slice(&width.to_le_bytes()); // Width
        buffer.extend_from_slice(&height.to_le_bytes()); // Height
        buffer.extend_from_slice(&[1, 0]); // Color planes (1)
        buffer.extend_from_slice(&bits_per_pixel.to_le_bytes()); // Bits per pixel
        buffer.extend_from_slice(&[0u8; 24]); // Remaining BITMAPINFOHEADER fields (compression=0, etc.)

        // Padding between DIB header and image data if image_data_offset > (14 + 40)
        let header_end_offset = file_header_size.checked_add(dib_header_size)
             .ok_or_else(|| format!("Header end offset overflow")).unwrap();

        if image_data_offset > header_end_offset {
            let padding_size = image_data_offset.checked_sub(header_end_offset)
                 .ok_or_else(|| format!("Padding size overflow")).unwrap() as usize;
            buffer.extend(vec![0u8; padding_size]);
        } else if image_data_offset < header_end_offset {
             panic!("Invalid image_data_offset ({}) is less than header end ({})", image_data_offset, header_end_offset);
        }


        // Image Data
        buffer.extend_from_slice(data_content);

        buffer
    }

     // Helper function to read BMP from a generic reader (similar to internal logic)
     fn read_bmp_from_reader<R: Read + Seek>(reader: &mut R) -> Result<BmpImage, FileSystemError> {
          // BMP Dosya Başlığını oku (14 bayt)
          let mut file_header_bytes = [0u8; 14];
          reader.read_exact(&mut file_header_bytes).map_err(|e| FileSystemError::IOError(format!("File header read error: {:?}", e)))?;

          if &file_header_bytes[0..2] != b"BM" {
               return Err(FileSystemError::InvalidData(format!("Geçersiz BMP sihirli sayısı: {:x?}", &file_header_bytes[0..2])));
          }

          let file_size = u32::from_le_bytes(file_header_bytes[2..6].try_into().map_err(|_| FileSystemError::InvalidData(format!("Dosya boyutu baytları geçersiz")))?);
          let image_data_offset = u32::from_le_bytes(file_header_bytes[10..14].try_into().map_err(|_| FileSystemError::InvalidData(format!("Piksel verisi ofset baytları geçersiz")))?);

          // DIB Başlığını oku (ilk 4 bayt boyutu verir)
          let mut dib_header_size_bytes = [0u8; 4];
          reader.read_exact(&mut dib_header_size_bytes).map_err(|e| FileSystemError::IOError(format!("DIB header size read error: {:?}", e)))?;
          let dib_header_size = u32::from_le_bytes(dib_header_size_bytes.try_into().map_err(|_| FileSystemError::InvalidData(format!("DIB başlık boyutu baytları geçersiz")))?);

          if dib_header_size < 40 {
               return Err(FileSystemError::InvalidData(format!("Geçersiz DIB başlık boyutu: {}", dib_header_size)));
          }

          // Okunması gereken kalan DIB başlığı
          let mut dib_header_remaining_bytes = vec![0u8; (dib_header_size - 4) as usize];
          reader.read_exact(&mut dib_header_remaining_bytes).map_err(|e| FileSystemError::IOError(format!("DIB remaining header read error: {:?}", e)))?;

          // DIB başlığından gerekli alanları ayrıştır
          let width = u32::from_le_bytes(dib_header_remaining_bytes[0..4].try_into().map_err(|_| FileSystemError::InvalidData(format!("Genişlik baytları geçersiz")))?);
          let height = u32::from_le_bytes(dib_header_remaining_bytes[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("Yükseklik baytları geçersiz")))?);
          let bits_per_pixel = u16::from_le_bytes(dib_header_remaining_bytes[10..12].try_into().map_err(|_| FileSystemError::InvalidData(format!("Bit derinliği baytları geçersiz")))?);


          let header = BmpHeader {
              file_size,
              image_data_offset,
              width,
              height,
              bits_per_pixel,
              dib_header_size,
          };

          // Görüntü verisine atla
          reader.seek(SeekFrom::Start(header.image_data_offset as u64)).map_err(|e| FileSystemError::IOError(format!("Seek to image data error: {:?}", e)))?;

          // Görüntü verisinin boyutunu hesapla (Basitçe file_size - image_data_offset)
          let image_data_size = file_size.checked_sub(image_data_offset)
               .ok_or_else(|| FileSystemError::InvalidData(format!("Geçersiz görüntü verisi boyutu hesaplaması (file_size {} < data_offset {})", file_size, image_data_offset)))? as usize;

          // Görüntü verisini oku
          let mut image_data = Vec::new();
          image_data.resize(image_data_size, 0);
          reader.read_exact(&mut image_data).map_err(|e| FileSystemError::IOError(format!("Image data read error: {:?}", e)))?;

          Ok(BmpImage {
              header,
              data: image_data,
          })
     }

      // Helper function to write BMP to a generic writer (similar to internal logic)
      fn write_bmp_to_writer<W: Write + Seek>(writer: &mut W, bmp_image: &BmpImage) -> Result<(), FileSystemError> {
           // BMP File Header (14 bytes)
           writer.write_all(b"BM").map_err(|e| FileSystemError::IOError(format!("Magic write error: {:?}", e)))?;
           writer.write_all(&bmp_image.header.file_size.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("File size write error: {:?}", e)))?;
           writer.write_all(&[0u8; 4]).map_err(|e| FileSystemError::IOError(format!("Reserved write error: {:?}", e)))?; // Reserved
           writer.write_all(&bmp_image.header.image_data_offset.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Offset write error: {:?}", e)))?;

           // DIB Header (40 bytes for BITMAPINFOHEADER)
           writer.write_all(&bmp_image.header.dib_header_size.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("DIB size write error: {:?}", e)))?; // DIB header size
           writer.write_all(&bmp_image.header.width.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Width write error: {:?}", e)))?;
           writer.write_all(&bmp_image.header.height.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Height write error: {:?}", e)))?;
           writer.write_all(&[1, 0]).map_err(|e| FileSystemError::IOError(format!("Planes write error: {:?}", e)))?; // Color planes
           writer.write_all(&bmp_image.header.bits_per_pixel.to_le_bytes()).map_err(|e| FileSystemError::IOError(format!("Bpp write error: {:?}", e)))?;

           // Write remaining DIB header fields (assuming BITMAPINFOHEADER size 40)
           let bytes_written_in_dib_header_so_far = 4 + 4 + 4 + 2 + 2;
           let remaining_dib_bytes = bmp_image.header.dib_header_size.checked_sub(bytes_written_in_dib_header_so_far as u32)
                .ok_or_else(|| FileSystemError::InvalidData(format!("Geçersiz DIB başlık boyutu veya yazma hatası")))? as usize;
            writer.write_all(&vec![0u8; remaining_dib_bytes]).map_err(|e| FileSystemError::IOError(format!("Remaining DIB write error: {:?}", e)))?; // Write dummy bytes


           // Pad between DIB header and image data if image_data_offset > (14 + dib_header_size)
           let header_end_offset = 14u32.checked_add(bmp_image.header.dib_header_size)
                .ok_or_else(|| FileSystemError::InvalidData(format!("Başlık sonu ofseti hesaplanırken taşma")))?;

           if bmp_image.header.image_data_offset > header_end_offset {
                let padding_size = bmp_image.header.image_data_offset.checked_sub(header_end_offset)
                     .ok_or_else(|| FileSystemError::InvalidData(format!("Padding boyutu hesaplanırken taşma")))? as usize;
                writer.write_all(&vec![0u8; padding_size]).map_err(|e| FileSystemError::IOError(format!("Padding write error: {:?}", e)))?;
           } else if bmp_image.header.image_data_offset < header_end_offset {
                 return Err(FileSystemError::InvalidData(format!("Görüntü verisi ofseti ({}) başlık sonundan ({}) küçük.", bmp_image.header.image_data_offset, header_end_offset)));
           }


           // Image Data
           writer.write_all(&bmp_image.data).map_err(|e| FileSystemError::IOError(format!("Image data write error: {:?}", e)))?;

           Ok(())
      }


    #[test]
    fn test_read_write_bmp_in_memory() -> Result<(), FileSystemError> { // Return FileSystemError
         let width = 4;
         let height = 2;
         let bpp = 24;
         let row_size_bytes = ((width * bpp as u32 + 31) / 32) * 4;
         let data_size = row_size_bytes * height;
         let dummy_data: Vec<u8> = vec![0x00, 0xFF, 0x00; data_size / 3]; // Green pixels
         let file_header_size = 14;
         let dib_header_size = 40; // BITMAPINFOHEADER
         let image_data_offset = file_header_size + dib_header_size; // 54

         let header = BmpHeader {
              file_size: (image_data_offset as u32).checked_add(data_size as u32)
                   .ok_or_else(|| FileSystemError::InvalidData(format!("Dosya boyutu hesaplanırken taşma"))).unwrap(),
              image_data_offset: image_data_offset as u32,
              width,
              height,
              bits_per_pixel: bpp,
              dib_header_size: dib_header_size as u32,
         };

         let bmp_image_to_write = BmpImage {
             header,
             data: dummy_data,
         };

         // Write to an in-memory Cursor
         let mut cursor = Cursor::new(Vec::new());
         write_bmp_to_writer(&mut cursor, &bmp_image_to_write)?;

         // Get the written data and read it back with a new Cursor
         let written_data = cursor.into_inner();
         let mut read_cursor = Cursor::new(written_data.clone());

         // Read the BMP image from the Cursor
         let loaded_bmp_image = read_bmp_from_reader(&mut read_cursor)?;

         // Assert the loaded image matches the original
         assert_eq!(loaded_bmp_image.header.file_size, bmp_image_to_write.header.file_size);
         assert_eq!(loaded_bmp_image.header.image_data_offset, bmp_image_to_write.header.image_data_offset);
         assert_eq!(loaded_bmp_image.header.width, bmp_image_to_write.header.width);
         assert_eq!(loaded_bmp_image.header.height, bmp_image_to_write.header.height);
         assert_eq!(loaded_bmp_image.header.bits_per_pixel, bmp_image_to_write.header.bits_per_pixel);
         assert_eq!(loaded_bmp_image.header.dib_header_size, bmp_image_to_write.header.dib_header_size);
         assert_eq!(loaded_bmp_image.data, bmp_image_to_write.data);


         // Test with padding
         let width_padded = 2;
         let height_padded = 1;
         let bpp_padded = 24;
         let row_size_bytes_padded = ((width_padded * bpp_padded as u32 + 31) / 32) * 4; // (2*24+31)/32 * 4 = (48+31)/32 * 4 = 79/32 * 4 = 2 * 4 = 8. Row size is 8 bytes.
         let data_size_padded = row_size_bytes_padded * height_padded; // 8 * 1 = 8 bytes
         let dummy_data_padded: Vec<u8> = vec![0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]; // Blue pixel + padding
          let image_data_offset_padded = 70; // Padding needed: 70 - 54 = 16 bytes.

          let header_padded = BmpHeader {
               file_size: (image_data_offset_padded as u32).checked_add(data_size_padded as u32)
                    .ok_or_else(|| FileSystemError::InvalidData(format!("Dosya boyutu hesaplanırken taşma"))).unwrap(),
               image_data_offset: image_data_offset_padded as u32,
               width: width_padded,
               height: height_padded,
               bits_per_pixel: bpp_padded,
               dib_header_size: dib_header_size as u32, // Still 40 for BITMAPINFOHEADER
          };

         let bmp_image_to_write_padded = BmpImage {
             header: header_padded,
             data: dummy_data_padded,
         };

          let mut cursor_padded = Cursor::new(Vec::new());
          write_bmp_to_writer(&mut cursor_padded, &bmp_image_to_write_padded)?;

          let written_data_padded = cursor_padded.into_inner();
          let mut read_cursor_padded = Cursor::new(written_data_padded.clone());

          let loaded_bmp_image_padded = read_bmp_from_reader(&mut read_cursor_padded)?;

          assert_eq!(loaded_bmp_image_padded.header.file_size, bmp_image_to_write_padded.header.file_size);
          assert_eq!(loaded_bmp_image_padded.header.image_data_offset, bmp_image_to_write_padded.header.image_data_offset);
          assert_eq!(loaded_bmp_image_padded.header.width, bmp_image_to_write_padded.header.width);
          assert_eq!(loaded_bmp_image_padded.header.height, bmp_image_to_write_padded.header.height);
          assert_eq!(loaded_bmp_image_padded.header.bits_per_pixel, bmp_image_to_write_padded.header.bits_per_pixel);
          assert_eq!(loaded_bmp_image_padded.header.dib_header_size, bmp_image_to_write_padded.header.dib_header_size);
          assert_eq!(loaded_bmp_image_padded.data, bmp_image_to_write_padded.data);

           // Verify the padding bytes in the written data
           let expected_padding_size = (image_data_offset_padded - (file_header_size + dib_header_size)) as usize;
           let padding_start_offset = (file_header_size + dib_header_size) as usize;
           let written_padding = &written_data_padded[padding_start_offset .. padding_start_offset + expected_padding_size];
           assert!(written_padding.iter().all(|&b| b == 0), "Padding bytes should be zero");


    }


     // TODO: Add tests for error conditions (invalid magic, truncated file, invalid header values)
     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
}

// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_bmp", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
