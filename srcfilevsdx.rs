use crate::fs::{FileSystem, VfsNode};
use std::io::{Read, Seek, Cursor, ErrorKind};
use zip::ZipArchive;

pub struct VsdxFile {
    archive: ZipArchive<Box<dyn Read + Seek>>,
    // Cache the content of the first file in the archive for read operations.
    // For simplicity, we'll just read the first file for now.
    first_file_content: Option<Vec<u8>>,
}

impl VsdxFile {
    pub fn new(data: Box<dyn Read + Seek>) -> Result<Self, zip::result::ZipError> {
        let mut archive = ZipArchive::new(data)?;
        let first_file_content = if archive.len() > 0 {
            let first_filename = archive.file_names().next().map(|s| s.to_string());
            if let Some(filename) = first_filename {
                match archive.by_name(&filename) {
                    Ok(mut file) => {
                        let mut buffer = Vec::new();
                        if let Err(_) = file.read_to_end(&mut buffer) {
                            None // Fail to read, but continue with empty content. Handle error in read()
                        } else {
                            Some(buffer)
                        }
                    },
                    Err(_) => None, // File not found, continue with empty content. Handle error in read()
                }
            } else {
                None // No files in archive, continue with empty content. Handle error in read()
            }
        } else {
            None // Empty archive, continue with empty content. Handle error in read()
        };


        Ok(Self { archive, first_file_content })
    }

    pub fn get_file(&mut self, filename: &str) -> Result<Vec<u8>, zip::result::ZipError> {
        let mut file = self.archive.by_name(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    // ... diğer VSDX'e özgü fonksiyonlar ...
}

impl VfsNode for VsdxFile {
    fn read(&mut self, offset: usize, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        if let Some(content) = &self.first_file_content {
            let content_len = content.len();
            if offset >= content_len {
                return Ok(0); // Offset is beyond the content, so read 0 bytes.
            }

            let bytes_available = content_len - offset;
            let bytes_to_read = std::cmp::min(buffer.len(), bytes_available);

            if bytes_to_read > 0 {
                buffer[..bytes_to_read].copy_from_slice(&content[offset..offset + bytes_to_read]);
            }
            Ok(bytes_to_read)
        } else {
            Err(std::io::Error::new(ErrorKind::Other, "Could not read content from VSDX file"))
        }
    }

    fn write(&mut self, offset: usize, buffer: &[u8]) -> Result<usize, std::io::Error> {
        // For simplicity, VSDX write is not implemented in this example.
        // Modifying zip archives is complex and depends on what needs to be changed.
        Err(std::io::Error::new(ErrorKind::Unsupported, "Write operation is not supported for VSDX files in this example."))
    }

    // ... diğer VfsNode trait fonksiyonları ... (e.g., metadata, etc. - implement as needed)
}

pub fn load_vsdx(fs: &mut FileSystem, path: &str, data: Box<dyn Read + Seek>) -> Result<(), zip::result::ZipError> {
    let vsdx_file = VsdxFile::new(data)?;
    fs.add_node(path, Box::new(vsdx_file));
    Ok(())
}