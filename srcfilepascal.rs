use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

pub struct PascalFile {
    pub lines: Vec<String>, // Pascal dosyasının satırlarını saklar
}

impl PascalFile {
    pub fn new() -> PascalFile {
        PascalFile {
            lines: Vec::new(),
        }
    }

    // İyileştirilmiş read_from_file fonksiyonu: Satırları doğrudan `lines` vektörüne okur.
    pub fn read_from_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        self.lines.clear(); // Mevcut satırları temizle
        for line_result in reader.lines() {
            let line = line_result?;
            self.lines.push(line);
        }
        Ok(())
    }

    // İyileştirilmiş write_to_file fonksiyonu: Saklanan satırları dosyaya yazar.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for line in &self.lines {
            writeln!(writer, "{}", line)?; // Satırları dosyaya yaz
        }
        writer.flush()?; // Tampon belleği temizle
        Ok(())
    }

    // Dosyaya satır ekleme fonksiyonu
    pub fn append_line(&mut self, line: String) {
        self.lines.push(line);
    }

    // Belirli bir satırı alma fonksiyonu (isteğe bağlı)
    pub fn get_line(&self, index: usize) -> Option<&String> {
        self.lines.get(index)
    }

    // Satır sayısını döndürme fonksiyonu (isteğe bağlı)
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::ErrorKind;

    #[test]
    fn test_pascal_file_read_write_optimized() {
        let mut pascal_file = PascalFile::new();
        let test_file_path = "test_pascal_file_optimized.pas";

        // Örnek bir Pascal dosya içeriği (basit bir örnek)
        let pascal_content = "program TestProgram;\nbegin\n  WriteLn('Hello, Pascal!');\nend.";

        // Dosyayı oluştur ve içeriği yaz
        fs::write(test_file_path, pascal_content).expect("Dosya oluşturulamadı");

        // Dosyayı oku
        pascal_file.read_from_file(test_file_path).expect("Dosya okunamadı");

        // Okunan satır sayısını kontrol et
        assert_eq!(pascal_file.line_count(), 4);
        // İlk satırı kontrol et
        assert_eq!(pascal_file.get_line(0), Some(&"program TestProgram;".to_string()));
        // Son satırı kontrol et
        assert_eq!(pascal_file.get_line(3), Some(&"end.".to_string()));


        // Yeni bir satır ekle
        pascal_file.append_line("// Bu bir test satırıdır".to_string());

        // Dosyayı tekrar yaz
        pascal_file.write_to_file(test_file_path).expect("Dosya yazılamadı");

        // Yazılan dosyanın içeriğini kontrol et (isteğe bağlı)
        let read_back_content = fs::read_to_string(test_file_path).expect("Dosya tekrar okunamadı");
        let expected_content = "program TestProgram;\nbegin\n  WriteLn('Hello, Pascal!');\nend.\n// Bu bir test satırıdır\n";
        assert_eq!(read_back_content, expected_content);


        // Dosyayı sil
        fs::remove_file(test_file_path).expect("Dosya silinemedi");
    }


    #[test]
    fn test_pascal_file_not_found() {
        let mut pascal_file = PascalFile::new();
        let non_existent_file_path = "non_existent_pascal_file.pas";

        let result = pascal_file.read_from_file(non_existent_file_path);
        assert!(result.is_err()); // Hata döndürmeli
        match result.unwrap_err().kind() {
            ErrorKind::NotFound => {
                // Dosya bulunamadı hatası bekleniyor, test başarılı
            }
            _ => {
                panic!("Dosya bulunamadı hatası bekleniyordu, ancak farklı bir hata alındı");
            }
        }
    }
}