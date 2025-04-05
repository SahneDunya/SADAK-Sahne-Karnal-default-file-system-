use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub struct JavaProperties {
    properties: HashMap<String, String>,
}

impl JavaProperties {
    pub fn new() -> Self {
        JavaProperties {
            properties: HashMap::new(),
        }
    }

    // İyileştirilmiş load_from_file fonksiyonu: Daha az string ayırma ve hatayı erken kontrol etme
    pub fn load_from_file_optimized<P: AsRef<Path>>(&mut self, path: P) -> Result<(), std::io::Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut buffer = String::new(); // Satır okuma için buffer'ı yeniden kullan

        loop {
            buffer.clear(); // Her satır için buffer'ı temizle
            let bytes_read = reader.read_line(&mut buffer)?;

            if bytes_read == 0 { // EOF kontrolü
                break;
            }

            let line = buffer.trim(); // Satırı sadece bir kez trimle

            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some(equal_pos) = line.find('=') {
                // Satır dilimlerinden string oluşturmaktan kaçının, referansları kullanın
                let key = line[..equal_pos].trim().to_string();
                let value = line[equal_pos + 1..].trim().to_string();
                self.properties.insert(key, value);
            }
        }

        Ok(())
    }


    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let mut file = File::create(path)?;

        for (key, value) in &self.properties {
            writeln!(file, "{} = {}", key, value)?;
        }

        Ok(())
    }

    pub fn get_property(&self, key: &str) -> Option<&String> {
        self.properties.get(key)
    }

    pub fn set_property(&mut self, key: String, value: String) {
        self.properties.insert(key, value);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::fs;

    #[test]
    fn test_java_properties_optimized() {
        let mut props = JavaProperties::new();
        let path = "test_optimized.properties";

        // Test dosyası oluştur
        let mut test_file = fs::File::create(path).unwrap();
        writeln!(test_file, "key1 = value1").unwrap();
        writeln!(test_file, "# Bu bir yorum satırı").unwrap();
        writeln!(test_file, "key2=value2").unwrap();
        writeln!(test_file, "  key3 =   value3  ").unwrap();
        drop(test_file); // Dosyayı kapat

        props.load_from_file_optimized(path).unwrap();

        assert_eq!(props.get_property("key1"), Some(&"value1".to_string()));
        assert_eq!(props.get_property("key2"), Some(&"value2".to_string()));
        assert_eq!(props.get_property("key3"), Some(&"value3".to_string()));
        assert_eq!(props.get_property("yorum"), None); // Yorum satırı kontrolü

        // Kaydetme testini de kontrol edelim
        let save_path = "test_save_optimized.properties";
        props.save_to_file(save_path).unwrap();

        let mut loaded_props = JavaProperties::new();
        loaded_props.load_from_file_optimized(save_path).unwrap();
        assert_eq!(loaded_props.properties, props.properties);


        fs::remove_file(path).unwrap();
        fs::remove_file(save_path).unwrap();
    }
}