use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub struct LipsFile {
    pub definitions: Vec<String>,
    pub expressions: Vec<String>,
}

impl LipsFile {
    pub fn new() -> LipsFile {
        LipsFile {
            definitions: Vec::new(),
            expressions: Vec::new(),
        }
    }

    // İyileştirilmiş load_from_file fonksiyonu: Buffer yeniden kullanımı ve daha az String operasyonu.
    pub fn load_from_file_optimized<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut buffer = String::new(); // Buffer'ı yeniden kullan
        let mut in_definitions = true;

        loop {
            buffer.clear(); // Buffer'ı her satır için temizle
            let bytes_read = reader.read_line(&mut buffer)?;
            if bytes_read == 0 { // EOF kontrolü
                break;
            }

            let trimmed_line = buffer.trim(); // &str olarak trimlenmiş satır

            if trimmed_line.starts_with(";") || trimmed_line.is_empty() {
                continue; // Yorum satırlarını ve boş satırları atla
            }

            if trimmed_line == "(expressions)" {
                in_definitions = false;
                continue;
            }

            if in_definitions {
                self.definitions.push(trimmed_line.to_string()); // Gerekli yerlerde Stringe dönüştür
            } else {
                self.expressions.push(trimmed_line.to_string()); // Gerekli yerlerde Stringe dönüştür
            }
        }

        Ok(())
    }

    pub fn print_contents(&self) {
        println!("Definitions:");
        for definition in &self.definitions {
            println!("{}", definition);
        }

        println!("\nExpressions:");
        for expression in &self.expressions {
            println!("{}", expression);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_lips_file_optimized() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_optimized.lips");
        let mut file = File::create(&file_path).unwrap();

        writeln!(file, "; Bu bir LIPS dosyasıdır").unwrap();
        writeln!(file, "(define x 10)").unwrap();
        writeln!(file, "(define y 20)").unwrap();
        writeln!(file, "(expressions)").unwrap();
        writeln!(file, "(+ x y)").unwrap();
        writeln!(file, "(* x y)").unwrap();

        let mut lips_file = LipsFile::new();
        // Optimize edilmiş load fonksiyonunu kullan
        lips_file.load_from_file_optimized(&file_path).unwrap();

        assert_eq!(lips_file.definitions.len(), 2);
        assert_eq!(lips_file.expressions.len(), 2);

        assert_eq!(lips_file.definitions[0], "(define x 10)");
        assert_eq!(lips_file.expressions[1], "(* x y)");
    }
}