#![allow(unused_imports)] // Henüz kullanılmayan importlar için uyarı vermesin
#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Standart kütüphane kullanımını Sahne64 kütüphanesi ile değiştiriyoruz
use crate::fs;
use crate::memory; // Belki ileride bellek yönetimi gerekebilir
use crate::SahneError;
use byteorder::{BigEndian, ReadBytesExt};

#[derive(Debug)]
pub struct Mp4Atom {
    pub size: u32,
    pub atom_type: [u8; 4], // Atom tipi şimdi [u8; 4] olarak temsil ediliyor
    pub data: Vec<u8>,
}

pub fn parse_mp4(file_path: &str) -> Result<Vec<Mp4Atom>, SahneError> {
    // Dosyayı Sahne64'ün fs modülünü kullanarak aç
    let fd_result = fs::open(file_path, fs::O_RDONLY);
    let fd = match fd_result {
        Ok(fd) => fd,
        Err(e) => return Err(e), // SahneError'ı doğrudan döndür
    };

    let mut atoms = Vec::new();
    let mut current_position = 0;

    loop {
        // Atom boyutunu oku (4 byte, BigEndian)
        let mut size_buffer = [0u8; 4];
        let read_result = fs::read(fd, &mut size_buffer);
        match read_result {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    // Dosyanın sonuna gelindi
                    break;
                }
                if bytes_read != 4 {
                    fs::close(fd).unwrap_or_default(); // Hata durumunda dosyayı kapat
                    return Err(SahneError::InvalidFileDescriptor); // Veya daha uygun bir hata
                }
                let size = BigEndian::read_u32(&size_buffer);

                if size < 8 { // Atom boyutu en az 8 byte olmalı (size + type)
                    fs::close(fd).unwrap_or_default();
                    return Err(SahneError::InvalidParameter); // Daha uygun bir hata türü olabilir
                }

                // Atom tipini oku (4 byte)
                let mut atom_type_bytes = [0u8; 4];
                let read_type_result = fs::read(fd, &mut atom_type_bytes);
                if let Err(_) = read_type_result {
                    fs::close(fd).unwrap_or_default();
                    return Err(SahneError::InvalidFileDescriptor); // Veya daha uygun bir hata
                }
                let atom_type = atom_type_bytes;

                // Atom verilerini oku (size - 8 byte)
                let data_size = size - 8;
                let mut data = vec![0; data_size as usize];
                let read_data_result = fs::read(fd, &mut data);
                match read_data_result {
                    Ok(bytes_read) => {
                        if bytes_read != data_size as usize {
                            fs::close(fd).unwrap_or_default();
                            return Err(SahneError::InvalidFileDescriptor); // Veya daha uygun bir hata
                        }
                        atoms.push(Mp4Atom {
                            size,
                            atom_type,
                            data,
                        });
                    }
                    Err(e) => {
                        fs::close(fd).unwrap_or_default();
                        return Err(e); // fs::read'den dönen SahneError
                    }
                }
            }
            Err(e) => {
                // fs::read'den dönen SahneError'ı doğrudan döndür
                fs::close(fd).unwrap_or_default();
                return Err(e);
            }
        }
    }

    // Dosyayı kapatmayı unutma
    if let Err(e) = fs::close(fd) {
        eprintln!("Dosya kapatılırken hata oluştu: {:?}", e);
    }

    Ok(atoms)
}

// Bu main fonksiyonu Sahne64 ortamında çalışmayabilir.
// Bu sadece bir örnektir ve Sahne64'ün kendi çalıştırma mekanizmasına uygun olmalıdır.
#[cfg(feature = "std")]
fn main() {
    match parse_mp4("ornek.mp4") {
        Ok(atoms) => {
            for atom in atoms {
                let atom_type_str = String::from_utf8_lossy(&atom.atom_type);
                println!("Atom: {}", atom_type_str);
            }
        }
        Err(e) => {
            eprintln!("Hata: {:?}", e);
        }
    }
}

// srcfilemp4.rs dosyası içeriği bu şekilde güncellenmiştir.