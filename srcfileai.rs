#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel tipleri içeri aktar
use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri ve Handle
// Sahne64 resource modülü (no_std implementasyonu için)
#[cfg(not(feature = "std"))]
use crate::resource;

// std kütüphanesi kullanılıyorsa gerekli std importları
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{BufReader, Read as StdRead, Result as StdResult, Error as StdIOError};

// Hata Eşleme Yardımcıları (srcfileaac.rs dosyasında tanımlananların kopyası veya oradan import)
// Bu yardımcıların merkezi bir yerde olması IDEALDIR. Burada tekrar tanımlanmıştır.
#[cfg(feature = "std")]
fn map_io_error_to_fs_error(e: StdIOError) -> FileSystemError {
    FileSystemError::IOError(alloc::string::String::from(alloc::format!("IO Error: {}", e)))
}

#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(alloc::string::String::from(alloc::format!("SahneError: {:?}", e)))
}

// no_std ortamında println! ve eprintln! makroları için
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülden import edildiği varsayılır

/// Belirtilen dosya yolundaki (kaynak ID'si) dosyanın AI (Adobe Illustrator)
/// uyumlu olup olmadığını (yani %PDF ile başlayıp başlamadığını) kontrol eder.
#[cfg(feature = "std")]
fn read_ai_file(file_path: &str) -> Result<(), FileSystemError> { // FileSystemError döner
    use alloc::vec::Vec; // Vec için alloc kullanıyoruz
    use alloc::string::String; // String için alloc kullanıyoruz
    use alloc::format; // format! için alloc kullanıyoruz

    let file = File::open(file_path).map_err(map_io_error_to_fs_error)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 4]; // Sadece ilk 4 baytı okumak için sabit boyutlu buffer
    let bytes_read = reader.read(&mut buffer).map_err(map_io_error_to_fs_error)?; // En fazla 4 bayt oku

    if bytes_read == 4 && buffer == *b"%PDF" { // Okunan bayt sayısı 4 ise ve "%PDF" ile başlıyorsa
        println!("Dosya bir AI dosyası (PDF uyumlu).");
        // PDF uyumlu AI dosyalarının içeriğini okumak için PDF kütüphaneleri kullanılabilir.
        // Bu örnekte, sadece dosya türünü doğruluyoruz.
    } else {
        println!("Dosya bir AI dosyası (PDF uyumlu değil veya çok kısa).");
        // PDF uyumlu olmayan AI dosyalarının içeriğini okumak daha karmaşıktır.
        // Bu örnekte, sadece dosya türünü doğruluyoruz.
    }

    Ok(())
}

/// Belirtilen Sahne64 kaynak ID'sindeki dosyanın AI (Adobe Illustrator)
/// uyumlu olup olmadığını (%PDF ile başlayıp başlamadığını) kontrol eder (no_std).
#[cfg(not(feature = "std"))]
fn read_ai_file(resource_id: &str) -> Result<(), FileSystemError> { // FileSystemError döner
    // alloc::vec::Vec kullanımı için alloc crate'i etkin.
    // alloc::string::String ve alloc::format! kullanımı için de.

    // Kaynağı edin
    let handle = resource::acquire(resource_id, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    let mut buffer = [0u8; 4]; // Sadece ilk 4 baytı okumak için sabit boyutlu buffer
    // resource::read Result<usize, SahneError> döner
    let bytes_read = resource::read(handle, &mut buffer).map_err(|e| {
         let _ = resource::release(handle); // Kaynağı serbest bırakmayı dene
         map_sahne_error_to_fs_error(e)
     })?; // En fazla 4 bayt oku

    // Kaynağı serbest bırak
    let _ = resource::release(handle).map_err(|e| {
         // Kaynak serbest bırakma hatası (kritik değilse sadece logla)
         eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print makrosu
         map_sahne_error_to_fs_error(e) // Yine de hatayı FileSystemError'a çevir
     });


    if bytes_read == 4 && buffer == *b"%PDF" {
        println!("Dosya bir AI dosyası (PDF uyumlu).");
    } else {
        // Eğer 4 bayttan az okunduysa veya başlangıç farklıysa
        println!("Dosya bir AI dosyası değil (PDF uyumlu değil veya çok kısa).");
    }

    Ok(())
}

// Örnek main fonksiyonları
#[cfg(feature = "example_ai")] // Farklı bir özellik bayrağı kullanıldı
fn main() { // main fonksiyonu Result dönmeyebilir, hataları kendisi handle etmeli
    #[cfg(not(feature = "std"))]
    { // no_std println!/eprintln! makrolarının scope'u
         // Varsayımsal bir konsol handle'ı ayarlayalım.
          crate::init_console(crate::Handle(3)); // init_console'ı çağırabilmek için Handle tipi ve init_console fonksiyonu pub olmalı.
         // Şimdilik çağrıyı yorum satırı yapalım, test amaçlı main'de dışarıdan init edilmesi gerekir.
         eprintln!("AI file example (no_std) starting...");
    }
    #[cfg(feature = "std")]
    { // std println!/eprintln! makrolarının scope'u
         eprintln!("AI file example (std) starting...");
    }


    // Test amaçlı varsayımsal dosya yolu/kaynak ID'si
    let file_path_or_resource_id = "sahne://files/design.ai"; // Sahne64 kaynak ID'si

    if let Err(e) = read_ai_file(file_path_or_resource_id) {
        eprintln!("'{}' dosyası işlenirken hata oluştu: {}", file_path_or_resource_id, e);
    } else {
         println!("'{}' dosyası başarıyla işlendi.", file_path_or_resource_id);
    }

     #[cfg(not(feature = "std"))]
     eprintln!("AI file example (no_std) finished.");
     #[cfg(feature = "std")]
     eprintln!("AI file example (std) finished.");

}


// Tekrarlanan no_std print modülü ve panic handler kaldırıldı.
