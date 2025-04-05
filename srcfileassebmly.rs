use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub struct AssemblyFile {
    pub lines: Vec<String>,
}

impl AssemblyFile {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let lines = reader.lines().collect::<Result<_, _>>()?;
        Ok(AssemblyFile { lines })
    }

    pub fn print_lines(&self) {
        for line in &self.lines {
            println!("{}", line);
        }
    }

    // Additional functions can be added as needed.
    // For example, parsing lines, creating symbol table, etc.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_read_assembly_file() {
        // Create a temporary file for testing
        let mut temp_file = fs::File::create("test.asm").expect("Failed to create temp file");
        writeln!(temp_file, "MOV AX, 0x1234").expect("Failed to write to temp file");
        writeln!(temp_file, "ADD AX, BX").expect("Failed to write to temp file");
        writeln!(temp_file, "INT 0x80").expect("Failed to write to temp file");

        let assembly_file = AssemblyFile::new("test.asm").expect("Failed to read assembly file");
        assert_eq!(assembly_file.lines.len(), 3);
        assert_eq!(assembly_file.lines[0], "MOV AX, 0x1234");
        assert_eq!(assembly_file.lines[1], "ADD AX, BX");
        assert_eq!(assembly_file.lines[2], "INT 0x80");

        // Delete the temporary file
        fs::remove_file("test.asm").expect("Failed to remove temp file");
    }
}