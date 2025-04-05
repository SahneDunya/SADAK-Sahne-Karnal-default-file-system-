#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz

#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Gerekli Sahne64 modüllerini ve sabitlerini içe aktar
use crate::{
    arch::{
        SYSCALL_FILE_CLOSE, SYSCALL_FILE_OPEN, SYSCALL_FILE_READ,
    },
    fs::{O_RDONLY},
    memory, process, sync, ipc, kernel,
    SahneError,
};

// byteorder kütüphanesi yerine elle byte okuma ve dönüştürme yapacağız.
use core::mem::size_of;

// AVI Başlık Yapıları için Sabitler ve Yardımcı Fonksiyonlar (değişmeden kalabilir)
const CKID_RIFF: u32 = 0x46464952; // "RIFF"
const CKID_AVI : u32 = 0x20495641; // "AVI "
const CKID_LIST: u32 = 0x5453494C; // "LIST"
const CKID_hdrl: u32 = 0x6C726468; // "hdrl"
const CKID_avih: u32 = 0x68697661; // "avih"
const CKID_strl: u32 = 0x6C727473; // "strl"
const CKID_strh: u32 = 0x68727473; // "strh"
const CKID_strf: u32 = 0x66727473; // "strf"
const CKID_movi: u32 = 0x69766F6D; // "movi"
const CKID_00dc: u32 = 0x63643030; // "00dc" - Video Stream 0
const CKID_01wb: u32 = 0x62773130; // "01wb" - Audio Stream 1 (örnek)

fn fourcc_to_string(fourcc: u32) -> String {
    let bytes = fourcc.to_le_bytes();
    // no_std ortamında String::from_utf8_lossy direkt kullanılamayabilir.
    // Basit bir çözüm olarak, geçersiz UTF-8 için bir placeholder kullanabiliriz.
    core::str::from_utf8(&bytes).unwrap_or("???").into()
}

#[derive(Debug)]
struct AviMainHeader {
    dwMicroSecPerFrame: u32,
    dwMaxBytesPerSec: u32,
    dwPaddingGranularity: u32,
    dwFlags: u32,
    dwTotalFrames: u32,
    dwInitialFrames: u32,
    dwStreams: u32,
    dwSuggestedBufferSize: u32,
    dwWidth: u32,
    dwHeight: u32,
    dwSampleSize: u32,
    dwReserved: [u32; 4],
}

#[derive(Debug)]
struct AviStreamHeader {
    fccType: u32,
    fccHandler: u32,
    dwFlags: u32,
    dwPriority: u16,
    dwLanguage: u16,
    dwInitialFrames: u32,
    dwScale: u32,
    dwRate: u32,
    dwStart: u32,
    dwLength: u32,
    dwSuggestedBufferSize: u32,
    dwSampleSize: u32,
    rcFrame: [i16; 4],
}

#[derive(Debug)]
struct AviChunk {
    id: u32,
    size: u32,
    data: Vec<u8>,
}

#[derive(Debug)]
struct AviData {
    main_header: AviMainHeader,
    stream_headers: Vec<AviStreamHeader>,
    chunks: Vec<AviChunk>,
}

// Yardımcı fonksiyon: Belirtilen boyutta veri okur ve LittleEndian olarak yorumlar
fn read_u32_le(fd: u64) -> Result<u32, SahneError> {
    let mut buffer = [0u8; 4];
    fs::read(fd, &mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

fn read_u16_le(fd: u64) -> Result<u16, SahneError> {
    let mut buffer = [0u8; 2];
    fs::read(fd, &mut buffer)?;
    Ok(u16::from_le_bytes(buffer))
}

fn read_i16_le(fd: u64) -> Result<i16, SahneError> {
    let mut buffer = [0u8; 2];
    fs::read(fd, &mut buffer)?;
    Ok(i16::from_le_bytes(buffer))
}

fn read_exact(fd: u64, buffer: &mut [u8]) -> Result<(), SahneError> {
    let bytes_to_read = buffer.len();
    let mut bytes_read = 0;
    while bytes_read < bytes_to_read {
        let result = fs::read(fd, &mut buffer[bytes_read..])?;
        if result == 0 {
            return Err(SahneError::InvalidOperation); // Dosya sonuna beklenmedik şekilde ulaşıldı
        }
        bytes_read += result;
    }
    Ok(())
}

// Yardımcı fonksiyon: Dosyada belirtilen kadar ilerler (seek yerine)
fn skip_bytes(fd: u64, count: u32) -> Result<(), SahneError> {
    let mut buffer = [0u8; 1024]; // Tampon boyutu ayarlanabilir
    let mut remaining = count as usize;
    while remaining > 0 {
        let read_size = core::cmp::min(remaining, buffer.len());
        let result = fs::read(fd, &mut buffer[..read_size])?;
        if result == 0 {
            return Err(SahneError::InvalidOperation); // Dosya sonuna beklenmedik şekilde ulaşıldı
        }
        remaining -= result;
    }
    Ok(())
}

fn parse_avi(file_path: &str) -> Result<AviData, SahneError> {
    let fd = fs::open(file_path, O_RDONLY)?;

    // RIFF Başlığını Okuma ve Doğrulama
    let riff_header = read_u32_le(fd)?;
    if riff_header != CKID_RIFF {
        fs::close(fd)?;
        return Err(SahneError::InvalidData);
    }

    let file_size = read_u32_le(fd)?; // Dosya boyutunu okur, şimdilik kullanmıyoruz.
    let avi_header_ckid = read_u32_le(fd)?;
    if avi_header_ckid != CKID_AVI {
        fs::close(fd)?;
        return Err(SahneError::InvalidData);
    }

    // LIST 'hdrl' Başlığını Okuma ve Doğrulama
    let list_hdrl_ckid = read_u32_le(fd)?;
    if list_hdrl_ckid != CKID_LIST {
        fs::close(fd)?;
        return Err(SahneError::InvalidData);
    }
    let list_hdrl_size = read_u32_le(fd)?; // LIST boyutunu okur, şimdilik kullanmıyoruz.
    let hdrl_ckid = read_u32_le(fd)?;
    if hdrl_ckid != CKID_hdrl {
        fs::close(fd)?;
        return Err(SahneError::InvalidData);
    }

    // 'avih' (AVI Main Header) Başlığını Okuma ve Ayrıştırma
    let avih_ckid = read_u32_le(fd)?;
    if avih_ckid != CKID_avih {
        fs::close(fd)?;
        return Err(SahneError::InvalidData);
    }
    let avih_size = read_u32_le(fd)?;
    if avih_size as usize != size_of::<AviMainHeader>() - size_of::<[u32; 4]>() { // 'dwReserved' alanı hariç 56 bayt
        fs::close(fd)?;
        return Err(SahneError::InvalidData);
    }

    let main_header = parse_avi_main_header(fd)?;

    let mut stream_headers = Vec::new();
    let mut chunks = Vec::new();

    // LIST 'movi' ve 'strl' Başlıklarını ve Öbekleri Okuma
    loop {
        let list_or_chunk_ckid_result = read_u32_le(fd);
        let list_or_chunk_ckid = match list_or_chunk_ckid_result {
            Ok(ckid) => ckid,
            Err(e) => {
                if let SahneError::InvalidFileDescriptor = e { // Dosya sonuna ulaşıldığında bu hatayı alabiliriz.
                    break;
                }
                fs::close(fd)?;
                return Err(e);
            }
        };

        if list_or_chunk_ckid == CKID_LIST {
            let list_size = read_u32_le(fd)?;
            let list_type_ckid = read_u32_le(fd)?;

            if list_type_ckid == CKID_strl {
                // STREAM LIST - 'strl'
                let strl_header = parse_stream_list(fd)?;
                stream_headers.push(strl_header);
            } else if list_type_ckid == CKID_movi {
                 // MOVIE LIST - 'movi'
                 parse_movie_data(fd, &mut chunks, list_size)?;
            } else {
                // Bilinmeyen LIST türü, atla
                skip_bytes(fd, list_size - 4)?; // 4 byte 'list_type_ckid' boyutu kadar geri gitmeye gerek yok, zaten okuduk.
                println!("Bilinmeyen LIST türü: {}", fourcc_to_string(list_type_ckid));
            }

        } else {
            // DATA CHUNK
            let chunk_size = read_u32_le(fd)?;
            let chunk_id = list_or_chunk_ckid;

            let mut data = vec![0; chunk_size as usize];
            read_exact(fd, &mut data)?;

            chunks.push(AviChunk { id: chunk_id, size: chunk_size, data });
        }
    }

    fs::close(fd)?;
    Ok(AviData { main_header, stream_headers, chunks })
}

fn parse_avi_main_header(fd: u64) -> Result<AviMainHeader, SahneError> {
    Ok(AviMainHeader {
        dwMicroSecPerFrame: read_u32_le(fd)?,
        dwMaxBytesPerSec: read_u32_le(fd)?,
        dwPaddingGranularity: read_u32_le(fd)?,
        dwFlags: read_u32_le(fd)?,
        dwTotalFrames: read_u32_le(fd)?,
        dwInitialFrames: read_u32_le(fd)?,
        dwStreams: read_u32_le(fd)?,
        dwSuggestedBufferSize: read_u32_le(fd)?,
        dwWidth: read_u32_le(fd)?,
        dwHeight: read_u32_le(fd)?,
        dwSampleSize: read_u32_le(fd)?,
        dwReserved: [
            read_u32_le(fd)?,
            read_u32_le(fd)?,
            read_u32_le(fd)?,
            read_u32_le(fd)?,
        ],
    })
}

fn parse_stream_list(fd: u64) -> Result<AviStreamHeader, SahneError> {
    let strh_ckid = read_u32_le(fd)?;
    let strh_size = read_u32_le(fd)?;
    if strh_ckid != CKID_strh {
        return Err(SahneError::InvalidData);
    }
    if strh_size as usize != size_of::<AviStreamHeader>() - 8 { // 'fccType' ve 'fccHandler' hariç 48 bayt olmalı
        return Err(SahneError::InvalidData);
    }

    let stream_header = AviStreamHeader {
        fccType: read_u32_le(fd)?,
        fccHandler: read_u32_le(fd)?,
        dwFlags: read_u32_le(fd)?,
        dwPriority: read_u16_le(fd)?,
        dwLanguage: read_u16_le(fd)?,
        dwInitialFrames: read_u32_le(fd)?,
        dwScale: read_u32_le(fd)?,
        dwRate: read_u32_le(fd)?,
        dwStart: read_u32_le(fd)?,
        dwLength: read_u32_le(fd)?,
        dwSuggestedBufferSize: read_u32_le(fd)?,
        dwSampleSize: read_u32_le(fd)?,
        rcFrame: [
            read_i16_le(fd)?,
            read_i16_le(fd)?,
            read_i16_le(fd)?,
            read_i16_le(fd)?,
        ],
    };

    // 'strf' (Stream Format) Başlığını atla, şu anda ayrıştırmıyoruz.
    let strf_ckid = read_u32_le(fd)?;
    let strf_size = read_u32_le(fd)?;
    if strf_ckid != CKID_strf {
        return Err(SahneError::InvalidData);
    }
    skip_bytes(fd, strf_size)?; // strf verisini atla

    // 'strd' (Stream Data) ve 'strn' (Stream Name) gibi diğer isteğe bağlı başlıkları atlayabiliriz.

    Ok(stream_header)
}

fn parse_movie_data(fd: u64, chunks: &mut Vec<AviChunk>, list_size: u32) -> Result<(), SahneError> {
    let mut current_pos = 0;
    while current_pos < list_size - 4 { // LIST başlığı (4 bayt) hariç
        let chunk_id = read_u32_le(fd)?;
        let chunk_size = read_u32_le(fd)?;

        if current_pos + 8 + chunk_size > list_size { // 8 = chunk_id + chunk_size
            return Err(SahneError::InvalidData); // Öbek 'movi' LIST boyutunu aşıyor
        }

        let mut data = vec![0; chunk_size as usize];
        read_exact(fd, &mut data)?;
        chunks.push(AviChunk { id: chunk_id, size: chunk_size, data });

        current_pos += 8 + chunk_size;
    }
    Ok(())
}


#[cfg(feature = "std")]
fn main() -> Result<(), SahneError> {
    let avi_data_result = parse_avi("test.avi");

    match avi_data_result {
        Ok(avi_data) => {
            println!("AVI Dosyası Başarıyla Ayrıştırıldı!\n");
            println!("AVI Main Header: {:?}", avi_data.main_header);
            println!("\nStream Headers: {:?}", avi_data.stream_headers);
            println!("\nToplam Öbek Sayısı: {}", avi_data.chunks.len());
            // İsteğe bağlı olarak öbek verilerini işleyebilirsiniz.
             for chunk in &avi_data.chunks {
                 if chunk.id == CKID_00dc || chunk.id == CKID_01wb {
                     println!("\nVeri Öbeği ID: {}, Boyut: {} bytes, İlk 16 bayt: {:?}...",
                              fourcc_to_string(chunk.id), chunk.size, &chunk.data[..std::cmp::min(16, chunk.data.len())]);
                 } else {
                     // Diğer öbek türleri (örneğin 'idx1')
                     println!("\nÖbek ID: {}, Boyut: {} bytes", fourcc_to_string(chunk.id), chunk.size);
                 }
             }


        }
        Err(e) => {
            eprintln!("AVI Ayrıştırma Hatası: {:?}", e);
        }
    }

    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() -> Result<(), SahneError> {
    let avi_data_result = parse_avi("test.avi");

    match avi_data_result {
        Ok(avi_data) => {
            println!("AVI Dosyası Başarıyla Ayrıştırıldı!\n");
            println!("AVI Main Header: {:?}", avi_data.main_header);
            println!("\nStream Headers: {:?}", avi_data.stream_headers);
            println!("\nToplam Öbek Sayısı: {}", avi_data.chunks.len());
            // İsteğe bağlı olarak öbek verilerini işleyebilirsiniz.
             for chunk in &avi_data.chunks {
                 if chunk.id == CKID_00dc || chunk.id == CKID_01wb {
                     println!("\nVeri Öbeği ID: {}, Boyut: {} bytes, İlk 16 bayt: {:?}...",
                              fourcc_to_string(chunk.id), chunk.size, &chunk.data[..core::cmp::min(16, chunk.data.len())]);
                 } else {
                     // Diğer öbek türleri (örneğin 'idx1')
                     println!("\nÖbek ID: {}, Boyut: {} bytes", fourcc_to_string(chunk.id), chunk.size);
                 }
             }


        }
        Err(e) => {
            eprintln!("AVI Ayrıştırma Hatası: {:?}", e);
        }
    }

    Ok(())
}

// Bu kısım, no_std ortamında println! gibi makroların çalışması için gereklidir.
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

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}