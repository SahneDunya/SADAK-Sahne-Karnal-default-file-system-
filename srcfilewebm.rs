#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz

#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Gerekli modülleri içeri aktar
use crate::{SahneError, fs};
use byteorder::{BigEndian, ByteOrder};

pub struct WebM {
    pub file_path: String,
}

impl WebM {
    pub fn new(file_path: String) -> WebM {
        WebM { file_path }
    }

    pub fn parse(&self) -> Result<(), SahneError> {
        // Dosyayı aç
        let open_result = fs::open(&self.file_path, fs::O_RDONLY);
        let fd = match open_result {
            Ok(fd) => fd,
            Err(e) => return Err(e),
        };

        // EBML başlığını oku
        let ebml_header = self.read_ebml_header(fd)?;
        println!("EBML Header: {:?}", ebml_header);

        // Segmenti oku
        let segment_header = self.read_segment_header(fd)?;
        println!("Segment Header: {:?}", segment_header);

        // Dosyayı kapat
        let close_result = fs::close(fd);
        if let Err(e) = close_result {
            eprintln!("Dosya kapatılırken hata oluştu: {:?}", e);
        }

        // Meta verileri oku (isteğe bağlı)
        // ...

        Ok(())
    }

    fn read_ebml_header(&self, fd: u64) -> Result<EBMLHeader, SahneError> {
        let mut buffer = [0u8; 4];
        let read_result = fs::read(fd, &mut buffer);
        let bytes_read = match read_result {
            Ok(len) => len,
            Err(e) => return Err(e),
        };

        if bytes_read != 4 {
            return Err(SahneError::InvalidData); // Dosya sonuna ulaşıldı veya yeterli veri okunamadı
        }

        let id = BigEndian::read_u32(&buffer);
        if id != 0x1A45DFA3 {
            return Err(SahneError::InvalidData);
        }
        Ok(EBMLHeader { id })
    }

    fn read_segment_header(&self, fd: u64) -> Result<SegmentHeader, SahneError> {
        let mut buffer = [0u8; 4];
        let read_result = fs::read(fd, &mut buffer);
        let bytes_read = match read_result {
            Ok(len) => len,
            Err(e) => return Err(e),
        };

        if bytes_read != 4 {
            return Err(SahneError::InvalidData); // Dosya sonuna ulaşıldı veya yeterli veri okunamadı
        }

        let id = BigEndian::read_u32(&buffer);
        if id != 0x18538067 {
            return Err(SahneError::InvalidData);
        }
        Ok(SegmentHeader { id })
    }
}

#[derive(Debug)]
struct EBMLHeader {
    id: u32,
}

#[derive(Debug)]
struct SegmentHeader {
    id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{O_CREAT, O_WRONLY};

    // Yardımcı fonksiyon: Bir dosyaya byte yazmak için Sahne64 fonksiyonlarını kullanır
    fn write_bytes_to_file(path: &str, data: &[u8]) -> Result<(), SahneError> {
        let open_result = fs::open(path, O_CREAT | O_WRONLY);
        let fd = match open_result {
            Ok(fd) => fd,
            Err(e) => return Err(e),
        };

        let write_result = fs::write(fd, data);
        if let Err(e) = write_result {
            fs::close(fd).unwrap_or_default(); // Hata durumunda dosyayı kapat
            return Err(e);
        }

        let close_result = fs::close(fd);
        close_result
    }

    #[test]
    fn test_parse_webm() {
        // Test için örnek bir WebM dosyası oluştur (EBML ve Segment başlıkları eklendi)
        let ebml_header = &[0x1A, 0x45, 0xDF, 0xA3, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let segment_header = &[0x18, 0x53, 0x80, 0x67, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        assert!(write_bytes_to_file("test.webm", &[ebml_header, segment_header].concat()).is_ok());

        let webm = WebM::new("test.webm".to_string());
        assert!(webm.parse().is_ok());

        // Test dosyasını sil (isteğe bağlı)
        let _ = fs::open("test.webm", fs::O_RDONLY).and_then(fs::close);
    }

    #[test]
    fn test_parse_webm_invalid_ebml_header() {
        // Test için geçersiz EBML header ile WebM dosyası oluştur
        let invalid_ebml_header = &[0x00, 0x00, 0x00, 0x00];
        assert!(write_bytes_to_file("invalid_ebml.webm", invalid_ebml_header).is_ok());

        let webm = WebM::new("invalid_ebml.webm".to_string());
        assert!(webm.parse().is_err());

        // Test dosyasını sil (isteğe bağlı)
        let _ = fs::open("invalid_ebml.webm", fs::O_RDONLY).and_then(fs::close);
    }

    #[test]
    fn test_parse_webm_invalid_segment_header() {
        // Test için geçerli EBML header ama geçersiz Segment header ile WebM dosyası oluştur
        let ebml_header = &[0x1A, 0x45, 0xDF, 0xA3, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let invalid_segment_header = &[0x00, 0x00, 0x00, 0x00];
        assert!(write_bytes_to_file("invalid_segment.webm", &[ebml_header, invalid_segment_header].concat()).is_ok());

        let webm = WebM::new("invalid_segment.webm".to_string());
        assert!(webm.parse().is_err());

        // Test dosyasını sil (isteğe bağlı)
        let _ = fs::open("invalid_segment.webm", fs::O_RDONLY).and_then(fs::close);
    }
}