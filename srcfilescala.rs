use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};

pub struct ScalaFile {
    pub data: std::collections::HashMap<String, String>,
}

impl ScalaFile {
    pub fn new() -> ScalaFile {
        ScalaFile {
            data: std::collections::HashMap::new(),
        }
    }

    // İyileştirilmiş load fonksiyonu: Daha az string ayırma için read_line kullanılıyor.
    pub fn load_optimized(filename: &str) -> Result<ScalaFile> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut scala_file = ScalaFile::new();
        let mut buffer = String::new(); // String buffer'ı yeniden kullan

        loop {
            buffer.clear(); // Buffer'ı her satır için temizle
            let bytes_read = reader.read_line(&mut buffer)?;
            if bytes_read == 0 { // EOF kontrolü
                break;
            }

            let line = &buffer; // &String olarak satıra referans al

            let parts: Vec<&str> = line.splitn(2, '=').collect();

            if parts.len() == 2 {
                let key = parts[0].trim().to_string();
                let value = parts[1].trim().to_string();
                scala_file.data.insert(key, value);
            }
        }

        Ok(scala_file)
    }

    pub fn save(&self, filename: &str) -> Result<()> {
        let mut file = File::create(filename)?;

        for (key, value) in &self.data {
            writeln!(file, "{} = {}", key, value)?;
        }

        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scala_file_optimized() {
        let mut scala_file = ScalaFile::new();
        scala_file.set("name".to_string(), "John Doe".to_string());
        scala_file.set("age".to_string(), "30".to_string());

        let filename = "test_optimized.scala";
        scala_file.save(filename).unwrap();

        // Optimize edilmiş load fonksiyonunu kullan
        let loaded_file = ScalaFile::load_optimized(filename).unwrap();
        assert_eq!(loaded_file.get("name"), Some(&"John Doe".to_string()));
        assert_eq!(loaded_file.get("age"), Some(&"30".to_string()));

        fs::remove_file(filename).unwrap();
    }
}