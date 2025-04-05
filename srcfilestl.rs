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

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{BufReader, Read};
#[cfg(feature = "std")]
use std::convert::TryInto;

#[derive(Debug)]
pub struct Triangle {
    normal: [f32; 3],
    vertices: [[f32; 3]; 3],
    attribute_byte_count: u16,
}

#[derive(Debug)]
pub struct Stl {
    pub triangles: Vec<Triangle>,
}

impl Stl {
    #[cfg(feature = "std")]
    pub fn from_file(path: &str) -> Result<Stl, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // İlk 80 bayt başlık (header)
        let mut header = [0; 80];
        reader.read_exact(&mut header)?;

        // Sonraki 4 bayt üçgen sayısı
        let mut triangle_count_bytes = [0; 4];
        reader.read_exact(&mut triangle_count_bytes)?;
        let triangle_count = u32::from_le_bytes(triangle_count_bytes) as usize;

        let mut triangles = Vec::with_capacity(triangle_count);
        let mut triangle_bytes = [0; 50]; // Tek bir tampon oluşturuluyor

        for _ in 0..triangle_count {
            reader.read_exact(&mut triangle_bytes)?;

            let normal = [
                f32::from_le_bytes(triangle_bytes[0..4].try_into().unwrap()),
                f32::from_le_bytes(triangle_bytes[4..8].try_into().unwrap()),
                f32::from_le_bytes(triangle_bytes[8..12].try_into().unwrap()),
            ];

            let vertices = [
                [
                    f32::from_le_bytes(triangle_bytes[12..16].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[16..20].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[20..24].try_into().unwrap()),
                ],
                [
                    f32::from_le_bytes(triangle_bytes[24..28].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[28..32].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[32..36].try_into().unwrap()),
                ],
                [
                    f32::from_le_bytes(triangle_bytes[36..40].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[40..44].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[44..48].try_into().unwrap()),
                ],
            ];

            let attribute_byte_count = u16::from_le_bytes(triangle_bytes[48..50].try_into().unwrap());

            triangles.push(Triangle {
                normal,
                vertices,
                attribute_byte_count,
            });
        }

        Ok(Stl { triangles })
    }

    #[cfg(not(feature = "std"))]
    pub fn from_file(path: &str) -> Result<Stl, SahneError> {
        let fd = fs::open(path, fs::O_RDONLY)?;

        // İlk 80 bayt başlık (header)
        let mut header = [0; 80];
        fs::read(fd, &mut header)?;

        // Sonraki 4 bayt üçgen sayısı
        let mut triangle_count_bytes = [0; 4];
        fs::read(fd, &mut triangle_count_bytes)?;
        let triangle_count = u32::from_le_bytes(triangle_count_bytes) as usize;

        let mut triangles = Vec::with_capacity(triangle_count);
        let mut triangle_bytes = [0; 50]; // Tek bir tampon oluşturuluyor

        for _ in 0..triangle_count {
            fs::read(fd, &mut triangle_bytes)?;

            let normal = [
                f32::from_le_bytes(triangle_bytes[0..4].try_into().unwrap()),
                f32::from_le_bytes(triangle_bytes[4..8].try_into().unwrap()),
                f32::from_le_bytes(triangle_bytes[8..12].try_into().unwrap()),
            ];

            let vertices = [
                [
                    f32::from_le_bytes(triangle_bytes[12..16].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[16..20].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[20..24].try_into().unwrap()),
                ],
                [
                    f32::from_le_bytes(triangle_bytes[24..28].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[28..32].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[32..36].try_into().unwrap()),
                ],
                [
                    f32::from_le_bytes(triangle_bytes[36..40].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[40..44].try_into().unwrap()),
                    f32::from_le_bytes(triangle_bytes[44..48].try_into().unwrap()),
                ],
            ];

            let attribute_byte_count = u16::from_le_bytes(triangle_bytes[48..50].try_into().unwrap());

            triangles.push(Triangle {
                normal,
                vertices,
                attribute_byte_count,
            });
        }

        fs::close(fd)?; // Dosyayı kapatmayı unutma

        Ok(Stl { triangles })
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