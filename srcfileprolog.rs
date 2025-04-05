use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub struct PrologParser {
    // Prolog dosyası içeriğini tutacak yapılar
    // Örneğin, terimler, kurallar, vb.
}

impl PrologParser {
    pub fn new() -> PrologParser {
        PrologParser {}
    }

    // İyileştirilmiş parse_file fonksiyonu: Daha az String oluşturma ve daha verimli satır okuma
    pub fn parse_file_optimized(&mut self, file_path: &str) -> io::Result<()> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut line_buffer = String::new(); // Satırları okumak için String buffer'ı yeniden kullan

        loop {
            line_buffer.clear(); // Buffer'ı her döngüde temizle
            let bytes_read = reader.read_line(&mut line_buffer)?;
            if bytes_read == 0 { // Dosya sonu kontrolü
                break; // Dosya sonuna gelindi, döngüden çık
            }

            let trimmed_line = line_buffer.trim(); // Trimmed satırı işle

            if trimmed_line.is_empty() || trimmed_line.starts_with('%') {
                continue; // Boş veya yorum satırı, atla
            }

            self.parse_line_optimized(trimmed_line); // İyileştirilmiş satır ayrıştırma fonksiyonunu kullan
        }

        Ok(())
    }

    // İyileştirilmiş parse_line fonksiyonu: Daha az String manipülasyonu
    fn parse_line_optimized(&mut self, line: &str) {
        if line.ends_with('.') {
            println!("Terim: {}", line);
            // Terim ayrıştırma mantığı burada
        } else if line.contains(":-") {
            println!("Kural: {}", line);
            // Kural ayrıştırma mantığı burada
        } else {
            println!("Bilinmeyen: {}", line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_prolog_parser_optimized() {
        let mut file = fs::File::create("test_optimized.pl").unwrap();
        writeln!(file, "parent(john, mary).").unwrap();
        writeln!(file, "parent(john, peter).").unwrap();
        writeln!(file, "ancestor(X, Y) :- parent(X, Y).").unwrap();
        writeln!(file, "ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).").unwrap();

        let mut parser = PrologParser::new();
        parser.parse_file_optimized("test_optimized.pl").unwrap(); // İyileştirilmiş fonksiyonu test et

        fs::remove_file("test_optimized.pl").unwrap();
    }
}