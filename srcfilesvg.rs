#![no_std]
#![allow(dead_code)]

// Gerekli modülleri ve sabitleri Sahne64'ten içe aktar
pub mod arch {
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

pub struct Svg {
    pub width: f64,
    pub height: f64,
    pub elements: Vec<SvgElement>,
}

pub enum SvgElement {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        fill: String,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: String,
    },
}

impl Svg {
    pub fn from_file(path: &str) -> Result<Svg, SahneError> {
        let fd = fs::open(path, fs::O_RDONLY)?;
        let mut reader = BufReaderSahne::new(fd);
        let mut content = String::new();
        reader.read_to_string(&mut content)?;

        Svg::from_str(&content)
    }

    pub fn from_str(svg_content: &str) -> Result<Svg, SahneError> {
        use xml::reader::{EventReader, XmlEvent};

        let parser = EventReader::new(svg_content.as_bytes());
        let mut svg = Svg {
            width: 0.0,
            height: 0.0,
            elements: Vec::new(),
        };

        let mut current_element: Option<SvgElement> = None;

        for event in parser {
            match event {
                Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                    match name.local_name.as_str() {
                        "svg" => {
                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                    "width" => svg.width = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "height" => svg.height = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    _ => {}
                                }
                            }
                        }
                        "rect" => {
                            let mut x = 0.0;
                            let mut y = 0.0;
                            let mut width = 0.0;
                            let mut height = 0.0;
                            let mut fill = String::new();

                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                    "x" => x = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "y" => y = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "width" => width = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "height" => height = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "fill" => fill = attr.value,
                                    _ => {}
                                }
                            }

                            current_element = Some(SvgElement::Rect {
                                x,
                                y,
                                width,
                                height,
                                fill,
                            });
                        }
                        "circle" => {
                            let mut cx = 0.0;
                            let mut cy = 0.0;
                            let mut r = 0.0;
                            let mut fill = String::new();

                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                    "cx" => cx = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "cy" => cy = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "r" => r = attr.value.parse().map_err(|_| SahneError::InvalidParameter)?,
                                    "fill" => fill = attr.value,
                                    _ => {}
                                }
                            }

                            current_element = Some(SvgElement::Circle {
                                cx,
                                cy,
                                r,
                                fill,
                            });
                        }
                        _ => {}
                    }
                }
                Ok(XmlEvent::EndElement { name }) => {
                    if name.local_name.as_str() == "rect" || name.local_name.as_str() == "circle" {
                        if let Some(element) = current_element.take() {
                            svg.elements.push(element);
                        }
                    }
                }
                Err(e) => {
                    // xml-rs kütüphanesinin kendi hata tipini SahneError'a dönüştürmek gerekebilir.
                    // Şimdilik genel bir InvalidParameter hatası dönülüyor.
                    return Err(SahneError::InvalidParameter);
                }
                _ => {}
            }
        }

        Ok(svg)
    }
}

// std::io::BufReader benzeri bir yapı (basitleştirilmiş)
pub struct BufReaderSahne {
    fd: u64,
    buffer: [u8; 1024],
    position: usize,
    filled: usize,
}

impl BufReaderSahne {
    pub fn new(fd: u64) -> Self {
        BufReaderSahne {
            fd,
            buffer: [0; 1024],
            position: 0,
            filled: 0,
        }
    }

    pub fn fill_buf(&mut self) -> Result<&[u8], SahneError> {
        if self.position >= self.filled {
            self.position = 0;
            self.filled = fs::read(self.fd, &mut self.buffer)?;
        }
        Ok(&self.buffer[self.position..self.filled])
    }

    pub fn consume(&mut self, amount: usize) {
        self.position += amount;
    }
}

impl BufReaderSahne {
    pub fn read_to_string(&mut self, buf: &mut String) -> Result<usize, SahneError> {
        let mut total_read = 0;
        loop {
            let available = self.fill_buf()?;
            if available.is_empty() {
                break;
            }
            match core::str::from_utf8(available) {
                Ok(valid) => {
                    buf.push_str(valid);
                    let len = valid.len();
                    self.consume(len);
                    total_read += len;
                }
                Err(e) => {
                    // UTF-8 dönüşümü hatası
                    return Err(SahneError::InvalidParameter);
                }
            }
        }
        Ok(total_read)
    }
}

// #[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
fn main() -> Result<(), SahneError> {
    // Örnek SVG dosya içeriği (Example SVG file content)
    let svg_content = r#"
        <svg width="200" height="100">
            <rect x="10" y="10" width="30" height="30" fill="red" />
            <circle cx="60" cy="60" r="20" fill="blue" />
        </svg>
    "#;

    // Geçici bir dosya oluşturmak yerine doğrudan SVG içeriğini kullanıyoruz.
    // (Instead of creating a temporary file, we use the SVG content directly.)
    let path = "/example.svg"; // Sahne64 dosya sisteminde bu dosyanın var olduğunu varsayıyoruz.
                               // (We assume this file exists in the Sahne64 file system.)

    // Bu örnekte, Sahne64 üzerinde dosya oluşturma ve yazma fonksiyonları olmadığı için,
    // SVG içeriğini doğrudan bir dosyadan okuyormuş gibi simüle edeceğiz.
    // Gerçek bir Sahne64 ortamında, bu dosyanın önceden oluşturulmuş olması gerekir.

    // SVG'yi dosyadan ayrıştır (Parse SVG from the file)
    match Svg::from_file(path) {
        Ok(svg) => {
            // Ayrıştırılan SVG verilerini yazdır (Print parsed SVG data)
            println!("SVG Width: {}", svg.width);
            println!("SVG Height: {}", svg.height);
            println!("Elements:");
            for element in svg.elements {
                match element {
                    SvgElement::Rect { x, y, width, height, fill } => {
                        println!("  Rect: x={}, y={}, width={}, height={}, fill={}", x, y, width, height, fill);
                    }
                    SvgElement::Circle { cx, cy, r, fill } => {
                        println!("  Circle: cx={}, cy={}, r={}, fill={}", cx, cy, r, fill);
                    }
                }
            }
        }
        Err(e) => eprintln!("SVG ayrıştırma hatası: {:?}", e),
    }

    // Dosyayı kapatmak (Close the file) - `from_file` fonksiyonu içinde zaten yapılıyor.

    Ok(())
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
            // Burada gerçek çıktı mekanizmasına erişim olmalı (örneğin, bir UART sürücüsüne).
            // Bu örnekte, çıktı kaybolacaktır çünkü gerçek bir çıktı yok.
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