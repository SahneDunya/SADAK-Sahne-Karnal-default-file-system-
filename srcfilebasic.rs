use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write, BufWriter};
use std::path::Path;

pub struct BasicFile {
    path: String,
}

impl BasicFile {
    pub fn new(path: &str) -> BasicFile {
        BasicFile {
            path: path.to_string(),
        }
    }

    // İyileştirilmiş read_line fonksiyonu: Gereksiz String kopyalamaları önleniyor.
    pub fn read_line_optimized(&self, line_number: usize) -> io::Result<Option<String>> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        for (index, line_result) in reader.lines().enumerate() {
            let line = line_result?; // Hata kontrolü satır döngüsü içinde
            if index + 1 == line_number {
                return Ok(Some(line));
            }
        }

        Ok(None) // Satır bulunamadı veya dosya boş
    }

    // İyileştirilmiş write_line fonksiyonu: Daha az vektör yönetimi ve erken hata kontrolü
    pub fn write_line_optimized(&self, line_number: usize, text: &str) -> io::Result<()> {
        if line_number == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Satır numarası 1'den büyük olmalı",
            ));
        }

        let path = Path::new(&self.path);
        let mut lines = Vec::new();

        if path.exists() {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            for line_result in reader.lines() {
                lines.push(line_result?); // Hata kontrolü satır okuma sırasında
            }
        }

        if line_number > lines.len() + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Satır numarası dosya boyutunu aşıyor",
            ));
        }

        if line_number > lines.len() {
            lines.push(text.to_string());
        } else {
            lines[line_number - 1] = text.to_string();
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file); // BufWriter kullanılarak yazma optimize ediliyor.
        for line in lines {
            writeln!(writer, "{}", line)?; // Direkt olarak BufWriter'a yazılıyor
        }
        writer.flush()?; // Tampon belleği temizle

        Ok(())
    }

    // BASIC'in özel dosya formatına özgü diğer işlevleri buraya ekleyebilirsiniz.
    // Örneğin, değişken okuma/yazma, satır numarası yönetimi vb.
}