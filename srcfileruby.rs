use std::fs::File;
use std::io::{BufRead, BufReader, Result};

pub fn read_ruby_file(filepath: &str) -> Result<Vec<String>> {
    let file = File::open(filepath)?;
    let reader = BufReader::new(file);

    let mut lines = Vec::new();
    for line in reader.lines() {
        lines.push(line?);
    }

    Ok(lines)
}

// İyileştirilmiş parse_ruby_lines fonksiyonu: Gereksiz vector oluşturmayı önler.
pub fn parse_ruby_lines_optimized(lines: &[String]) -> Vec<String> {
    let mut parsed_lines = Vec::new();

    for line in lines {
        // split_whitespace() doğrudan iterator döndürür, collect() gereksiz.
        for word in line.split_whitespace() {
            parsed_lines.push(word.to_string());
        }
    }

    parsed_lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::tempdir;

    #[test]
    fn test_read_and_parse_ruby_file_optimized() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_optimized.rb");
        let file_path_str = file_path.to_str().unwrap();

        let ruby_code = "puts 'Merhaba, Dünya!'\n  x = 10\n  puts x";
        write(file_path_str, ruby_code).unwrap();

        let lines = read_ruby_file(file_path_str).unwrap();
        // Optimize edilmiş fonksiyonu kullan
        let parsed_lines = parse_ruby_lines_optimized(&lines);

        assert_eq!(
            parsed_lines,
            vec!["puts", "'Merhaba,", "Dünya!'", "x", "=", "10", "puts", "x"]
        );
    }
}