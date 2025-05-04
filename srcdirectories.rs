#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// HashMap, String, Vec gibi yapıları alloc crate'inden içeri aktar
#[cfg(not(feature = "std"))]
use alloc::collections::hash_map::HashMap; // no_std için core::collections yerine alloc::collections daha yaygın olabilir
#[cfg(feature = "std")]
use std::collections::HashMap; // std için

use alloc::string::String;
use alloc::vec::Vec;

// Sahne64 API modüllerinden bu dosyada doğrudan kullanılmayanları kaldırıyoruz.
// SahneError, eğer buradaki metotlar OS etkileşiminden kaynaklanan hataları
// döndürecek olursa gerekli olabilir, şimdilik methotlar Result dönmüyor.
// Eğer SahneError'ı bir yerlerde kullanmak gerekiyorsa, sadece onu içeri aktarabiliriz:
use crate::SahneError;


// Depolama aygıtı türleri
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StorageDeviceType {
    HDD,
    SSD,
    SATA,
    NVMe,
    SAS,
    UFS,
    eMMC,
    USB,
}

// Depolama aygıtı yapısı
// Bu yapı, Sahne64'teki Handle veya kaynak ID'si ile ilişkilendirilmelidir
// ancak bu dosya sadece bellek içi temsili sağlıyor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorageDevice {
    pub device_type: StorageDeviceType,
    pub device_id: String, // Aygıtı benzersiz şekilde tanımlamak için (örn. Sahne64 kaynak ID'si olabilir)
}

// Dizin girişi yapısı
// Bu, dosyaların veya alt dizinlerin inode numarası ve adını tutar.
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub inode: u64,        // Dosyanın/dizinin inode numarası
    pub name: String,      // Dosyanın/dizinin adı
    // pub device: StorageDevice, // Dizin girişinin bulunduğu aygıt - Bu bilgi Directory yapısında üst seviyede saklanabilir
}

// Dizin yapısı
// Dizin adlarını inode numaralarına (veya DirectoryEntry'lere) eşleyen bellek içi harita.
pub struct Directory {
    // pub entries: HashMap<String, DirectoryEntry>, // Orijinal, DirectoryEntry içinde tekrar device tutuyordu
    pub entries: HashMap<String, u64>, // Dizin adı -> Inode numarası eşlemesi daha tipik
    // Not: Bir dizin içindeki tüm girdilerin aynı aygıtta olduğu varsayılıyor.
    // Farklı aygıtlardaki girdileri temsil etmek FileSystem yapısının sorumluluğundadır.
}

impl Directory {
    // Yeni boş bir dizin oluşturma
    pub fn new() -> Directory {
        Directory {
            entries: HashMap::new(),
        }
    }

    // Dizin girişini ekleme
    // Sadece dosya adını ve inode numarasını saklarız. Aygıt bilgisi üst katmandan gelir.
    pub fn add_entry(&mut self, name: String, inode: u64) { // device parametresi kaldırıldı
        self.entries.insert(name, inode);
    }

    // Dizin girişini alma (inode numarası)
    pub fn get_entry_inode(&self, name: &str) -> Option<u64> { // İsim düzeltildi, u64 döner
        self.entries.get(name).copied() // copied() Option<&u64> -> Option<u64> dönüştürür
    }

    // Dizin girişini silme
    pub fn remove_entry(&mut self, name: &str) {
        self.entries.remove(name);
    }

    // Dizin giriş adlarını listeleme
    pub fn list_entry_names(&self) -> Vec<&String> { // Vec<&String> döner
        self.entries.keys().collect()
    }

     // Dizin girişlerini (isim ve inode) listeleme
     pub fn list_entries(&self) -> Vec<(String, u64)> {
         self.entries.iter()
             .map(|(name, inode)| (name.clone(), *inode)) // name.clone() String döndürmek için gerekli
             .collect()
     }


    // ... diğer işlevler ...
}

// Dosya sistemi yapısı
// Aygıtları ve onlara ait kök dizin yapılarını yönetir.
pub struct FileSystem {
     // StorageDevice'in Hash ve Eq türetilmiş olması gereklidir.
    pub devices: HashMap<StorageDevice, Directory>, // Aygıt -> Kök Dizin eşlemesi
    // Not: Bu yapı, bir aygıttaki tüm dizin hiyerarşisini bellek içinde
    // tutmak için tasarlanmış basit bir örnektir. Gerçek bir dosya sistemi
    // disk üzerindeki yapıyı okuyup yazacaktır.
}

impl FileSystem {
    // Yeni boş bir dosya sistemi oluşturma
    pub fn new() -> FileSystem {
        FileSystem {
            devices: HashMap::new(),
        }
    }

    // Aygıt ekleme
    // Yeni eklenen aygıt için boş bir kök dizin oluşturulur.
    pub fn add_device(&mut self, device: StorageDevice) {
        self.devices.insert(device, Directory::new());
    }

    // Belirtilen aygıttaki kök dizine giriş ekleme
    // name: eklenecek dosya/dizin adı
    // inode: ilgili inode numarası
    pub fn add_entry_to_device_root(&mut self, device: &StorageDevice, name: String, inode: u64) {
        if let Some(dir) = self.devices.get_mut(device) {
            dir.add_entry(name, inode); // Directory metodunu çağırır
        }
         // Hata yönetimi eklenebilir: Eğer aygıt bulunamazsa SahneError dönebilir.
         // Örn: Result<(), SahneError> dönüp aygıt bulunamazsa Err(SahneError::ResourceNotFound) dönebilir.
    }

    // Belirtilen aygıttaki kök dizinden giriş alma (inode numarası)
    pub fn get_entry_inode_from_device_root(&self, device: &StorageDevice, name: &str) -> Option<u64> {
        if let Some(dir) = self.devices.get(device) {
            dir.get_entry_inode(name) // Directory metodunu çağırır
        } else {
            None // Aygıt bulunamadı
        }
    }

    // Belirtilen aygıttaki kök dizinden giriş silme
    pub fn remove_entry_from_device_root(&mut self, device: &StorageDevice, name: &str) {
        if let Some(dir) = self.devices.get_mut(device) {
            dir.remove_entry(name); // Directory metodunu çağırır
        }
        // Hata yönetimi eklenebilir (aygıt bulunamazsa).
    }

    // Belirtilen aygıttaki kök dizin girişlerini listeleme (isim ve inode)
    pub fn list_entries_on_device_root(&self, device: &StorageDevice) -> Vec<(String, u64)> {
        if let Some(dir) = self.devices.get(device) {
            dir.list_entries() // Directory metodunu çağırır
        } else {
            Vec::new() // Aygıt bulunamadı, boş liste dön
        }
    }

    // ... diğer FileSystem seviyesi işlevler (örneğin, aygıtlara göre mount/unmount) ...
}

// Testler ve örnek kullanım (std veya alloc gerektirir)
// Bu kısım Sahne64 API'sına doğrudan bağımlı değildir, sadece veri yapılarının kullanımını gösterir.
#[cfg(test)]
#[cfg(feature = "std")] // veya #[cfg(any(feature = "std", feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::string::ToString; // to_string() metodu için gereklidir

    #[test]
    fn test_file_system_add_get_remove() {
        let mut fs = FileSystem::new();
        let device1 = StorageDevice {
            device_type: StorageDeviceType::SSD,
            device_id: "ssd1".to_string(),
        };
        let device2 = StorageDevice {
            device_type: StorageDeviceType::HDD,
            device_id: "hdd1".to_string(),
        };

        fs.add_device(device1.clone());
        fs.add_device(device2.clone());

        fs.add_entry_to_device_root(&device1, "file1.txt".to_string(), 10);
        fs.add_entry_to_device_root(&device2, "dir1".to_string(), 20);

        assert_eq!(
            fs.get_entry_inode_from_device_root(&device1, "file1.txt"),
            Some(10)
        );
        assert_eq!(
            fs.get_entry_inode_from_device_root(&device2, "dir1"),
            Some(20)
        );
        assert_eq!(
             fs.get_entry_inode_from_device_root(&device1, "nonexistent"),
             None
         );


        fs.remove_entry_from_device_root(&device1, "file1.txt");
        assert!(fs.get_entry_inode_from_device_root(&device1, "file1.txt").is_none());
    }

    #[test]
    fn test_file_system_list() {
        use alloc::string::ToString; // to_string() metodu için gereklidir

        let mut fs = FileSystem::new();
        let device1 = StorageDevice {
            device_type: StorageDeviceType::SSD,
            device_id: "ssd1".to_string(),
        };
        let device2 = StorageDevice {
            device_type: StorageDeviceType::HDD,
            device_id: "hdd1".to_string(),
        };

        fs.add_device(device1.clone());
        fs.add_device(device2.clone());

        fs.add_entry_to_device_root(&device1, "file_a".to_string(), 100);
        fs.add_entry_to_device_root(&device1, "file_b".to_string(), 101);
        fs.add_entry_to_device_root(&device2, "file_c".to_string(), 200);

        let entries1 = fs.list_entries_on_device_root(&device1);
        assert_eq!(entries1.len(), 2);
         // Sıralama garanti edilmez, içerikleri kontrol edelim.
         assert!(entries1.contains(&("file_a".to_string(), 100)));
         assert!(entries1.contains(&("file_b".to_string(), 101)));


        let entries2 = fs.list_entries_on_device_root(&device2);
        assert_eq!(entries2.len(), 1);
        assert_eq!(entries2[0], ("file_c".to_string(), 200));

         let entries3 = fs.list_entries_on_device_root(&StorageDevice { device_type: StorageDeviceType::USB, device_id: "nonexistent".to_string() });
         assert!(entries3.is_empty()); // Olmayan aygıtın listesi boş olmalı
    }
}

// Tek örnek kullanım senaryosu (std veya alloc gerektirir)
#[cfg(feature = "example_directories")] // Farklı bir özellik bayrağı kullanıldı
#[cfg(any(feature = "std", feature = "alloc"))]
fn main() { // main fonksiyonu Result dönmeyebilir
    use super::*;
    use alloc::string::ToString; // to_string() metodu için gereklidir
    // no_std ortamında print/println makrolarının kullanılabilir olduğundan emin olun.
    #[cfg(not(feature = "std"))]
    crate::init_console(crate::Handle(3)); // Varsayımsal konsol handle'ı ayarlama


    // Yeni bir dosya sistemi oluştur
    let mut fs = FileSystem::new();

    // Depolama aygıtları oluştur
    let ssd_device = StorageDevice {
        device_type: StorageDeviceType::SSD,
        device_id: "main-ssd".to_string(),
    };

    let usb_device = StorageDevice {
        device_type: StorageDeviceType::USB,
        device_id: "backup-usb".to_string(),
    };

    // Aygıtları dosya sistemine ekle
    fs.add_device(ssd_device.clone());
    fs.add_device(usb_device.clone());

    // SSD aygıtının kök dizinine giriş ekle
    fs.add_entry_to_device_root(&ssd_device, "application.exe".to_string(), 1024);
    fs.add_entry_to_device_root(&ssd_device, "documents".to_string(), 2048);
    fs.add_entry_to_device_root(&ssd_device, "image.png".to_string(), 4096);

    // USB aygıtının kök dizinine giriş ekle (yedekleme amaçlı)
    fs.add_entry_to_device_root(&usb_device, "backup_documents.zip".to_string(), 8192);

    // SSD aygıtındaki kök dizin girişlerini listele
    println!("SSD Aygıtındaki Kök Dizin Girişleri:");
    let ssd_entries = fs.list_entries_on_device_root(&ssd_device);
    for (name, inode) in &ssd_entries {
        println!("- {} (Inode: {})", name, inode); // DirectoryEntry yerine tuple kullanıldı
    }

    // USB aygıtındaki kök dizin girişlerini listele
    println!("\nUSB Aygıtındaki Kök Dizin Girişleri:");
    let usb_entries = fs.list_entries_on_device_root(&usb_device);
     for (name, inode) in &usb_entries {
         println!("- {} (Inode: {})", name, inode);
     }


    // Bir dizin girişini al ve kontrol et (inode numarası)
    if let Some(doc_inode) = fs.get_entry_inode_from_device_root(&ssd_device, "documents") {
        println!("\n'documents' Dizin Girişi bulundu: Inode: {}", doc_inode);
    } else {
        println!("\n'documents' Dizin Girişi bulunamadı.");
    }
     if let Some(_) = fs.get_entry_inode_from_device_root(&ssd_device, "nonexistent") {
         println!("Hata: Olmayan giriş bulundu!");
     } else {
         println!("Olmayan giriş 'nonexistent' doğru şekilde bulunamadı.");
     }


    // Bir dizin girişini sil
    fs.remove_entry_from_device_root(&ssd_device, "image.png");
    println!("\n'image.png' Dizin Girişi silindi.");

    // SSD aygıtındaki güncel kök dizin girişlerini listele
    println!("\nSSD Aygıtındaki Güncel Girişler (image.png silindikten sonra):");
    let current_ssd_entries = fs.list_entries_on_device_root(&ssd_device);
     for (name, inode) in &current_ssd_entries {
         println!("- {} (Inode: {})", name, inode);
     }

     // Silinen girişin artık olmadığını kontrol et
     if let Some(_) = fs.get_entry_inode_from_device_root(&ssd_device, "image.png") {
         println!("Hata: Silinen giriş hala bulundu!");
     } else {
         println!("'image.png' girişi başarıyla silindi.");
     }

}
