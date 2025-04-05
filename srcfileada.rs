use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub struct AdaFile {
    pub name: String,
    pub lines: Vec<String>,
}

impl AdaFile {
    pub fn new(name: String, lines: Vec<String>) -> AdaFile {
        AdaFile { name, lines }
    }

    // Optimize edilmiş read_from_file fonksiyonu: read_line kullanılarak string ayırmaları azaltılıyor.
    pub fn read_from_file_optimized(path: &Path) -> io::Result<AdaFile> {
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
            lines.push(buffer.trim_end().to_string()); // Satırı buffer'dan kopyala ve lines'a ekle
        }

        let name = path.file_name().unwrap().to_string_lossy().to_string();
        Ok(AdaFile::new(name, lines))
    }

    pub fn print_contents(&self) {
        println!("Ada File: {}", self.name);
        for line in &self.lines {
            println!("{}", line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::fs;

    #[test]
    fn test_ada_file_optimized() {
        let path = Path::new("test_ada_file_optimized.txt");
        let mut file = File::create(path).unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        // Optimize edilmiş fonksiyonu kullan
        let ada_file_optimized = AdaFile::read_from_file_optimized(path).unwrap();

        assert_eq!(ada_file_optimized.name, "test_ada_file_optimized.txt");
        assert_eq!(ada_file_optimized.lines.len(), 3);
        assert_eq!(ada_file_optimized.lines[0], "Line 1");
        assert_eq!(ada_file_optimized.lines[1], "Line 2");
        assert_eq!(ada_file_optimized.lines[2], "Line 3");

        fs::remove_file(path).unwrap();
    }
}