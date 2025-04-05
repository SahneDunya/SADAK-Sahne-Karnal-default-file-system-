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
    srcblockdevice::BlockDevice,
};

#[cfg(not(feature = "std"))]
use core::result::Result;

#[cfg(not(feature = "std"))]
use core::option::Option;

#[cfg(feature = "std")]
use crate::srcblockdevice::BlockDevice;
#[cfg(feature = "std")]
use std::io::{Error, ErrorKind, Read as StdRead, Result as StdResult, Seek as StdSeek, SeekFrom as StdSeekFrom, Write as StdWrite};
#[cfg(feature = "std")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SahneError>;
}

#[cfg(not(feature = "std"))]
pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize, SahneError>;
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

pub struct EMMC {
    fd: u64, // Sahne64 dosya tanımlayıcısı
    block_size: u32,
    block_count: u32,
    #[cfg(feature = "std")]
    device_file: File,
}

impl EMMC {
    // EMMC aygıtını belirli bir dosya yolu ve blok boyutu ile başlatır.
    // Gerçek bir eMMC aygıtı için, bu fonksiyon aygıt sürücüsü ile etkileşim kurarak
    // aygıt dosyasını açmalı ve blok boyutu/sayısı gibi bilgileri almalıdır.
    #[cfg(feature = "std")]
    pub fn new(device_path: &str, block_size: u32, block_count: u32) -> StdResult<EMMC> {
        let device_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(device_path)?; // Dosyayı hem okuma hem de yazma modunda açar. Hata durumunda Result döner.

        Ok(EMMC {
            device_file,
            block_size,
            block_count,
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn new(device_path: &str, block_size: u32, block_count: u32) -> Result<EMMC, SahneError> {
        let flags = fs::O_RDWR;
        let fd = fs::open(device_path, flags)?;
        Ok(EMMC {
            fd,
            block_size,
            block_count,
        })
    }

    // Belirtilen blok adresinden veri okuma işlemini gerçekleştirir.
    #[cfg(feature = "std")]
    fn read_block_internal(&self, block_address: u32, buffer: &mut [u8]) -> StdResult<()> {
        let offset = (block_address * self.block_size) as u64; // Blok adresini byte offset'ine çevirir.

        // Dosya işaretçisini doğru pozisyona getirir.
        self.device_file.seek(StdSeekFrom::Start(offset))?;

        // Tam blok boyutunda veri okumaya çalışır.
        let bytes_read = self.device_file.read(buffer)?;

        // Okunan byte sayısı beklenen blok boyutundan azsa hata döndürür.
        if bytes_read != buffer.len() {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "Blok okuma hatası: Beklenen veri miktarı okunamadı.",
            ));
        }

        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn read_block_internal(&self, block_address: u32, buffer: &mut [u8]) -> Result<(), SahneError> {
        let offset = (block_address * self.block_size) as u64; // Blok adresini byte offset'ine çevirir.

        // Dosya işaretçisini doğru pozisyona getirir.
        fs::lseek(self.fd, offset as i64, fs::SEEK_SET)?;

        // Tam blok boyutunda veri okumaya çalışır.
        let bytes_read = fs::read(self.fd, buffer)?;

        // Okunan byte sayısı beklenen blok boyutundan azsa hata döndürür.
        if bytes_read != buffer.len() {
            return Err(SahneError::IOError("Blok okuma hatası: Beklenen veri miktarı okunamadı.".to_string()));
        }

        Ok(())
    }

    // Belirtilen blok adresine veri yazma işlemini gerçekleştirir.
    #[cfg(feature = "std")]
    fn write_block_internal(&self, block_address: u32, buffer: &[u8]) -> StdResult<()> {
        let offset = (block_address * self.block_size) as u64; // Blok adresini byte offset'ine çevirir.

        // Dosya işaretçisini doğru pozisyona getirir.
        self.device_file.seek(StdSeekFrom::Start(offset))?;

        // Tam blok boyutunda veri yazmaya çalışır.
        let bytes_written = self.device_file.write(buffer)?;

        // Yazılan byte sayısı beklenen blok boyutundan azsa hata döndürür.
        if bytes_written != buffer.len() {
            return Err(Error::new(
                ErrorKind::WriteZero,
                "Blok yazma hatası: Beklenen veri miktarı yazılamadı.",
            ));
        }

        // Yazma işlemlerinin diskeFlush edilmesini sağlar.
        self.device_file.flush()?;

        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn write_block_internal(&self, block_address: u32, buffer: &[u8]) -> Result<(), SahneError> {
        let offset = (block_address * self.block_size) as u64; // Blok adresini byte offset'ine çevirir.

        // Dosya işaretçisini doğru pozisyona getirir.
        fs::lseek(self.fd, offset as i64, fs::SEEK_SET)?;

        // Tam blok boyutunda veri yazmaya çalışır.
        let bytes_written = fs::write(self.fd, buffer)?;

        // Yazılan byte sayısı beklenen blok boyutundan azsa hata döndürür.
        if bytes_written != buffer.len() {
            return Err(SahneError::IOError("Blok yazma hatası: Beklenen veri miktarı yazılamadı.".to_string()));
        }

        // Sahne64'te flush benzeri bir sistem çağrısı gerekebilir.
        // Şimdilik bu adımı atlıyoruz.

        Ok(())
    }
}

impl BlockDevice for EMMC {
    // BlockDevice trait'inden gelen read_block fonksiyonunun implementasyonu.
    // Kendi read_block_internal fonksiyonunu çağırır.
    fn read_block(&self, block_address: u32, buffer: &mut [u8]) -> Result<(), SahneError> {
        // Adres aralığı kontrolü: Geçersiz blok adreslerini engeller.
        if block_address >= self.block_count {
            return Err(SahneError::InvalidInput("Geçersiz blok adresi: Blok adresi blok sayısının dışında.".to_string()));
        }
        self.read_block_internal(block_address, buffer)
    }

    // BlockDevice trait'inden gelen write_block fonksiyonunun implementasyonu.
    // Kendi write_block_internal fonksiyonunu çağırır.
    fn write_block(&self, block_address: u32, buffer: &[u8]) -> Result<(), SahneError> {
        // Adres aralığı kontrolü: Geçersiz blok adreslerini engeller.
        if block_address >= self.block_count {
            return Err(SahneError::InvalidInput("Geçersiz blok adresi: Blok adresi blok sayısının dışında.".to_string()));
        }
        // Buffer boyut kontrolü: Buffer'ın blok boyutuna eşit olmasını zorlar.
        if buffer.len() as u32 != self.block_size {
            return Err(SahneError::InvalidInput("Geçersiz buffer boyutu: Buffer boyutu blok boyutuna eşit olmalı.".to_string()));
        }
        self.write_block_internal(block_address, buffer)
    }

    // eMMC blok boyutunu döndüren fonksiyon.
    fn block_size(&self) -> u32 {
        self.block_size
    }

    // eMMC blok sayısını döndüren fonksiyon.
    fn block_count(&self) -> u32 {
        self.block_count
    }
}

// Örnek kullanım için test modülü
#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use std::vec::Vec;
    use tempfile::NamedTempFile;
    use std::io::Result as IoResult;

    #[test]
    fn test_emmc_read_write() -> IoResult<()> {
        // Geçici bir dosya oluşturur ve EMMC aygıtını simüle etmek için kullanır.
        let temp_file = NamedTempFile::new()?;
        let device_path = temp_file.path().to_str().unwrap();
        let block_size: u32 = 512;
        let block_count: u32 = 1024;

        // EMMC yapısını oluşturur.
        let emmc = EMMC::new(device_path, block_size, block_count).unwrap();

        // Yazılacak veri için bir buffer oluşturur (blok boyutunda).
        let write_block_address: u32 = 0;
        let write_data: Vec<u8> = vec![0xAA; block_size as usize];

        // Veriyi EMMC'ye yazar.
        emmc.write_block(write_block_address, &write_data).unwrap();

        // Okunacak veri için bir buffer oluşturur (blok boyutunda).
        let read_block_address: u32 = 0;
        let mut read_buffer: Vec<u8> = vec![0x00; block_size as usize];

        // Veriyi EMMC'den okur.
        emmc.read_block(read_block_address, &mut read_buffer).unwrap();

        // Yazılan ve okunan verinin aynı olup olmadığını kontrol eder.
        assert_eq!(write_data, read_buffer, "Okunan veri yazılan veriyle eşleşmiyor.");

        Ok(())
    }

    #[test]
    fn test_emmc_invalid_address() -> IoResult<()> {
        // Geçici bir dosya oluşturur.
        let temp_file = NamedTempFile::new()?;
        let device_path = temp_file.path().to_str().unwrap();
        let block_size: u32 = 512;
        let block_count: u32 = 1024;

        // EMMC yapısını oluşturur.
        let emmc = EMMC::new(device_path, block_size, block_count).unwrap();

        // Geçersiz blok adresine yazma denemesi yapar.
        let invalid_block_address: u32 = block_count; // Blok sayısına eşit adres geçersizdir.
        let write_data: Vec<u8> = vec![0xFF; block_size as usize];
        let write_result = emmc.write_block(invalid_block_address, &write_data);

        // Geçersiz adres hatası alıp almadığını kontrol eder.
        assert!(write_result.is_err(), "Geçersiz adres yazma işlemi hata döndürmeliydi.");
        assert_eq!(write_result.unwrap_err().kind(), ErrorKind::InvalidInput, "Yanlış hata türü döndürüldü.");


        // Geçersiz blok adresinden okuma denemesi yapar.
        let read_result = emmc.read_block(invalid_block_address, &mut vec![0u8; block_size as usize]);

        // Geçersiz adres hatası alıp almadığını kontrol eder.
        assert!(read_result.is_err(), "Geçersiz adres okuma işlemi hata döndürmeliydi.");
        assert_eq!(read_result.unwrap_err().kind(), ErrorKind::InvalidInput, "Yanlış hata türü döndürüldü.");

        Ok(())
    }

    #[test]
    fn test_emmc_invalid_buffer_size_write() -> IoResult<()> {
            // Geçici bir dosya oluşturur.
        let temp_file = NamedTempFile::new()?;
        let device_path = temp_file.path().to_str().unwrap();
        let block_size: u32 = 512;
        let block_count: u32 = 1024;

        // EMMC yapısını oluşturur.
        let emmc = EMMC::new(device_path, block_size, block_count).unwrap();

        // Geçersiz buffer boyutu ile yazma denemesi yapar.
        let valid_block_address: u32 = 0;
        let invalid_write_data: Vec<u8> = vec![0xFF; (block_size/2) as usize]; // Blok boyutundan küçük buffer
        let write_result = emmc.write_block(valid_block_address, &invalid_write_data);

        // Geçersiz buffer boyutu hatası alıp almadığını kontrol eder.
        assert!(write_result.is_err(), "Geçersiz buffer boyutu yazma işlemi hata döndürmeliydi.");
        assert_eq!(write_result.unwrap_err().kind(), ErrorKind::InvalidInput, "Yanlış hata türü döndürüldü.");
        Ok(())
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