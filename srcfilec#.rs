use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind, Result};
use std::collections::HashMap;

pub fn parse_config_file_optimized(filepath: &str) -> Result<HashMap<String, String>, Error> {
    let file = File::open(filepath)?;
    let reader = BufReader::new(file);
    let mut config = HashMap::new();
    let mut buffer = String::new(); // Buffer'ı yeniden kullan

    loop {
        buffer.clear(); // Buffer'ı her döngüde temizle
        let bytes_read = reader.read_line(&mut buffer)?;

        if bytes_read == 0 { // EOF kontrolü
            break;
        }

        let trimmed_line = buffer.trim();

        // Boş satırları ve yorumları atla
        if trimmed_line.is_empty() || trimmed_line.starts_with("#") {
            continue;
        }

        // Anahtar=değer çiftini ayır
        if let Some(equal_sign_pos) = trimmed_line.find('=') {
            let key = trimmed_line[..equal_sign_pos].trim().to_string();
            let value = trimmed_line[equal_sign_pos + 1..].trim().to_string();
            config.insert(key, value);
        } else {
            return Err(Error::new(ErrorKind::InvalidData, format!("Geçersiz satır: {}", buffer)));
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_parse_config_file_optimized() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_optimized.config");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "anahtar1=deger1").unwrap();
        writeln!(file, "anahtar2=deger2").unwrap();

        let config = parse_config_file_optimized(file_path.to_str().unwrap()).unwrap();
        assert_eq!(config.get("anahtar1"), Some(&"deger1".to_string()));
        assert_eq!(config.get("anahtar2"), Some(&"deger2".to_string()));
    }
}