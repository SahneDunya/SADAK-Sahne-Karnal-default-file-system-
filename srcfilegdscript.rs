use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

pub struct FileGDScript {
    pub path: String,
    pub content: String,
}

impl FileGDScript {
    pub fn new(path: &str) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        BufReader::new(&mut file).read_to_string(&mut content)?;

        Ok(FileGDScript {
            path: path.to_string(),
            content,
        })
    }

    pub fn save(&self, path: &str) -> io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(self.content.as_bytes())?;
        Ok(())
    }

    pub fn parse(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for line in self.content.lines() {
            lines.push(line.to_string());
        }
        lines
    }

    // GDScript'e özgü ayrıştırma ve yorumlama işlevleri burada eklenebilir.
    // Örneğin, değişkenleri, fonksiyonları ve sınıfları ayıklamak için.
}