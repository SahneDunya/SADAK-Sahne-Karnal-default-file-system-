#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

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

#[cfg(not(feature = "std"))]
use core::result::Result;

#[cfg(not(feature = "std"))]
use core::collections::hash_map::HashMap; // Eğer Sahne64'te bir HashMap benzeri yapı varsa onu kullanmalıyız.

#[cfg(feature = "std")]
use std::collections::HashMap;

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorageDevice {
    pub device_type: StorageDeviceType,
    pub device_id: String, // Aygıtı benzersiz şekilde tanımlamak için
}

// Dizin girişi yapısı
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub inode: u64,
    pub name: String,
    pub device: StorageDevice, // Dizin girişinin bulunduğu aygıt
}

// Dizin yapısı
#[derive(Debug, Clone)]
pub struct Directory {
    pub entries: HashMap<String, DirectoryEntry>,
}

impl Directory {
    // Yeni bir dizin oluşturma
    pub fn new() -> Directory {
        Directory {
            entries: HashMap::new(),
        }
    }

    // Dizin girişini ekleme
    pub fn add_entry(&mut self, name: String, inode: u64, device: StorageDevice) {
        self.entries.insert(
            name.clone(),
            DirectoryEntry {
                inode,
                name,
                device,
            },
        );
    }

    // Dizin girişini alma
    pub fn get_entry(&self, name: &str) -> Option<&DirectoryEntry> {
        self.entries.get(name)
    }

    // Dizin girişini silme
    pub fn remove_entry(&mut self, name: &str) {
        self.entries.remove(name);
    }

    // Dizin girişlerini listeleme
    pub fn list_entries(&self) -> Vec<&DirectoryEntry> {
        self.entries.values().collect()
    }

    // ... diğer işlevler ...
}

// Dosya sistemi yapısı
#[derive(Debug, Clone)]
pub struct FileSystem {
    pub devices: HashMap<StorageDevice, Directory>,
}

impl FileSystem {
    // Yeni bir dosya sistemi oluşturma
    pub fn new() -> FileSystem {
        FileSystem {
            devices: HashMap::new(),
        }
    }

    // Aygıt ekleme
    pub fn add_device(&mut self, device: StorageDevice) {
        self.devices.insert(device, Directory::new());
    }

    // Dizin girişini ekleme (sürücüler arası)
    // Optimizasyon: `device` parametresini referans olarak alıyoruz
    pub fn add_entry(&mut self, device: &StorageDevice, name: String, inode: u64) {
        if let Some(dir) = self.devices.get_mut(device) {
            dir.add_entry(name, inode, device.clone());
        }
    }

    // Dizin girişini alma (sürücüler arası)
    // Optimizasyon: `device` parametresini referans olarak alıyoruz
    pub fn get_entry(&self, device: &StorageDevice, name: &str) -> Option<&DirectoryEntry> {
        if let Some(dir) = self.devices.get(device) {
            dir.get_entry(name)
        } else {
            None
        }
    }

    // Dizin girişini silme (sürücüler arası)
    // Optimizasyon: `device` parametresini referans olarak alıyoruz
    pub fn remove_entry(&mut self, device: &StorageDevice, name: &str) {
        if let Some(dir) = self.devices.get_mut(device) {
            dir.remove_entry(name);
        }
    }

    // Dizin girişlerini listeleme (sürücüler arası)
    // Optimizasyon: `device` parametresini referans olarak alıyoruz
    pub fn list_entries(&self, device: &StorageDevice) -> Vec<&DirectoryEntry> {
        if let Some(dir) = self.devices.get(device) {
            dir.list_entries()
        } else {
            Vec::new()
        }
    }

    // ... diğer işlevler ...
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;

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

        fs.add_entry(&device1, "file1.txt".to_string(), 10);
        fs.add_entry(&device2, "dir1".to_string(), 20);

        assert_eq!(
            fs.get_entry(&device1, "file1.txt").unwrap().inode,
            10
        );
        assert_eq!(
            fs.get_entry(&device2, "dir1").unwrap().name,
            "dir1"
        );

        fs.remove_entry(&device1, "file1.txt");
        assert!(fs.get_entry(&device1, "file1.txt").is_none());
    }

    #[test]
    fn test_file_system_list() {
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

        fs.add_entry(&device1, "file1.txt".to_string(), 10);
        fs.add_entry(&device2, "dir1".to_string(), 20);

        let entries1 = fs.list_entries(&device1);
        assert_eq!(entries1.len(), 1);
        assert_eq!(entries1[0].name, "file1.txt");

        let entries2 = fs.list_entries(&device2);
        assert_eq!(entries2.len(), 1);
        assert_eq!(entries2[0].name, "dir1");
    }
}

// Tek örnek kullanım senaryosu (isteğe bağlı olarak main.rs dosyasına veya yeni bir test fonksiyonuna eklenebilir).
#[cfg(feature = "std")]
#[test]
fn single_example_usage() {
    use super::*;

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

    // SSD aygıtına dizin girişleri ekle
    fs.add_entry(&ssd_device, "application.exe".to_string(), 1024);
    fs.add_entry(&ssd_device, "documents".to_string(), 2048);
    fs.add_entry(&ssd_device, "image.png".to_string(), 4096);

    // USB aygıtına dizin girişleri ekle (yedekleme amaçlı)
    fs.add_entry(&usb_device, "backup_documents.zip".to_string(), 8192);

    // SSD aygıtındaki dizin girişlerini listele
    println!("SSD Aygıtındaki Girişler:");
    let ssd_entries = fs.list_entries(&ssd_device);
    for entry in &ssd_entries {
        println!("- {} (Inode: {}, Aygıt Türü: {:?})", entry.name, entry.inode, entry.device.device_type);
    }

    // USB aygıtındaki dizin girişlerini listele
    println!("\nUSB Aygıtındaki Girişler:");
    let usb_entries = fs.list_entries(&usb_device);
    for entry in &usb_entries {
        println!("- {} (Inode: {}, Aygıt Türü: {:?})", entry.name, entry.inode, entry.device.device_type);
    }

    // Bir dizin girişini al ve kontrol et
    if let Some(doc_entry) = fs.get_entry(&ssd_device, "documents") {
        println!("\n'documents' Dizin Girişi bulundu: Name: {}, Inode: {}, Aygıt ID: {}", doc_entry.name, doc_entry.inode, doc_entry.device.device_id);
    } else {
        println!("\n'documents' Dizin Girişi bulunamadı.");
    }

    // Bir dizin girişini sil
    fs.remove_entry(&ssd_device, "image.png");
    println!("\n'image.png' Dizin Girişi silindi.");

    // SSD aygıtındaki güncel dizin girişlerini listele
    println!("\nSSD Aygıtındaki Güncel Girişler (image.png silindikten sonra):");
    let current_ssd_entries = fs.list_entries(&ssd_device);
    for entry in &current_ssd_entries {
        println!("- {} (Inode: {}, Aygıt Türü: {:?})", entry.name, entry.inode, entry.device.device_type);
    }
}

#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Gerçek çıktı mekanizmasına erişim olmalı (örneğin, UART).
            Ok(())
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => ({
            let mut stdout = $crate::print::Stdout;
            core::fmt::write(&mut stdout, core::format_args!($($arg)*)).unwrap();
        });
    }

    #[macro_export]
    macro_rules! println {
        () => ($crate::print!("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
    }
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}