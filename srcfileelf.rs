#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)]

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Gerekli temel Sahne64 tipleri ve modülleri
use crate::{FileSystemError, SahneError, Handle}; // Hata tipleri ve Handle
// Sahne64 resource modülü
#[cfg(not(feature = "std"))]
use crate::resource;
// Sahne64 fs modülü (fs::read_at, fs::fstat için varsayım)
#[cfg(not(feature = "std"))]
use crate::fs;

// alloc crate for String, Vec
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

// core::io traits and types needed for SahneResourceReader
#[cfg(not(feature = "std"))]
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind}; // core::io
use core::mem; // core::mem

// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülden import edildiği varsayılır


// Helper function to map SahneError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
    // TODO: Implement a proper mapping based on SahneError variants
}

// Helper function to map CoreIOError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_core_io_error_to_fs_error(e: CoreIOError) -> FileSystemError {
     FileSystemError::IOError(format!("CoreIOError: {:?}", e))
     // TODO: Implement a proper mapping based on CoreIOErrorKind
}

// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilebmp.rs'den kopyalandı)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
// fstat ile dosya boyutunu alarak seek(End) desteği sağlar.
// Sahne64 API'sının bu syscall'ları Handle üzerinde sağladığı varsayılır.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReader {
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu
}

#[cfg(not(feature = "std"))]
impl SahneResourceReader {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReader { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl Read for SahneResourceReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CoreIOError> {
        if self.position >= self.file_size {
            return Ok(0); // EOF
        }
        let bytes_available = (self.file_size - self.position) as usize;
        let bytes_to_read = core::cmp::min(buf.len(), bytes_available);

        if bytes_to_read == 0 {
             return Ok(0);
        }

        // Assuming fs::read_at(handle, offset, buf) Result<usize, SahneError>
        let bytes_read = fs::read_at(self.handle, self.position, &mut buf[..bytes_to_read])
            .map_err(|e| CoreIOError::new(CoreIOErrorKind::Other, format!("fs::read_at error: {:?}", e)))?;

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read
}

#[cfg(not(feature = "std"))]
impl Seek for SahneResourceReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, CoreIOError> {
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize,
            SeekFrom::End(offset) => {
                file_size_isize.checked_add(offset)
                    .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from end)")))?
            },
            SeekFrom::Current(offset) => {
                (self.position as isize).checked_add(offset)
                     .ok_or_else(|| CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Seek position out of bounds (from current)")))?
            },
        };

        if new_pos < 0 {
            return Err(CoreIOError::new(CoreIOErrorKind::InvalidInput, format!("Invalid seek position (result is negative)")));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
    // stream_position has a default implementation in core::io::Seek
}


// ELF başlık yapısı (safely read field by field)
// Note: This assumes ELF64 and Little Endian for simplicity.
// A real parser must check e_ident[4] for class and e_ident[5] for endianness.
#[derive(Debug)] // Add Debug trait for easy printing
pub struct ElfHeader {
    pub e_ident: [u8; 16],
    pub e_type: u16,      // object file type
    pub e_machine: u16,   // architecture
    pub e_version: u32,   // object file version
    pub e_entry: u64,     // entry point address
    pub e_phoff: u64,     // program header offset
    pub e_shoff: u64,     // section header offset
    pub e_flags: u32,     // processor flags
    pub e_ehsize: u16,    // ELF header size
    pub e_phentsize: u16, // program header entry size
    pub e_phnum: u16,     // number of program header entries
    pub e_shentsize: u16, // section header entry size
    pub e_shnum: u16,     // number of section header entries
    pub e_shstrndx: u16,  // section header string table index
}

// Program başlık yapısı (safely read field by field)
// Assumes ELF64
#[derive(Debug)] // Add Debug trait
pub struct ProgramHeader {
    pub p_type: u32,    // type of segment
    pub p_flags: u32,   // segment flags
    pub p_offset: u64,  // offset in file
    pub p_vaddr: u64,   // virtual address in memory
    pub p_paddr: u64,   // physical address (OS specific)
    pub p_filesz: u64,  // size of segment in file
    pub p_memsz: u64,   // size of segment in memory
    pub p_align: u64,   // segment alignment
}

// ELF dosyasını temsil eden yapı
#[derive(Debug)] // Add Debug trait
pub struct ElfFile {
    pub header: ElfHeader,
    pub program_headers: Vec<ProgramHeader>,
    // We don't store the entire file content here.
    // Reading sections/segments would happen via methods on ElfFile,
    // using the underlying Handle and header information.
    // handle: Handle, // Storing Handle here might be problematic with ownership/lifetimes.
    // file_size: usize, // File size can be stored if needed for checks.
    // The handle is ideally managed by the entity that creates ElfFile,
    // or ElfFile takes a Reader/Seeker trait object.
}

impl ElfFile {
    /// Reads and parses an ELF file from the given Sahne64 resource ID.
    ///
    /// # Arguments
    ///
    /// * `resource_id` - The Sahne64 resource ID (file path).
    ///
    /// # Returns
    ///
    /// A Result containing the parsed ElfFile or a FileSystemError.
    #[cfg(not(feature = "std"))] // Only for no_std Sahne64
    pub fn open(resource_id: &str) -> Result<Self, FileSystemError> { // FileSystemError döner
        // Kaynağı edin
        let handle = resource::acquire(resource_id, resource::MODE_READ)
            .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

        // Dosyanın boyutunu al
         let file_stat = fs::fstat(handle)
             .map_err(|e| {
                  let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
                  map_sahne_error_to_fs_error(e)
              })?;
         let file_size = file_stat.size as u64;

        // SahneResourceReader oluştur
        let mut reader = SahneResourceReader::new(handle, file_size);

        // Başlığı oku ve ayrıştır
        let header = ElfFile::read_header(&mut reader)
            .map_err(|e| {
                 let _ = resource::release(handle).map_err(|release_e| eprintln!("WARN: Kaynak serbest bırakma hatası after header read error: {:?}", release_e));
                 e // Pass the original parsing error
             })?;


        // Program başlıklarını oku ve ayrıştır
        let program_headers = ElfFile::read_program_headers(&mut reader, &header)
            .map_err(|e| {
                 let _ = resource::release(handle).map_err(|release_e| eprintln!("WARN: Kaynak serbest bırakma hatası after program header read error: {:?}", release_e));
                 e // Pass the original parsing error
             })?;


        // Kaynağı serbest bırak (ELF verileri bellekte tutulduğu için handle'a artık ihtiyaç yok)
        let _ = resource::release(handle).map_err(|e| {
             eprintln!("WARN: Sahne64 kaynak serbest bırakma hatası: {:?}", e);
             map_sahne_error_to_fs_error(e)
         });


        Ok(ElfFile {
            header,
            program_headers,
        })
    }

    /// Reads and parses the ELF header from the provided reader.
    /// Assumes the reader is positioned at the start of the header (offset 0).
    /// Assumes ELF64 and Little Endian for parsing the fields after e_ident.
    #[cfg(not(feature = "std"))] // Only compile for no_std Sahne64
    fn read_header<R: Read + Seek>(reader: &mut R) -> Result<ElfHeader, FileSystemError> { // FileSystemError döner
        let header_size = mem::size_of::<ElfHeader>(); // Size of our ElfHeader struct (assuming ELF64)
        let mut buffer = [0u8; 64]; // Buffer to read into, large enough for ElfHeader

        // Read the first 64 bytes (or header_size if smaller, though 64 is standard for ELF64)
        reader.seek(SeekFrom::Start(0)).map_err(map_core_io_error_to_fs_error)?; // Ensure we are at the start
        let bytes_read = reader.read(&mut buffer[..header_size]).map_err(map_core_io_error_to_fs_error)?;
        if bytes_read != header_size {
            return Err(FileSystemError::InvalidData(format!("Beklenen {} bayt yerine {} bayt okundu", header_size, bytes_read)));
        }

        // Safely parse fields from the buffer (assuming Little Endian for multi-byte fields)
        let mut e_ident = [0u8; 16];
        e_ident.copy_from_slice(&buffer[0..16]);

        // Check ELF magic number (\x7F E L F)
        if &e_ident[0..4] != b"\x7fELF" {
            return Err(FileSystemError::InvalidData(format!("Geçersiz ELF sihirli sayısı: {:x?}", &e_ident[0..4])));
        }
        // TODO: Check e_ident[EI_CLASS] for ELFCLASS32 or ELFCLASS64
        // TODO: Check e_ident[EI_DATA] for ELFDATA2LSB (Little Endian) or ELFDATA2MSB (Big Endian)
        // And adjust byte parsing below based on endianness.
        // For now, assuming ELFCLASS64 and ELFDATA2LSB (Little Endian)

        let e_type = u16::from_le_bytes(buffer[16..18].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_type baytları geçersiz")))?);
        let e_machine = u16::from_le_bytes(buffer[18..20].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_machine baytları geçersiz")))?);
        let e_version = u32::from_le_bytes(buffer[20..24].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_version baytları geçersiz")))?);
        let e_entry = u64::from_le_bytes(buffer[24..32].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_entry baytları geçersiz")))?);
        let e_phoff = u64::from_le_bytes(buffer[32..40].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_phoff baytları geçersiz")))?);
        let e_shoff = u64::from_le_bytes(buffer[40..48].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_shoff baytları geçersiz")))?);
        let e_flags = u32::from_le_bytes(buffer[48..52].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_flags baytları geçersiz")))?);
        let e_ehsize = u16::from_le_bytes(buffer[52..54].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_ehsize baytları geçersiz")))?);
        let e_phentsize = u16::from_le_bytes(buffer[54..56].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_phentsize baytları geçersiz")))?);
        let e_phnum = u16::from_le_bytes(buffer[56..58].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_phnum baytları geçersiz")))?);
        let e_shentsize = u16::from_le_bytes(buffer[58..60].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_shentsize baytları geçersiz")))?);
        let e_shnum = u16::from_le_bytes(buffer[60..62].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_shnum baytları geçersiz")))?);
        let e_shstrndx = u16::from_le_bytes(buffer[62..64].try_into().map_err(|_| FileSystemError::InvalidData(format!("e_shstrndx baytları geçersiz")))?);


        // Optional: Verify e_ehsize matches the expected header size for the ELF class
        if e_ehsize as usize != header_size {
             eprintln!("WARN: e_ehsize ({}) does not match expected ELF header size ({})", e_ehsize, header_size); // no_std print
             // Depending on strictness, this could be an error
             // return Err(FileSystemError::InvalidData(format!("ELF header size mismatch")));
        }
        // Optional: Verify e_phentsize and e_shentsize match expected sizes based on ELF class

        Ok(ElfHeader {
            e_ident,
            e_type,
            e_machine,
            e_version,
            e_entry,
            e_phoff,
            e_shoff,
            e_flags,
            e_ehsize,
            e_phentsize,
            e_phnum,
            e_shentsize,
            e_shnum,
            e_shstrndx,
        })
    }

    /// Reads and parses the ELF program headers from the provided reader.
    /// Assumes the reader provides seek functionality.
    /// Assumes ELF64 and Little Endian for parsing.
    #[cfg(not(feature = "std"))] // Only compile for no_std Sahne64
    fn read_program_headers<R: Read + Seek>(
        reader: &mut R,
        header: &ElfHeader,
    ) -> Result<Vec<ProgramHeader>, FileSystemError> { // FileSystemError döner
        let program_header_size = mem::size_of::<ProgramHeader>(); // Size of our ProgramHeader struct (assuming ELF64)

        // Seek to the start of the program headers
        reader.seek(SeekFrom::Start(header.e_phoff)).map_err(map_core_io_error_to_fs_error)?;

        let mut program_headers = Vec::with_capacity(header.e_phnum as usize);

        for i in 0..header.e_phnum {
            let mut buffer = [0u8; 56]; // Buffer for ProgramHeader (56 bytes for ELF64) - size_of::<ProgramHeader>()
            let expected_read_size = program_header_size; // Expected size to read

            // Check if we have enough bytes remaining in the file for this program header
            let current_pos = reader.stream_position().map_err(map_core_io_error_to_fs_error)?;
            let bytes_remaining = reader.file_size.checked_sub(current_pos) // Using file_size from SahneResourceReader
                 .ok_or_else(|| FileSystemError::IOError(format!("Dosya boyutu hesaplanırken hata")))?;

            if bytes_remaining < expected_read_size as u64 {
                 return Err(FileSystemError::InvalidData(format!("Beklenenden erken dosya sonu: Program başlığı {} okunurken (Kalan {} bayt, Gerekli {})", i, bytes_remaining, expected_read_size)));
            }

            // Read the program header bytes
            let bytes_read = reader.read(&mut buffer[..expected_read_size]).map_err(map_core_io_error_to_fs_error)?;
            if bytes_read != expected_read_size {
                 // This check might be redundant if read_exact was used, but good for Read trait.
                 return Err(FileSystemError::IOError(format!("Program başlığı {} için beklenen {} yerine {} bayt okundu", i, expected_read_size, bytes_read)));
            }


            // Safely parse fields from the buffer (assuming Little Endian)
            let p_type = u32::from_le_bytes(buffer[0..4].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_type baytları geçersiz")))?);
            let p_flags = u32::from_le_bytes(buffer[4..8].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_flags baytları geçersiz")))?);
            let p_offset = u64::from_le_bytes(buffer[8..16].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_offset baytları geçersiz")))?);
            let p_vaddr = u64::from_le_bytes(buffer[16..24].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_vaddr baytları geçersiz")))?);
            let p_paddr = u64::from_le_bytes(buffer[24..32].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_paddr baytları geçersiz")))?);
            let p_filesz = u64::from_le_bytes(buffer[32..40].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_filesz baytları geçersiz")))?);
            let p_memsz = u64::from_le_bytes(buffer[40..48].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_memsz baytları geçersiz")))?);
            let p_align = u64::from_le_bytes(buffer[48..56].try_into().map_err(|_| FileSystemError::InvalidData(format!("p_align baytları geçersiz")))?);


             // Optional: Verify p_phentsize matches the expected size for the ELF class
             if header.e_phentsize as usize != program_header_size {
                  eprintln!("WARN: e_phentsize ({}) does not match expected ProgramHeader size ({})", header.e_phentsize, program_header_size); // no_std print
                  // This could be an error depending on strictness
                  return Err(FileSystemError::InvalidData(format!("Program header entry size mismatch")));
             }
             // Optional: Verify header.e_phoff + i * header.e_phentsize is within file bounds


            program_headers.push(ProgramHeader {
                p_type,
                p_flags,
                p_offset,
                p_vaddr,
                p_paddr,
                p_filesz,
                p_memsz,
                p_align,
            });
        }

        Ok(program_headers)
    }

    /// Prints basic information about the ELF file.
    #[cfg(not(feature = "std"))] // Use no_std print
    pub fn print_info(&self) {
         // Use crate::println! which is available in no_std
         crate::println!("ELF Türü: {}", self.header.e_type);
         crate::println!("Makine: {}", self.header.e_machine);
         crate::println!("Giriş Noktası: 0x{:x}", self.header.e_entry);

         crate::println!("Program Başlıkları ({} adet):", self.program_headers.len());
         for header in &self.program_headers {
             crate::println!("  Tip: {}, Offset: 0x{:x}, Boyut (File/Mem): {} / {}", header.p_type, header.p_offset, header.p_filesz, header.p_memsz);
             // Add more details if needed
         }
    }

     /// Prints basic information about the ELF file (std version).
     #[cfg(feature = "std")] // Use std print
     pub fn print_info(&self) {
          Use std::println!
          std::println!("ELF Türü: {}", self.header.e_type);
          std::println!("Makine: {}", self.header.e_machine);
          std::println!("Giriş Noktası: 0x{:x}", self.header.e_entry);

          std::println!("Program Başlıkları ({} adet):", self.program_headers.len());
          for header in &self.program_headers {
              std::println!("  Tip: {}, Offset: 0x{:x}, Boyut (File/Mem): {} / {}", header.p_type, header.p_offset, header.p_filesz, header.p_memsz);
              // Add more details if needed
          }
     }

    // Add methods to read segments/sections based on program/section headers if needed.
    // This would involve using the underlying Handle and read_at/seek.
}

// TODO: Add a std implementation for ElfFile::open if needed, wrapping std::fs::File
 #[cfg(feature = "std")]
 impl ElfFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, FileSystemError> { ... }
    fn read_header<R: std::io::Read + std::io::Seek>(reader: &mut R) -> Result<ElfHeader, FileSystemError> { ... }
    fn read_program_headers<R: std::io::Read + std::io::Seek>(reader: &mut R, header: &ElfHeader) -> Result<Vec<ProgramHeader>, FileSystemError> { ... }
 }


// Example main function (no_std)
#[cfg(feature = "example_elf")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("ELF file example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy ELF file and simulate read_at/fstat syscalls.
     // This is complex and requires a testing framework or simulation.

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/my_elf" exists.
      let elf_file_res = ElfFile::open("sahne://files/my_elf");
      match elf_file_res {
          Ok(elf_file) => {
              elf_file.print_info();
     //         // Further processing of ELF sections/segments can be done here
              // e.g., loading segments into memory based on program headers.
          },
          Err(e) => eprintln!("Error opening or parsing ELF file: {}", e),
      }

     eprintln!("ELF file example (no_std) needs Sahne64 mocks to run.");

     Ok(()) // Dummy return
}

// Example main function (std) - Placeholder
#[cfg(feature = "example_elf")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> Result<(), Box<dyn std::error::Error>> { // Return Std Error for std example
     eprintln!("ELF file example (std) starting...");
     eprintln!("ELF file example (std) not fully implemented.");
     // A std example would likely open a real file and call ElfFile::open (if implemented for std)
     // Or call the no_std version with a mock Sahne64 layer over std::fs::File.
     Ok(()) // Dummy return
}


// Test module (requires a mock Sahne64 environment for no_std)
#[cfg(test)]
#[cfg(not(feature = "std"))] // Only compile tests for no_std
mod tests_no_std {
    use super::*;
    // Need a mock Sahne64 filesystem layer for testing

    // TODO: Implement tests for ElfFile using a mock Sahne64 environment.
    // This involves simulating resource acquisition/release, fs::read_at, fs::fstat.
    // Test cases should include valid ELF files (different architectures, endianness if supported),
    // invalid magic numbers, truncated files, incorrect header/program header sizes/offsets.
}

// Test module (for std implementation - if added)
#[cfg(test)]
#[cfg(feature = "std")] // Only compile tests for std
mod tests_std {
    // Need a std implementation of ElfFile or a mock for testing
    // This is complex and requires creating dummy ELF byte data in memory or temporary files.

    // TODO: Implement tests for ElfFile using std::fs::File and Cursor (for in-memory tests).
}

// This line indicates that this file is a library.
// If this is an executable file (e.g., a user-space application),
// you might need to define a `fn main() { ... }` function here.
// However, as this code is designed as a library, this line should remain.
#[cfg(not(any(feature = "std", feature = "example_elf", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
