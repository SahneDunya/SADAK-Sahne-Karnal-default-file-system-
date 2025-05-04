#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// VFS trait'leri ve SahneError
use crate::vfs::{VfsNode, VfsNodeType}; // VFS modülü dışarıdan import edilir
use crate::SahneError; // Sahne64 API'sından gelen temel hata tipi (veya genel proje hatası)

// Bellek için Vec
use alloc::vec::Vec;

// core kütüphanesinden gerekli modüller
use core::cmp;
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io importları

// unused Sahne64 module imports removed: fs, memory, process, sync, kernel, arch

pub struct AccdbFile {
    /// ACCDB dosyasının içeriği (bellekte tutulur).
    data: Vec<u8>,
    /// İç okuma/yazma pozisyonunu takip etmek için imleç.
    cursor: usize,
}

impl AccdbFile {
    /// Yeni bir AccdbFile örneği oluşturur ve verilen veriyi içeriği olarak kullanır.
    ///
    /// # Not
    /// Verinin kalıcı depolamadan (örn. Sahne64 kaynağından) yüklenmesi bu metodun
    /// sorumluluğunda değildir. Bu metod sadece bellekteki bir Vec<u8> ile çalışır.
    pub fn new(data: Vec<u8>) -> Self {
        AccdbFile { data, cursor: 0 } // İmleci başlangıçta 0 olarak ayarla
    }

    /// Dosyanın içeriğine doğrudan erişim sağlar. İmleci etkilemez.
    ///
    /// # Arguments
    ///
    /// * `offset` - Okunacak verinin başlangıç ofseti (byte).
    /// * `size` - Okunacak verinin boyutu (byte).
    ///
    /// # Returns
    ///
    /// Belirtilen ofset ve boyuttaki veri dilimi (slice) veya ofset geçersizse None.
    pub fn read_at(&self, offset: usize, size: usize) -> Option<&[u8]> {
        if offset >= self.data.len() {
            return None; // Offset dosya boyutunun dışında
        }
        let end = cmp::min(offset + size, self.data.len()); // Bitişi dosya boyutu ile sınırla
        Some(&self.data[offset..end])
    }

    /// Dosyanın belirli bir ofsetine doğrudan veri yazar. İmleci etkilemez.
    /// Yazılacak verinin dosya boyutunu aşmamasına dikkat edilmelidir.
    ///
    /// # Arguments
    ///
    /// * `offset` - Yazılacak verinin başlangıç ofseti (byte).
    /// * `data` - Yazılacak veri dilimi (slice).
    ///
    /// # Returns
    ///
    /// İşlem başarılı olursa Ok(()), ofset veya boyut geçersizse SahneError::InvalidParameter.
    ///
    /// # Not
    /// Bu metod, dosyanın boyutunu otomatik olarak artırmaz. Sadece mevcut veri içinde yazar.
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> Result<(), SahneError> {
        if offset >= self.data.len() {
            // Ofset dosya boyutunun dışında
            println!("ERROR: AccdbFile::write_at offset {} dosya boyutu {} dışında!", offset, self.data.len()); // no_std uyumlu println!
            return Err(SahneError::InvalidParameter); // Daha spesifik hata türü olabilir.
        }
        if offset + data.len() > self.data.len() {
            // Yazılacak veri dosya boyutunu aşıyor
            println!("ERROR: AccdbFile::write_at yazma işlemi dosya boyutunu aşıyor!"); // no_std uyumlu println!
            return Err(SahneError::InvalidParameter); // Daha spesifik hata türü olabilir (örn. SahneError::OutOfSpace, SahneError::InvalidOperation).
        }
        self.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Bellekteki veriyi içeren Vec<u8>'e erişim sağlar.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Bellekteki veriyi içeren Vec<u8>'e değiştirilebilir erişim sağlar.
      #[cfg(feature = "std")] // Sadece testler için gerekebilir, production kodunda dikkatli kullanılmalı
      pub fn as_bytes_mut(&mut self) -> &mut [u8] {
          &mut self.data
      }

     /// Dosyanın toplam boyutunu döndürür (bellekteki veri boyutu).
     pub fn len(&self) -> usize {
         self.data.len()
     }

     /// Dosyanın boş olup olmadığını kontrol eder.
     pub fn is_empty(&self) -> bool {
         self.data.is_empty()
     }

     /// Dosyanın imleç pozisyonunu döndürür.
     pub fn stream_position(&self) -> usize {
         self.cursor
     }
}

impl VfsNode for AccdbFile {
    /// Düğüm tipini döndürür (File).
    fn get_type(&self) -> VfsNodeType {
        VfsNodeType::File
    }

    /// Dosyanın boyutunu döndürür.
    fn get_size(&self) -> usize {
        self.len() // len() metodunu kullanalım
    }
    // VfsNode trait'i başka metotlar da gerektirebilir (izinler, sahiplik vb.)
}

// core::io::Read trait implementasyonu
impl Read for AccdbFile {
    /// İmleçten başlayarak veriyi okur ve imleci ilerletir.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        if self.cursor >= self.data.len() {
            return Ok(0); // Dosya sonuna gelindi
        }
        let bytes_available = self.data.len() - self.cursor;
        let bytes_to_read = cmp::min(buf.len(), bytes_available);

        buf[..bytes_to_read].copy_from_slice(&self.data[self.cursor..self.cursor + bytes_to_read]);
        self.cursor += bytes_to_read; // İmleci ilerlet

        Ok(bytes_to_read)
    }

    // core::io::Read trait'inin diğer metotları (read_to_end, read_exact vb.)
    // otomatik olarak read() üzerine default implementasyonlara sahiptir.
}

// core::io::Seek trait implementasyonu
impl Seek for AccdbFile {
    /// İmlecin pozisyonunu ayarlar ve yeni pozisyonu döndürür.
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize, // isize kullanarak negatif ofsetleri handle et
            SeekFrom::End(offset) => {
                (self.data.len() as isize).checked_add(offset)
                    .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, "Seek position out of bounds (from end)"))?
            },
            SeekFrom::Current(offset) => {
                (self.cursor as isize).checked_add(offset)
                     .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, "Seek position out of bounds (from current)"))?
            },
        };

        if new_pos < 0 {
             // Negatif seek pozisyonu hatadır.
            return Err(CoreIOError::new(CoreIOErrorKind::InvalidInput, "Invalid seek position (result is negative)"));
        }

        let new_pos_usize = new_pos as usize;

         // Yeni pozisyon dosya boyutunu aşıyorsa ne yapılacağına karar verilmeli.
         // Standart davranış, dosya sonuna seek etmeye izin vermektir, ancak dosya dışına okuma/yazma hata verir.
         // Seek metodu, dosya sonunu aşan bir konuma gitmeye izin verebilir.
         // read/write metotları daha sonra dosya sonunu kontrol eder.
         // Bu implementasyon, yeni pozisyonu dosya sonu ile sınırlamaz.

        self.cursor = new_pos_usize;
        Ok(self.cursor as u64)
    }

    // core::io::Seek trait'inin diğer metotları (stream_position vb.)
    // otomatik olarak seek() üzerine default implementasyonlara sahiptir.
     stream_position() metodunu yukarıda zaten ekledik, burada da kullanabiliriz.
     fn stream_position(&mut self) -> Result<u64, CoreIOError> {
         Ok(self.cursor as u64)
     } // Default implementasyon yeterli olabilir.
}

// Testler (std veya alloc gerektirir)
#[cfg(test)]
#[cfg(any(feature = "std", feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec; // vec! macro'su için
    use alloc::string::ToString; // to_string() metodu için

    // core::io::Read ve Seek testleri için helpers
    // Bunlar std::io::Read ve Seek test helperlarına benzer olabilir.
    // Basit manüel testler yazalım.

    #[test]
    fn test_accdbfile_read() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut file = AccdbFile::new(data.clone());

        let mut buf = [0u8; 5];
        let bytes_read = file.read(&mut buf).unwrap();
        assert_eq!(bytes_read, 5);
        assert_eq!(&buf, &[1, 2, 3, 4, 5]);
        assert_eq!(file.stream_position(), 5);

        let mut buf2 = [0u8; 10];
        let bytes_read2 = file.read(&mut buf2).unwrap();
        assert_eq!(bytes_read2, 5);
        assert_eq!(&buf2[..5], &[6, 7, 8, 9, 10]);
        assert_eq!(file.stream_position(), 10);

        let mut buf3 = [0u8; 5];
        let bytes_read3 = file.read(&mut buf3).unwrap();
        assert_eq!(bytes_read3, 0); // Dosya sonu
        assert_eq!(file.stream_position(), 10);
    }

    #[test]
    fn test_accdbfile_read_at() {
        let data = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let file = AccdbFile::new(data.clone());

        let slice1 = file.read_at(3, 4).unwrap();
        assert_eq!(slice1, &[40, 50, 60, 70]);

        let slice2 = file.read_at(0, 5).unwrap();
        assert_eq!(slice2, &[10, 20, 30, 40, 50]);

        let slice3 = file.read_at(8, 5).unwrap(); // Sondan sonra okuma
        assert_eq!(slice3, &[90, 100]); // min(8+5, 10) = min(13, 10) = 10, slice[8..10]

        let slice4 = file.read_at(10, 5); // Tam dosya sonunda
        assert!(slice4.is_none()); // veya Some(&[]) ? Mevcut implementasyon None dönüyor.

        let slice5 = file.read_at(15, 5); // Dosya dışı
        assert!(slice5.is_none());
    }

    #[test]
    fn test_accdbfile_write_at() {
         use crate::SahneError; // SahneError'ı kullanıyoruz
         use alloc::string::String; // String için

        let mut data = vec![0u8; 10];
        let mut file = AccdbFile::new(data.clone());

        let write_data1 = &[1, 2, 3];
        file.write_at(2, write_data1).unwrap();
        assert_eq!(file.as_bytes(), &[0, 0, 1, 2, 3, 0, 0, 0, 0, 0]);

        let write_data2 = &[99, 98];
        file.write_at(8, write_data2).unwrap();
        assert_eq!(file.as_bytes(), &[0, 0, 1, 2, 3, 0, 0, 0, 99, 98]);

        let write_data3 = &[10, 20, 30];
        let write_result3 = file.write_at(8, write_data3); // Boyutu aşıyor
        assert!(write_result3.is_err());
        assert_eq!(write_result3.unwrap_err(), SahneError::InvalidParameter); // SahneError::InvalidParameter döner.
         // assert!(matches!(write_result3.unwrap_err(), SahneError::InvalidParameter)); // Rust 1.39+

        let write_data4 = &[100];
        let write_result4 = file.write_at(10, write_data4); // Tam dosya sonunda, boyut 10
        assert!(write_result4.is_err()); // Ofset dosya boyutu >= olduğu için hata döner.
         assert_eq!(write_result4.unwrap_err(), SahneError::InvalidParameter);

        let write_data5 = &[100];
        let write_result5 = file.write_at(15, write_data5); // Dosya dışında
        assert!(write_result5.is_err());
         assert_eq!(write_result5.unwrap_err(), SahneError::InvalidParameter);
    }


     #[test]
     fn test_accdbfile_seek() {
         let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
         let mut file = AccdbFile::new(data.clone());

         let pos1 = file.seek(SeekFrom::Start(5)).unwrap();
         assert_eq!(pos1, 5);
         assert_eq!(file.stream_position(), 5);

         let mut buf1 = [0u8; 3];
         file.read(&mut buf1).unwrap();
         assert_eq!(&buf1, &[6, 7, 8]);
         assert_eq!(file.stream_position(), 8);

         let pos2 = file.seek(SeekFrom::Current(-2)).unwrap(); // 8 - 2 = 6
         assert_eq!(pos2, 6);
         assert_eq!(file.stream_position(), 6);

         let mut buf2 = [0u8; 3];
         file.read(&mut buf2).unwrap();
         assert_eq!(&buf2, &[7, 8, 9]); // 6, 7, 8, 9 -> imleç 6'dan okur 7, 8, 9.
         assert_eq!(file.stream_position(), 9);

         let pos3 = file.seek(SeekFrom::End(-1)).unwrap(); // data.len() - 1 = 10 - 1 = 9
         assert_eq!(pos3, 9);
         assert_eq!(file.stream_position(), 9);

         let mut buf3 = [0u8; 5];
         let bytes_read3 = file.read(&mut buf3).unwrap();
         assert_eq!(bytes_read3, 1); // Sadece 10 okunur
         assert_eq!(&buf3[..1], &[10]);
         assert_eq!(file.stream_position(), 10);

         let pos4 = file.seek(SeekFrom::End(0)).unwrap(); // Tam dosya sonu
         assert_eq!(pos4, 10);
         assert_eq!(file.stream_position(), 10);

         let pos5 = file.seek(SeekFrom::Start(15)); // Dosya dışı
          // core::io::Seek default implementasyonu dosya dışına seek etmeye izin verir.
          // Cursor dosya boyutundan büyük olabilir.
          assert!(pos5.is_ok()); // core::io::Seek hata dönmüyor
          assert_eq!(pos5.unwrap(), 15);
          assert_eq!(file.stream_position(), 15);
          let mut buf4 = [0u8; 5];
          let bytes_read4 = file.read(&mut buf4).unwrap();
          assert_eq!(bytes_read4, 0); // Dosya dışında okuma 0 byte döner.


     }
}

// Tekrarlanan no_std print modülü ve panic handler kaldırıldı.
