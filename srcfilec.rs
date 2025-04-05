use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

// C dosya formatının temel yapısını temsil eden örnek bir yapı
#[derive(Debug)]
struct CFile {
    header: CHeader,
    data: Vec<u8>,
}

// C dosya başlığını temsil eden örnek bir yapı
#[derive(Debug)]
struct CHeader {
    magic_number: u32,
    version: u16,
    data_size: u32,
}

impl CFile {
    // Yeni bir CFile örneği oluşturur
    fn new(header: CHeader, data: Vec<u8>) -> Self {
        CFile { header, data }
    }

    // İyileştirilmiş dosya okuma fonksiyonu: Daha açıklayıcı değişken isimleri ve yorumlar eklendi.
    fn read_from_file_optimized(path: &Path) -> io::Result<Self> {
        // Dosyayı aç
        let mut file = File::open(path)?;

        // Başlığı dosyadan oku
        let header = CHeader::read_from_file_optimized(&mut file)?;

        // Başlıkta belirtilen veri boyutuna göre bir vektör oluştur
        let data_size = header.data_size as usize;
        let mut data = vec![0; data_size];

        // Tam olarak 'data_size' boyutunda veri oku. Eğer dosya beklenenden kısaysa hata döndürür.
        file.read_exact(&mut data)?;

        // Oluşturulan CFile örneğini döndür
        Ok(CFile::new(header, data))
    }

    // CFile'ı dosyaya yazar
    fn write_to_file(&self, path: &Path) -> io::Result<()> {
        let mut file = File::create(path)?;
        self.header.write_to_file(&mut file)?;
        file.write_all(&self.data)?;
        Ok(())
    }
}

impl CHeader {
    // İyileştirilmiş başlık okuma fonksiyonu: Daha açıklayıcı değişken isimleri ve yorumlar eklendi.
    fn read_from_file_optimized(file: &mut File) -> io::Result<Self> {
        // Magic number için byte dizisi (4 byte - u32)
        let mut magic_number_bytes = [0; 4];
        // Tam olarak 4 byte oku. Eğer okuma başarısız olursa (dosya sonu vb.) hata döndürür.
        file.read_exact(&mut magic_number_bytes)?;
        // Little-endian byte dizisini u32'ye dönüştür
        let magic_number = u32::from_le_bytes(magic_number_bytes);

        // Version için byte dizisi (2 byte - u16)
        let mut version_bytes = [0; 2];
        // Tam olarak 2 byte oku.
        file.read_exact(&mut version_bytes)?;
        // Little-endian byte dizisini u16'ya dönüştür
        let version = u16::from_le_bytes(version_bytes);

        // Data size için byte dizisi (4 byte - u32)
        let mut data_size_bytes = [0; 4];
        // Tam olarak 4 byte oku.
        file.read_exact(&mut data_size_bytes)?;
        // Little-endian byte dizisini u32'ye dönüştür
        let data_size = u32::from_le_bytes(data_size_bytes);

        // Okunan başlık bilgilerini kullanarak CHeader örneği oluştur ve döndür
        Ok(CHeader {
            magic_number,
            version,
            data_size,
        })
    }

    // CHeader'ı dosyaya yazar
    fn write_to_file(&self, file: &mut File) -> io::Result<()> {
        file.write_all(&self.magic_number.to_le_bytes())?;
        file.write_all(&self.version.to_le_bytes())?;
        file.write_all(&self.data_size.to_le_bytes())?;
        Ok(())
    }
}

// Örnek kullanım
fn main() -> io::Result<()> {
    let header = CHeader {
        magic_number: 0x12345678,
        version: 1,
        data_size: 10,
    };
    let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let c_file = CFile::new(header, data);

    // Dosyaya yazma
    let path = Path::new("example_optimized.cfile");
    c_file.write_to_file(path)?;

    // Dosyadan okuma (optimize edilmiş fonksiyon kullanılarak)
    let read_c_file = CFile::read_from_file_optimized(path)?;
    println!("{:?}", read_c_file);

    Ok(())
}