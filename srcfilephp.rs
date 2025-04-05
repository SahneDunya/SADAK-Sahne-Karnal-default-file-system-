use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub fn process_php_file_optimized(file_path: &Path) -> io::Result<()> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    // Daha performanslı satır işleme için döngüyü optimize ediyoruz.
    for line_result in reader.lines() {
        let line = line_result?; // Satırı bir değişkene alıyoruz, böylece tekrar tekrar okumak zorunda kalmayız.

        // 'line' değişkenini kullanarak kontrolleri yapıyoruz.
        if line.contains("<?php") {
            println!("PHP başlangıç etiketi bulundu: {}", line);
        } else if line.contains("echo") {
            println!("Echo ifadesi bulundu: {}", line);
        }
        // Daha gelişmiş işlemler için, PHP ayrıştırıcı kütüphaneleri kullanılabilir.
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_process_php_file_optimized() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_optimized.php"); // Test dosyası için farklı bir isim kullanıyoruz.
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "<?php").unwrap();
        writeln!(file, "echo 'Merhaba, Dünya!';").unwrap();
        writeln!(file, "?>").unwrap();

        // Optimize edilmiş fonksiyonu test ediyoruz.
        let result = process_php_file_optimized(&file_path);
        assert!(result.is_ok());

        // Burada, çıktıyı kontrol etmek için daha gelişmiş testler ekleyebilirsiniz.

        // Test dosyalarını temizlemek iyi bir uygulamadır.
        std::fs::remove_file(file_path).unwrap();
        temp_dir.close().unwrap(); // tempdir'i de kapatıyoruz.
    }
}