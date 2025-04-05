use std::fs::File;
use std::io::{BufReader, Read, Result};
use std::path::Path;

#[derive(Debug)]
pub struct FileCangjie {
    header: Header,
    records: Vec<Record>,
}

#[derive(Debug)]
struct Header {
    magic_number: [u8; 4],
    version: u32,
    record_count: u32,
    // ... diğer başlık bilgileri ...
}

#[derive(Debug)]
struct Record {
    id: u32,
    data: Vec<u8>,
    // ... diğer kayıt bilgileri ...
}

impl FileCangjie {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let header = Self::read_header(&mut reader)?;
        let records = Self::read_records(&mut reader, header.record_count)?;

        Ok(FileCangjie { header, records })
    }

    fn read_header<R: Read>(reader: &mut R) -> Result<Header> {
        let mut magic_number = [0; 4];
        reader.read_exact(&mut magic_number)?;

        let mut version_bytes = [0; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);

        let mut record_count_bytes = [0; 4];
        reader.read_exact(&mut record_count_bytes)?;
        let record_count = u32::from_le_bytes(record_count_bytes);

        // ... diğer başlık bilgileri okuma işlemleri ...

        Ok(Header {
            magic_number,
            version,
            record_count,
            // ... diğer başlık bilgileri ...
        })
    }

    fn read_records<R: Read>(reader: &mut R, record_count: u32) -> Result<Vec<Record>> {
        let mut records = Vec::with_capacity(record_count as usize);

        for _ in 0..record_count {
            let mut id_bytes = [0; 4];
            reader.read_exact(&mut id_bytes)?;
            let id = u32::from_le_bytes(id_bytes);

            // ... kayıt verisi boyutu okuma işlemi ...
            let data_size = 10; // Örnek olarak sabit bir boyut kullanıyoruz. Gerçek boyutu okumanız gerekebilir.
            let mut data = vec![0; data_size];
            reader.read_exact(&mut data)?;

            // ... diğer kayıt bilgileri okuma işlemleri ...

            records.push(Record { id, data });
        }

        Ok(records)
    }
}

fn main() -> Result<()> {
    let cangjie_file = FileCangjie::load("example.cj")?; // "example.cj" dosyasını yükle
    println!("{:?}", cangjie_file);
    Ok(())
}