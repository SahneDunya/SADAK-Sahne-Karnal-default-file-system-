#![no_std]
#![allow(dead_code)]

// Gerekli Sahne64 modüllerini içeri aktar
#[cfg(not(feature = "std"))]
use crate::{
    fs,
    memory,
    process,
    sync,
    kernel,
    SahneError,
    arch,
};

// Gerekli modülleri içe aktar
use crate::vfs::{VfsNode, VfsNodeType};
use crate::{SahneError}; // SahneError'ı içe aktar
use core::cmp;

pub struct AccdbFile {
    data: Vec<u8>,
    cursor: usize, // İç okuma/yazma pozisyonunu takip etmek için imleç
}

impl AccdbFile {
    pub fn new(data: Vec<u8>) -> Self {
        AccdbFile { data, cursor: 0 } // İmleci başlangıçta 0 olarak ayarla
    }

    // Doğrudan erişim için geliştirilmiş read metodu, imleci etkilemez
    pub fn read_at(&self, offset: usize, size: usize) -> Option<&[u8]> {
        if offset >= self.data.len() {
            return None; // Offset dosya boyutunun dışında
        }
        let end = cmp::min(offset + size, self.data.len()); // Bitişi dosya boyutu ile sınırla
        Some(&self.data[offset..end])
    }

    // Doğrudan erişim için geliştirilmiş write metodu, imleci etkilemez ve SahneError döndürür
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> Result<(), SahneError> {
        if offset >= self.data.len() {
            return Err(SahneError::InvalidParameter);
        }
        if offset + data.len() > self.data.len() {
            return Err(SahneError::InvalidParameter);
        }
        self.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}

impl VfsNode for AccdbFile {
    fn get_type(&self) -> VfsNodeType {
        VfsNodeType::File
    }

    fn get_size(&self) -> usize {
        self.data.len()
    }
}

// no_std ortamında Read trait'ini kendimiz tanımlamamız gerekebilir veya
// core::io::Read trait'ini kullanabiliriz. core::io::Read'i kullanalım.
impl core::io::Read for AccdbFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> {
        if self.cursor >= self.data.len() {
            return Ok(0); // Dosya sonuna gelindi
        }
        let bytes_available = self.data.len() - self.cursor;
        let bytes_to_read = cmp::min(buf.len(), bytes_available);

        buf[..bytes_to_read].copy_from_slice(&self.data[self.cursor..self.cursor + bytes_to_read]);
        self.cursor += bytes_to_read; // İmleci ilerlet

        Ok(bytes_to_read)
    }
}

// Benzer şekilde Seek trait'i için de core::io::Seek kullanalım.
impl core::io::Seek for AccdbFile {
    fn seek(&mut self, pos: core::io::SeekFrom) -> Result<u64, core::io::Error> {
        use core::io::SeekFrom;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as usize,
            SeekFrom::End(offset) => {
                let end_pos = (self.data.len() as isize + offset) as usize;
                if end_pos < 0 { // Negatif pozisyon kontrolü
                    0
                } else {
                    end_pos
                }
            },
            SeekFrom::Current(offset) => {
                let current_pos = (self.cursor as isize + offset) as usize;
                if current_pos < 0 { // Negatif pozisyon kontrolü
                    0
                } else {
                    current_pos
                }
            },
        };

        if new_pos > self.data.len() {
            self.cursor = self.data.len(); // İmleci dosya sonuna ayarla, hata döndürmek yerine
            return Ok(self.cursor as u64);
             // Alternatif olarak hata döndürülebilir:
             // return Err(Error::new(ErrorKind::InvalidInput, "Invalid seek position"));
        }

        self.cursor = new_pos;
        Ok(self.cursor as u64)
    }
}