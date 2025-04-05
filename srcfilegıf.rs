#![no_std]
#![allow(dead_code)]

// Gerekli modülleri ve sabitleri içe aktar
mod arch {
    pub const SYSCALL_FILE_OPEN: u64 = 5;
    pub const SYSCALL_FILE_READ: u64 = 6;
    pub const SYSCALL_FILE_CLOSE: u64 = 8;
}

#[derive(Debug)]
pub enum SahneError {
    OutOfMemory,
    InvalidAddress,
    InvalidParameter,
    FileNotFound,
    PermissionDenied,
    FileAlreadyExists,
    InvalidFileDescriptor,
    ResourceBusy,
    Interrupted,
    NoMessage,
    InvalidOperation,
    NotSupported,
    UnknownSystemCall,
    ProcessCreationFailed,
}

extern "sysv64" {
    fn syscall(number: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64;
}

pub mod fs {
    use super::{arch, syscall, SahneError};

    pub const O_RDONLY: u32 = 0;

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

#[derive(Debug)]
struct GifHeader {
    signature: [u8; 3],
    version: [u8; 3],
    logical_screen_width: u16,
    logical_screen_height: u16,
    global_color_table_flag: bool,
    color_resolution: u8,
    sort_flag: bool,
    global_color_table_size: u8,
    background_color_index: u8,
    pixel_aspect_ratio: u8,
}

fn read_gif_header(fd: u64) -> Result<GifHeader, SahneError> {
    let mut header_bytes = [0u8; 13];
    let bytes_read = fs::read(fd, &mut header_bytes)?;
    if bytes_read != 13 {
        return Err(SahneError::InvalidParameter); // Beklenen sayıda byte okunmadı
    }

    let signature: [u8; 3] = header_bytes[0..3].try_into().unwrap();
    let version: [u8; 3] = header_bytes[3..6].try_into().unwrap();
    let logical_screen_width = (header_bytes[7] as u16) << 8 | (header_bytes[6] as u16); // LittleEndian manuel okuma
    let logical_screen_height = (header_bytes[9] as u16) << 8 | (header_bytes[8] as u16); // LittleEndian manuel okuma
    let packed_fields = header_bytes[10];
    let background_color_index = header_bytes[11];
    let pixel_aspect_ratio = header_bytes[12];

    let global_color_table_flag = (packed_fields & 0x80) != 0;
    let color_resolution = (packed_fields & 0x70) >> 4;
    let sort_flag = (packed_fields & 0x08) != 0;
    let global_color_table_size = packed_fields & 0x07;

    Ok(GifHeader {
        signature,
        version,
        logical_screen_width,
        logical_screen_height,
        global_color_table_flag,
        color_resolution,
        sort_flag,
        global_color_table_size,
        background_color_index,
        pixel_aspect_ratio,
    })
}

// std::io::Error'ı SahneError'a dönüştürmek için basit bir fonksiyon (gerekirse daha detaylı yapılabilir)
#[cfg(feature = "std")]
fn map_io_error_to_sahne(error: std::io::Error) -> SahneError {
    match error.kind() {
        std::io::ErrorKind::NotFound => SahneError::FileNotFound,
        std::io::ErrorKind::PermissionDenied => SahneError::PermissionDenied,
        _ => SahneError::UnknownSystemCall, // Daha genel bir hata türü
    }
}

// SahneError'ı std::io::Error'a dönüştürmek için basit bir fonksiyon
#[cfg(feature = "std")]
fn map_sahne_error_to_io(error: SahneError) -> std::io::Error {
    match error {
        SahneError::FileNotFound => std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"),
        SahneError::PermissionDenied => std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied"),
        SahneError::InvalidFileDescriptor => std::io::Error::new(std::io::ErrorKind::Other, "Invalid file descriptor"),
        SahneError::InvalidParameter => std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid parameter"),
        _ => std::io::Error::new(std::io::ErrorKind::Other, "Unknown Sahne error"),
    }
}

// Standart kütüphane özelliği aktifse çalışacak main fonksiyonu
#[cfg(feature = "std")]
fn main() -> Result<(), std::io::Error> {
    let path = "example.gif";
    match fs::open(path, fs::O_RDONLY) {
        Ok(fd) => {
            let header_result = read_gif_header(fd);
            match header_result {
                Ok(header) => println!("{:?}", header),
                Err(e) => eprintln!("GIF header okuma hatası: {:?}", e),
            }
            match fs::close(fd) {
                Ok(_) => {},
                Err(e) => eprintln!("Dosya kapatma hatası: {:?}", e),
            }
            Ok(())
        }
        Err(e) => Err(map_sahne_error_to_io(e)),
    }
}

// Standart kütüphane özelliği aktif değilse çalışacak main fonksiyonu (örnek bir çıktı mekanizması olmadığı için bu kısım pratik bir çıktı vermez)
#[cfg(not(feature = "std"))]
fn main() -> Result<(), SahneError> {
    let path = "example.gif";
    match fs::open(path, fs::O_RDONLY) {
        Ok(fd) => {
            let header_result = read_gif_header(fd);
            match header_result {
                Ok(header) => {
                    // Burada header bilgisini bir şekilde göstermeniz gerekirdi (örneğin, bir UART üzerinden).
                    // println! makrosu standart kütüphane olmadan çalışmayacağı için burada bir çıktı olmayacaktır.
                    // Gerçek bir Sahne64 ortamında, bu çıktıyı çekirdek üzerinden bir sistem çağrısı ile yapmanız gerekebilir.
                }
                Err(e) => {
                    // Hata durumunda da benzer şekilde bir çıktı mekanizması gereklidir.
                }
            }
            match fs::close(fd) {
                Ok(_) => {},
                Err(e) => {}
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Gerçek çıktı mekanizması (örneğin, UART sürücüsü) buraya gelmeli.
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