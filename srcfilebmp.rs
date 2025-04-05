#![no_std]
#![allow(dead_code)]

// Modüllerin ve sabitlerin Sahne64'ten içe aktarılması
use crate::{
    arch::*,
    fs::{self, O_CREAT, O_RDONLY, O_WRONLY},
    memory, process, sync,
    SahneError,
};

#[cfg(feature = "std")]
use std::vec::Vec;

#[derive(Debug)]
pub struct BmpHeader {
    pub file_size: u32,
    pub image_data_offset: u32,
    pub width: u32,
    pub height: u32,
    pub bits_per_pixel: u16,
}

#[derive(Debug)]
pub struct BmpImage {
    pub header: BmpHeader,
    pub data: Vec<u8>,
}

impl BmpImage {
    pub fn read_from_file(filename: &str) -> Result<Self, SahneError> {
        let fd = fs::open(filename, O_RDONLY)?;

        let mut header_bytes = [0u8; 14];
        fs::read(fd, &mut header_bytes)?;

        if &header_bytes[0..2] != b"BM" {
            fs::close(fd)?;
            return Err(SahneError::InvalidFileDescriptor); // Daha uygun bir hata türü olabilir
        }

        let file_size = u32::from_le_bytes(header_bytes[2..6].try_into().unwrap());
        let image_data_offset = u32::from_le_bytes(header_bytes[10..14].try_into().unwrap());

        let mut dib_header_bytes = [0u8; 40];
        fs::read(fd, &mut dib_header_bytes)?;

        let width = u32::from_le_bytes(dib_header_bytes[4..8].try_into().unwrap());
        let height = u32::from_le_bytes(dib_header_bytes[8..12].try_into().unwrap());
        let bits_per_pixel = u16::from_le_bytes(dib_header_bytes[14..16].try_into().unwrap());

        let header = BmpHeader {
            file_size,
            image_data_offset,
            width,
            height,
            bits_per_pixel,
        };

        let image_data_size = (file_size - image_data_offset) as usize;
        let mut image_data = vec![0u8; image_data_size];
        fs::read(fd, &mut image_data)?;

        fs::close(fd)?;

        Ok(BmpImage {
            header,
            data: image_data,
        })
    }

    pub fn write_to_file_optimized(&self, filename: &str) -> Result<(), SahneError> {
        let fd = fs::open(filename, O_CREAT | O_WRONLY)?;

        // BMP Header (14 bytes)
        fs::write(fd, b"BM")?;
        fs::write(fd, &self.header.file_size.to_le_bytes())?;
        fs::write(fd, &[0u8; 4])?; // Reserved
        fs::write(fd, &self.header.image_data_offset.to_le_bytes())?;

        // DIB Header (40 bytes) - BITMAPINFOHEADER
        fs::write(fd, &[40u8, 0, 0, 0])?; // DIB header size
        fs::write(fd, &self.header.width.to_le_bytes())?;
        fs::write(fd, &self.header.height.to_le_bytes())?;
        fs::write(fd, &[1, 0])?; // Planes
        fs::write(fd, &self.header.bits_per_pixel.to_le_bytes())?;
        fs::write(fd, &[0u8; 24])?; // Compression, image size, etc.

        // Image Data
        fs::write(fd, &self.data)?;

        fs::close(fd)?;
        Ok(())
    }

    pub fn write_to_file(&self, filename: &str) -> Result<(), SahneError> {
        let fd = fs::open(filename, O_CREAT | O_WRONLY)?;

        fs::write(fd, b"BM")?;
        fs::write(fd, &self.header.file_size.to_le_bytes())?;
        fs::write(fd, &[0u8; 4])?; // Reserved
        fs::write(fd, &self.header.image_data_offset.to_le_bytes())?;

        fs::write(fd, &[40u8, 0, 0, 0])?; // DIB header size
        fs::write(fd, &self.header.width.to_le_bytes())?;
        fs::write(fd, &self.header.height.to_le_bytes())?;
        fs::write(fd, &[1, 0])?; // Planes
        fs::write(fd, &self.header.bits_per_pixel.to_le_bytes())?;
        fs::write(fd, &[0u8; 24])?; // Compression, image size, etc.

        fs::write(fd, &self.data)?;

        fs::close(fd)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{self, O_RDONLY};

    // Yardımcı fonksiyon: Bir dosyayı okuyup içeriğini byte dizisi olarak döndürür
    fn read_file_bytes(filename: &str) -> Result<Vec<u8>, SahneError> {
        let fd = fs::open(filename, O_RDONLY)?;
        let mut buffer = Vec::new();
        let mut temp_buffer = [0u8; 1024]; // Okuma için geçici bir arabellek
        loop {
            let bytes_read = fs::read(fd, &mut temp_buffer)?;
            if bytes_read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp_buffer[..bytes_read]);
        }
        fs::close(fd)?;
        Ok(buffer)
    }

    #[test]
    fn test_read_write_bmp() {
        // "test_data" dizinini oluşturmak için Sahne64'te bir yolumuz olmadığı için bu adım atlanıyor.
        // Test dosyası elle oluşturulmalı veya test ortamı tarafından sağlanmalıdır.

        // Basit bir test.bmp oluşturuyoruz (elle veya test ortamı tarafından sağlanmalı)
        let mut dummy_image_data = vec![255u8; 24]; // Beyaz 2x1 görüntü, 24bpp
        let header = BmpHeader {
            file_size: 54 + 24,
            image_data_offset: 54,
            width: 2,
            height: 1,
            bits_per_pixel: 24,
        };
        let dummy_image = BmpImage { header, data: dummy_image_data };
        dummy_image.write_to_file("test_data/test.bmp").unwrap();

        let image = BmpImage::read_from_file("test_data/test.bmp").unwrap();
        image.write_to_file("test_data/test_copy_original.bmp").unwrap();
        image.write_to_file_optimized("test_data/test_copy_optimized.bmp").unwrap();

        let image_copy_original = BmpImage::read_from_file("test_data/test_copy_original.bmp").unwrap();
        let image_copy_optimized = BmpImage::read_from_file("test_data/test_copy_optimized.bmp").unwrap();

        assert_eq!(image.header.file_size, image_copy_original.header.file_size);
        assert_eq!(image.data, image_copy_original.data);

        assert_eq!(image.header.file_size, image_copy_optimized.header.file_size);
        assert_eq!(image.data, image_copy_optimized.data);

        // Dosyaları byte byte karşılaştır
        let original_bytes = read_file_bytes("test_data/test_copy_original.bmp").unwrap();
        let optimized_bytes = read_file_bytes("test_data/test_copy_optimized.bmp").unwrap();
        assert_eq!(original_bytes, optimized_bytes, "Orijinal ve optimize edilmiş dosyalar aynı olmalı");
    }
}

// Bu kısım, no_std ortamında Vec kullanabilmek için gereklidir (eğer feature="std" aktif değilse bile).
// Eğer Sahne64 kendi bellek yönetimini sağlıyorsa, bu kısım farklı şekilde implemente edilebilir.
#[cfg(not(feature = "std"))]
mod std {
    pub mod io {
        pub use crate::SahneError as Error;
        pub type Result<T, E = Error> = core::result::Result<T, E>;
    }
    pub mod vec {
        pub struct Vec<T> {
            data: *mut T,
            len: usize,
            capacity: usize,
        }

        impl<T> Vec<T> {
            pub fn new() -> Self {
                Vec {
                    data: core::ptr::null_mut(),
                    len: 0,
                    capacity: 0,
                }
            }

            pub fn with_capacity(capacity: usize) -> Self {
                // Burada Sahne64'ün bellek ayırma mekanizması kullanılmalı
                let layout = core::alloc::Layout::array::<T>(capacity).unwrap();
                let ptr = unsafe { memory::allocate(layout.size()) };
                if ptr.is_err() {
                    panic!("Bellek ayırma hatası");
                }
                Vec {
                    data: ptr.unwrap() as *mut T,
                    len: 0,
                    capacity,
                }
            }

            pub fn push(&mut self, value: T) {
                if self.len == self.capacity {
                    // Kapasiteyi artırma (basit bir örnek)
                    let new_capacity = if self.capacity == 0 { 4 } else { self.capacity * 2 };
                    let new_layout = core::alloc::Layout::array::<T>(new_capacity).unwrap();
                    let new_ptr = unsafe { memory::allocate(new_layout.size()) }.unwrap() as *mut T;

                    // Mevcut verileri yeni alana kopyala
                    if !self.data.is_null() {
                        unsafe {
                            core::ptr::copy_nonoverlapping(self.data, new_ptr, self.len);
                            let old_layout = core::alloc::Layout::array::<T>(self.capacity).unwrap();
                            memory::free(self.data as *mut u8, old_layout.size()).unwrap();
                        }
                    }
                    self.data = new_ptr;
                    self.capacity = new_capacity;
                }
                unsafe {
                    core::ptr::write(self.data.add(self.len), value);
                }
                self.len += 1;
            }

            pub fn extend_from_slice(&mut self, slice: &[T])
            where
                T: Copy,
            {
                for &item in slice {
                    self.push(item);
                }
            }

            pub fn as_mut_ptr(&mut self) -> *mut T {
                self.data
            }

            pub fn len(&self) -> usize {
                self.len
            }

            pub fn is_empty(&self) -> bool {
                self.len == 0
            }

            pub fn get_ref(&self) -> &[T] {
                unsafe { core::slice::from_raw_parts(self.data, self.len) }
            }
        }

        impl<T> Drop for Vec<T> {
            fn drop(&mut self) {
                if !self.data.is_null() {
                    let layout = core::alloc::Layout::array::<T>(self.capacity).unwrap();
                    unsafe {
                        memory::free(self.data as *mut u8, layout.size()).unwrap();
                    }
                }
            }
        }
    }
    pub mod prelude {
        pub use core::result::Result;
        pub use core::option::Option;
        pub use super::vec::Vec;
    }
}

// Test modülünde std::fs kullanıldığı için bu da Sahne64'e uyarlanmalı
#[cfg(test)]
mod tests_sahne {
    use super::*;
    use crate::fs::{self, O_RDONLY, O_CREAT, O_WRONLY};

    // Yardımcı fonksiyon: Bir dosyayı okuyup içeriğini byte dizisi olarak döndürür
    fn read_file_bytes(filename: &str) -> Result<Vec<u8>, SahneError> {
        let fd = fs::open(filename, O_RDONLY)?;
        let mut buffer = Vec::new();
        let mut temp_buffer = [0u8; 1024]; // Okuma için geçici bir arabellek
        loop {
            let bytes_read = fs::read(fd, &mut temp_buffer)?;
            if bytes_read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp_buffer[..bytes_read]);
        }
        fs::close(fd)?;
        Ok(buffer)
    }

    // Yardımcı fonksiyon: Bir byte dizisini bir dosyaya yazar
    fn write_bytes_to_file(filename: &str, bytes: &[u8]) -> Result<(), SahneError> {
        let fd = fs::open(filename, O_CREAT | O_WRONLY)?;
        fs::write(fd, bytes)?;
        fs::close(fd)?;
        Ok(())
    }

    #[test]
    fn test_read_write_bmp_sahne() {
        // Basit bir test.bmp oluşturuyoruz (elle veya test ortamı tarafından sağlanmalı)
        let mut dummy_image_data = vec![255u8; 24]; // Beyaz 2x1 görüntü, 24bpp
        let header = BmpHeader {
            file_size: 54 + 24,
            image_data_offset: 54,
            width: 2,
            height: 1,
            bits_per_pixel: 24,
        };
        let dummy_image = BmpImage { header, data: dummy_image_data };
        dummy_image.write_to_file("test_data/test.bmp").unwrap();

        let image = BmpImage::read_from_file("test_data/test.bmp").unwrap();
        image.write_to_file("test_data/test_copy_original.bmp").unwrap();
        image.write_to_file_optimized("test_data/test_copy_optimized.bmp").unwrap();

        let image_copy_original = BmpImage::read_from_file("test_data/test_copy_original.bmp").unwrap();
        let image_copy_optimized = BmpImage::read_from_file("test_data/test_copy_optimized.bmp").unwrap();

        assert_eq!(image.header.file_size, image_copy_original.header.file_size);
        assert_eq!(image.data, image_copy_original.data);

        assert_eq!(image.header.file_size, image_copy_optimized.header.file_size);
        assert_eq!(image.data, image_copy_optimized.data);

        // Dosyaları byte byte karşılaştır
        let original_bytes = read_file_bytes("test_data/test_copy_original.bmp").unwrap();
        let optimized_bytes = read_file_bytes("test_data/test_copy_optimized.bmp").unwrap();
        assert_eq!(original_bytes, optimized_bytes, "Orijinal ve optimize edilmiş dosyalar aynı olmalı");
    }
}

// Bu satır, bu dosyanın bir kütüphane olduğunu belirtir.
// Eğer bu bir çalıştırılabilir dosya ise (örneğin, bir kullanıcı alanı uygulaması),
// `fn main() { ... }` fonksiyonunu burada tanımlamanız gerekebilir.
// Ancak, bu kod bir kütüphane olarak tasarlandığı için bu satır kalmalıdır.
pub mod lib {}