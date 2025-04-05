#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Gerekli Sahne64 modüllerini içeri aktar
#[cfg(not(feature = "std"))]
use crate::{
    fs,
    memory,
    process,
    sync,
    kernel,
    SahneError,
    arch,
};

#[cfg(not(feature = "std"))]
use core::result::Result;

#[cfg(not(feature = "std"))]
use core::fmt;

#[cfg(not(feature = "std"))]
use core::mem::size_of;

#[cfg(not(feature = "std"))]
use core::convert::TryInto;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom};
#[cfg(feature = "std")]
use byteorder::{BigEndian as StdBigEndian, ReadBytesExt as StdReadBytesExt};
#[cfg(feature = "std")]
use std::error::Error as StdError;
#[cfg(feature = "std")]
use std::fmt as StdFmt;

// Özel hata türü tanımla, bu hem daha açıklayıcı hatalar sağlar hem de hata işlemeyi kolaylaştırır.
#[derive(Debug)]
pub enum AiffError {
    IoError(SahneError),
    InvalidData(String),
}

impl fmt::Display for AiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiffError::IoError(e) => write!(f, "Dosya okuma hatası: {}", e),
            AiffError::InvalidData(msg) => write!(f, "Geçersiz AIFF verisi: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl StdError for AiffError {}

#[cfg(not(feature = "std"))]
impl From<SahneError> for AiffError {
    fn from(err: SahneError) -> Self {
        AiffError::IoError(err)
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for AiffError {
    fn from(err: std::io::Error) -> Self {
        AiffError::IoError(err.into())
    }
}

#[cfg(not(feature = "std"))]
pub struct BigEndian;

#[cfg(not(feature = "std"))]
pub trait ReadBytesExt {
    fn read_u16<T: ByteOrder>(&mut self) -> Result<u16, AiffError>;
    fn read_u32<T: ByteOrder>(&mut self) -> Result<u32, AiffError>;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), AiffError>;
    fn read_uint<T: ByteOrder>(&mut self, size: usize) -> Result<u64, AiffError>;
    fn read_u8(&mut self) -> Result<u8, AiffError>;
}

#[cfg(not(feature = "std"))]
pub trait ByteOrder {}

#[cfg(not(feature = "std"))]
impl ByteOrder for BigEndian {}

#[cfg(not(feature = "std"))]
impl<R: Read> ReadBytesExt for R {
    fn read_u16<T: ByteOrder>(&mut self) -> Result<u16, AiffError> {
        let mut buf = [0; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn read_u32<T: ByteOrder>(&mut self) -> Result<u32, AiffError> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), AiffError> {
        let read = self.read(buf)?;
        if read == buf.len() {
            Ok(())
        } else {
            Err(AiffError::IoError(SahneError::UnexpectedEof))
        }
    }

    fn read_uint<T: ByteOrder>(&mut self, size: usize) -> Result<u64, AiffError> {
        let mut buf = [0; 8];
        if size > 8 {
            return Err(AiffError::InvalidData("Boyut 8'den büyük".into()));
        }
        self.read_exact(&mut buf[..size])?;
        let mut value: u64 = 0;
        for i in 0..size {
            value |= (buf[i] as u64) << ((size - 1 - i) * 8);
        }
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8, AiffError> {
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

#[cfg(not(feature = "std"))]
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SahneError>;
}

#[cfg(not(feature = "std"))]
pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError>;
}

#[cfg(not(feature = "std"))]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

pub struct AiffMetadata {
    pub num_channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub num_frames: u32,
}

#[cfg(feature = "std")]
pub fn read_aiff_metadata(file_path: &str) -> Result<AiffMetadata, AiffError> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    read_metadata_from_reader(&mut reader)
}

#[cfg(not(feature = "std"))]
pub fn read_aiff_metadata(file_path: &str) -> Result<AiffMetadata, AiffError> {
    let fd = fs::open(file_path, fs::O_RDONLY)?;
    let mut reader = FileReader { fd, position: 0 };
    read_metadata_from_reader(&mut reader)
}

#[cfg(feature = "std")]
fn read_metadata_from_reader<R: StdRead + StdSeek>(reader: &mut R) -> Result<AiffMetadata, AiffError> {
    // AIFF dosya başlığını (FORM chunk) kontrol et. Hata mesajlarını iyileştir.
    let mut form_chunk_id = [0; 4];
    reader.read_exact(&mut form_chunk_id)?;
    if &form_chunk_id != b"FORM" {
        return Err(AiffError::InvalidData(
            "Geçersiz AIFF dosyası: FORM chunk ID bulunamadı veya hatalı.".to_string(),
        ));
    }

    let _form_chunk_size = reader.read_u32::<StdBigEndian>()?; // Chunk boyutu şu an kullanılmıyor, gerekirse kullanılabilir

    let mut aiff_type = [0; 4];
    reader.read_exact(&mut aiff_type)?;
    if &aiff_type != b"AIFF" {
        return Err(AiffError::InvalidData(
            "Geçersiz AIFF dosyası: AIFF türü bulunamadı veya hatalı.".to_string(),
        ));
    }

    // Common chunk'u bul ve meta verileri oku
    loop {
        let mut chunk_id = [0; 4];
        let bytes_read = reader.read_exact(&mut chunk_id);

        match bytes_read {
            Ok(_) => {
                let chunk_size = reader.read_u32::<StdBigEndian>()?;

                if &chunk_id == b"COMM" {
                    let num_channels = reader.read_u16::<StdBigEndian>()?;
                    let num_frames = reader.read_u32::<StdBigEndian>()?;
                    let bits_per_sample = reader.read_u16::<StdBigEndian>()?;

                    // Sample rate için genişletilmiş formatı oku
                    let sample_rate_extended = reader.read_u8()?;
                    let sample_rate_mantissa = reader.read_uint::<StdBigEndian>(10)?;

                    // Sample rate'i genişletilmiş formattan çöz
                    let sample_rate = (sample_rate_mantissa as f64 * 2f64.powi(sample_rate_extended as i32 - 16383)) as u32;

                    return Ok(AiffMetadata {
                        num_channels,
                        sample_rate,
                        bits_per_sample,
                        num_frames,
                    });
                } else {
                    // Common chunk değilse, sonraki chunk'a atla. Hata durumunda çıkışı kontrol et.
                    reader.seek(StdSeekFrom::Current(chunk_size as i64))?;
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    // Dosya sonuna gelindi ve COMM chunk bulunamadı
                    return Err(AiffError::InvalidData("COMM chunk bulunamadı: Dosya sonuna ulaşıldı.".to_string()));
                } else {
                    // Diğer IO hataları
                    return Err(e.into()); // std::io::Error -> AiffError dönüşümü
                }
            }
        }
    }
}

#[cfg(not(feature = "std"))]
fn read_metadata_from_reader<R: Read + Seek>(reader: &mut R) -> Result<AiffMetadata, AiffError> {
    // AIFF dosya başlığını (FORM chunk) kontrol et. Hata mesajlarını iyileştir.
    let mut form_chunk_id = [0; 4];
    reader.read_exact(&mut form_chunk_id)?;
    if &form_chunk_id != b"FORM" {
        return Err(AiffError::InvalidData(
            "Geçersiz AIFF dosyası: FORM chunk ID bulunamadı veya hatalı.".to_string(),
        ));
    }

    let _form_chunk_size = reader.read_u32::<BigEndian>()?; // Chunk boyutu şu an kullanılmıyor, gerekirse kullanılabilir

    let mut aiff_type = [0; 4];
    reader.read_exact(&mut aiff_type)?;
    if &aiff_type != b"AIFF" {
        return Err(AiffError::InvalidData(
            "Geçersiz AIFF dosyası: AIFF türü bulunamadı veya hatalı.".to_string(),
        ));
    }

    // Common chunk'u bul ve meta verileri oku
    loop {
        let mut chunk_id = [0; 4];
        let bytes_read = reader.read_exact(&mut chunk_id);

        match bytes_read {
            Ok(_) => {
                let chunk_size = reader.read_u32::<BigEndian>()?;

                if &chunk_id == b"COMM" {
                    let num_channels = reader.read_u16::<BigEndian>()?;
                    let num_frames = reader.read_u32::<BigEndian>()?;
                    let bits_per_sample = reader.read_u16::<BigEndian>()?;

                    // Sample rate için genişletilmiş formatı oku
                    let sample_rate_extended = reader.read_u8()?;
                    let sample_rate_mantissa = reader.read_uint::<BigEndian>(10)?;

                    // Sample rate'i genişletilmiş formattan çöz
                    let sample_rate = (sample_rate_mantissa as f64 * 2f64.powi(sample_rate_extended as i32 - 16383)) as u32;

                    return Ok(AiffMetadata {
                        num_channels,
                        sample_rate,
                        bits_per_sample,
                        num_frames,
                    });
                } else {
                    // Common chunk değilse, sonraki chunk'a atla. Hata durumunda çıkışı kontrol et.
                    reader.seek(SeekFrom::Current(chunk_size as i64))?;
                }
            }
            Err(e) => {
                if let AiffError::IoError(io_err) = e {
                    if io_err == SahneError::UnexpectedEof {
                        // Dosya sonuna gelindi ve COMM chunk bulunamadı
                        return Err(AiffError::InvalidData("COMM chunk bulunamadı: Dosya sonuna ulaşıldı.".to_string()));
                    } else {
                        // Diğer IO hataları
                        return Err(AiffError::IoError(io_err));
                    }
                } else {
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(not(feature = "std"))]
pub struct FileReader {
    fd: u64,
    position: u64,
}

#[cfg(not(feature = "std"))]
impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SahneError> {
        let bytes_read = fs::read_at(self.fd, self.position, buf)?;
        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

#[cfg(not(feature = "std"))]
impl Seek for FileReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError> {
        match pos {
            SeekFrom::Start(offset) => {
                self.position = offset;
                Ok(self.position)
            }
            SeekFrom::End(offset) => {
                let file_size = fs::fstat(self.fd)?.size;
                self.position = (file_size as i64 + offset) as u64;
                Ok(self.position)
            }
            SeekFrom::Current(offset) => {
                self.position = (self.position as i64 + offset) as u64;
                Ok(self.position)
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use std::io::Cursor;
    use byteorder::{WriteBytesExt, BigEndian as StdBigEndian};
    use std::fs;

    // Yardımcı fonksiyon: Test için basit bir AIFF dosyası oluşturur (bellekte).
    fn create_test_aiff_file() -> Vec<u8> {
        let mut buffer = Vec::new();

        // FORM chunk başlığı
        buffer.extend_from_slice(b"FORM");
        buffer.write_u32::<StdBigEndian>(26).unwrap(); // Toplam chunk boyutu (COMM + padding) + 8 (FORM ve boyut alanı)
        buffer.extend_from_slice(b"AIFF");

        // COMM chunk
        buffer.extend_from_slice(b"COMM");
        buffer.write_u32::<StdBigEndian>(18).unwrap(); // COMM chunk veri boyutu
        buffer.write_u16::<StdBigEndian>(2).unwrap();    // num_channels = 2
        buffer.write_u32::<StdBigEndian>(44100 * 5).unwrap(); // num_frames = 5 saniye @ 44100Hz
        buffer.write_u16::<StdBigEndian>(16).unwrap();   // bits_per_sample = 16

        // Sample Rate (genişletilmiş format) - 44100 Hz
        let sample_rate_f64: f64 = 44100.0;
        let exponent: i16 = sample_rate_f64.to_exponent() as i16;
        let mantissa: u64 = (sample_rate_f64 / 2f64.powi(exponent as i32)).round() as u64;

        buffer.write_u8(exponent.wrapping_add(16383) as u8).unwrap(); // sample_rate_extended
        buffer.write_uint::<StdBigEndian>(mantissa, 10).unwrap();        // sample_rate_mantissa

        buffer
    }


    #[test]
    fn test_read_aiff_metadata() {
        // Test için bellekte AIFF dosyası oluştur
        let aiff_data = create_test_aiff_file();

        // Bellek içi dosyayı simüle etmek için Cursor kullan
        let mut cursor = Cursor::new(aiff_data);

        // Geçici bir dosya yolu oluştur ve Cursor içeriğini bu dosyaya yaz
        let file_path = "test.aiff";
        let mut temp_file = File::create(file_path).unwrap();
        std::io::copy(&mut cursor, &mut temp_file).unwrap();


        let metadata = read_aiff_metadata(file_path).unwrap();

        // Meta verilerin doğru olduğunu doğrula (test dosyamıza göre)
        assert_eq!(metadata.num_channels, 2);
        assert_eq!(metadata.sample_rate, 44100);
        assert_eq!(metadata.bits_per_sample, 16);
        assert_eq!(metadata.num_frames, 44100 * 5);

        // Test dosyasını temizle (isteğe bağlı)
        fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_read_aiff_metadata_invalid_form_chunk() {
        let file_path = "invalid_form.aiff";
        let mut invalid_file = File::create(file_path).unwrap();
        invalid_file.write_all(b"FORXAIFF...").unwrap(); // Yanlış FORM chunk ID
        let result = read_aiff_metadata(file_path);
        assert!(result.is_err());
        match result.err().unwrap() {
            AiffError::InvalidData(msg) => assert_eq!(msg, "Geçersiz AIFF dosyası: FORM chunk ID bulunamadı veya hatalı."),
            _ => panic!("Yanlış hata türü"),
        }
        fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_read_aiff_metadata_no_comm_chunk() {
         let file_path = "no_comm.aiff";
        let mut no_comm_file = File::create(file_path).unwrap();
        // FORM başlığı ve AIFF türü, ancak COMM chunk yok
        no_comm_file.write_all(b"FORM\x00\x00\x00\x04AIFF").unwrap();
        let result = read_aiff_metadata(file_path);
        assert!(result.is_err());
        match result.err().unwrap() {
            AiffError::InvalidData(msg) => assert_eq!(msg, "COMM chunk bulunamadı: Dosya sonuna ulaşıldı."),
            _ => panic!("Yanlış hata türü"),
        }
        fs::remove_file(file_path).unwrap();
    }
}

#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Gerçek çıktı mekanizmasına erişim olmalı (örneğin, UART).
            Ok(())
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => ({
            let mut stdout = $crate::print::Stdout;
            core::fmt::write(&mut stdout, core::format_args!($($arg)*)).unwrap();
        });
    }

    #[macro_export]
    macro_rules! println {
        () => ($crate::print!("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
    }
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}