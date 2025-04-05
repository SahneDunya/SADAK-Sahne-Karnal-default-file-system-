use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

pub struct CrystalFile {
    pub data: Vec<u8>,
}

impl CrystalFile {
    pub fn new() -> CrystalFile {
        CrystalFile { data: Vec::new() }
    }

    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<CrystalFile, std::io::Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file); // BufReader already good, keep it
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?; // read_to_end is efficient for reading all file data
        Ok(CrystalFile { data })
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file); // BufWriter already good, keep it
        writer.write_all(&self.data)?; // write_all is efficient for writing all data
        Ok(())
    }

    // Custom read/write operations for Crystal file format can be added here.
    // For example, reading or writing data with a specific structure.
    pub fn read_crystal_data(&self) -> Result<Vec<u32>, std::io::Error> {
        // Custom reading logic for Crystal file format
        // ...
        // Example: Reading data as u32 values.
        if self.data.len() % 4 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data length is not a multiple of 4",
            ));
        }

        let mut result = Vec::with_capacity(self.data.len() / 4); // Pre-allocate for performance
        for chunk in self.data.chunks_exact(4) { // Iterate in chunks of 4 for efficiency
            let value = u32::from_le_bytes(chunk.try_into().unwrap()); // try_into here is safe due to chunks_exact(4)
            result.push(value);
        }
        Ok(result)
    }

    pub fn write_crystal_data(&mut self, data: &[u32]) -> Result<(), std::io::Error> {
        // Custom writing logic for Crystal file format
        // ...
        // Example: Writing data as u32 values.
        self.data.clear();
        self.data.reserve(data.len() * 4); // Pre-allocate needed space for performance
        for value in data {
            self.data.extend_from_slice(&value.to_le_bytes()); // extend_from_slice is efficient
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_read_write_crystal_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = vec![1, 2, 3, 4, 5];
        let mut crystal_file = CrystalFile::new();
        crystal_file.write_crystal_data(&data).unwrap();
        temp_file.write_all(&crystal_file.data).unwrap();

        let read_crystal_file = CrystalFile::read_from_file(temp_file.path()).unwrap();
        let read_data = read_crystal_file.read_crystal_data().unwrap();
        assert_eq!(data, read_data);
    }
}