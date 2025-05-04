#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// no_std ortamında print/println kullanabilmek için makroları içeri aktar
// Eğer std feature'ı aktifse, Rust'ın kendi println!'i kullanılır.
// Değilse, bizim tanımladığımız no_std uyumlu makrolar kullanılır.
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc; // Eğer MemBlockDevice gibi alloc kullanan yapılar config dosyasında da kullanılacaksa gerekebilir.
                     // Ancak config dosyasının kendisi genellikle alloc kullanmaz.
                     // Sadece print macro'su için alloc gerekebilir format_args! içinde.
                     // Emin olmak için alloc crate'ini kullanıyoruz.


// Donanım aygıtı yapılandırması
// Sahne64 API'sı geleneksel /dev/sda yolları yerine kaynak URI'ları kullanır.
// Bu nedenle, aygıt yolu Sahne64'e özgü bir kaynak tanımlayıcısı olmalıdır.
pub const DEVICE_PATH: &str = "sahne://devices/disk0"; // Sahne64 kaynak tanımlayıcısı
pub const BLOCK_SIZE: u32 = 4096; // Blok boyutu (bayt cinsinden)
pub const TOTAL_BLOCKS: u64 = 1048576; // Toplam blok sayısı

// Dosya sistemi yapılandırması
pub const SUPERBLOCK_LOCATION: u64 = 0; // Superblock'un blok numarası
pub const INODE_TABLE_LOCATION: u64 = 1; // Inode tablosunun başlangıç blok numarası
pub const DATA_BLOCKS_LOCATION: u64 = 1024; // Veri bloklarının başlangıç blok numarası
pub const INODES_COUNT: u32 = 1024; // Inode sayısı
pub const ROOT_INODE: u32 = 0; // Kök dizinin inode numarası

// Boş alan yönetimi yapılandırması
pub const FREE_SPACE_MAP_LOCATION: u64 = 2; // Boş alan haritasının başlangıç blok numarası

// Hata ayıklama yapılandırması
pub const LOG_LEVEL: LogLevel = LogLevel::Debug; // Günlük kaydı seviyesi

// Günlük kaydı seviyesi enum'u
#[derive(Debug, PartialEq)]
pub enum LogLevel {
    Error,
    Warning,
    Info,
    Debug,
}

// Yapılandırma parametrelerini yazdırma işlevi
// Bu fonksiyon, no_std ortamında da çalışan println! makromuzu kullanacaktır.
pub fn print_config() {
    // println! makrosu no_std ortamında tanımlı değilse, bizimkini kullanır.
    println!("Donanım Aygıtı Yapılandırması:");
    println!("  Aygıt Yolu: {}", DEVICE_PATH);
    println!("  Blok Boyutu: {} bayt", BLOCK_SIZE);
    println!("  Toplam Blok Sayısı: {}", TOTAL_BLOCKS);

    println!("\nDosya Sistemi Yapılandırması:");
    println!("  Superblock Konumu: Blok {}", SUPERBLOCK_LOCATION);
    println!("  Inode Tablosu Konumu: Blok {}", INODE_TABLE_LOCATION);
    println!("  Veri Blokları Konumu: Blok {}", DATA_BLOCKS_LOCATION);
    println!("  Inode Sayısı: {}", INODES_COUNT);
    println!("  Kök Inode: {}", ROOT_INODE);

    println!("\nBoş Alan Yönetimi Yapılandırması:");
    println!("  Boş Alan Haritası Konumu: Blok {}", FREE_SPACE_MAP_LOCATION);

    println!("\nHata Ayıklama Yapılandırması:");
    println!("  Günlük Kaydı Seviyesi: {:?}", LOG_LEVEL);
}
