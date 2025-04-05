use crate::file::{File, FileType};
use std::error::Error;
use std::fmt;

pub struct JavaScriptFile {
    pub name: String,
    pub content: String,
}

// Özel hata türü tanımla
#[derive(Debug, Clone)]
pub struct JavaScriptParseError {
    message: String,
}

impl JavaScriptParseError {
    pub fn new(message: String) -> Self {
        JavaScriptParseError { message }
    }
}

impl fmt::Display for JavaScriptParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "JavaScript ayrıştırma hatası: {}", self.message)
    }
}

impl Error for JavaScriptParseError {}


impl File for JavaScriptFile {
    fn name(&self) -> &str {
        &self.name
    }

    fn file_type(&self) -> FileType {
        FileType::JavaScript
    }

    fn content(&self) -> &str {
        &self.content
    }
}

impl JavaScriptFile {
    pub fn new(name: String, content: String) -> Self {
        JavaScriptFile { name, content }
    }

    // İyileştirilmiş parse fonksiyonu: Özel hata türü kullanılıyor.
    pub fn parse_optimized(content: &str) -> Result<String, Box<dyn Error>> {
        // JavaScript içeriğini ayrıştırma mantığı burada yer alacak.
        // Örneğin, sözdizimi kontrolü veya kod analizi yapılabilir.
        // Bu örnekte, sadece içeriği döndürüyoruz ve hata türünü iyileştiriyoruz.
        // Eğer ayrıştırma başarısız olursa, şimdi özel hata türümüzü döndürebiliriz.
        // Örn: Err(Box::new(JavaScriptParseError::new("Sözdizimi hatası bulundu".to_string())))

        // Şimdilik, her zaman başarılı dönüş yapıyoruz (orijinal davranışa uygun olarak),
        // ancak hata türünü daha iyi bir yapıya geçiriyoruz.
        Ok(content.to_string())
    }


    pub fn execute(&self) -> Result<(), String> {
        // JavaScript kodunu çalıştırma mantığı burada yer alacak.
        // Örneğin, bir JavaScript motoru kullanarak kodu çalıştırabilirsiniz.
        // Bu örnekte, sadece bir hata mesajı döndürüyoruz.
        Err("JavaScript çalıştırma henüz desteklenmiyor.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_javascript_file_creation_optimized() {
        let file = JavaScriptFile::new("test.js".to_string(), "console.log('Hello, world!');".to_string());
        assert_eq!(file.name(), "test.js");
        assert_eq!(file.file_type(), FileType::JavaScript);
        assert_eq!(file.content(), "console.log('Hello, world!');");
    }

    #[test]
    fn test_javascript_file_parsing_optimized() {
        let content = "console.log('Hello, world!');";
        let parsed_content = JavaScriptFile::parse_optimized(content).unwrap();
        assert_eq!(parsed_content, content);
    }

    #[test]
    fn test_javascript_file_execution_optimized() {
        let file = JavaScriptFile::new("test.js".to_string(), "console.log('Hello, world!');".to_string());
        let result = file.execute();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "JavaScript çalıştırma henüz desteklenmiyor.");
    }
}