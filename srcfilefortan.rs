use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::str::FromStr;

pub struct FortranFile {
    pub data: Vec<f64>,
}

impl FortranFile {
    pub fn new() -> Self {
        FortranFile { data: Vec::new() }
    }

    // İyileştirilmiş read_from_file fonksiyonu: Buffer yeniden kullanımı ve hatasız okuma
    pub fn read_from_file_optimized<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut data = Vec::new();
        let mut line_buffer = String::new(); // Satır buffer'ı yeniden kullanım için

        loop {
            line_buffer.clear(); // Buffer'ı temizle
            let bytes_read = reader.read_line(&mut line_buffer)?;
            if bytes_read == 0 { // EOF kontrolü
                break;
            }

            // Satırı whitespace'e göre ayır ve sayıları işle
            for value_str in line_buffer.split_whitespace() {
                match f64::from_str(value_str) {
                    Ok(num) => data.push(num),
                    Err(_) => {
                        // İsteğe bağlı: Hatalı sayı formatını işle veya yok say
                        // Şu anda hatalı formatları yok sayıyoruz.
                        eprintln!("Uyarı: Geçersiz sayı formatı bulundu: {}", value_str);
                    }
                }
            }
        }

        Ok(FortranFile { data })
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let mut file = File::create(path)?;
        for value in &self.data {
            writeln!(file, "{}", value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_read_write_fortran_file_optimized() {
        let test_file_path = "test_fortran_file_optimized.dat";

        // Test dosyası oluştur ve içine veri yaz
        let mut test_file = File::create(test_file_path).unwrap();
        writeln!(test_file, "1.0 2.0 3.0").unwrap();
        writeln!(test_file, "4.0   5.0 ").unwrap();
        test_file.flush().unwrap();


        let mut fortran_file = FortranFile::new();
        fortran_file.data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        // Dosyaya yazma (orijinal fonksiyon ile)
        assert!(fortran_file.write_to_file(test_file_path).is_ok());

        // Dosyadan okuma (optimize edilmiş fonksiyon ile)
        let read_file = FortranFile::read_from_file_optimized(test_file_path).unwrap();
        assert_eq!(fortran_file.data, read_file.data);

        // Test dosyasını sil
        fs::remove_file(test_file_path).unwrap();
    }
}