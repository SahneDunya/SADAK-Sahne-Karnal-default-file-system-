#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz (Sahne64 bağlamında)
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin (Sahne64 bağlamında)

// Modül bildirimleri (Sahne64'den alınan)
pub mod arch {
    pub const SYSCALL_FILE_OPEN: u64 = 5;
    pub const SYSCALL_FILE_READ: u64 = 6;
    pub const SYSCALL_FILE_WRITE: u64 = 7;
    pub const SYSCALL_FILE_CLOSE: u64 = 8;
}

#[derive(Debug)]
pub enum SahneError {
    FileNotFound,
    PermissionDenied,
    InvalidFileDescriptor,
    UnknownSystemCall,
    OutOfMemory,
    InvalidData,
    UnsupportedVersion,
    UnsupportedDepth,
    IOError, // Genel IO hatası
}

extern "sysv64" {
    fn syscall(number: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64;
}

pub mod fs {
    use super::{arch, SahneError, syscall};

    pub const O_RDONLY: u32 = 0;
    pub const O_WRONLY: u32 = 1;
    pub const O_RDWR: u32 = 2;
    pub const O_CREAT: u32 = 0x0100;
    pub const O_EXCL: u32 = 0x0200;
    pub const O_TRUNC: u32 = 0x0400;

    pub fn open(path: &str, flags: u32) -> Result<u64, SahneError> {
        let path_ptr = path.as_ptr() as u64;
        let path_len = path.len() as u64;
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_OPEN, path_ptr, path_len, flags as u64, 0, 0)
        };
        if result < 0 {
            match result {
                -2 => Err(SahneError::FileNotFound),
                -13 => Err(SahneError::PermissionDenied),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(result as u64)
        }
    }

    pub fn read(fd: u64, buffer: &mut [u8]) -> Result<usize, SahneError> {
        let buffer_ptr = buffer.as_mut_ptr() as u64;
        let buffer_len = buffer.len() as u64;
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_READ, fd, buffer_ptr, buffer_len, 0, 0)
        };
        if result < 0 {
            match result {
                -9 => Err(SahneError::InvalidFileDescriptor),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(result as usize)
        }
    }

    pub fn write(fd: u64, buffer: &[u8]) -> Result<usize, SahneError> {
        let buffer_ptr = buffer.as_ptr() as u64;
        let buffer_len = buffer.len() as u64;
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_WRITE, fd, buffer_ptr, buffer_len, 0, 0)
        };
        if result < 0 {
            match result {
                -9 => Err(SahneError::InvalidFileDescriptor),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(result as usize)
        }
    }

    pub fn close(fd: u64) -> Result<(), SahneError> {
        let result = unsafe {
            syscall(arch::SYSCALL_FILE_CLOSE, fd, 0, 0, 0, 0)
        };
        if result < 0 {
            match result {
                -9 => Err(SahneError::InvalidFileDescriptor),
                _ => Err(SahneError::UnknownSystemCall),
            }
        } else {
            Ok(())
        }
    }
}

pub struct Psd {
    header: PsdHeader,
    // Diğer PSD verileri (katmanlar, kanallar vb.)
}

#[derive(Debug)]
pub struct PsdHeader {
    signature: [u8; 4],
    version: u16,
    reserved: [u8; 6],
    channels: u16,
    height: u32,
    width: u32,
    depth: u16,
    color_mode: u16,
}

impl Psd {
    pub fn open(path: &str) -> Result<Self, SahneError> {
        let fd = fs::open(path, fs::O_RDONLY)?;

        let header = Self::read_header(fd)?;

        fs::close(fd)?; // Dosyayı okuduktan sonra kapat

        // Diğer PSD verilerini okuma (katmanlar, kanallar vb.)

        Ok(Psd { header })
    }

    fn read_header(fd: u64) -> Result<PsdHeader, SahneError> {
        let mut header = PsdHeader {
            signature: [0; 4],
            version: 0,
            reserved: [0; 6],
            channels: 0,
            height: 0,
            width: 0,
            depth: 0,
            color_mode: 0,
        };

        // Signature (4 bytes): Always '8BPS'
        let signature_bytes = &mut header.signature;
        if fs::read(fd, signature_bytes)? != signature_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        if header.signature != *b"8BPS" {
            return Err(SahneError::InvalidData);
        }

        // Version (2 bytes): Always 1 or 2
        let mut version_bytes = [0; core::mem::size_of::<u16>()];
        if fs::read(fd, &mut version_bytes)? != version_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        header.version = u16::from_be_bytes(version_bytes);
        if header.version != 1 && header.version != 2 {
            return Err(SahneError::UnsupportedVersion);
        }

        // Reserved (6 bytes): Must be zero
        let reserved_bytes = &mut header.reserved;
        if fs::read(fd, reserved_bytes)? != reserved_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        // Kontrol isteğe bağlı (yorumdaki gibi devam ediyoruz)

        // Channels (2 bytes): Number of color channels
        let mut channels_bytes = [0; core::mem::size_of::<u16>()];
        if fs::read(fd, &mut channels_bytes)? != channels_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        header.channels = u16::from_be_bytes(channels_bytes);

        // Height (4 bytes): Height of the image in pixels
        let mut height_bytes = [0; core::mem::size_of::<u32>()];
        if fs::read(fd, &mut height_bytes)? != height_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        header.height = u32::from_be_bytes(height_bytes);

        // Width (4 bytes): Width of the image in pixels
        let mut width_bytes = [0; core::mem::size_of::<u32>()];
        if fs::read(fd, &mut width_bytes)? != width_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        header.width = u32::from_be_bytes(width_bytes);

        // Depth (2 bytes): Bits per channel (1, 8, 16, or 32)
        let mut depth_bytes = [0; core::mem::size_of::<u16>()];
        if fs::read(fd, &mut depth_bytes)? != depth_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        header.depth = u16::from_be_bytes(depth_bytes);
        if ![1, 8, 16, 32].contains(&header.depth) {
            return Err(SahneError::UnsupportedDepth);
        }

        // Color Mode (2 bytes): Color mode of the file
        let mut color_mode_bytes = [0; core::mem::size_of::<u16>()];
        if fs::read(fd, &mut color_mode_bytes)? != color_mode_bytes.len() {
            return Err(SahneError::InvalidData);
        }
        header.color_mode = u16::from_be_bytes(color_mode_bytes);
        // You might want to validate color_mode against known values if needed

        Ok(header)
    }
}

// Standart kütüphanenin bazı temel fonksiyonlarının (örneğin println!) kendi implementasyonunuz
// veya harici bir crate (örneğin core::fmt) kullanılarak sağlanması gerekebilir.
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// Bu kısım, no_std ortamında println! gibi makroların çalışması için gereklidir.
// Gerçek bir CustomOS ortamında, bu işlevselliği çekirdek üzerinden bir sistem çağrısı ile
// veya özel bir donanım sürücüsü ile sağlamanız gerekebilir.
// Aşağıdaki kod, core::fmt kütüphanesini kullanarak basit bir formatlama örneği sunar.
// Ancak, gerçek bir çıktı mekanizması (örneğin, UART) olmadan bu çıktıları göremezsiniz.
#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Burada gerçek çıktı mekanizmasına (örneğin, bir UART sürücüsüne) erişim olmalı.
            // Bu örnekte, çıktı kaybolacaktır çünkü gerçek bir çıktı yok.
            // Gerçek bir işletim sisteminde, bu kısım donanıma özel olacaktır.
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

#[cfg(feature = "std")]
fn main() -> Result<(), SahneError> {
    // Dummy example.psd data
    let example_psd_data: Vec<u8> = vec![
        0x38, 0x42, 0x50, 0x53, // Signature "8BPS"
        0x00, 0x01,             // Version 1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Reserved
        0x00, 0x03,             // Channels (3)
        0x00, 0x00, 0x01, 0x00, // Height (256)
        0x00, 0x00, 0x01, 0x00, // Width (256)
        0x00, 0x08,             // Depth (8 bits)
        0x00, 0x03,             // Color Mode (CMYK)
    ];
    std::fs::write("example.psd", example_psd_data).map_err(|e| SahneError::IOError)?;

    let psd = Psd::open("example.psd")?;
    println!("{:?}", psd.header);
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() -> Result<(), SahneError> {
    // Dummy example.psd data oluşturma (Sahne64 fs kullanarak)
    let example_psd_data: Vec<u8> = vec![
        0x38, 0x42, 0x50, 0x53, // Signature "8BPS"
        0x00, 0x01,             // Version 1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Reserved
        0x00, 0x03,             // Channels (3)
        0x00, 0x00, 0x01, 0x00, // Height (256)
        0x00, 0x00, 0x01, 0x00, // Width (256)
        0x00, 0x08,             // Depth (8 bits)
        0x00, 0x03,             // Color Mode (CMYK)
    ];

    let create_fd = fs::open("example.psd", fs::O_CREAT | fs::O_WRONLY)?;
    fs::write(create_fd, &example_psd_data)?;
    fs::close(create_fd)?;

    let psd = Psd::open("example.psd")?;
    println!("{:?}", psd.header);
    Ok(())
}