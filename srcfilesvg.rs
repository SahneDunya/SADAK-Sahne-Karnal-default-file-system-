#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt};
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs::{self, O_RDONLY}, resource, SahneError, FileSystemError, Handle}; // fs, O_RDONLY, resource, SahneError, FileSystemError, Handle

// alloc crate for String, Vec, format!
use alloc::string::{String, ToString}; // Import ToString trait for to_string()
use alloc::vec::Vec;
use alloc::format;


// core::result, core::option, core::fmt, core::cmp, core::ops::Drop, core::io
use core::result::Result;
use core::option::Option;
use core::fmt;
use core::cmp; // For min
use core::ops::Drop; // For Drop trait
use core::io::{Read, Seek, SeekFrom, Error as CoreIOError, ErrorKind as CoreIOErrorKind, ReadExt as CoreReadExt}; // core::io


// Assuming a no_std compatible xml parsing crate like xml-rs is available
// This typically requires a feature flag or careful dependency management in no_std.
// We assume the necessary xml parsing types are available via a configured dependency.
// For xml-rs, these would be under `xml::reader`.
#[cfg(feature = "xml_parser")] // Assume a feature flag controls xml parser availability
mod xml_parser {
    // Re-export required types from the xml parser crate
    #[cfg(feature = "std")] // Use std xml-rs if std feature is enabled
    pub use xml::reader::{EventReader, XmlEvent, Result as XmlResult};
    #[cfg(all(not(feature = "std"), feature = "no_std_xml_parser"))] // Use no_std compatible xml parser if available
    pub use no_std_xml_parser::{EventReader, XmlEvent, Result as XmlResult}; // Replace `no_std_xml_parser` with the actual crate name

     // Assuming Error type is part of the XmlResult or has a known path
     #[cfg(feature = "std")]
     pub type XmlError = xml::reader::Error;
     #[cfg(all(not(feature = "std"), feature = "no_std_xml_parser"))]
     pub type XmlError = no_std_xml_parser::Error; // Replace with actual error type
}
#[cfg(feature = "xml_parser")]
use xml_parser::{EventReader, XmlEvent, XmlResult, XmlError};


// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden veya ortak modülünden import edildiği varsayılır


// Helper function to map SahneError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_sahne_error_to_fs_error(e: SahneError) -> FileSystemError {
    FileSystemError::IOError(format!("SahneError: {:?}", e)) // Using Debug format for SahneError
    // TODO: Implement a proper mapping based on SahneError variants
}

// Helper function to map std::io::Error to FileSystemError (copied from other files)
#[cfg(feature = "std")]
fn map_std_io_error_to_fs_error(e: StdIOError) -> FileSystemError {
    FileSystemError::IOError(format!("IO Error: {}", e))
}

// Helper function to map CoreIOError to FileSystemError (copied from other files)
#[cfg(not(feature = "std"))]
fn map_core_io_error_to_fs_error(e: CoreIOError) -> FileSystemError {
     FileSystemError::IOError(format!("CoreIOError: {:?}", e))
     // TODO: Implement a proper mapping based on CoreIOErrorKind
}

/// Helper function to map XmlError to FileSystemError.
#[cfg(feature = "xml_parser")]
fn map_xml_error_to_fs_error(e: XmlError) -> FileSystemError {
     #[cfg(feature = "std")]
     { // Use std Error::source() or similar if available for better mapping
         if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<StdIOError>()) {
              return map_std_io_error_to_fs_error(io_err.clone()); // Clone is needed if source returns reference
         }
     }

    FileSystemError::InvalidData(format!("XML parsing error: {:?}", e)) // Generic mapping
    // TODO: Implement a proper mapping based on XmlError variants if possible in no_std parser
}


/// Custom error type for SVG parsing issues.
#[derive(Debug)]
pub enum SvgError {
    XmlParsingError(String), // Errors from the underlying XML parser
    AttributeParsingError(String, String), // Error parsing attribute value (e.g., f64)
    MissingAttribute(String, String), // Required attribute is missing
    UnexpectedEof(String), // During reading
    SeekError(u64), // Failed to seek
    InvalidRtfSignature, // This should not happen in SVG parser, placeholder?
    InvalidUtf8, // Error converting bytes to UTF-8 string (if reading to string)
    // Add other SVG specific parsing errors here
}

// Implement Display for SvgError
impl fmt::Display for SvgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvgError::XmlParsingError(msg) => write!(f, "XML ayrıştırma hatası: {}", msg),
            SvgError::AttributeParsingError(attr, val) => write!(f, "Özellik değeri ayrıştırma hatası ({} = '{}')", attr, val),
            SvgError::MissingAttribute(element, attr) => write!(f, "Eksik özellik: Element '{}' için '{}' özelliği gerekli", element, attr),
            SvgError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
            SvgError::SeekError(pos) => write!(f, "Seek hatası pozisyon: {}", pos),
            SvgError::InvalidRtfSignature => write!(f, "Geçersiz RTF imzası (SVG dosyasında beklenmiyor)"), // Placeholder
            SvgError::InvalidUtf8 => write!(f, "Geçersiz UTF-8 verisi"),
        }
    }
}

// Helper function to map SvgError to FileSystemError
fn map_svg_error_to_fs_error(e: SvgError) -> FileSystemError {
    match e {
        SvgError::UnexpectedEof(_) | SvgError::SeekError(_) | SvgError::InvalidUtf8 => FileSystemError::IOError(format!("SVG IO hatası: {}", e)), // Map IO related errors
        _ => FileSystemError::InvalidData(format!("SVG ayrıştırma hatası: {}", e)), // Map parsing/validation errors
    }
}


// Sahne64 Handle'ı için core::io::Read ve Seek implementasyonu (srcfilestl.rs'den kopyalandı)
// Bu yapı, dosya pozisyonunu kullanıcı alanında takip eder ve fs::read_at ile okuma yapar.
// fstat ile dosya boyutını alarak seek(End) desteği sağlar.
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
impl core::io::Read for SahneResourceReader { // Use core::io::Read trait
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> { // Return core::io::Error
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
            .map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, format!("fs::read_at error: {:?}", e)))?; // Map SahneError to core::io::Error

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
    // read_exact has a default implementation in core::io::Read that uses read
    // read_to_end has a default implementation in core::io::ReadExt that uses read
    // read_to_string has a default implementation in core::io::ReadExt that uses read and from_utf8
}

#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReader { // Use core::io::Seek trait
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, core::io::Error> { // Return core::io::Error
        let file_size_isize = self.file_size as isize;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as isize,
            SeekFrom::End(offset) => {
                file_size_isize.checked_add(offset)
                    .ok_or_else(|| core::io::Error::new(core::io::ErrorKind::InvalidInput, format!("Seek position out of bounds (from end)")))?
            },
            SeekFrom::Current(offset) => {
                (self.position as isize).checked_add(offset)
                     .ok_or_else(|| core::io::Error::new(core::io::ErrorKind::InvalidInput, format!("Seek position out of bounds (from current)")))?
            },
        };

        if new_pos < 0 {
            return Err(core::io::Error::new(core::io::ErrorKind::InvalidInput, format!("Invalid seek position (result is negative)")));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
    // stream_position has a default implementation in core::io::Seek that uses seek(Current(0))
}

#[cfg(not(feature = "std"))]
impl Drop for SahneResourceReader {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReader is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReader drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant arch, SahneError, syscall, fs module definitions.
// Removed custom BufReaderSahne struct.
// Removed redundant print module and panic handler boilerplate.


/// Represents a parsed SVG file.
#[derive(Debug, PartialEq)] // Add PartialEq for tests
pub struct Svg {
    pub width: f64, // Requires floating-point support
    pub height: f64, // Requires floating-point support
    pub elements: Vec<SvgElement>, // Requires alloc
}

/// Represents a basic SVG element.
#[derive(Debug, PartialEq)] // Add PartialEq for tests
pub enum SvgElement {
    Rect {
        x: f64, y: f64, width: f64, height: f64, // Requires floating-point support
        fill: String, // Requires alloc
    },
    Circle {
        cx: f64, cy: f64, r: f64, // Requires floating-point support
        fill: String, // Requires alloc
    },
    // Add other supported SVG elements here
}


impl Svg {
    /// Parses SVG content from a reader.
    /// This function uses an XML event reader to process the SVG structure.
    /// Requires the 'xml_parser' feature flag to be enabled.
    ///
    /// # Arguments
    ///
    /// * `reader`: A mutable reference to the reader implementing Read.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed Svg data or a FileSystemError.
    #[cfg(feature = "xml_parser")] // Only compile if xml parser is available
    pub fn from_reader<R: Read>(reader: R) -> Result<Svg, FileSystemError> { // Return FileSystemError
        // Create an XML event reader from the provided reader
        let parser = EventReader::new(reader);
        let mut svg = Svg {
            width: 0.0,
            height: 0.0,
            elements: Vec::new(), // Requires alloc
        };

        let mut current_element_type: Option<xml_parser::XmlEvent> = None; // Keep track of current element type

        for event in parser {
            let event = event.map_err(|e| map_xml_error_to_fs_error(e))?; // Map XML parser errors to FileSystemError

            match event {
                XmlEvent::StartElement { name, attributes, .. } => {
                    match name.local_name.as_str() {
                        "svg" => {
                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                    "width" => {
                                         svg.width = attr.value.parse().map_err(|_| {
                                              map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("width"), attr.value)) // Requires alloc
                                         })?;
                                    },
                                    "height" => {
                                         svg.height = attr.value.parse().map_err(|_| {
                                              map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("height"), attr.value)) // Requires alloc
                                         })?;
                                    },
                                    _ => { /* Ignore other svg attributes */ }
                                }
                            }
                             current_element_type = Some(XmlEvent::StartElement { name: name, attributes: Vec::new(), namespace: xml_parser::xml::namespace::Namespace::empty() }); // Store element type
                        }
                        "rect" => {
                            let mut x = None;
                            let mut y = None;
                            let mut width = None;
                            let mut height = None;
                            let mut fill = None;

                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                    "x" => x = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("x"), attr.value)))?),
                                    "y" => y = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("y"), attr.value)))?),
                                    "width" => width = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("width"), attr.value)))?),
                                    "height" => height = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("height"), attr.value)))?),
                                    "fill" => fill = Some(attr.value),
                                    _ => { /* Ignore other rect attributes */ }
                                }
                            }

                            // Check for required attributes and create the element
                            if let (Some(x), Some(y), Some(width), Some(height), Some(fill)) = (x, y, width, height, fill) {
                                let rect_element = SvgElement::Rect { x: x?, y: y?, width: width?, height: height?, fill }; // Use ? to propagate parsing errors
                                svg.elements.push(rect_element);
                                current_element_type = Some(XmlEvent::StartElement { name: name, attributes: Vec::new(), namespace: xml_parser::xml::namespace::Namespace::empty() }); // Store element type
                            } else {
                                // Report missing attributes if necessary. This is basic validation.
                                if x.is_none() { eprintln!("WARN: rect missing x attribute"); }
                                if y.is_none() { eprintln!("WARN: rect missing y attribute"); }
                                if width.is_none() { eprintln!("WARN: rect missing width attribute"); }
                                if height.is_none() { eprintln!("WARN: rect missing height attribute"); }
                                if fill.is_none() { eprintln!("WARN: rect missing fill attribute"); }
                                // Decide if missing attributes should be a hard error
                                // return Err(map_svg_error_to_fs_error(SvgError::MissingAttribute(String::from("rect"), ...)));
                            }
                        }
                         "circle" => {
                            let mut cx = None;
                            let mut cy = None;
                            let mut r = None;
                            let mut fill = None;

                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                     "cx" => cx = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("cx"), attr.value)))?),
                                     "cy" => cy = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("cy"), attr.value)))?),
                                     "r" => r = Some(attr.value.parse::<f64>().map_err(|_| map_svg_error_to_fs_error(SvgError::AttributeParsingError(String::from("r"), attr.value)))?),
                                    "fill" => fill = Some(attr.value),
                                    _ => { /* Ignore other circle attributes */ }
                                }
                            }

                            // Check for required attributes and create the element
                            if let (Some(cx), Some(cy), Some(r), Some(fill)) = (cx, cy, r, fill) {
                                let circle_element = SvgElement::Circle { cx: cx?, cy: cy?, r: r?, fill }; // Use ? to propagate parsing errors
                                svg.elements.push(circle_element);
                                current_element_type = Some(XmlEvent::StartElement { name: name, attributes: Vec::new(), namespace: xml_parser::xml::namespace::Namespace::empty() }); // Store element type
                            } else {
                                 // Report missing attributes if necessary.
                                 if cx.is_none() { eprintln!("WARN: circle missing cx attribute"); }
                                 if cy.is_none() { eprintln!("WARN: circle missing cy attribute"); }
                                 if r.is_none() { eprintln!("WARN: circle missing r attribute"); }
                                 if fill.is_none() { eprintln!("WARN: circle missing fill attribute"); }
                                 // Decide if missing attributes should be a hard error
                                 // return Err(map_svg_error_to_fs_error(SvgError::MissingAttribute(String::from("circle"), ...)));
                             }
                         }
                        _ => { /* Ignore other elements */ }
                    }
                }
                XmlEvent::EndElement { name } => {
                     // Clear current element type when its end tag is encountered
                    if let Some(start_event) = &current_element_type {
                        if let XmlEvent::StartElement { name: start_name, .. } = start_event {
                            if start_name.local_name == name.local_name {
                                current_element_type = None; // Matched end tag
                            }
                        }
                    }
                     // Note: For a robust parser, matching start/end tags and handling nesting is important.
                     // This basic parser just collects top-level rect/circle elements.
                }
                // Ignore other XML events like characters, comments, processing instructions etc.
                _ => {}
            }
        }

        Ok(svg) // Return the parsed Svg struct
    }
}


/// Opens an SVG file from the given path (std) or resource ID (no_std)
/// and parses its content.
/// Requires the 'xml_parser' feature flag to be enabled.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing the parsed Svg data or a FileSystemError.
#[cfg(feature = "xml_parser")] // Only compile if xml parser is available
#[cfg(feature = "std")]
pub fn open_svg_file<P: AsRef<Path>>(file_path: P) -> Result<Svg, FileSystemError> { // Return FileSystemError
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    let reader = BufReader::new(file); // BufReader implements StdRead + StdSeek

    // Get file size (optional for from_reader, but good practice)
    // Seek to end to get size, then seek back to start
     let mut temp_file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
     let file_size = temp_file.seek(SeekFrom::End(0)).map_err(map_std_io_error_to_fs_error)?;
    // No need to seek temp_file back, it will be dropped.


    // Parse the SVG data directly from the reader using the XML parser
    Svg::from_reader(reader) // Call the generic from_reader function
}

#[cfg(feature = "xml_parser")] // Only compile if xml parser is available
#[cfg(not(feature = "std"))]
pub fn open_svg_file(file_path: &str) -> Result<Svg, FileSystemError> { // Return FileSystemError
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutunu al (needed for SahneResourceReader and potential validation)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;

    // SahneResourceReader oluştur
    let reader = SahneResourceReader::new(handle, file_size); // Implements core::io::Read + Seek


    // Parse the SVG data directly from the reader using the XML parser
    Svg::from_reader(reader) // Call the generic from_reader function

    // File handle is released when 'reader' goes out of scope (due to Drop on SahneResourceReader).
}


// Example main function (std)
#[cfg(feature = "example_svg")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
#[cfg(feature = "xml_parser")] // Only compile if xml parser is available
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for std example
     eprintln!("SVG parser example (std) starting...");
     eprintln!("SVG parser example (std) using xml parser.");

     // Example SVG file content
     let svg_content = r#"
         <svg width="200" height="100">
             <rect x="10" y="10" width="30" height="30" fill="red" />
             <circle cx="60" cy="60" r="20" fill="blue" />
              <path d="M150 0 L75 200 L225 200 Z" /> <!-- Ignored by this parser -->
         </svg>
     "#;


     let file_path = Path::new("example.svg");

      // Write example content to a temporary file for std test
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(svg_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy SVG file: {}", e);
                       return Err(e); // Return error if file creation/write fails
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy SVG file: {}", e);
                  return Err(e); // Return error if file creation fails
             }
        }


     match open_svg_file(file_path) { // Call the function that opens and parses
         Ok(svg) => {
             println!("Parsed SVG Data:");
             println!(" SVG Width: {}", svg.width);
             println!(" SVG Height: {}", svg.height);
             println!(" Elements:");
             for element in svg.elements {
                 match element {
                     SvgElement::Rect { x, y, width, height, fill } => {
                         println!("  Rect: x={}, y={}, width={}, height={}, fill={}", x, y, width, height, fill);
                          // Assert rect properties
                          assert_eq!(x, 10.0);
                          assert_eq!(y, 10.0);
                          assert_eq!(width, 30.0);
                          assert_eq!(height, 30.0);
                          assert_eq!(fill, "red");
                     }
                     SvgElement::Circle { cx, cy, r, fill } => {
                         println!("  Circle: cx={}, cy={}, r={}, fill={}", cx, cy, r, fill);
                          // Assert circle properties
                          assert_eq!(cx, 60.0);
                          assert_eq!(cy, 60.0);
                          assert_eq!(r, 20.0);
                          assert_eq!(fill, "blue");
                     }
                 }
             }
             // Assert the total number of elements parsed
             assert_eq!(svg.elements.len(), 2);


             // File is automatically closed when the underlying reader/handle goes out of scope (due to Drop)
         }
         Err(e) => {
              eprintln!("Error opening/parsing SVG file: {}", e); // std error display
              return Err(e);
         }
     }

     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy SVG file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("SVG parser example (std) finished.");

     Ok(())
}

// Example main function (no_std)
#[cfg(feature = "example_svg")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
#[cfg(feature = "xml_parser")] // Only compile if xml parser is available
fn main() -> Result<(), FileSystemError> { // Return FileSystemError
     eprintln!("SVG parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // for a dummy SVG file and simulate resource/fs syscalls.
     // This is complex and requires a testing framework or simulation.

     // Create dummy SVG content bytes for the mock filesystem
     let svg_content = r#"
         <svg width="150" height="75">
             <rect x="5" y="5" width="20" height="20" fill="green" />
         </svg>
     "#;
     let dummy_svg_data: Vec<u8> = svg_content.as_bytes().to_vec(); // Requires alloc


      // Assuming the mock filesystem is set up to provide this data for "sahne://files/shape.svg"

     // Hypothetical usage:
     // // Assume a mock filesystem is set up and "sahne://files/shape.svg" exists with the dummy data.
     // let svg_res = open_svg_file("sahne://files/shape.svg");
     // match svg_res {
     //     Ok(svg) => {
     //         crate::println!("Parsed SVG Data:");
     //         crate::println!(" SVG Width: {}", svg.width);
     //         crate::println!(" SVG Height: {}", svg.height);
     //         crate::println!(" Elements:");
     //         for element in svg.elements {
     //              match element {
     //                  SvgElement::Rect { x, y, width, height, fill } => {
     //                      crate::println!("  Rect: x={}, y={}, width={}, height={}, fill={}", x, y, width, height, fill);
     //                  }
     //                  SvgElement::Circle { cx, cy, r, fill } => {
     //                       crate::println!("  Circle: cx={}, cy={}, r={}, fill={}", cx, cy, r, fill);
     //                  }
     //              }
     //         }
     //         // Assert the total number of elements parsed
     //         // assert_eq!(svg.elements.len(), 1);
     //
     //         // File is automatically closed when the underlying reader/handle goes out of scope (due to Drop)
     //     },
     //     Err(e) => crate::eprintln!("Error opening/parsing SVG file: {:?}", e),
     // }


     eprintln!("SVG parser example (no_std) needs Sahne64 mocks and xml parser to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real).
     // 2. The dummy SVG data to be available at the specified path.
     // 3. A no_std compatible xml parser crate configured.

     Ok(()) // Dummy return
}


// Test module (requires a mock Sahne64 environment or dummy files for testing)
#[cfg(test)]
#[cfg(feature = "std")] // Only run tests with std feature enabled
#[cfg(feature = "xml_parser")] // Only run tests if xml parser is available
mod tests {
     // Needs std::io::Cursor for testing Read+Seek on dummy data
     use std::io::Cursor;
     use std::io::{Read, Seek, SeekFrom};
     use std::fs::remove_file; // For cleanup
     use std::path::Path;
     use std::io::Write; // For creating dummy files


     use super::*; // Import items from the parent module
     use alloc::string::String; // For String
     use alloc::vec::Vec; // For Vec
     use alloc::string::ToString as AllocToString; // to_string() for string conversion in tests
     use alloc::boxed::Box; // For Box<dyn Error> in std tests
     use std::error::Error; // For Box<dyn Error> source()


     // Helper function to create dummy SVG bytes
      #[cfg(feature = "std")] // Uses std::io::Cursor
       fn create_dummy_svg_bytes(content: &str) -> Result<Vec<u8>, Box<dyn Error>> {
           Ok(content.as_bytes().to_vec())
       }


     // Test parsing a valid SVG string using from_reader with Cursor
      #[test]
      fn test_from_reader_valid_svg_cursor() -> Result<(), FileSystemError> { // Return FileSystemError
           let svg_content = r#"
               <svg width="100" height="50">
                   <rect x="5" y="5" width="20" height="20" fill="green" />
                   <circle cx="80" cy="25" r="15" fill="yellow" />
               </svg>
           "#;

           let dummy_svg_bytes = create_dummy_svg_bytes(svg_content).map_err(|e| FileSystemError::IOError(format!("Test data creation error: {}", e)))?;


           // Use Cursor as a Read + Seek reader
           let cursor = Cursor::new(dummy_svg_bytes.clone());

           // Parse the SVG data from the reader
           let svg = Svg::from_reader(cursor)?;

           // Assert parsed SVG data
           assert_eq!(svg.width, 100.0);
           assert_eq!(svg.height, 50.0);
           assert_eq!(svg.elements.len(), 2);

           // Assert elements
           if let Some(SvgElement::Rect { x, y, width, height, fill }) = svg.elements.get(0) {
               assert_eq!(*x, 5.0);
               assert_eq!(*y, 5.0);
               assert_eq!(*width, 20.0);
               assert_eq!(*height, 20.0);
               assert_eq!(fill, "green");
           } else {
                panic!("First element is not a Rect or missing");
           }

            if let Some(SvgElement::Circle { cx, cy, r, fill }) = svg.elements.get(1) {
                assert_eq!(*cx, 80.0);
                assert_eq!(*cy, 25.0);
                assert_eq!(*r, 15.0);
                assert_eq!(fill, "yellow");
            } else {
                 panic!("Second element is not a Circle or missing");
            }


           Ok(())
      }

     // Test handling of invalid XML/SVG data
      #[test]
      fn test_from_reader_invalid_xml_cursor() {
           let invalid_svg_content = r#"
               <svg width="100" height="50">
                   <rect x="10" y="10" width="30" height="30" fill="red" />
               </svg This is invalid XML"
           "#; // Missing closing angle bracket on svg tag

           let dummy_svg_bytes = create_dummy_svg_bytes(invalid_svg_content).unwrap();


           let cursor = Cursor::new(dummy_svg_bytes);
           // Attempt to parse from the reader, expect an error
           let result = Svg::from_reader(cursor);

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from SvgError::XmlParsingError (via map_xml_error_to_fs_error)
                   assert!(msg.contains("XML ayrıştırma hatası"));
                    // Check if the underlying XML error message is included
                    #[cfg(feature = "std")] // std xml-rs error message check
                    assert!(msg.contains("unexpected token"));
               },
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

      // Test handling of attribute parsing error (e.g., non-numeric value for width)
       #[test]
       fn test_from_reader_attribute_parsing_error_cursor() {
            let svg_content_invalid_attr = r#"
                <svg width="abc" height="50"> <!-- Invalid width value -->
                    <rect x="10" y="10" width="30" height="30" fill="red" />
                </svg>
            "#;

            let dummy_svg_bytes = create_dummy_svg_bytes(svg_content_invalid_attr).unwrap();

            let cursor = Cursor::new(dummy_svg_bytes);
            // Attempt to parse from the reader, expect an error
            let result = Svg::from_reader(cursor);

            assert!(result.is_err());
            match result.unwrap_err() {
                FileSystemError::InvalidData(msg) => { // Mapped from SvgError::AttributeParsingError
                    assert!(msg.contains("Özellik değeri ayrıştırma hatası"));
                     assert!(msg.contains("width = 'abc'"));
                },
                _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
            }
       }


     // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
     // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
     // Test cases should include opening valid/invalid files, handling IO errors during reading,
     // and correctly parsing SVG data from mock data using the no_std xml parser.
}


// Redundant print module and panic handler are removed.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_svg", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
