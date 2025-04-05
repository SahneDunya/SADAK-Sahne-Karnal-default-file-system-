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

#[cfg(not(feature = "std"))]
use core::fmt;

#[cfg(not(feature = "std"))]
use core::convert::TryInto;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom};
#[cfg(feature = "std")]
use byteorder::{LittleEndian as StdLittleEndian, ReadBytesExt as StdReadBytesExt};
#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(feature = "std")]
use std::string::String;

// FBX dosya formatının temel yapıları

// FBX Başlığı yapısı
#[derive(Debug)]
struct FbxHeader {
    magic_number: [u8; 21], // "Kaydaz FBX " + \x00
    unknown: [u8; 2],      // [0x1A, 0x00]
    version: u32,         // Versiyon numarası
}

// FBX Düğüm Özelliği (Property) enum'ı
#[derive(Debug)]
enum FbxProperty {
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    RawBytes(Vec<u8>), // Ham veri için
    Bool(bool),
    // ... Diğer FBX veri tipleri eklenebilir ...
}

// FBX Düğümü yapısı
#[derive(Debug)]
struct FbxNode {
    end_offset: u32,          // Düğüm sonu offset'i (dosya başından itibaren)
    num_properties: u32,      // Özellik sayısı
    property_list_len: u32,   // Özellik listesinin uzunluğu
    name_len: u8,             // İsim uzunluğu
    name: String,             // Düğüm ismi
    properties: Vec<FbxProperty>, // Özellikler listesi
    nested_nodes: Vec<FbxNode>, // İç içe düğümler
}

// Yardımcı fonksiyonlar (byteorder yerine)
#[cfg(not(feature = "std"))]
fn read_u32_le<R: Read>(reader: &mut R) -> Result<u32, SahneError> {
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))]
fn read_i32_le<R: Read>(reader: &mut R) -> Result<i32, SahneError> {
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer)?;
    Ok(i32::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))]
fn read_i64_le<R: Read>(reader: &mut R) -> Result<i64, SahneError> {
    let mut buffer = [0; 8];
    reader.read_exact(&mut buffer)?;
    Ok(i64::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))]
fn read_f32_le<R: Read>(reader: &mut R) -> Result<f32, SahneError> {
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer)?;
    Ok(f32::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))]
fn read_f64_le<R: Read>(reader: &mut R) -> Result<f64, SahneError> {
    let mut buffer = [0; 8];
    reader.read_exact(&mut buffer)?;
    Ok(f64::from_le_bytes(buffer))
}

#[cfg(not(feature = "std"))]
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SahneError>;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), SahneError> {
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.read(&mut buf[total_read..]) {
                Ok(0) => break,
                Ok(n) => total_read += n,
                Err(e) => return Err(e),
            }
        }
        if total_read == buf.len() {
            Ok(())
        } else {
            Err(SahneError::IOError("Unexpected end of file".to_string()))
        }
    }
}

#[cfg(not(feature = "std"))]
pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError>;
    fn stream_position(&mut self) -> Result<u64, SahneError>;
}

#[cfg(not(feature = "std"))]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

// FBX başlığını okuma fonksiyonu
fn read_fbx_header<R: Read>(reader: &mut R) -> Result<FbxHeader, SahneError> {
    let mut header = FbxHeader {
        magic_number: [0; 21],
        unknown: [0; 2],
        version: 0,
    };

    reader.read_exact(&mut header.magic_number)?;
    reader.read_exact(&mut header.unknown)?;
    #[cfg(feature = "std")]
    {
        let mut buf_reader = BufReader::new(reader);
        header.version = buf_reader.read_u32::<StdLittleEndian>()?;
    }
    #[cfg(not(feature = "std"))]
    {
        header.version = read_u32_le(reader)?;
    }

    Ok(header)
}

// FBX özelliği okuma fonksiyonu
fn read_fbx_property<R: Read>(reader: &mut R) -> Result<FbxProperty, SahneError> {
    let type_code = reader.read_u8()?;
    match type_code as char {
        'C' => Ok(FbxProperty::Bool(reader.read_u8()? != 0)), // Boolean (1: true, 0: false)
        'I' => {
            #[cfg(feature = "std")]
            return Ok(FbxProperty::Integer(BufReader::new(reader).read_i32::<StdLittleEndian>()?));
            #[cfg(not(feature = "std"))]
            return Ok(FbxProperty::Integer(read_i32_le(reader)?));
        }
        'L' => {
            #[cfg(feature = "std")]
            return Ok(FbxProperty::Long(BufReader::new(reader).read_i64::<StdLittleEndian>()?));
            #[cfg(not(feature = "std"))]
            return Ok(FbxProperty::Long(read_i64_le(reader)?));
        }
        'F' => {
            #[cfg(feature = "std")]
            return Ok(FbxProperty::Float(BufReader::new(reader).read_f32::<StdLittleEndian>()?));
            #[cfg(not(feature = "std"))]
            return Ok(FbxProperty::Float(read_f32_le(reader)?));
        }
        'D' => {
            #[cfg(feature = "std")]
            return Ok(FbxProperty::Double(BufReader::new(reader).read_f64::<StdLittleEndian>()?));
            #[cfg(not(feature = "std"))]
            return Ok(FbxProperty::Double(read_f64_le(reader)?));
        }
        'S' | 'R' => { // String ve Raw
            #[cfg(feature = "std")]
            let len = BufReader::new(reader).read_u32::<StdLittleEndian>()?;
            #[cfg(not(feature = "std"))]
            let len = read_u32_le(reader)?;
            let mut buffer = vec![0; len as usize];
            reader.read_exact(&mut buffer)?;
            if type_code as char == 'S' {
                match String::from_utf8(buffer) {
                    Ok(s) => Ok(FbxProperty::String(s)),
                    Err(_) => Err(SahneError::InvalidData("UTF8 hatası".to_string())),
                }
            } else {
                Ok(FbxProperty::RawBytes(buffer))
            }
        },
        _ => Err(SahneError::InvalidData(format!("Bilinmeyen özellik tipi: {}", type_code))),
    }
}

// FBX düğümünü okuma fonksiyonu
fn read_fbx_node<R: Read + Seek>(reader: &mut R) -> Result<Option<FbxNode>, SahneError> {
    #[cfg(feature = "std")]
    let mut buf_reader = BufReader::new(reader);

    #[cfg(feature = "std")]
    let end_offset = buf_reader.read_u32::<StdLittleEndian>()?;
    #[cfg(not(feature = "std"))]
    let end_offset = read_u32_le(reader)?;

    #[cfg(feature = "std")]
    let num_properties = buf_reader.read_u32::<StdLittleEndian>()?;
    #[cfg(not(feature = "std"))]
    let num_properties = read_u32_le(reader)?;

    #[cfg(feature = "std")]
    let property_list_len = buf_reader.read_u32::<StdLittleEndian>()?;
    #[cfg(not(feature = "std"))]
    let property_list_len = read_u32_le(reader)?;

    let name_len = reader.read_u8()?;

    // Boş düğüm kontrolü (end_offset == 0)
    if end_offset == 0 && num_properties == 0 && property_list_len == 0 && name_len == 0 {
        return Ok(None); // Boş düğüm, okumayı durdur
    }

    let mut name_bytes = vec![0; name_len as usize];
    reader.read_exact(&mut name_bytes)?;
    match String::from_utf8(name_bytes) {
        Ok(name) => {
            let mut properties = Vec::with_capacity(num_properties as usize);
            #[cfg(feature = "std")]
            let properties_end = reader.seek(StdSeekFrom::Current(property_list_len as i64))?; // Özellik listesinin sonunu hesapla
            #[cfg(not(feature = "std"))]
            let current_pos = reader.stream_position()?;
            #[cfg(not(feature = "std"))]
            let properties_end = current_pos.checked_add(property_list_len as u64).ok_or(SahneError::IOError("Offset overflow".to_string()))?;

            for _ in 0..num_properties {
                properties.push(read_fbx_property(reader)?);
            }

            #[cfg(feature = "std")]
            reader.seek(StdSeekFrom::Start(properties_end))?; // Özellik listesinden sonraki pozisyona geri dön
            #[cfg(not(feature = "std"))]
            reader.seek(SeekFrom::Start(properties_end))?;

            let mut nested_nodes = Vec::new();
            #[cfg(feature = "std")]
            while reader.stream_position()? < end_offset as u64 {
                if let Some(nested_node) = read_fbx_node(&mut reader)? {
                    nested_nodes.push(nested_node);
                } else {
                    break; // Boş düğüm bulunca iç içe düğüm okumasını durdur
                }
            }
            #[cfg(not(feature = "std"))]
            while reader.stream_position()? < end_offset as u64 {
                if let Some(nested_node) = read_fbx_node(reader)? {
                    nested_nodes.push(nested_node);
                } else {
                    break; // Boş düğüm bulunca iç içe düğüm okumasını durdur
                }
            }


            Ok(Some(FbxNode {
                end_offset,
                num_properties,
                property_list_len,
                name_len,
                name,
                properties,
                nested_nodes,
            }))
        }
        Err(_) => Err(SahneError::InvalidData("UTF8 hatası".to_string())),
    }
}


// FBX dosyasını okuma ve ayrıştırma fonksiyonu
fn read_fbx_file(file_path: &str) -> Result<Vec<FbxNode>, SahneError> {
    #[cfg(feature = "std")]
    let file = File::open(file_path)?;
    #[cfg(not(feature = "std"))]
    let file = fs::open(file_path, fs::O_RDONLY)?;

    #[cfg(feature = "std")]
    let mut reader = BufReader::new(file);
    #[cfg(not(feature = "std"))]
    let mut reader = file; // fs::open zaten bir Read + Seek döndürüyor

    let header = read_fbx_header(&mut reader)?;
    #[cfg(feature = "std")]
    println!("FBX Başlığı: {:?}", header); // Başlığı yazdır
    #[cfg(not(feature = "std"))]
    crate::println!("FBX Başlığı: {:?}", header);

    let mut nodes = Vec::new();
    while let Some(node) = read_fbx_node(&mut reader)? {
        nodes.push(node);
    }

    Ok(nodes)
}

#[cfg(feature = "std")]
fn main() -> io::Result<()> {
    let file_path = "model.fbx"; // Örnek FBX dosya yolu
    match read_fbx_file(file_path) {
        Ok(nodes) => {
            println!("FBX Dosyası Başarıyla Okundu ve Ayrıştırıldı.\n");
            // Kök düğümleri işle
            for node in &nodes {
                println!("Kök Düğüm: {:?}", node.name);
                // İstenirse düğüm özelliklerini ve iç içe düğümleri işle
                // print_node_recursive(node, 1); // Tüm düğüm yapısını yazdırmak için kullanılabilir
            }
        }
        Err(err) => {
            eprintln!("FBX dosyası okuma hatası: {}", err);
            return Err(io::Error::new(io::ErrorKind::Other, format!("{}", err)));
        }
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() -> Result<(), SahneError> {
    let file_path = "model.fbx"; // Örnek FBX dosya yolu
    match read_fbx_file(file_path) {
        Ok(nodes) => {
            crate::println!("FBX Dosyası Başarıyla Okundu ve Ayrıştırıldı.\n");
            // Kök düğümleri işle
            for node in &nodes {
                crate::println!("Kök Düğüm: {:?}", node.name);
                // İstenirse düğüm özelliklerini ve iç içe düğümleri işle
                // print_node_recursive(node, 1); // Tüm düğüm yapısını yazdırmak için kullanılabilir
            }
        }
        Err(err) => {
            crate::println!("FBX dosyası okuma hatası: {}", err);
            return Err(err);
        }
    }
    Ok(())
}

// (Opsiyonel) Düğüm yapısını recursive olarak yazdırma fonksiyonu (detaylı inceleme için)
#[allow(dead_code)] // Şimdilik kullanılmadığı için uyarıyı kapat
fn print_node_recursive(node: &FbxNode, indent_level: usize) {
    let indent = "  ".repeat(indent_level);
    #[cfg(feature = "std")]
    println!("{}{}: {:?}", indent, node.name, node.properties);
    #[cfg(not(feature = "std"))]
    crate::println!("{}{}: {:?}", indent, node.name, node.properties);
    for nested_node in &node.nested_nodes {
        print_node_recursive(nested_node, indent_level + 1);
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