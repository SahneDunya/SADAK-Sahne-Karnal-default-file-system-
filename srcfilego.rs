use std::fs::File;
use std::io::{Read, Write};
use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct GoData {
    name: String,
    version: u32,
    enabled: bool,
}

/// Reads GoData from a file using bincode.
pub fn read_go_file(filename: &str) -> Result<GoData, Box<dyn std::error::Error>> {
    // Open the file in read mode.
    let mut file = File::open(filename)?;
    // Create a buffer to store the file contents.
    let mut buffer = Vec::new();
    // Read all bytes from the file into the buffer.
    file.read_to_end(&mut buffer)?;

    // Deserialize the buffer into GoData using bincode.
    let data: GoData = deserialize(&buffer)?;
    Ok(data)
}

/// Writes GoData to a file using bincode.
pub fn write_go_file(filename: &str, data: &GoData) -> Result<(), Box<dyn std::error::Error>> {
    // Serialize the GoData into a byte vector using bincode.
    let encoded: Vec<u8> = serialize(data)?;
    // Create a file in write mode.
    let mut file = File::create(filename)?;
    // Write the encoded data to the file.
    file.write_all(&encoded)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_go_file_read_write() {
        // Arrange: Create test data and a temporary file.
        let data = GoData {
            name: "test".to_string(),
            version: 1,
            enabled: true,
        };

        // Create a named temporary file. File will be automatically deleted when temp_file is dropped.
        let mut temp_file = NamedTempFile::new().unwrap();
        // Get the file path as a string.
        let filename = temp_file.path().to_str().unwrap();

        // Act: Write data to the temporary file and then read it back.
        write_go_file(filename, &data).unwrap();
        let read_data = read_go_file(filename).unwrap();

        // Assert: Check if the written data and read data are the same.
        assert_eq!(data, read_data);

        // Explicitly drop temp_file to ensure the temporary file is deleted immediately after the test.
        drop(temp_file);
    }
}