use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub struct PerlFile {
    pub path: String,
    pub lines: Vec<String>,
}

impl PerlFile {
    // İyileştirilmiş new fonksiyonu: Daha az string ayırma için `read_line` kullanılıyor.
    pub fn new_optimized(path: &str) -> io::Result<PerlFile> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        let mut buffer = String::new(); // String buffer'ı yeniden kullan

        loop {
            buffer.clear(); // Buffer'ı her satır için temizle
            let bytes_read = reader.read_line(&mut buffer)?;
            if bytes_read == 0 { // EOF kontrolü
                break;
            }
            lines.push(buffer.trim_end().to_string()); // Satırı `lines` vektörüne ekle
        }

        Ok(PerlFile {
            path: path.to_string(),
            lines,
        })
    }

    pub fn print_lines(&self) {
        for line in &self.lines {
            println!("{}", line);
        }
    }

    // İhtiyacınıza göre ek fonksiyonlar ekleyebilirsiniz.
    // Örneğin, Perl kodunu ayrıştırma, değişkenleri çıkarma vb.
}

// Örnek kullanım (iyileştirilmiş fonksiyon ile)
fn main() -> io::Result<()> {
    // "ornek.pl" dosyası oluşturulmalı ve içeriği doldurulmalıdır.
    let perl_file = PerlFile::new_optimized("ornek.pl")?;
    perl_file.print_lines();
    Ok(())
}