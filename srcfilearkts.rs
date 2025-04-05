use std::fs;
use std::io;
use std::path::Path;

pub struct FileArkTS {
    pub content: String,
}

impl FileArkTS {
    // Yeni bir FileArkTS örneği oluşturur.
    pub fn new() -> FileArkTS {
        FileArkTS {
            content: String::new(),
        }
    }

    // Belirtilen yoldaki ArkTS dosyasını okur.
    pub fn read_from_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.content = fs::read_to_string(path)?;
        Ok(())
    }

    // ArkTS içeriğini belirtilen yola yazar.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        fs::write(path, &self.content)?;
        Ok(())
    }

    // ArkTS içeriğine metin ekler.
    pub fn append_content(&mut self, text: &str) {
        self.content.push_str(text);
    }

    // ArkTS içeriğini temizler.
    pub fn clear_content(&mut self) {
        self.content.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_read_write() {
        let mut file_arkts = FileArkTS::new();
        let test_file_path = "test.arkts";

        // Test dosyası oluştur ve içine yaz.
        let mut test_file = fs::File::create(test_file_path).unwrap();
        writeln!(test_file, "test içeriği").unwrap();

        // Dosyayı oku.
        file_arkts.read_from_file(test_file_path).unwrap();
        assert_eq!(file_arkts.content, "test içeriği\n");

        // İçeriği değiştir ve dosyaya yaz.
        file_arkts.append_content(" ek içerik");
        file_arkts.write_to_file(test_file_path).unwrap();

        // Dosyayı tekrar oku ve kontrol et.
        let mut file_arkts2 = FileArkTS::new();
        file_arkts2.read_from_file(test_file_path).unwrap();
        assert_eq!(file_arkts2.content, "test içeriği\nek içerik");

        // Test dosyasını sil.
        fs::remove_file(test_file_path).unwrap();
    }
}