use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write, ErrorKind};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, PartialEq)]
struct FSharpRecord {
    id: u32,
    name: String,
    value: f64,
}

impl FSharpRecord {
    fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let id = reader.read_u32::<LittleEndian>()?;
        let name_len = reader.read_u32::<LittleEndian>()? as usize;
        let mut name_bytes = vec![0; name_len];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|e| io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decode name as UTF-8: {}", e) // Daha açıklayıcı hata mesajı
            ))?;
        let value = reader.read_f64::<LittleEndian>()?;

        Ok(FSharpRecord { id, name, value })
    }

    fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u32::<LittleEndian>(self.id)?;
        writer.write_u32::<LittleEndian>(self.name.len() as u32)?;
        writer.write_all(self.name.as_bytes())?;
        writer.write_f64::<LittleEndian>(self.value)?;

        Ok(())
    }
}

pub fn read_fsharp_file(file_path: &str) -> io::Result<Vec<FSharpRecord>> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut records = Vec::new();

    loop { // Döngüyü `while let` yerine `loop` ile değiştir
        match FSharpRecord::read(&mut reader) {
            Ok(record) => records.push(record),
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => break, // EOF durumunu açıkça işle
            Err(e) => return Err(e), // Diğer hataları doğrudan döndür
        }
    }

    Ok(records)
}

pub fn write_fsharp_file(file_path: &str, records: &[FSharpRecord]) -> io::Result<()> {
    let file = File::create(file_path)?;
    let mut writer = BufWriter::new(file);

    for record in records {
        record.write(&mut writer)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_fsharp_record_read_write() {
        let record = FSharpRecord { id: 1, name: "test".to_string(), value: 3.14 };
        let mut buffer: Vec<u8> = Vec::new();
        record.write(&mut buffer).unwrap();

        let mut reader = buffer.as_slice();
        let read_record = FSharpRecord::read(&mut reader).unwrap();
        assert_eq!(record, read_record);
    }

    #[test]
    fn test_read_write_fsharp_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap();

        let records = vec![
            FSharpRecord { id: 1, name: "test1".to_string(), value: 1.0 },
            FSharpRecord { id: 2, name: "test2".to_string(), value: 2.0 },
        ];

        write_fsharp_file(file_path, &records).unwrap();
        let read_records = read_fsharp_file(file_path).unwrap();

        assert_eq!(records, read_records);
    }

    #[test]
    fn test_read_fsharp_file_eof_handling() { // Yeni test fonksiyonu EOF işleme için
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap();

        let records_to_write = vec![
            FSharpRecord { id: 1, name: "record1".to_string(), value: 1.0 },
            FSharpRecord { id: 2, name: "record2".to_string(), value: 2.0 },
        ];
        write_fsharp_file(file_path, &records_to_write).unwrap();

        // Dosyayı kasıtlı olarak keserek EOF durumu oluştur
        let file = File::open(file_path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.take(10).read_to_end(&mut buffer).unwrap(); // Sadece ilk 10 baytı oku, dosyanın tamamını değil
        let mut incomplete_reader = buffer.as_slice();

        let read_records = read_fsharp_file_with_reader(&mut incomplete_reader).unwrap(); // Yeni fonksiyonu kullan
        // Dosyanın sadece bir kısmını okuduğumuz için, okunan kayıtların orijinal kayıtlarla eşleşmesini bekleyemeyiz.
        // Ancak, fonksiyonun EOF hatası almadan düzgün bir şekilde çalışmasını bekliyoruz.
        assert!(!read_records.is_empty() || records_to_write.is_empty()); // Dosya boş değilse kayıt okumuş olmalı
    }

    // Yeni yardımcı fonksiyon EOF testini desteklemek için
    pub fn read_fsharp_file_with_reader<R: Read>(reader: &mut R) -> io::Result<Vec<FSharpRecord>> {
        let mut buf_reader = BufReader::new(reader); // Read trait objesini BufReader'a sarmala
        let mut records = Vec::new();

        loop {
            match FSharpRecord::read(&mut buf_reader) {
                Ok(record) => records.push(record),
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
        }

        Ok(records)
    }
}