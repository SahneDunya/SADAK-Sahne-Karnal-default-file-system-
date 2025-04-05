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
use core::str;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom};
#[cfg(feature = "std")]
use std::str as StdStr;

#[cfg(not(feature = "std"))]
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SahneError>;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), SahneError> {
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.read(&mut buf[total_read..]) {
                Ok(0) => return Err(SahneError::IOError("Unexpected end of file".to_string())),
                Ok(bytes_read) => total_read += bytes_read,
                Err(e) => return Err(e),
            }
        }
        Ok(())
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

#[derive(Debug)]
pub struct AlacMetadata {
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    // Daha fazla meta veri alanı eklenebilir
}

#[cfg(feature = "std")]
pub fn read_alac_metadata(file_path: &str) -> Result<AlacMetadata, io::Error> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // ftyp atomunu kontrol et
    check_ftyp_atom(&mut reader)?;

    // moov atomunu bul ve işle
    let moov_atom_offset = find_moov_atom(&mut reader)?;
    let metadata = process_moov_atom(&mut reader, moov_atom_offset)?;

    Ok(metadata)
}

#[cfg(not(feature = "std"))]
pub fn read_alac_metadata(file_path: &str) -> Result<AlacMetadata, SahneError> {
    let fd = fs::open(file_path, fs::O_RDONLY)?;
    let mut reader = FileDescReader { fd };

    // ftyp atomunu kontrol et
    check_ftyp_atom(&mut reader)?;

    // moov atomunu bul ve işle
    let moov_atom_offset = find_moov_atom(&mut reader)?;
    let metadata = process_moov_atom(&mut reader, moov_atom_offset)?;

    fs::close(fd)?;
    Ok(metadata)
}

#[cfg(feature = "std")]
fn check_ftyp_atom<R: StdRead>(reader: &mut R) -> Result<(), io::Error> {
    let mut ftyp_header = [0; 8];
    reader.read_exact(&mut ftyp_header)?;

    let ftyp_size = u32::from_be_bytes([ftyp_header[0], ftyp_header[1], ftyp_header[2], ftyp_header[3]]);
    let ftyp_type = &ftyp_header[4..8];

    if ftyp_type != b"ftyp" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Geçersiz MP4 dosyası: ftyp atomu bulunamadı",
        ));
    }

    if ftyp_size < 12 { // ftyp atomu en az 12 bayt olmalı (size + type + major_brand + ...)
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Geçersiz ftyp atom boyutu",
        ));
    }

    let mut major_brand = [0; 4];
    reader.read_exact(&mut major_brand)?;
    if &major_brand != b"M4A " && &major_brand != b"mp42" && &major_brand != b"isom" { // Yaygın major brandler
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Beklenmeyen major brand: {:?}", StdStr::from_utf8(&major_brand)),
        ));
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
fn check_ftyp_atom<R: Read>(reader: &mut R) -> Result<(), SahneError> {
    let mut ftyp_header = [0; 8];
    reader.read_exact(&mut ftyp_header)?;

    let ftyp_size = u32::from_be_bytes([ftyp_header[0], ftyp_header[1], ftyp_header[2], ftyp_header[3]]);
    let ftyp_type = &ftyp_header[4..8];

    if ftyp_type != b"ftyp" {
        return Err(SahneError::IOError("Geçersiz MP4 dosyası: ftyp atomu bulunamadı".to_string()));
    }

    if ftyp_size < 12 { // ftyp atomu en az 12 bayt olmalı (size + type + major_brand + ...)
        return Err(SahneError::IOError("Geçersiz ftyp atom boyutu".to_string()));
    }

    let mut major_brand = [0; 4];
    reader.read_exact(&mut major_brand)?;
    if &major_brand != b"M4A " && &major_brand != b"mp42" && &major_brand != b"isom" { // Yaygın major brandler
        return Err(SahneError::IOError(format!("Beklenmeyen major brand: {:?}", str::from_utf8(&major_brand).unwrap_or(""))));
    }
    Ok(())
}

#[cfg(feature = "std")]
fn find_moov_atom<R: StdRead + StdSeek>(reader: &mut R) -> Result<u64, io::Error> {
    reader.seek(StdSeekFrom::Start(0))?; // Dosyanın başına git
    loop {
        let mut header = [0; 8];
        let bytes_read = reader.read(&mut header)?;
        if bytes_read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "moov atomu bulunamadı",
            ));
        }
        if bytes_read < 8 {
            continue; // Yeterli veri yok, okumaya devam et
        }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let atom_type = &header[4..8];

        if atom_type == b"moov" {
            return Ok(reader.stream_position()? - 8); // 'moov' atomunun başlangıç pozisyonunu döndür
        }

        // Bir sonraki atomun başına atla
        let size_to_skip = atom_size.saturating_sub(8) as u64; // Zaten 8 bayt okuduk
        if size_to_skip > 0 {
             reader.seek(StdSeekFrom::Current(size_to_skip))?;
        } else if atom_size == 0 {
            // Atom boyutu 0 ise, dosyanın sonuna kadar oku (bu pratikte MP4'te pek olası değil ama ihtiyatlı olalım)
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "moov atomu bulunamadı (boyut 0 atom)",
            ));
        }
        // Atom boyutu 1 ise, 64-bit boyut takip eder (şimdilik basit tutalım ve bu durumu atlayalım, gerekirse eklenebilir)
        else if atom_size == 1 {
             return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "64-bit atom boyutları henüz desteklenmiyor",
            ));
        }
    }
}

#[cfg(not(feature = "std"))]
fn find_moov_atom<R: Read + Seek>(reader: &mut R) -> Result<u64, SahneError> {
    reader.seek(SeekFrom::Start(0))?; // Dosyanın başına git
    loop {
        let mut header = [0; 8];
        let bytes_read = reader.read(&mut header)?;
        if bytes_read == 0 {
            return Err(SahneError::IOError("moov atomu bulunamadı".to_string()));
        }
        if bytes_read < 8 {
            continue; // Yeterli veri yok, okumaya devam et
        }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let atom_type = &header[4..8];

        if atom_type == b"moov" {
            return Ok(reader.stream_position()? - 8); // 'moov' atomunun başlangıç pozisyonunu döndür
        }

        // Bir sonraki atomun başına atla
        let size_to_skip = atom_size.saturating_sub(8) as u64; // Zaten 8 bayt okuduk
        if size_to_skip > 0 {
             reader.seek(SeekFrom::Current(size_to_skip))?;
        } else if atom_size == 0 {
            // Atom boyutu 0 ise, dosyanın sonuna kadar oku (bu pratikte MP4'te pek olası değil ama ihtiyatlı olalım)
            return Err(SahneError::IOError("moov atomu bulunamadı (boyut 0 atom)".to_string()));
        }
        // Atom boyutu 1 ise, 64-bit boyut takip eder (şimdilik basit tutalım ve bu durumu atlayalım, gerekirse eklenebilir)
        else if atom_size == 1 {
             return Err(SahneError::NotSupported);
        }
    }
}

#[cfg(feature = "std")]
fn process_moov_atom<R: StdRead + StdSeek>(reader: &mut R, moov_atom_offset: u64) -> Result<AlacMetadata, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
    reader.seek(StdSeekFrom::Start(moov_atom_offset))?;
    let mut moov_header = [0; 8];
    reader.read_exact(&mut moov_header)?; // moov başlığını tekrar oku (boyut ve tip)
    let moov_size = u32::from_be_bytes([moov_header[0], moov_header[1], moov_header[2], moov_header[3]]) as u64;

    let mut current_offset = moov_atom_offset + 8; // moov başlığından sonraki pozisyon

    while current_offset < moov_atom_offset + moov_size {
        reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() { // Dosya sonuna ulaşılmış olabilir
            break;
        }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"trak" {
            let metadata = process_trak_atom(reader, current_offset)?;
            if let Some(metadata) = metadata {
                return Ok(metadata);
            }
        }

        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Geçersiz atom boyutu (taşma)",
            ));
        }
        current_offset = next_offset;

        if atom_size == 0 { // Boyut 0 ise, bu geçersiz bir durum veya dosya sonu (daha önce kontrol ettik)
             break; // Güvenli çıkış
        }
        if atom_size == 1 { // 64-bit boyut (şimdilik atla)
            reader.seek(StdSeekFrom::Current(8))?; // 64-bit boyut alanını atla
            current_offset += 8;
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "ALAC meta verileri bulunamadı (trak atomu içinde)",
    ))
}

#[cfg(not(feature = "std"))]
fn process_moov_atom<R: Read + Seek>(reader: &mut R, moov_atom_offset: u64) -> Result<AlacMetadata, SahneError> {
    reader.seek(SeekFrom::Start(moov_atom_offset))?;
    let mut moov_header = [0; 8];
    reader.read_exact(&mut moov_header)?; // moov başlığını tekrar oku (boyut ve tip)
    let moov_size = u32::from_be_bytes([moov_header[0], moov_header[1], moov_header[2], moov_header[3]]) as u64;

    let mut current_offset = moov_atom_offset + 8; // moov başlığından sonraki pozisyon

    while current_offset < moov_atom_offset + moov_size {
        reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() { // Dosya sonuna ulaşılmış olabilir
            break;
        }

        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"trak" {
            let metadata = process_trak_atom(reader, current_offset)?;
            if let Some(metadata) = metadata {
                return Ok(metadata);
            }
        }

        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(SahneError::IOError("Geçersiz atom boyutu (taşma)".to_string()));
        }
        current_offset = next_offset;

        if atom_size == 0 { // Boyut 0 ise, bu geçersiz bir durum veya dosya sonu (daha önce kontrol ettik)
             break; // Güvenli çıkış
        }
        if atom_size == 1 { // 64-bit boyut (şimdilik atla)
            reader.seek(SeekFrom::Current(8))?; // 64-bit boyut alanını atla
            current_offset += 8;
        }
    }

    Err(SahneError::IOError("ALAC meta verileri bulunamadı (trak atomu içinde)".to_string()))
}

#[cfg(feature = "std")]
fn process_trak_atom<R: StdRead + StdSeek>(reader: &mut R, trak_atom_offset: u64) -> Result<Option<AlacMetadata>, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
    let mut trak_header = [0; 8];
    reader.seek(StdSeekFrom::Start(trak_atom_offset))?;
    reader.read_exact(&mut trak_header)?;
    let trak_size = u32::from_be_bytes([trak_header[0], trak_header[1], trak_header[2], trak_header[3]]) as u64;


    let mut current_offset = trak_atom_offset + 8;

    while current_offset < trak_atom_offset + trak_size {
         reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"mdia" {
            let metadata = process_mdia_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Geçersiz atom boyutu (taşma)",
            ));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(StdSeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(not(feature = "std"))]
fn process_trak_atom<R: Read + Seek>(reader: &mut R, trak_atom_offset: u64) -> Result<Option<AlacMetadata>, SahneError> {
    let mut trak_header = [0; 8];
    reader.seek(SeekFrom::Start(trak_atom_offset))?;
    reader.read_exact(&mut trak_header)?;
    let trak_size = u32::from_be_bytes([trak_header[0], trak_header[1], trak_header[2], trak_header[3]]) as u64;


    let mut current_offset = trak_atom_offset + 8;

    while current_offset < trak_atom_offset + trak_size {
         reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"mdia" {
            let metadata = process_mdia_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(SahneError::IOError("Geçersiz atom boyutu (taşma)".to_string()));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(SeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(feature = "std")]
fn process_mdia_atom<R: StdRead + StdSeek>(reader: &mut R, mdia_atom_offset: u64) -> Result<Option<AlacMetadata>, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
     let mut mdia_header = [0; 8];
    reader.seek(StdSeekFrom::Start(mdia_atom_offset))?;
    reader.read_exact(&mut mdia_header)?;
    let mdia_size = u32::from_be_bytes([mdia_header[0], mdia_header[1], mdia_header[2], mdia_header[3]]) as u64;

    let mut current_offset = mdia_atom_offset + 8;

    while current_offset < mdia_atom_offset + mdia_size {
         reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];


        if atom_type == b"minf" {
            let metadata = process_minf_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Geçersiz atom boyutu (taşma)",
            ));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(StdSeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(not(feature = "std"))]
fn process_mdia_atom<R: Read + Seek>(reader: &mut R, mdia_atom_offset: u64) -> Result<Option<AlacMetadata>, SahneError> {
     let mut mdia_header = [0; 8];
    reader.seek(SeekFrom::Start(mdia_atom_offset))?;
    reader.read_exact(&mut mdia_header)?;
    let mdia_size = u32::from_be_bytes([mdia_header[0], mdia_header[1], mdia_header[2], mdia_header[3]]) as u64;

    let mut current_offset = mdia_atom_offset + 8;

    while current_offset < mdia_atom_offset + mdia_size {
         reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];


        if atom_type == b"minf" {
            let metadata = process_minf_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(SahneError::IOError("Geçersiz atom boyutu (taşma)".to_string()));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(SeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(feature = "std")]
fn process_minf_atom<R: StdRead + StdSeek>(reader: &mut R, minf_atom_offset: u64) -> Result<Option<AlacMetadata>, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
    let mut minf_header = [0; 8];
    reader.seek(StdSeekFrom::Start(minf_atom_offset))?;
    reader.read_exact(&mut minf_header)?;
    let minf_size = u32::from_be_bytes([minf_header[0], minf_header[1], minf_header[2], minf_header[3]]) as u64;

    let mut current_offset = minf_atom_offset + 8;

    while current_offset < minf_atom_offset + minf_size {
         reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stbl" {
            let metadata = process_stbl_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Geçersiz atom boyutu (taşma)",
            ));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(StdSeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(not(feature = "std"))]
fn process_minf_atom<R: Read + Seek>(reader: &mut R, minf_atom_offset: u64) -> Result<Option<AlacMetadata>, SahneError> {
    let mut minf_header = [0; 8];
    reader.seek(SeekFrom::Start(minf_atom_offset))?;
    reader.read_exact(&mut minf_header)?;
    let minf_size = u32::from_be_bytes([minf_header[0], minf_header[1], minf_header[2], minf_header[3]]) as u64;

    let mut current_offset = minf_atom_offset + 8;

    while current_offset < minf_atom_offset + minf_size {
         reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
        if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stbl" {
            let metadata = process_stbl_atom(reader, current_offset)?;
            if metadata.is_some() {
                return Ok(metadata);
            }
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(SahneError::IOError("Geçersiz atom boyutu (taşma)".to_string()));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(SeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(feature = "std")]
fn process_stbl_atom<R: StdRead + StdSeek>(reader: &mut R, stbl_atom_offset: u64) -> Result<Option<AlacMetadata>, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
     let mut stbl_header = [0; 8];
    reader.seek(StdSeekFrom::Start(stbl_atom_offset))?;
    reader.read_exact(&mut stbl_header)?;
    let stbl_size = u32::from_be_bytes([stbl_header[0], stbl_header[1], stbl_header[2], stbl_header[3]]) as u64;

    let mut current_offset = stbl_atom_offset + 8;

    while current_offset < stbl_atom_offset + stbl_size {
         reader.seek(StdSeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
         if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stsd" {
            return process_stsd_atom(reader, current_offset);
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Geçersiz atom boyutu (taşma)",
            ));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(StdSeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(not(feature = "std"))]
fn process_stbl_atom<R: Read + Seek>(reader: &mut R, stbl_atom_offset: u64) -> Result<Option<AlacMetadata>, SahneError> {
     let mut stbl_header = [0; 8];
    reader.seek(SeekFrom::Start(stbl_atom_offset))?;
    reader.read_exact(&mut stbl_header)?;
    let stbl_size = u32::from_be_bytes([stbl_header[0], stbl_header[1], stbl_header[2], stbl_header[3]]) as u64;

    let mut current_offset = stbl_atom_offset + 8;

    while current_offset < stbl_atom_offset + stbl_size {
         reader.seek(SeekFrom::Start(current_offset))?;
        let mut header = [0; 8];
         if reader.read_exact(&mut header).is_err() {
            break;
        }
        let atom_size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let atom_type = &header[4..8];

        if atom_type == b"stsd" {
            return process_stsd_atom(reader, current_offset);
        }
        let next_offset = current_offset + atom_size;
        if next_offset <= current_offset { // Taşma kontrolü
            return Err(SahneError::IOError("Geçersiz atom boyutu (taşma)".to_string()));
        }
        current_offset = next_offset;
         if atom_size == 0 {
             break; // Güvenli çıkış
        }
         if atom_size == 1 { // 64-bit boyut
            reader.seek(SeekFrom::Current(8))?;
            current_offset += 8;
        }
    }
    Ok(None)
}

#[cfg(feature = "std")]
fn process_stsd_atom<R: StdRead + StdSeek>(reader: &mut R, stsd_atom_offset: u64) -> Result<Option<AlacMetadata>, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
    reader.seek(StdSeekFrom::Start(stsd_atom_offset + 8))?; // stsd başlığını atla (boyut+tip)

    let mut version_flags = [0; 4];
    reader.read_exact(&mut version_flags)?; // version(1 byte) + flags(3 bytes) (şimdilik kullanılmıyor)

    let mut entry_count_bytes = [0; 4];
    reader.read_exact(&mut entry_count_bytes)?;
    let entry_count = u32::from_be_bytes(entry_count_bytes);

    for _ in 0..entry_count {
        let mut entry_header = [0; 8];
        reader.read_exact(&mut entry_header)?;
        let entry_size = u32::from_be_bytes([entry_header[0], entry_header[1], entry_header[2], entry_header[3]]) as u64;
        let entry_type = &entry_header[4..8];


        if entry_type == b"alac" || entry_type == b"enca" { // 'enca' şifrelenmiş ALAC için olabilir, emin değiliz, kontrol etmek gerek
            return read_alac_desc_data(reader, reader.stream_position()? - 8); // ALAC descriptor offset
        } else {
            reader.seek(StdSeekFrom::Current(entry_size - 8))?; // Diğer descriptor'ı atla
        }
    }

    Ok(None) // ALAC descriptor bulunamadı
}

#[cfg(not(feature = "std"))]
fn process_stsd_atom<R: Read + Seek>(reader: &mut R, stsd_atom_offset: u64) -> Result<Option<AlacMetadata>, SahneError> {
    reader.seek(SeekFrom::Start(stsd_atom_offset + 8))?; // stsd başlığını atla (boyut+tip)

    let mut version_flags = [0; 4];
    reader.read_exact(&mut version_flags)?; // version(1 byte) + flags(3 bytes) (şimdilik kullanılmıyor)

    let mut entry_count_bytes = [0; 4];
    reader.read_exact(&mut entry_count_bytes)?;
    let entry_count = u32::from_be_bytes(entry_count_bytes);

    for _ in 0..entry_count {
        let mut entry_header = [0; 8];
        reader.read_exact(&mut entry_header)?;
        let entry_size = u32::from_be_bytes([entry_header[0], entry_header[1], entry_header[2], entry_header[3]]) as u64;
        let entry_type = &entry_header[4..8];


        if entry_type == b"alac" || entry_type == b"enca" { // 'enca' şifrelenmiş ALAC için olabilir, emin değiliz, kontrol etmek gerek
            return read_alac_desc_data(reader, reader.stream_position()? - 8); // ALAC descriptor offset
        } else {
            reader.seek(SeekFrom::Current(entry_size - 8))?; // Diğer descriptor'ı atla
        }
    }

    Ok(None) // ALAC descriptor bulunamadı
}

#[cfg(feature = "std")]
fn read_alac_desc_data<R: StdRead + StdSeek>(reader: &mut R, alac_desc_offset: u64) -> Result<Option<AlacMetadata>, io::Error> {
    // ... (Kalan fonksiyonlar benzer şekilde güncellenecektir)
    reader.seek(StdSeekFrom::Start(alac_desc_offset + 8))?; // alac atom başlığını atla

    // Descriptor versiyon ve revizyon numaralarını atla (ilk 4 bayt)
    reader.seek(StdSeekFrom::Current(4))?;

    let mut sample_size_bytes = [0; 2];
    reader.read_exact(&mut sample_size_bytes)?;
    let bits_per_sample = sample_size_bytes[1]; // SampleSize'ın ikinci baytı bitsPerSample içerir


    let mut num_channels_bytes = [0; 2];
    reader.read_exact(&mut num_channels_bytes)?;
    let channels = num_channels_bytes[1]; // NumChannels'ın ikinci baytı kanalları içerir


    // Aşağıdaki alanlar için 4'er bayt atla:
    // 3:  Always 0x0010
    // 4:  SampleRate
    // 5:  bytesPerFrame
    // 6:  framesPerPacket
    // 7:  compatibleVersion
    // 8:  bitRate
    // 9:  sampleRate
    reader.seek(StdSeekFrom::Current(4 + 4 + 4 + 4 + 4 + 4 + 4))?;

    let mut sample_rate_bytes = [0; 4];
    reader.read_exact(&mut sample_rate_bytes)?;
    // Sample rate Big Endian olarak saklanır
    let sample_rate = u32::from_be_bytes(sample_rate_bytes);


    Ok(Some(AlacMetadata {
        sample_rate,
        channels,
        bits_per_sample,
    }))
}

#[cfg(not(feature = "std"))]
fn read_alac_desc_data<R: Read + Seek>(reader: &mut R, alac_desc_offset: u64) -> Result<Option<AlacMetadata>, SahneError> {
    reader.seek(SeekFrom::Start(alac_desc_offset + 8))?; // alac atom başlığını atla

    // Descriptor versiyon ve revizyon numaralarını atla (ilk 4 bayt)
    reader.seek(SeekFrom::Current(4))?;

    let mut sample_size_bytes = [0; 2];
    reader.read_exact(&mut sample_size_bytes)?;
    let bits_per_sample = sample_size_bytes[1]; // SampleSize'ın ikinci baytı bitsPerSample içerir


    let mut num_channels_bytes = [0; 2];
    reader.read_exact(&mut num_channels_bytes)?;
    let channels = num_channels_bytes[1]; // NumChannels'ın ikinci baytı kanalları içerir


    // Aşağıdaki alanlar için 4'er bayt atla:
    // 3:  Always 0x0010
    // 4:  SampleRate
    // 5:  bytesPerFrame
    // 6:  framesPerPacket
    // 7:  compatibleVersion
    // 8:  bitRate
    // 9:  sampleRate
    reader.seek(SeekFrom::Current(4 + 4 + 4 + 4 + 4 + 4 + 4))?;

    let mut sample_rate_bytes = [0; 4];
    reader.read_exact(&mut sample_rate_bytes)?;
    // Sample rate Big Endian olarak saklanır
    let sample_rate = u32::from_be_bytes(sample_rate_bytes);


    Ok(Some(AlacMetadata {
        sample_rate,
        channels,
        bits_per_sample,
    }))
}

#[cfg(feature = "std")]
fn main() -> Result<(), io::Error> {
    let file_path = "example.m4a"; // Lütfen geçerli bir ALAC (.m4a) dosya yolu sağlayın
    match read_alac_metadata(file_path) {
        Ok(metadata) => {
            println!("Sample Rate: {}", metadata.sample_rate);
            println!("Channels: {}", metadata.channels);
            println!("Bits per Sample: {}", metadata.bits_per_sample);
        }
        Err(e) => {
            eprintln!("Meta veri okuma hatası: {}", e);
        }
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() -> Result<(), SahneError> {
    let file_path = "example.m4a"; // Lütfen geçerli bir ALAC (.m4a) dosya yolu sağlayın
    match read_alac_metadata(file_path) {
        Ok(metadata) => {
            crate::println!("Sample Rate: {}", metadata.sample_rate);
            crate::println!("Channels: {}", metadata.channels);
            crate::println!("Bits per Sample: {}", metadata.bits_per_sample);
        }
        Err(e) => {
            crate::println!("Meta veri okuma hatası: {}", e);
        }
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
struct FileDescReader {
    fd: u64,
}

#[cfg(not(feature = "std"))]
impl Read for FileDescReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SahneError> {
        fs::read(self.fd, buf)
    }
}

#[cfg(not(feature = "std"))]
impl Seek for FileDescReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError> {
        let offset = match pos {
            SeekFrom::Start(o) => o as i64,
            SeekFrom::End(o) => o,
            SeekFrom::Current(o) => o,
        };
        fs::lseek(self.fd, offset, match pos {
            SeekFrom::Start(_) => fs::SEEK_SET,
            SeekFrom::End(_) => fs::SEEK_END,
            SeekFrom::Current(_) => fs::SEEK_CUR,
        })
    }

    fn stream_position(&mut self) -> Result<u64, SahneError> {
        fs::lseek(self.fd, 0, fs::SEEK_CUR)
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