#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz

use serde::Deserialize;
use serde_yaml::Value;

use crate::{fs, SahneError}; // Sahne64'e özgü modülleri ve hata türünü kullanıyoruz

#[derive(Debug, Deserialize)]
pub struct YamlFile {
    pub data: Value,
}

impl YamlFile {
    pub fn load_from_file(path: &str) -> Result<Self, SahneError> {
        // Dosyayı sadece okuma modunda aç
        let fd = fs::open(path, fs::O_RDONLY)?;

        let mut contents = String::new();
        let mut buffer = [0u8; 128]; // Okuma için bir tampon oluşturuyoruz

        loop {
            match fs::read(fd, &mut buffer) {
                Ok(bytes_read) if bytes_read > 0 => {
                    // Okunan byte'ları String'e ekle
                    if let Ok(s) = core::str::from_utf8(&buffer[..bytes_read]) {
                        contents.push_str(s);
                    } else {
                        fs::close(fd)?;
                        return Err(SahneError::InvalidParameter); // Geçersiz UTF-8 verisi
                    }
                }
                Ok(_) => {
                    // Dosyanın sonuna ulaşıldı
                    break;
                }
                Err(e) => {
                    // Okuma sırasında bir hata oluştu
                    fs::close(fd)?;
                    return Err(e);
                }
            }
        }

        // Dosyayı kapat
        fs::close(fd)?;

        // YAML içeriğini parse et
        match serde_yaml::from_str(&contents) {
            Ok(data) => Ok(YamlFile { data }),
            Err(_) => Err(SahneError::InvalidParameter), // Geçersiz YAML formatı
        }
    }

    pub fn get_value<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data.get(key).and_then(serde_yaml::from_value::<T>.ok())
    }
}