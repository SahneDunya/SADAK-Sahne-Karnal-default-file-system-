use std::io::{Error, ErrorKind};

// Assuming the previous code is in a file like `src/lib.rs`
// We need to bring the `sahne64` module into scope.
// Assuming the previous code was in the root of the project.
mod sahne64;

pub struct JpegImage {
    pub width: u32,
    pub height: u32,
    // More JPEG metadata and pixel data can be added here.
}

pub fn parse_jpeg(file_path: &str) -> Result<JpegImage, Error> {
    let fd_result = sahne64::fs::open(file_path, sahne64::fs::O_RDONLY);
    let fd = match fd_result {
        Ok(fd) => fd,
        Err(e) => {
            return Err(Error::new(
                ErrorKind::Other, // Using Other as a general mapping for SahneError
                format!("Error opening file: {:?}", e),
            ));
        }
    };

    // Helper function to read exactly n bytes using Sahne64's read
    fn read_bytes_sahne64(fd: u64, buffer: &mut [u8], segment_name: &str) -> Result<(), Error> {
        let bytes_read_result = sahne64::fs::read(fd, buffer);
        match bytes_read_result {
            Ok(n) if n == buffer.len() => Ok(()),
            Ok(n) => Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!("Invalid JPEG file: Unexpected end of file while reading {} segment (read {} bytes, expected {})", segment_name, n, buffer.len()),
            )),
            Err(e) => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid JPEG file: Error reading {} segment - {:?}", segment_name, e),
            )),
        }
    }

    // Read SOI marker (Start of Image)
    let mut soi_buffer = [0; 2];
    if let Err(e) = read_bytes_sahne64(fd, &mut soi_buffer, "SOI marker") {
        sahne64::fs::close(fd).unwrap_or_default();
        return Err(e);
    }
    if soi_buffer != [0xFF, 0xD8] {
        sahne64::fs::close(fd).unwrap_or_default();
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Invalid JPEG file: SOI marker not found",
        ));
    }

    let mut width: u32 = 0;
    let mut height: u32 = 0;

    loop {
        let mut marker_buffer = [0; 2];
        if let Err(e) = read_bytes_sahne64(fd, &mut marker_buffer, "segment marker") {
            sahne64::fs::close(fd).unwrap_or_default();
            if e.kind() == ErrorKind::UnexpectedEof {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid JPEG file: Unexpected end of file before SOF0"));
            } else {
                return Err(e);
            }
        }

        if marker_buffer[0] != 0xFF {
            sahne64::fs::close(fd).unwrap_or_default();
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid JPEG file: Invalid marker {:?}", marker_buffer),
            ));
        }

        match marker_buffer[1] {
            0xC0 => { // SOF0 marker (Start-of-frame, baseline DCT)
                let mut sof0_length_buffer = [0; 2];
                if let Err(e) = read_bytes_sahne64(fd, &mut sof0_length_buffer, "SOF0 length") {
                    sahne64::fs::close(fd).unwrap_or_default();
                    return Err(e);
                }
                let sof0_length = u16::from_be_bytes(sof0_length_buffer) as usize;

                if sof0_length < 8 { // Minimal SOF0 segment length check
                    sahne64::fs::close(fd).unwrap_or_default();
                    return Err(Error::new(ErrorKind::InvalidData, "Invalid JPEG file: SOF0 segment length too short"));
                }

                let mut sof0_data = vec![0; sof0_length - 2];
                if let Err(e) = read_bytes_sahne64(fd, &mut sof0_data.as_mut_slice(), "SOF0 data") {
                    sahne64::fs::close(fd).unwrap_or_default();
                    return Err(e);
                }

                height = u16::from_be_bytes([sof0_data[1], sof0_data[2]]) as u32;
                width = u16::from_be_bytes([sof0_data[3], sof0_data[4]]) as u32;
                sahne64::fs::close(fd).unwrap_or_default();
                break; // Stop after finding SOF0 and extracting dimensions
            }
            0xD9 => { // EOI marker (End of Image)
                sahne64::fs::close(fd).unwrap_or_default();
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Invalid JPEG file: SOF0 marker not found before EOI",
                ));
            }
            _ => { // Skip other segments
                let mut length_buffer = [0; 2];
                if let Err(e) = read_bytes_sahne64(fd, &mut length_buffer, "segment length") {
                    sahne64::fs::close(fd).unwrap_or_default();
                    return Err(e);
                }
                let segment_length = u16::from_be_bytes(length_buffer) as usize;
                if segment_length > 2 {
                    let skip_length = segment_length - 2;
                    let mut skip_buffer = vec![0; skip_length];
                    if let Err(e) = read_bytes_sahne64(fd, &mut skip_buffer.as_mut_slice(), "segment data") {
                        sahne64::fs::close(fd).unwrap_or_default();
                        return Err(e);
                    }
                }
            }
        }
    }

    if width == 0 || height == 0 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Invalid JPEG file: Could not parse image dimensions",
        ));
    }

    Ok(JpegImage { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Helper function to simulate file creation and reading with Sahne64 functions
    fn simulate_sahne64_file(path: &str, contents: &[u8]) -> Result<(), Error> {
        // In a real Sahne64 environment, this would involve system calls.
        // For testing in a standard environment, we can use std::fs.
        let mut file = std::fs::File::create(path)?;
        file.write_all(contents)?;
        Ok(())
    }

    fn read_sahne64_file(path: &str, buffer: &mut [u8]) -> Result<usize, Error> {
        // Simulation of sahne64::fs::read using std::fs
        let mut file = std::fs::File::open(path)?;
        file.read(buffer)
    }

    fn close_sahne64_file(fd: u64) -> Result<(), Error> {
        // In a simulated environment, we don't need to do anything with the fd.
        Ok(())
    }

    #[test]
    fn test_parse_jpeg() {
        // Create a minimal valid JPEG file for testing in memory
        let mut jpeg_bytes: Vec<u8> = Vec::new();
        // SOI
        jpeg_bytes.write(&[0xFF, 0xD8]).unwrap();
        // APP0 segment (minimal JFIF header - not strictly necessary for basic decode)
        jpeg_bytes.write(&[0xFF, 0xE0]).unwrap(); // APP0 marker
        jpeg_bytes.write(&[0x00, 0x10]).unwrap(); // Length (16 bytes)
        jpeg_bytes.write(&[0x4A, 0x46, 0x49, 0x46, 0x00]).unwrap(); // JFIF identifier
        jpeg_bytes.write(&[0x01, 0x01]).unwrap(); // JFIF version 1.1
        jpeg_bytes.write(&[0x00]).unwrap(); // Density units
        jpeg_bytes.write(&[0x00, 0x01]).unwrap(); // X density
        jpeg_bytes.write(&[0x00, 0x01]).unwrap(); // Y density
        jpeg_bytes.write(&[0x00, 0x00]).unwrap(); // Thumbnail width/height

        // SOF0 segment (Start of Frame 0) - Minimal version
        jpeg_bytes.write(&[0xFF, 0xC0]).unwrap(); // SOF0 marker
        jpeg_bytes.write(&[0x00, 0x11]).unwrap(); // Length (17 bytes)
        jpeg_bytes.write(&[0x08]).unwrap();       // Sample precision (8 bits)
        jpeg_bytes.write(&[0x00, 0xA0]).unwrap(); // Height (160 pixels)
        jpeg_bytes.write(&[0x00, 0xC8]).unwrap(); // Width (200 pixels)
        jpeg_bytes.write(&[0x03]).unwrap();       // Number of components (3 - YCbCr)
        jpeg_bytes.write(&[0x01, 0x22, 0x00]).unwrap(); // Component 1: Y, sampling factors 2x2, quantization table 0
        jpeg_bytes.write(&[0x02, 0x11, 0x01]).unwrap(); // Component 2: Cb, sampling factors 1x1, quantization table 1
        jpeg_bytes.write(&[0x03, 0x11, 0x01]).unwrap(); // Component 3: Cr, sampling factors 1x1, quantization table 1

        // Minimal SOS (Start of Scan) - Actual image data is not provided for minimal example
        jpeg_bytes.write(&[0xFF, 0xDA]).unwrap(); // SOS marker
        jpeg_bytes.write(&[0x00, 0x0C]).unwrap(); // Length (12 bytes)
        jpeg_bytes.write(&[0x03]).unwrap();       // Number of components in scan (3)
        jpeg_bytes.write(&[0x01, 0x00]).unwrap(); // Component 1: DC entropy coding table 0, AC entropy coding table 0
        jpeg_bytes.write(&[0x02, 0x11]).unwrap(); // Component 2: DC entropy coding table 1, AC entropy coding table 1
        jpeg_bytes.write(&[0x03, 0x11]).unwrap(); // Component 3: DC entropy coding table 1, AC entropy coding table 1
        jpeg_bytes.write(&[0x00, 0x3F, 0x00]).unwrap(); // Spectral selection start, spectral selection end, approximation bit position (default)

        // EOI
        jpeg_bytes.write(&[0xFF, 0xD9]).unwrap();

        let file_path = "test_minimal.jpg";
        simulate_sahne64_file(file_path, &jpeg_bytes).unwrap();


        match parse_jpeg(file_path) {
            Ok(image) => {
                println!("Width: {}, Height: {}", image.width, image.height);
                assert_eq!(image.width, 200); // Expected width from minimal JPEG
                assert_eq!(image.height, 160); // Expected height from minimal JPEG
            }
            Err(err) => {
                panic!("JPEG parsing error: {}", err);
            }
        }
        std::fs::remove_file(file_path).unwrap(); // Clean up test file
    }

    #[test]
    #[should_panic]
    fn test_parse_jpeg_invalid_soi() {
        let file_path = "test_invalid_soi.jpg";
        simulate_sahne64_file(file_path, &[0x00, 0x00, 0xFF, 0xD8]).unwrap(); // Invalid SOI
        match parse_jpeg(file_path) {
            Ok(_) => panic!("Should have failed with invalid SOI"),
            Err(e) => assert_eq!(e.kind(), ErrorKind::InvalidData),
        }
        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_parse_jpeg_no_sof0() {
        let file_path = "test_no_sof0.jpg";
        simulate_sahne64_file(file_path, &[0xFF, 0xD8, 0xFF, 0xD9]).unwrap(); // SOI then EOI, no SOF0
        match parse_jpeg(file_path) {
            Ok(_) => panic!("Should have failed with no SOF0"),
            Err(e) => assert_eq!(e.kind(), ErrorKind::InvalidData),
        }
        std::fs::remove_file(file_path).unwrap();
    }
}