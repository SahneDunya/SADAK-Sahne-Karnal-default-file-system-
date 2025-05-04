#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc; // format_args! gibi makrolar için gerekebilir

// Gerekli Sahne64 modüllerini içeri aktar
use crate::{
    resource, // fs modülü yerine resource modülü kullanıldı
    memory,
    task,     // process modülü yerine task modülü kullanıldı
    sync,
    kernel,
    SahneError,
    arch,
    Handle,   // Handle tipi eklendi
};

use core::result::Result;
use core::cmp::min; // core::cmp::min kullanıldı

// Not: SeekFrom enum tanımı bu dosyadan kaldırıldı, merkezi bir yerde tanımlanmalı.

/// Represents a block device implemented on a Sahne64 resource.
pub struct Device {
    /// The underlying resource Handle representing the device.
    handle: Handle, // fd yerine handle
    /// The size of each block in bytes.
    block_size: u64,
    /// The total size of the device in bytes (kaynağın gerçek boyutu API'ye bağlıdır).
    size: u64, // Bu, istenen boyuttur, kaynağın gerçek boyutu farklı olabilir.
}

impl Device {
    /// Creates a new Device instance by acquiring a Sahne64 resource.
    ///
    /// Acquires (or creates if it doesn't exist) a resource at the given ID,
    /// and initializes a Device struct with the specified block size and desired size.
    /// Note: Setting the resource's actual size might require a separate API call (like truncate/set_len)
    /// which may not be available in the current Sahne64 API. The `size` field in this struct
    /// stores the *desired* size, not necessarily the *actual* size of the underlying resource.
    ///
    /// # Arguments
    ///
    /// * `resource_id` - The Sahne64 resource ID to be used as the device.
    /// * `block_size` - The size of each block in bytes.
    /// * `desired_size` - The desired total size of the device in bytes.
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `Device` instance, or an error if resource operations fail.
    pub fn new(resource_id: &str, block_size: u64, desired_size: u64) -> Result<Self, SahneError> {
        // resource::acquire ile kaynağı edinme
        let flags = resource::MODE_READ | resource::MODE_WRITE | resource::MODE_CREATE | resource::MODE_TRUNCATE; // TRUNCATE ekleyelim, belki acquire boyutu sıfırlar?
        let handle = resource::acquire(resource_id, flags)?; // fs::open yerine resource::acquire

        // Sahne64'te kaynağın boyutunu ayarlamak için bir sistem çağrısı (örneğin, ftruncate benzeri)
        // mevcut olmayabilir. desired_size alanını saklıyoruz ama alttaki kaynağın
        // boyutu bu değere ayarlanmamış olabilir.
        // TODO: Eğer Sahne64 API'sında bir 'set_size' veya 'truncate' syscall'ı varsa, burada çağrılmalı.
         resource::control(handle, resource::CONTROL_SET_SIZE, desired_size)?;

        Ok(Device {
            handle,
            block_size,
            size: desired_size, // İstenen boyutu sakla
        })
    }

    /// Releases the underlying resource handle.
    pub fn close(&mut self) -> Result<(), SahneError> {
        resource::release(self.handle)
    }


    /// Reads a block from the device.
    ///
    /// Reads data from the block specified by `block_num` into the provided buffer `buf`.
    ///
    /// # DİKKAT: Sahne64 API Kısıtlaması
    /// Sahne64 resource::read syscall'ı doğrudan ofset parametresi almaz.
    /// resource::read muhtemelen kaynağın mevcut konumundan okur.
    /// Bu implementasyon, `block_num`'dan hesaplanan ofseti DOĞRUDAN KULLANMAZ.
    /// Bunun yerine, kaynağın mevcut konumundan okuma yapar. Gerçek bir blok cihazı
    /// gibi çalışması için Sahne64 API'sına ofsetli okuma/yazma veya seek syscall'ı
    /// eklenmelidir.
    ///
    /// # Arguments
    ///
    /// * `block_num` - The block number to read from (0-indexed).
    /// * `buf` - The buffer to read data into. The buffer should be large enough to hold a block.
    ///
    /// # Returns
    ///
    /// A `Result` containing the number of bytes read, or an error if the read operation fails
    /// (e.g., block number out of range, I/O error).
    pub fn read_block(&mut self, block_num: u64, buf: &mut [u8]) -> Result<usize, SahneError> {
        let offset = block_num * self.block_size;

        if offset >= self.size {
            // Bu kontrol sadece istenen boyuta göre yapılır, kaynağın gerçek boyutu farklı olabilir.
            println!("WARN: Device::read_block offset {} istenen cihaz boyutu {} dışında!", offset, self.size); // no_std print makrosu
            return Err(SahneError::InvalidParameter); // InvalidInput yerine InvalidParameter daha uygun olabilir.
        }

        // TODO: Eğer Sahne64'te seek benzeri bir resource::control komutu varsa,
        // burada önce o komut çağrılarak offset ayarlanmalıdır:
         resource::control(self.handle, resource::CONTROL_SEEK, offset)?;
        // Ardından resource::read çağrılır.

        // Şimdilik, offset parametresini yoksayarak doğrudan okuyoruz.
        // BU YANLIŞ DAVRANIŞTIR, Sahne64 API'sındaki eksikliği yansıtır.
        println!("WARN: Device::read_block block_num {} (offset {}) parametresini yoksayıyor!", block_num, offset); // no_std print makrosu

        if buf.len() as u64 != self.block_size {
             println!("ERROR: Device::read_block buffer boyutu blok boyutuna ({}) eşit değil ({})!", self.block_size, buf.len());
             return Err(SahneError::InvalidParameter); // Buffer boyutu blok boyutuna eşit olmalı
        }

        resource::read(self.handle, buf) // resource::read kullanıldı
         // Okuma başarılıysa, okunan byte sayısını döndürür.
         // resource::read tam olarak buffer.len() okumayabilir (örn. dosya sonu).
         // Blok cihaz mantığı tam blok okumayı bekler, bu durum burada ele alınmalıdır.
    }

    /// Writes a block to the device.
    ///
    /// Writes data from the buffer `buf` to the block specified by `block_num`.
    ///
     /// # DİKKAT: Sahne64 API Kısıtlaması
     /// Sahne64 resource::write syscall'ı doğrudan ofset parametresi almaz.
     /// resource::write muhtemelen kaynağın mevcut konumundan yazar.
     /// Bu implementasyon, `block_num`'dan hesaplanan ofseti DOĞRUDAN KULLANMAZ.
     /// Bunun yerine, kaynağın mevcut konumuna yazma yapar. Gerçek bir blok cihazı
     /// gibi çalışması için Sahne64 API'sına ofsetli okuma/yazma veya seek syscall'ı
     /// eklenmelidir.
    ///
    /// # Arguments
    ///
    /// * `block_num` - The block number to write to (0-indexed).
    /// * `buf` - The buffer containing the data to write. The buffer's length should ideally be equal to the block size.
    ///
    /// # Returns
    ///
    /// A `Result` containing the number of bytes written, or an error if the write operation fails
    /// (e.g., block number out of range, I/O error).
    pub fn write_block(&mut self, block_num: u64, buf: &[u8]) -> Result<usize, SahneError> {
        let offset = block_num * self.block_size;

        if offset >= self.size {
            // Bu kontrol sadece istenen boyuta göre yapılır, kaynağın gerçek boyutu farklı olabilir.
            println!("WARN: Device::write_block offset {} istenen cihaz boyutu {} dışında!", offset, self.size); // no_std print makrosu
            return Err(SahneError::InvalidParameter); // InvalidInput yerine InvalidParameter daha uygun olabilir.
        }

         // TODO: Eğer Sahne64'te seek benzeri bir resource::control komutu varsa,
         // burada önce o komut çağrılarak offset ayarlanmalıdır:
         // resource::control(self.handle, resource::CONTROL_SEEK, offset)?;
         // Ardından resource::write çağrılır.

        // Şimdilik, offset parametresini yoksayarak doğrudan yazıyoruz.
        // BU YANLIŞ DAVRANIŞTIR, Sahne64 API'sındaki eksikliği yansıtır.
        println!("WARN: Device::write_block block_num {} (offset {}) parametresini yoksayıyor!", block_num, offset); // no_std print makrosu

        if buf.len() as u64 != self.block_size {
            println!("ERROR: Device::write_block buffer boyutu blok boyutuna ({}) eşit değil ({})!", self.block_size, buf.len());
            return Err(SahneError::InvalidParameter); // Buffer boyutu blok boyutuna eşit olmalı
        }

        resource::write(self.handle, buf) // resource::write kullanıldı
         // Yazma başarılıysa, yazılan byte sayısını döndürür.
         // resource::write tam olarak buffer.len() yazmayabilir (örn. disk dolu).
         // Blok cihaz mantığı tam blok yazmayı bekler, bu durum burada ele alınmalıdır.
         // Orijinal kodda tam yazılmadıysa WriteFault dönüyordu, bu kontrol eklenebilir.
           match resource::write(self.handle, buf) {
              Ok(bytes_written) => {
                  if bytes_written == buf.len() { Ok(bytes_written) }
                  else { Err(SahneError::CommunicationError) } // Veya WriteFault
              },
              Err(e) => Err(e),
          }
    }

    /// Returns the block size of the device.
    pub fn block_size(&self) -> u64 {
        self.block_size
    }

    /// Returns the total size of the device in bytes.
    /// Note: This returns the *desired* size provided during creation, not necessarily
    /// the actual size of the underlying resource if the Sahne64 API lacks a way to get/set it.
    pub fn size(&self) -> u64 {
        self.size
    }

     // TODO: Bir `actual_size()` metodu ekleyerek Sahne64 API'sından
     // kaynağın gerçek boyutunu almaya çalışabiliriz (eğer size almak için
     // bir resource::control komutu varsa).
}

// Gerekli SeekFrom tanımı bu dosyadan kaldırıldı, merkezi bir yerde tanımlanmalı.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum SeekFrom { ... }


#[cfg(feature = "example")] // Sadece 'example' özelliği aktifse derle
fn main() -> Result<(), SahneError> {
    // no_std ortamında print/println makrolarının kullanılabilir olduğundan emin olun.
    #[cfg(not(feature = "std"))]
    crate::init_console(crate::Handle(3)); // Varsayımsal konsol handle'ı ayarlama

    let resource_id = "sahne://devices/disk1"; // Sahne64 kaynak tanımlayıcısı
    let block_size: u64 = 512; // Örnek blok boyutu
    let desired_size: u64 = 1024 * 1024; // 1MB örnek istenen boyut

    // Cihazı aç (kaynağı edin)
    let mut device = match Device::new(resource_id, block_size, desired_size) {
        Ok(dev) => dev,
        Err(e) => {
            eprintln!("Device açma hatası ('{}'): {:?}", resource_id, e);
            return Err(e);
        }
    };
    println!("Kaynak '{}' başarıyla edinildi ve Device yöneticisi başlatıldı.", resource_id);
    println!("İstenen cihaz boyutu: {} bayt, blok boyutu: {} bayt", device.size(), device.block_size());


    // Veri yazma
    let write_data: alloc::vec::Vec<u8> = (0..device.block_size as u8).collect(); // Blok boyutunda örnek veri (alloc gerektirir)
    let write_block_num = 5; // 5 numaralı bloğa yazılacak

    // DİKKAT: write_block şu an ofseti yoksayıyor! Muhtemelen kaynağın mevcut konumuna (örn. başı) yazar.
    println!("Blok {}'a (offset {}) yazma denemesi...", write_block_num, write_block_num * device.block_size());
    match device.write_block(write_block_num, &write_data) {
        Ok(bytes_written) => {
             println!("Yazma başarılı ({} byte yazıldı).", bytes_written);
             if bytes_written != device.block_size as usize {
                 println!("UYARI: Tam blok ({}) yazılamadı!", device.block_size());
             }
        }
        Err(e) => eprintln!("Yazma hatası: {:?}", e),
    }

    // Veri okuma
    let mut read_data: alloc::vec::Vec<u8> = alloc::vec![0u8; device.block_size as usize]; // Okuma için buffer (alloc gerektirir)
    let read_block_num = 5; // 5 numaralı bloktan okunacak (ancak ofset yine yoksayılır).

    // DİKKAT: read_block şu an ofseti yoksayıyor! Muhtemelen kaynağın mevcut konumundan okur.
    println!("Blok {}'dan (offset {}) okuma denemesi...", read_block_num, read_block_num * device.block_size());
     // Okuma öncesinde belki kaynağın konumunu başa almak mantıklı olabilir (seek desteklenseydi):
      device.seek(SeekFrom::Start(0))?; // Seek desteklenmiyor.
    match device.read_block(read_block_num, &mut read_data) {
        Ok(bytes_read) => {
            println!("Okuma başarılı ({} byte okundu).", bytes_read);
            if bytes_read > 0 {
                 println!("Okunan ilk 10 byte: {:?}", &read_data[..min(10, bytes_read)]);
            }
        }
        Err(e) => eprintln!("Okuma hatası: {:?}", e),
    }

     // Yazılan ve okunan veriyi karşılaştırma (std veya alloc gerektirir)
     // DİKKAT: Ofset sorunları nedeniyle bu karşılaştırma büyük olasılıkla başarısız olacaktır.
     // Çünkü yazma ve okuma muhtemelen farklı konumlara gerçekleşmiştir.
     #[cfg(any(feature = "std", feature = "alloc"))]
     if write_data == read_data {
         println!("Yazılan ve okunan veriler (muhtemelen offset 0'dan) eşleşiyor.");
     } else {
         println!("Yazılan ve okunan veriler EŞLEŞMİYOR. (Ofset sorunları nedeniyle bekleniyor).");
     }


    // Cihazı kapat (kaynağı serbest bırak)
     match device.close() {
         Ok(_) => println!("Device kapatıldı."),
         Err(e) => eprintln!("Device kapatma hatası: {:?}", e),
     }


    Ok(())
}
