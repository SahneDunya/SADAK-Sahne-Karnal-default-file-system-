#![no_std]
#![allow(dead_code)]

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

pub struct OdfFile {
    pub content: String,
}

impl OdfFile {
    pub fn open<P: AsRef<str>>(path: P) -> Result<Self, SahneError> {
        let path_str = path.as_ref();
        let fd = fs::open(path_str, fs::O_RDONLY)?;
        let mut content = String::new();
        let mut buffer = [0u8; 128]; // Okuma arabelleği

        loop {
            match fs::read(fd, &mut buffer) {
                Ok(0) => break, // Dosyanın sonu
                Ok(bytes_read) => {
                    match core::str::from_utf8(&buffer[..bytes_read]) {
                        Ok(s) => content.push_str(s),
                        Err(_) => {
                            fs::close(fd)?;
                            return Err(SahneError::InvalidParameter); // Veya daha uygun bir hata türü
                        }
                    }
                }
                Err(e) => {
                    fs::close(fd)?;
                    return Err(e);
                }
            }
        }

        fs::close(fd)?;
        Ok(OdfFile { content })
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }
}

#[cfg(feature = "std")] // Testler için std kütüphanesini kullanabiliriz (eğer bu kod ayrı bir test ortamında çalıştırılacaksa)
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_open_odf() {
        // Geçici bir dizin ve content.xml dosyası oluştur
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("content.xml");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"<office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"><office:body><office:text><text:p xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">Merhaba Dünya!</text:p></office:text></office:body></office:document-content>").unwrap();

        // .odf dosyasını (aslında content.xml) aç ve içeriğini kontrol et
        let odf_file = OdfFile::open(file_path.to_str().unwrap()).unwrap();
        assert_eq!(odf_file.get_content(), "<office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"><office:body><office:text><text:p xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">Merhaba Dünya!</text:p></office:text></office:body></office:document-content>");
    }
}