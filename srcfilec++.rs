use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use byteorder::{LittleEndian, ReadBytesExt};

// C++ dosya formatının başlık yapısı
struct Header {
    version: u32,
    creation_date: u64,
}

// C++ dosya formatının meta veri yapısı
struct Metadata {
    file_size: u64,
    author: String,
}

// C++ dosya formatını okuyan fonksiyon
fn read_cpp_file(mut file: File) -> io::Result<()> {
    // Başlığı oku
    let header = read_header(&mut file)?;
    println!("Header: Version={}, Creation Date={}", header.version, header.creation_date);

    // Meta veriyi oku
    let metadata = read_metadata(&mut file)?;
    println!("Metadata: File Size={}, Author={}", metadata.file_size, metadata.author);

    // Veriyi oku
    let data = read_data(&mut file)?;
    println!("Data: {:?}", data);

    Ok(())
}

// Başlığı okuyan yardımcı fonksiyon
fn read_header(file: &mut File) -> io::Result<Header> {
    let version = file.read_u32::<LittleEndian>()?;
    let creation_date = file.read_u64::<LittleEndian>()?;
    Ok(Header { version, creation_date })
}

// Meta veriyi okuyan yardımcı fonksiyon
fn read_metadata(file: &mut File) -> io::Result<Metadata> {
    let file_size = file.read_u64::<LittleEndian>()?;
    let author_length = file.read_u8()? as usize;
    // İyileştirme: author_bytes vektörünü doğru kapasiteyle oluşturarak yeniden boyutlandırmayı önleyin
    let mut author_bytes = Vec::with_capacity(author_length);
    // Mevcut vektörün kapasitesini ayarladıktan sonra, beklenen boyuta getirmek için uzunluğunu ayarlayın.
    // Bu, sonraki `read_exact` işleminin doğrudan vektöre yazabilmesini sağlar.
    author_bytes.resize(author_length, 0);
    file.read_exact(&mut author_bytes)?;
    let author = String::from_utf8_lossy(&author_bytes).to_string();
    Ok(Metadata { file_size, author })
}

// Veriyi okuyan yardımcı fonksiyon
fn read_data(file: &mut File) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

fn main() -> io::Result<()> {
    // Örnek bir C++ dosyası oluşturmak için (gerçek bir C++ dosyası formatı değil, sadece örnek için)
    let mut temp_file = File::create("example.cpp")?;

    // Başlık yaz
    temp_file.write_all(&1u32.to_le_bytes())?; // version = 1
    temp_file.write_all(&1678886400u64.to_le_bytes())?; // creation_date = 1678886400

    // Meta veri yaz
    let author_name = "Optimized Rust Code Example";
    temp_file.write_all(&8388608u64.to_le_bytes())?; // file_size = 8388608 (örnek boyut)
    temp_file.write_all(&author_name.len().to_le_bytes()[..1])?; // author_length (u8 olarak)
    temp_file.write_all(author_name.as_bytes())?; // author

    // Veri yaz (örnek veri)
    let sample_data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
    temp_file.write_all(&sample_data)?;


    let file = File::open("example.cpp")?;
    read_cpp_file(file)?;
    Ok(())
}