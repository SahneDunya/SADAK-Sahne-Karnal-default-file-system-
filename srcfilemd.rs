#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz

// Gerekli Sahne64 modüllerini içeri aktar
#[cfg(not(feature = "std"))]
use crate::{
    fs,
    memory,
    process,
    sync,
    kernel,
    SahneError,
    arch,
};


pub struct MdFile {
    pub path: String,
    pub content: Vec<String>,
}

impl MdFile {
    pub fn new(path: &str) -> Result<MdFile, super::SahneError> {
        let fd = super::fs::open(path, super::fs::O_RDONLY)?;
        let mut content = Vec::new();
        let mut buffer = [0u8; 128]; // Okuma için bir tampon oluşturuyoruz
        let mut current_line = String::new();

        loop {
            let bytes_read = super::fs::read(fd, &mut buffer)?;
            if bytes_read == 0 {
                // Dosyanın sonuna gelindi
                if !current_line.is_empty() {
                    content.push(current_line);
                }
                break;
            }

            for &byte in &buffer[..bytes_read] {
                if byte == b'\n' {
                    content.push(current_line);
                    current_line = String::new();
                } else {
                    current_line.push(byte as char);
                }
            }
        }

        super::fs::close(fd)?;

        Ok(MdFile {
            path: path.to_string(),
            content,
        })
    }

    pub fn print_content(&self) {
        for line in &self.content {
            // Burada doğrudan println! kullanamayız (no_std).
            // Sahne64'e özgü bir çıktı mekanizması (örneğin, bir sistem çağrısı) kullanmamız gerekir.
            // Şimdilik bu kısmı yorum olarak bırakıyorum.
            // println!("{}", line);
        }
    }

    pub fn word_count(&self) -> usize {
        self.content
            .iter()
            .map(|line| line.split_whitespace().count())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test ortamında standart kütüphaneyi kullanabiliriz (feature = "std" varsayımıyla)
    #[test]
    fn test_md_file_new() {
        let path = "test.md";
        let mut file = std::fs::File::create(path).unwrap();
        std::io::Write::write_all(&mut file, b"Hello, world!\n").unwrap();

        let md_file = MdFile::new(path).unwrap();
        assert_eq!(md_file.path, path);
        assert_eq!(md_file.content, vec!["Hello, world!".to_string()]);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_md_file_print_content() {
        let path = "test.md";
        let mut file = std::fs::File::create(path).unwrap();
        std::io::Write::write_all(&mut file, b"Hello, world!\n").unwrap();

        let md_file = MdFile::new(path).unwrap();
        // md_file.print_content(); // Konsola yazdırma işlemi Sahne64 ortamına özel olmalı

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_md_file_word_count() {
        let path = "test.md";
        let mut file = std::fs::File::create(path).unwrap();
        std::io::Write::write_all(&mut file, b"Hello, world!\n").unwrap();

        let md_file = MdFile::new(path).unwrap();
        assert_eq!(md_file.word_count(), 2);

        std::fs::remove_file(path).unwrap();
    }
}