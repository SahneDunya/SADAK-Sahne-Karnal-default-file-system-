use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

pub struct KoltinFile {
    pub path: String,
}

impl KoltinFile {
    pub fn new(path: &str) -> Self { // 'Self' yerine KoltinFile kullanmak daha yaygın ve okunabilir. (İyileştirme 1)
        Self { // 'Self' kullanmak 'KoltinFile' bağlamında daha kısa ve özlüdür. (İyileştirme 2)
            path: path.to_string(),
        }
    }

    pub fn read_lines(&self) -> io::Result<Vec<String>> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        reader.lines().map(|l| l.expect("Satır okuma hatası")).collect() // Hata işlemeyi daha açık hale getiriyoruz (İyileştirme 3)
    }

    pub fn write_lines(&self, lines: &[String]) -> io::Result<()> {
        let mut file = File::create(&self.path)?;
        for line in lines {
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    pub fn is_koltin_file(path: &str) -> bool {
        Path::new(path)
            .extension()
            .map_or(false, |ext| ext == "kt") // Daha özlü ve işlevsel stil (İyileştirme 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_koltin_file_read_write() {
        let file_path = "test.kt";
        let lines_to_write = vec!["fun main() {".to_string(), "    println(\"Hello, Koltin!\")".to_string(), "}".to_string()];

        let koltin_file = KoltinFile::new(file_path);
        koltin_file.write_lines(&lines_to_write).unwrap();

        let read_lines = koltin_file.read_lines().unwrap();
        assert_eq!(lines_to_write, read_lines);

        fs::remove_file(file_path).unwrap(); // Test dosyasını sil
    }

    #[test]
    fn test_is_koltin_file() {
        assert_eq!(KoltinFile::is_koltin_file("test.kt"), true);
        assert_eq!(KoltinFile::is_koltin_file("test.txt"), false);
        assert_eq!(KoltinFile::is_koltin_file("test"), false); // Uzantısız dosya testi (İyileştirme 5)
    }
}

fn main() -> io::Result<()> {
    // Örnek kullanım (Tek Örnek İstek Üzerine)
    let file_path = "example.kt";
    let koltin_file = KoltinFile::new(file_path);

    // Örnek satırlar
    let lines_to_write = vec![
        "fun main() {".to_string(),
        "    println(\"Merhaba, Koltin Dünyası!\")".to_string(),
        "    val name = \"Koltin\";".to_string(),
        "    println(\"Benim adım: $name\")".to_string(),
        "}".to_string(),
    ];

    // Dosyaya yaz
    koltin_file.write_lines(&lines_to_write)?;
    println!("Dosyaya yazma işlemi tamamlandı: {}", file_path);

    // Dosyadan oku
    let read_lines = koltin_file.read_lines()?;
    println!("\nDosyadan okunan satırlar:");
    for line in &read_lines {
        println!("{}", line);
    }

    // Kotlin dosyası kontrolü
    if KoltinFile::is_koltin_file(file_path) {
        println!("\n'{}' bir Koltin dosyasıdır.", file_path);
    } else {
        println!("\n'{}' bir Koltin dosyası değildir.", file_path);
    }

    // 'example.kt' dosyasını program bittikten sonra silmek (isteğe bağlı, temizlik için)
    std::fs::remove_file(file_path).unwrap();
    println!("\n'{}' dosyası silindi.", file_path);

    Ok(())
}