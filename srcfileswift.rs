use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Serialize, Deserialize, Debug)]
struct SwiftData {
    items: Vec<std::collections::HashMap<String, String>>,
}

pub fn write_swift_data(filename: &str, data: &SwiftData) -> Result<(), Box<dyn std::error::Error>> {
    // Dosya oluşturma işlemi için File::create kullanılıyor.
    // Bu işlem başarısız olabilir (örn. izin sorunları), bu yüzden '?' operatörü ile hata yayılımı yapılıyor.
    let file = File::create(filename)?;

    // BufWriter, yazma işlemlerini tamponlayarak performansı artırır.
    // Her küçük yazma işleminde dosya sistemine doğrudan erişmek yerine,
    // veriler önce bir tamponda toplanır ve sonra toplu olarak yazılır.
    let writer = BufWriter::new(file);

    // serde_json::to_writer fonksiyonu, SwiftData yapısını JSON formatına serileştirir
    // ve BufWriter aracılığıyla dosyaya yazar.
    // Bu fonksiyon da hata döndürebilir (örn. serileştirme hatası, I/O hatası),
    // bu yüzden '?' operatörü ile hata yayılımı yapılıyor.
    serde_json::to_writer(writer, data)?;

    // İşlem başarıyla tamamlandığında Ok(()) döndürülür.
    Ok(())
}

pub fn read_swift_data(filename: &str) -> Result<SwiftData, Box<dyn std::error::Error>> {
    // Dosya açma işlemi için File::open kullanılıyor.
    // Bu işlem başarısız olabilir (örn. dosya bulunamaz, izin sorunları), bu yüzden '?' operatörü ile hata yayılımı yapılıyor.
    let file = File::open(filename)?;

    // BufReader, okuma işlemlerini tamponlayarak performansı artırır.
    // Dosyadan her küçük okuma işleminde diskten doğrudan veri almak yerine,
    // veriler önce bir tampona okunur ve sonra tampondan okunur.
    let reader = BufReader::new(file);

    // serde_json::from_reader fonksiyonu, BufReader'dan JSON verisini okur ve
    // SwiftData yapısına deserileştirir.
    // Bu fonksiyon da hata döndürebilir (örn. deserileştirme hatası, I/O hatası),
    // bu yüzden '?' operatörü ile hata yayılımı yapılıyor.
    let data = serde_json::from_reader(reader)?;

    // Başarıyla okunan ve deserileştirilen veri döndürülür.
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_swift_data_rw() {
        let filename = "test.swiftdata";
        let mut item1 = HashMap::new();
        item1.insert("name".to_string(), "Example 1".to_string());
        item1.insert("value".to_string(), "123".to_string());

        let mut item2 = HashMap::new();
        item2.insert("name".to_string(), "Example 2".to_string());
        item2.insert("value".to_string(), "456".to_string());

        let data = SwiftData {
            items: vec![item1, item2],
        };

        // Veriyi dosyaya yazma işlemi. unwrap() ile hata durumunda testin durdurulması sağlanır.
        write_swift_data(filename, &data).unwrap();

        // Dosyadan veriyi okuma işlemi. unwrap() ile hata durumunda testin durdurulması sağlanır.
        let read_data = read_swift_data(filename).unwrap();

        // Yazılan ve okunan verinin aynı olup olmadığını kontrol eden doğrulama.
        assert_eq!(data.items, read_data.items);

        // Test dosyası oluşturulduysa, test sonrası silinebilir (isteğe bağlı, test ortamını temiz tutmak için).
        // std::fs::remove_file(filename).unwrap(); // Dosyayı silme satırı, test sonrası temizlik için eklenebilir.
    }
}