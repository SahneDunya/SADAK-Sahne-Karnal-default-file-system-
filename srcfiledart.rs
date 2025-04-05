use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub struct DartFile {
    pub lines: Vec<String>,
}

impl DartFile {
    pub fn new(path: &Path) -> io::Result<Self> {
        // Dosyayı açma işlemi oldukça hızlıdır. Rust, dosya işlemlerinde sistem çağrılarını etkili bir şekilde kullanır.
        let file = File::open(path)?;
        // BufReader, dosyayı arabelleğe alarak okuma performansını artırır. Bu, özellikle büyük dosyalar için önemlidir.
        let reader = BufReader::new(file);

        // `lines()` metodu, satır satır okuma için verimli bir iteratör sağlar.
        // `collect::<Result<_, _>>()` ile satırları doğrudan bir Vec<String> içinde topluyoruz.
        // Bu işlem, satırları okurken bellek yönetimini Rust'ın güvenli ve hızlı yapısına bırakır.
        let lines = reader.lines().collect::<Result<_, _>>()?;

        Ok(DartFile { lines })
    }

    pub fn print_lines(&self) {
        // Satırları basit bir for döngüsü ile yazdırmak oldukça etkilidir.
        // println! makrosu da Rust'ın formatlama ve çıktı mekanizmalarını verimli kullanır.
        for line in &self.lines {
            println!("{}", line);
        }
    }

    // İhtiyacınıza göre Dart dosyalarını işlemek için ek yöntemler ekleyebilirsiniz.
    // Örneğin, sözdizimi analizi, kod yürütme vb.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_read_dart_file() {
        // Tempdir kullanımı testler için geçici dosyalar oluşturmak ve temizlemek adına güvenli ve pratiktir.
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.dart");
        // Dosya oluşturma ve yazma işlemleri de Rust'ın standart kütüphanesi ile verimli bir şekilde yapılır.
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "void main() {{").unwrap();
        writeln!(file, "  print('Hello, Dart!');").unwrap();
        writeln!(file, "}}").unwrap();

        // DartFile::new fonksiyonu dosya okuma ve satırları işlemede zaten optimize edilmiştir.
        let dart_file = DartFile::new(&file_path).unwrap();
        // `assert_eq!` makrosu testlerde kontrol için hızlı ve etkilidir.
        assert_eq!(dart_file.lines.len(), 3);
        assert_eq!(dart_file.lines[1], "  print('Hello, Dart!');");

        // Tempdir otomatik olarak kapanır ve geçici dosyalar silinir.
        dir.close().unwrap();
    }
}