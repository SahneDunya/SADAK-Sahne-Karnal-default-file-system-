use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

pub struct PycFile {
    magic_number: u32,
    modification_timestamp: u32,
    code_object: Vec<u8>, // Kod nesnesini ham bayt dizisi olarak saklıyoruz.
}

impl PycFile {
    // İyileştirilmiş okuma fonksiyonu: Açıklayıcı hata yayılımı ve yorumlar eklendi.
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?; // Dosyayı aç

        // Magic number'ı oku (4 bayt, little-endian)
        let mut magic_number_bytes = [0; 4];
        file.read_exact(&mut magic_number_bytes)?; // Tam olarak 4 bayt oku
        let magic_number = u32::from_le_bytes(magic_number_bytes); // Little-endian'dan u32'ye dönüştür

        // Modification timestamp'i oku (4 bayt, little-endian)
        let mut modification_timestamp_bytes = [0; 4];
        file.read_exact(&mut modification_timestamp_bytes)?; // Tam olarak 4 bayt oku
        let modification_timestamp = u32::from_le_bytes(modification_timestamp_bytes); // Little-endian'dan u32'ye dönüştür

        // Kalan tüm baytları kod nesnesi olarak oku
        let mut code_object = Vec::new();
        file.read_to_end(&mut code_object)?; // Dosyanın sonuna kadar oku

        Ok(PycFile {
            magic_number,
            modification_timestamp,
            code_object,
        })
    }

    // Yazma fonksiyonu (değişmedi, zaten yeterince iyi)
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(&self.magic_number.to_le_bytes())?;
        file.write_all(&self.modification_timestamp.to_le_bytes())?;
        file.write_all(&self.code_object)?;
        Ok(())
    }

    // İsteğe bağlı: Python kod nesnesini ayrıştırmak için yardımcı fonksiyonlar eklenebilir.
    // Bu, `marshal` ve `dis` modüllerini kullanmayı gerektirecektir.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_read_write_pyc_optimized() {
        let temp_dir = tempdir().unwrap();
        let pyc_path = temp_dir.path().join("test_optimized.pyc");

        // Örnek bir .pyc dosyası oluşturalım.
        let magic_number = 3494; // Python 3.8 magic number (örnek)
        let modification_timestamp = 1678886400; // Örnek zaman damgası
        let code_object = vec![101, 0, 0, 0, 100, 0, 0, 0, 83, 0, 0, 0]; // Örnek kod nesnesi (basit)

        let pyc_file = PycFile {
            magic_number,
            modification_timestamp,
            code_object,
        };

        pyc_file.write_to_file(&pyc_path).unwrap();

        // Optimize edilmiş okuma fonksiyonunu kullan
        let read_pyc_file = PycFile::read_from_file(&pyc_path).unwrap();

        assert_eq!(pyc_file.magic_number, read_pyc_file.magic_number);
        assert_eq!(
            pyc_file.modification_timestamp,
            read_pyc_file.modification_timestamp
        );
        assert_eq!(pyc_file.code_object, read_pyc_file.code_object);

        fs::remove_file(pyc_path).unwrap();
    }
}