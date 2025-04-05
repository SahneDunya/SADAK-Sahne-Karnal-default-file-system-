#![no_std] // Eğer bu dosya da no_std ortamında çalışacaksa bu satırı ekleyin

use super::fs; // Sahne64 dosya sistemi modülünü kullan
use super::SahneError; // Sahne64 hata türünü kullan
use alloc::string::String; // Eğer no_std ise String için alloc crate'ini kullanın
use alloc::vec::Vec; // Eğer no_std ise Vec için alloc crate'ini kullanın
use toml::Value; // toml kütüphanesini kullanmaya devam ediyoruz (no_std uyumlu olduğunu varsayarak)

pub struct TomlFile {
    pub data: Value,
}

impl TomlFile {
    pub fn new(path: &str) -> Result<Self, SahneError> {
        // Dosyayı Sahne64 fonksiyonlarını kullanarak aç
        let fd = fs::open(path, fs::O_RDONLY)?;

        let mut contents = String::new();
        let mut buffer = [0u8; 128]; // Okuma için bir arabellek oluştur

        loop {
            // Dosyadan okuma işlemi
            let bytes_read = fs::read(fd, &mut buffer)?;
            if bytes_read == 0 {
                break; // Dosyanın sonuna ulaşıldı
            }
            // Okunan byte'ları String'e ekle
            match String::from_utf8(buffer[..bytes_read].to_vec()) {
                Ok(s) => contents.push_str(&s),
                Err(_) => {
                    // UTF-8 dönüşümü başarısız oldu, bu bir hata durumu
                    fs::close(fd)?;
                    return Err(SahneError::InvalidParameter); // Veya daha uygun bir hata türü
                }
            }
        }

        // Dosyayı kapat
        fs::close(fd)?;

        // TOML içeriğini ayrıştır
        let data: Result<Value, toml::de::Error> = toml::from_str(&contents);
        match data {
            Ok(parsed_data) => Ok(TomlFile { data: parsed_data }),
            Err(_) => Err(SahneError::InvalidParameter), // TOML ayrıştırma hatası
        }
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(Value::as_str)
    }

    pub fn get_integer(&self, key: &str) -> Option<i64> {
        self.data.get(key).and_then(Value::as_integer)
    }

    // Diğer veri türleri için benzer get metotları eklenebilir.
}