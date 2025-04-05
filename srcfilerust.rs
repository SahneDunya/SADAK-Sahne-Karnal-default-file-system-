use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

// Bu fonksiyon, verilen dosya yolundaki bir Rust dosyasını işler.
pub fn process_rust_file(filepath: &str) -> io::Result<()> {
    // Verilen dosya yolunu bir Path nesnesine dönüştürür.
    let path = Path::new(filepath);
    // Belirtilen dosya yolunda dosyayı açar. `File::open` hata döndürebilir, bu yüzden `?` operatörü ile hatayı yukarıya (çağıran fonksiyona) geçiriyoruz.
    let file = File::open(path)?;
    // Dosyayı `BufReader` ile sarmalayarak daha verimli okunmasını sağlar. `BufReader` tamponlanmış (buffered) okuma yaparak performansı artırır.
    let reader = io::BufReader::new(file);

    // `reader.lines()` metodu, dosyanın satırları üzerinde yineleme (iterasyon) yapmamızı sağlar. Her satır `io::Result<String>` türünde döner.
    for line in reader.lines() {
        // Her satırı alırken `line?` ile potansiyel IO hatalarını kontrol ediyoruz. Eğer bir hata oluşursa, fonksiyon hata döndürür. Aksi takdirde, satır `line` değişkenine `String` olarak atanır.
        let line = line?;
        // `trim()` metodu ile satırın başındaki ve sonundaki boşlukları temizliyoruz. `starts_with("fn ")` metodu, satırın "fn " (fonksiyon tanımı başlangıcı) ile başlayıp başlamadığını kontrol eder.
        if line.trim().starts_with("fn ") {
            // Eğer satır bir fonksiyon tanımı ise, bunu konsola yazdırıyoruz. `println!` makrosu ile formatlanmış bir şekilde çıktı veriyoruz. `line.trim()` tekrar kullanılarak, çıktıda da boşlukların olmaması sağlanır.
            println!("Fonksiyon tanımı bulundu: {}", line.trim());
        }
    }
    // Fonksiyon başarıyla tamamlandığında `Ok(())` değerini döndürür. `()` boş tuple anlamına gelir ve başarılı bir sonucu fakat değer döndürmeyen bir işlemi ifade eder.
    Ok(())
}

// `#[cfg(test)]` özniteliği, bu modülün sadece testler çalıştırılırken derlenmesini sağlar. Bu, test kodunu ana koddan ayırmak için iyi bir uygulamadır.
#[cfg(test)]
mod tests {
    // Üst modüldeki (`super`) fonksiyonları ve yapıları bu modüle getirir. `process_rust_file` fonksiyonunu test modülünde kullanabilmek için gereklidir.
    use super::*;
    // Standart kütüphaneden `fs` (dosya sistemi) modülünü ve `Write` trait'ini içeri aktarır. Test dosyasını oluşturmak ve yazmak için kullanılır.
    use std::fs;
    use std::io::Write;

    // `#[test]` özniteliği, bu fonksiyonun bir test fonksiyonu olduğunu belirtir. Test çalıştırıcı bu fonksiyonu otomatik olarak bulup çalıştırır.
    #[test]
    fn test_process_rust_file() {
        // Test için geçici bir .rs dosyası oluştur. `fs::File::create("test.rs")` dosyayı oluşturur ve `File` nesnesini döndürür. `unwrap()` metodu, `Result` türündeki dönüş değerini açar. Eğer dosya oluşturma başarısız olursa, `unwrap()` panikleyerek testin başarısız olmasına neden olur.
        let mut file = fs::File::create("test.rs").unwrap();
        // Oluşturulan dosyaya bazı satırlar yazar. `writeln!` makrosu, verilen metni dosyaya yazar ve otomatik olarak bir yeni satır karakteri ekler. `unwrap()` yine hata kontrolü için kullanılır.
        writeln!(file, "// Bu bir yorum").unwrap();
        writeln!(file, "fn test_function() {{").unwrap();
        writeln!(file, "    println!(\"Merhaba, Dünya!\");").unwrap();
        writeln!(file, "}}").unwrap();

        // Oluşturulan geçici dosyayı `process_rust_file` fonksiyonu ile işle. `process_rust_file("test.rs")` fonksiyonu çağırılır ve dosya yolu olarak "test.rs" verilir. `unwrap()` ile hata kontrolü yapılır. Eğer dosya işleme sırasında bir hata oluşursa, test başarısız olur.
        process_rust_file("test.rs").unwrap();

        // Test bittikten sonra geçici dosyayı sil. `fs::remove_file("test.rs")` dosyayı siler. `unwrap()` ile hata kontrolü yapılır. Dosya silme başarısız olursa test başarısız olur.
        fs::remove_file("test.rs").unwrap();
    }
}