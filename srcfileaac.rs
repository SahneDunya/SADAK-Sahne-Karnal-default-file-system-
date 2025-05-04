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
use std::io::{BufReader, Read as StdRead, Result as StdResult, Error as StdIOError, ErrorKind as StdIOErrorKind}; // std::io::Error, ErrorKind eklendi
#[cfg(feature = "std")]
use faad_rs::{Decoder, Frame}; // std path uses the real faad-rs crate
#[cfg(feature = "std")]
use std::vec::Vec as StdVec; // std::vec::Vec kullanımı için

#[cfg(not(feature = "std"))]
use alloc::vec::Vec; // alloc::vec::Vec kullanımı için

// Sahne64'e özel basit AAC dekoderi varsayımı (placeholder implementasyon)
// Gerçek bir no_std AAC dekoderi daha karmaşık olacaktır.
#[cfg(not(feature = "std"))]
mod faad_rs {
     // SahneError'ı dışarıdan import et
    use crate::SahneError;
     // Vec yapısını dışarıdan (alloc veya custom) import et
    use alloc::vec::Vec; // veya crate::memory::Vec eğer öyle tanımlıysa

    #[derive(Debug)] // Debug türetildi hata mesajı için
    pub struct Decoder<'a> {
        // Sahne64'e özel dekoder yapısı (basit placeholder)
        data: &'a [u8],
        position: usize,
    }

    #[derive(Debug)] // Debug türetildi
    pub enum DecodeError {
        // Dekoder hataları için placeholder
        InvalidData,
        Other(alloc::string::String), // String kullanmak alloc gerektirir
    }

    // SahneError'ı bu DecodeError'a dönüştürebiliriz
    impl From<SahneError> for DecodeError {
        fn from(err: SahneError) -> Self {
             // SahneError'dan daha spesifik bir DecodeError'a çevirme mantığı
             DecodeError::Other(alloc::string::String::from("Underlying SahneError")) // Basit placeholder
             // veya SahneError'ı Debug formatıyla stringe çevirip Other içine koyabiliriz
              DecodeError::Other(alloc::string::String::from(alloc::format!("{:?}", err)))
        }
    }


    impl<'a> Decoder<'a> {
        // new metodu Result<Self, DecodeError> dönmeli
        pub fn new(data: &'a [u8]) -> Result<Self, DecodeError> {
            // Sahne64'e özel dekoder başlatma mantığı
            // Gerçek dekoderde AAC header kontrolü vb. olur.
             if data.is_empty() {
                 return Err(DecodeError::InvalidData);
             }
            Ok(Decoder { data, position: 0 })
        }

        // decode_frame metodu Result<Option<Frame>, DecodeError> dönmeli
        pub fn decode_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
            // Sahne64'e özel frame dekodlama mantığı (çok basitleştirilmiş placeholder)
            if self.position >= self.data.len() {
                return Ok(None); // Dosya sonu
            }
            // Basit bir örnek: her 4 byte'ı bir i16 sample olarak kabul et
            if self.position + 1 < self.data.len() { // i16 için en az 2 byte lazım
                let sample = i16::from_le_bytes([
                    self.data[self.position],
                    self.data[self.position + 1],
                ]);
                self.position += 2; // i16 boyutu kadar ilerle
                Ok(Some(Frame::Short(Vec::from_slice(&[sample])))) // alloc::vec::Vec kullanır
            } else {
                 // Yeterli byte yoksa dosya sonuna gelindiğini varsay
                self.position = self.data.len();
                Ok(None)
            }
        }
    }

    #[derive(Debug)] // Debug türetildi
    pub enum Frame {
        Short(Vec<i16>), // alloc::vec::Vec kullanır
        Float(Vec<f32>), // alloc::vec::Vec kullanır
    }
}

// Hata Eşleme Yardımcıları
std::io::Error -> FileSystemError
#[cfg(feature = "std")]
fn map_io_error_to_fs_error(e: StdIOError) -> FileSystemError {
    // std::io::Error türlerine göre FileSystemError eşlemesi yapılabilir.
    // Örneğin, Dosya bulunamadı, Yetki reddedildi, Okuma/Yazma hatası vb.
    // Şimdilik genel bir IOError'a map edelim.
    FileSystemError::IOError(alloc::string::String::from(alloc::format!("IO Error: {}", e))) // alloc::format! kullanır
}

// SahneError -> FileSystemError
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    // SahneError türlerine göre FileSystemError eşlemesi yapılabilir.
    // Örneğin, ResourceNotFound -> FileSystemError::DeviceError veya IOError
    // InvalidParameter -> FileSystemError::Other veya daha spesifik
    // Şimdilik genel bir IOError'a map edelim veya SahneError'ı stringe çevirelim.
    FileSystemError::IOError(alloc::string::String::from(alloc::format!("SahneError: {:?}", e))) // Debug formatı stringe çevrilir
     // Daha iyi bir yaklaşım FileSystemError içinde SahneError'ı tutmaktır:
      FileSystemError::UnderlyingError(e) // Eğer böyle bir varyant varsa
}

// DecoderError -> FileSystemError (no_std)
#[cfg(not(feature = "std"))]
fn map_decode_error_to_fs_error(e: faad_rs::DecodeError) -> FileSystemError {
    // DecoderError türlerine göre FileSystemError eşlemesi yapılabilir.
    // Örneğin, InvalidData -> FileSystemError::DataBlockError veya DirectoryError (Dosya formatı hatası gibi)
    // Şimdilik genel bir Other hatasına map edelim veya yeni bir varyant ekleyelim.
    FileSystemError::Other(alloc::string::String::from(alloc::format!("Decoding Error: {:?}", e))) // Debug formatı stringe çevrilir
     // Belki FileSystemError'da DecodingError(String) gibi bir varyant daha iyi olur.
}


/// Belirtilen dosya yolundaki (kaynak ID'si) AAC dosyasını okur ve örnekleri (i16) döner.
#[cfg(feature = "std")]
pub fn read_aac_file(file_path: &str) -> Result<Vec<i16>, FileSystemError> { // FileSystemError döner
    use alloc::vec::Vec; // alloc::vec::Vec kullanımı için
    use alloc::string::String; // alloc::string::String kullanımı için
    use alloc::format; // alloc::format! kullanımı için

    let file = File::open(file_path).map_err(map_io_error_to_fs_error)?; // std::io::Error -> FileSystemError
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).map_err(map_io_error_to_fs_error)?; // std::io::Error -> FileSystemError

    // faad-rs::Decoder::new Result<Self, faad_rs::error::Error> dönüyor.
    // Bu hatayı da FileSystemError'a çevirmeliyiz.
    // Gerçek faad-rs crate'inin hata tipi için From veya map_err kullanılmalı.
    // Varsayım: faad-rs::error::Error display implement ediyor ve stringe çevrilebilir.
    let mut decoder = Decoder::new(&buffer).map_err(|e| FileSystemError::Other(format!("AAC decoder init error: {}", e)))?; // faad-rs hata -> FileSystemError

    let mut samples = Vec::new();
    samples.reserve(buffer.len()); // i16 boyutu için buffer.len() / 2 daha uygun olabilir ama güvenli olsun.

    loop {
        // decoder.decode_frame() Result<Option<Frame>, faad_rs::error::Error> dönüyor.
        match decoder.decode_frame() {
            Ok(Some(frame)) => {
                match frame {
                    Frame::Short(data) => samples.extend_from_slice(&data),
                    Frame::Float(data) => {
                        for &x in data.iter() {
                            samples.push((x * 32767.0) as i16);
                        }
                    }
                }
            }
            Ok(None) => break, // Dosya sonu
            Err(e) => return Err(FileSystemError::Other(format!("Frame decode error: {}", e))), // faad-rs hata -> FileSystemError
        }
    }
    Ok(samples)
}

/// Belirtilen Sahne64 kaynak ID'sindeki AAC dosyasını okur ve örnekleri (i16) döner (no_std).
///
/// # DİKKAT: Büyük dosyalar için tamamını belleğe okumak verimsiz ve tehlikelidir.
/// Akış tabanlı okuma/dekodlama tercih edilmelidir.
#[cfg(not(feature = "std"))]
pub fn read_aac_file(resource_id: &str) -> Result<Vec<i16>, FileSystemError> { // FileSystemError döner
     // alloc::vec::Vec kullanımı için zaten alloc crate'i etkin.
     // alloc::string::String ve alloc::format! kullanımı için de.

    // Kaynağı edin
    let handle = resource::acquire(resource_id, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Tüm veriyi belleğe oku (Büyük dosyalar için sorunlu!)
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 128]; // Okuma chunk boyutu
    loop {
        // resource::read Result<usize, SahneError> döner
        match resource::read(handle, &mut chunk) {
            Ok(0) => break, // Dosya sonu
            Ok(bytes_read) => {
                // buffer.extend_from_slice requires alloc
                buffer.extend_from_slice(&chunk[..bytes_read]);
            }
            Err(e) => {
                 let _ = resource::release(handle); // Kaynağı serbest bırakmayı dene
                 return Err(map_sahne_error_to_fs_error(e)); // SahneError -> FileSystemError
            }
        }
    }

    // Kaynağı serbest bırak
    let _ = resource::release(handle).map_err(|e| {
         // Kaynak serbest bırakma hatası (kritik değilse sadece logla)
         println!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print makrosu
         map_sahne_error_to_fs_error(e) // Yine de hatayı FileSystemError'a çevir
     });


    // AAC dekoderini başlat
    // faad_rs::Decoder::new Result<Self, DecodeError> dönüyor (no_std placeholder)
    let mut decoder = faad_rs::Decoder::new(&buffer)
        .map_err(map_decode_error_to_fs_error)?; // DecodeError -> FileSystemError

    // Örnekleri dekodla
    let mut samples = Vec::new();
    samples.reserve(buffer.len()); // i16 boyutu için buffer.len() / 2 daha uygun ama güvenli olsun.

    loop {
        // decoder.decode_frame() Result<Option<Frame>, DecodeError> dönüyor (no_std placeholder)
        match decoder.decode_frame() {
            Ok(Some(frame)) => {
                match frame {
                    faad_rs::Frame::Short(data) => samples.extend_from_slice(&data), // Vec<i16> extend
                    faad_rs::Frame::Float(data) => {
                        for &x in data.iter() {
                            samples.push((x * 32767.0) as i16); // f32 to i16
                        }
                    }
                }
            }
            Ok(None) => break, // Dekodlama sonu
            Err(e) => return Err(map_decode_error_to_fs_error(e)), // DecodeError -> FileSystemError
        }
    }

    Ok(samples)
}


// Test modülü (çoğunlukla std implementasyonunu test eder)
#[cfg(test)]
#[cfg(feature = "std")] // std feature'ı ve test özelliği varsa derle
mod tests {
    use super::*;
     use std::fs::write; // std::fs::write kullanımı için
     use alloc::vec::Vec as AllocVec; // test içinde alloc::vec::Vec kullanımı için
     use alloc::string::ToString; // test içinde to_string() kullanımı için

    // Örnek AAC verisi (test_data/sine_440hz.aac yoluna göre dahil edilmeli)
    // Bu satırın çalışması için Cargo.toml dosyasında [build] bölümü altında build script
    // veya test_data dizininin derleme sırasında erişilebilir olması gerekebilir.
    // Varsayım: include_bytes! std ortamında dosya sistemine erişebilir.
    const TEST_AAC_DATA: &[u8] = include_bytes!("../../test_data/sine_440hz.aac");


    #[test]
    fn test_read_aac_file_std() {
        // Test için örnek bir AAC dosyası oluştur
         // tempfile crate'i test bağımlılıklarına eklenmeli
        let temp_file_res = tempfile::NamedTempFile::new();
        let temp_file = temp_file_res.expect("Geçici dosya oluşturulamadı");
        let device_path = temp_file.path().to_str().expect("Dosya yolu stringe çevrilemedi");

        write(device_path, TEST_AAC_DATA).expect("Test dosyasına yazılamadı");

        // AAC dosyasını oku (std implementasyonu)
        let samples_res = read_aac_file(device_path);

        // Sonucun Ok ve örneklerin boş olmadığını kontrol et
        assert!(samples_res.is_ok(), "AAC dosyası okunurken hata oluştu: {:?}", samples_res.err());
        let samples = samples_res.unwrap();
        assert!(!samples.is_empty(), "AAC dosyasından örnek okunmadı.");
        println!("Okunan {} i16 örneği.", samples.len()); // std println!

        // Geçici dosyayı sil
        // temp_file Drop trait'i sayesinde otomatik silinir.
         std::fs::remove_file(device_path).unwrap_or_default(); // Manuel silmeye gerek yok.
    }

    // TODO: no_std implementasyonu için testler yazılmalı.
    // Bu testler Sahne64 ortamında veya bir emülatörde çalıştırılmalıdır.
    // resource::acquire, resource::read, resource::release ve resource::control
    // çağrılarını doğru şekilde simule eden bir altyapı gerektirir.
    // Ayrıca no_std placeholder faad_rs dekoderinin gerçek bir dekoderle değiştirilmesi
    // durumunda bu testlerin güncellenmesi gerekir.
}

// Tekrarlanan no_std print modülü ve panic handler kaldırıldı.
