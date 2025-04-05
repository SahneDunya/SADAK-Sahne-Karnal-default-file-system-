use crate::vfs::{VfsNode, VfsNodeType};
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::Mutex;

pub struct File {
    pub name: String,
    pub data: Mutex<Vec<u8>>,
    pub node: VfsNode,
}

impl File {
    pub fn new(name: String, node: VfsNode) -> Self {
        File {
            name,
            data: Mutex::new(Vec::new()),
            node,
        }
    }

    pub fn read(&self, offset: usize, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let data = self.data.lock().unwrap(); // Kilitlemeyi burada alıyoruz ve fonksiyon sonuna kadar tutuyoruz.
        let len = data.len();

        if offset >= len {
            return Ok(0); // Dosya sonuna ulaşıldı
        }

        let read_len = core::cmp::min(buffer.len(), len - offset);
        buffer[..read_len].copy_from_slice(&data[offset..offset + read_len]); // Veriyi kopyalıyoruz.

        Ok(read_len)
    }

    pub fn write(&self, offset: usize, buffer: &[u8]) -> Result<usize, &'static str> {
        let mut data = self.data.lock().unwrap(); // Kilitlemeyi burada alıyoruz ve fonksiyon sonuna kadar tutuyoruz.
        let len = data.len();

        if offset > len {
            return Err("Geçersiz ofset");
        }

        if offset == len {
            data.extend_from_slice(buffer); // Dosya sonuna ekleme yapıyoruz.
        } else {
            let write_len = buffer.len();
            if offset + write_len > len {
                data.resize(offset + write_len, 0); // Gerekirse vektörü yeniden boyutlandırıyoruz.
            }
            data[offset..offset + write_len].copy_from_slice(buffer); // Veriyi belirtilen ofsete kopyalıyoruz.
        }

        Ok(buffer.len())
    }

    pub fn size(&self) -> usize {
        self.data.lock().unwrap().len() // Dosya boyutunu döndürüyoruz.
    }
}