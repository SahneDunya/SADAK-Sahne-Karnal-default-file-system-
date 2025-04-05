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
use core::option::Option;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::BufReader;
#[cfg(feature = "std")]
use metaflac::Tag;

/// Represents a FLAC audio file and its metadata.
pub struct FlacFile {
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
}

impl FlacFile {
    /// Creates a new `FlacFile` instance by reading metadata from the given file path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the FLAC file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `FlacFile` struct on success, or a `String` error message on failure.
    #[cfg(feature = "std")]
    pub fn new(path: &str) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("Dosya açma hatası: {}", e))?; // Daha açıklayıcı hata mesajı
        let reader = BufReader::new(file);
        let tag = Tag::read_from(reader).map_err(|e| format!("Metaveri okuma hatası: {}", e))?; // Daha açıklayıcı hata mesajı

        let title = tag.get_title().map(|s| s.to_string());
        let artist = tag.get_artist().map(|s| s.to_string());
        let album = tag.get_album().map(|s| s.to_string());
        let track_number = tag.get_track_number();

        Ok(FlacFile {
            path: path.to_string(),
            title,
            artist,
            album,
            track_number,
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn new(path: &str) -> Result<Self, SahneError> {
        let fd = fs::open(path, fs::O_RDONLY)?;

        // Sahne64'te metaflac kütüphanesi veya benzeri bir FLAC metaveri okuma mekanizması olmayabilir.
        // Bu kısım, Sahne64'e özgü bir FLAC metaveri okuma implementasyonu gerektirebilir.
        // Şimdilik, metaverileri None olarak başlatıyoruz.
        let title = None;
        let artist = None;
        let album = None;
        let track_number = None;

        // Dosyayı kapatalım (gerçek bir FLAC parser implementasyonu okuma işlemini yapacaktır).
        let _ = fs::close(fd);

        Ok(FlacFile {
            path: path.to_string(),
            title,
            artist,
            album,
            track_number,
        })
    }

    /// Prints the metadata of the FLAC file to the console.
    pub fn print_metadata(&self) {
        #[cfg(feature = "std")]
        println!("FLAC Dosyası: {}", self.path);
        #[cfg(not(feature = "std"))]
        crate::println!("FLAC Dosyası: {}", self.path);

        if let Some(title) = &self.title {
            #[cfg(feature = "std")]
            println!("Başlık: {}", title);
            #[cfg(not(feature = "std"))]
            crate::println!("Başlık: {}", title);
        }
        if let Some(artist) = &self.artist {
            #[cfg(feature = "std")]
            println!("Sanatçı: {}", artist);
            #[cfg(not(feature = "std"))]
            crate::println!("Sanatçı: {}", artist);
        }
        if let Some(album) = &self.album {
            #[cfg(feature = "std")]
            println!("Albüm: {}", album);
            #[cfg(not(feature = "std"))]
            crate::println!("Albüm: {}", album);
        }
        if let Some(track_number) = self.track_number {
            #[cfg(feature = "std")]
            println!("Parça Numarası: {}", track_number);
            #[cfg(not(feature = "std"))]
            crate::println!("Parça Numarası: {}", track_number);
        }
    }
}

#[cfg(feature = "std")]
fn main() -> Result<(), String> {
    // Örnek bir FLAC dosyasının yolunu belirtin
    let flac_path = "example.flac"; // Bu dosyanın gerçekten var olması gerekir.

    match FlacFile::new(flac_path) {
        Ok(flac_file) => {
            flac_file.print_metadata();
            Ok(())
        }
        Err(e) => {
            eprintln!("Hata: {}", e);
            Err(e)
        }
    }
}

#[cfg(not(feature = "std"))]
fn main() -> Result<(), SahneError> {
    // Örnek bir FLAC dosyasının yolunu belirtin
    let flac_path = "example.flac"; // Bu dosyanın Sahne64 dosya sisteminde var olması gerekir.

    match FlacFile::new(flac_path) {
        Ok(flac_file) => {
            flac_file.print_metadata();
            Ok(())
        }
        Err(e) => {
            crate::println!("Hata: {:?}", e);
            Err(e)
        }
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