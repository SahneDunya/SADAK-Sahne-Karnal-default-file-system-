use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub struct JuliaFile {
    pub path: String,
    pub lines: Vec<String>,
}

impl JuliaFile {
    pub fn new(path: &str) -> io::Result<JuliaFile> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let lines = reader.lines().collect::<Result<_, _>>()?;

        Ok(JuliaFile {
            path: path.to_string(),
            lines,
        })
    }

    pub fn print_lines(&self) {
        for line in &self.lines {
            println!("{}", line);
        }
    }

    // Julia'ya özgü diğer işlevleri buraya ekleyebilirsiniz.
    // Örneğin, Julia kodunu ayrıştırma, sembolleri çıkarma vb.
}

// load_julia_file fonksiyonu kaldırıldı. JuliaFile::new doğrudan kullanılabilir.
// pub fn load_julia_file(path: &str) -> io::Result<JuliaFile> {
//     JuliaFile::new(path)
// }

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::tempdir;

    #[test]
    fn test_load_julia_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.jl");
        let file_path_str = file_path.to_str().unwrap();

        let content = "println(\"Hello, Julia!\")\nfunction add(a, b)\n    return a + b\nend";
        write(file_path_str, content).unwrap();

        // load_julia_file yerine doğrudan JuliaFile::new kullanılıyor.
        let julia_file = JuliaFile::new(file_path_str).unwrap();
        assert_eq!(julia_file.lines.len(), 3);
        assert_eq!(julia_file.lines[0], "println(\"Hello, Julia!\")");
    }
}