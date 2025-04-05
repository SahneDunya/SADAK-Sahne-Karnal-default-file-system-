use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Belirtilen dosya yolundaki bir Elixir dosyasını okur ve satırları bir String vektörü olarak döndürür.
/// Reads an Elixir file from the given file path and returns the lines as a vector of Strings.
///
/// # Arguments
///
/// * `filepath` - Okunacak Elixir dosyasının dosya yolu.
///   `filepath` - The file path of the Elixir file to be read.
///
/// # Returns
///
/// * `io::Result<Vec<String>>` - Eğer dosya başarıyla okunursa, satırları içeren bir Result döner.
///                                Okuma sırasında bir hata oluşursa, bir hata Result döner.
///   `io::Result<Vec<String>>` - A Result containing a vector of lines if the file is read successfully.
///                                Returns an error Result if an error occurs during reading.
pub fn read_elixir_file(filepath: &str) -> io::Result<Vec<String>> {
    // Dosya yolunu Path nesnesine dönüştürür.
    // Converts the file path to a Path object.
    let path = Path::new(filepath);

    // Dosyayı açar. Hata durumunda fonksiyonu erken döndürür.
    // Opens the file. Returns early if an error occurs.
    let file = File::open(path)?;

    // Dosyayı arabelleğe alınmış okuyucu ile sarar, bu da daha verimli okuma sağlar.
    // Wraps the file in a buffered reader for more efficient reading.
    let reader = BufReader::new(file);

    // Satırları depolamak için bir vektör oluşturur.
    // Creates a vector to store the lines.
    let mut lines = Vec::new();

    // Okuyucudaki satırları iterasyon yapar.
    // Iterates over the lines in the reader.
    for line in reader.lines() {
        // Her satırı okur. Hata durumunda fonksiyonu erken döndürür.
        // Reads each line. Returns early if an error occurs.
        let line = line?;
        // Elixir dosyasındaki satırları yorumlama veya işleme mantığı burada yer alabilir.
        // Örneğin, satırları ayrıştırıp Elixir ifadelerini değerlendirebilirsiniz.
        // Bu örnekte, satırları olduğu gibi bir vektöre ekliyoruz.
        // Logic for interpreting or processing lines in Elixir file may be placed here.
        // For example, you could parse lines and evaluate Elixir expressions.
        // In this example, we are adding lines to the vector as is.
        lines.push(line);
    }

    // Başarıyla okunan satırları içeren vektörü döndürür.
    // Returns the vector containing the successfully read lines.
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_read_elixir_file() {
        // Geçici bir dizin oluşturur.
        // Creates a temporary directory.
        let dir = tempdir().unwrap();
        // Geçici dizin içinde bir test dosyası için bir yol oluşturur.
        // Creates a path for a test file within the temporary directory.
        let file_path = dir.path().join("test.exs");
        // Test dosyasını oluşturur.
        // Creates the test file.
        let mut file = File::create(&file_path).unwrap();
        // Test dosyasına Elixir kod satırları yazar.
        // Writes Elixir code lines to the test file.
        writeln!(file, "IO.puts(\"Merhaba, Elixir!\")").unwrap();
        writeln!(file, "1 + 2 + 3").unwrap();

        // `read_elixir_file` fonksiyonunu test dosyası yolu ile çağırır.
        // Calls the `read_elixir_file` function with the test file path.
        let result = read_elixir_file(file_path.to_str().unwrap()).unwrap();
        // Sonuç vektörünün beklenen uzunlukta olup olmadığını kontrol eder.
        // Checks if the result vector has the expected length.
        assert_eq!(result.len(), 2);
        // İlk satırın beklenen değerde olup olmadığını kontrol eder.
        // Checks if the first line is the expected value.
        assert_eq!(result[0], "IO.puts(\"Merhaba, Elixir!\")");
        // İkinci satırın beklenen değerde olup olmadığını kontrol eder.
        // Checks if the second line is the expected value.
        assert_eq!(result[1], "1 + 2 + 3");

        // Geçici dizini ve içindeki dosyaları temizler.
        // Cleans up the temporary directory and files within it.
        dir.close().unwrap();
    }
}