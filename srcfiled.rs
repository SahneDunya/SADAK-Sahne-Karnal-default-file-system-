use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub struct DFile {
    file: File,
}

impl DFile {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<DFile> {
        let file = File::open(path)?;
        Ok(DFile { file })
    }

    pub fn read_header(&mut self) -> io::Result<DHeader> {
        let mut buffer = [0; 16];
        self.file.read_exact(&mut buffer)?;
        Ok(DHeader::from_bytes(&buffer))
    }

    pub fn read_data(&mut self, offset: u64, size: usize) -> io::Result<Vec<u8>> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0; size];
        self.file.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

pub struct DHeader {
    magic: [u8; 4],
    version: u32,
    data_offset: u64,
}

impl DHeader {
    pub fn from_bytes(bytes: &[u8; 16]) -> DHeader {
        DHeader {
            magic: [bytes[0], bytes[1], bytes[2], bytes[3]],
            version: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            data_offset: u64::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]]),
        }
    }

    pub fn magic(&self) -> &[u8; 4] {
        &self.magic
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }
}