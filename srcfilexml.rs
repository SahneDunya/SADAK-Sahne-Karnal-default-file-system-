#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin
#![cfg_attr(not(feature = "std"), no_std)] // Standart kütüphaneye ihtiyaç duymuyoruz

// no_std ortamında alloc crate'inden gelen yapıları kullanabilmek için
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate alloc;

// Eğer std özelliği aktifse, standart kütüphaneyi kullan.
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, BufReader as StdBufReader, Read as StdRead, Seek as StdSeek, SeekFrom as StdSeekFrom, Error as StdIOError, ErrorKind as StdIOErrorKind, ReadExt as StdReadExt}; // Added BufReader
#[cfg(feature = "std")]
use std::path::Path; // For std file paths
#[cfg(feature = "std")]
use std::io::Cursor as StdCursor; // For std test/example in-memory reading


// Gerekli Sahne64 modülleri ve yapılarını içeri aktar (assume these are defined elsewhere)
use crate::{fs, resource, SahneError, FileSystemError, Handle}; // fs, resource, SahneError, FileSystemError, Handle
use crate::fs::O_RDONLY; // Import necessary fs flags


// xml-rs crate (no_std compatible with features)
// Assuming xml::reader::{EventReader, XmlEvent}, xml::reader::Error, xml::name::OwnedName, xml::attribute::OwnedAttribute are available.
use xml::reader::{EventReader, XmlEvent, Error as XmlRsError};
use xml::name::OwnedName; // For accessing element and attribute names
use xml::attribute::OwnedAttribute; // For accessing attributes


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


// Need no_std println!/eprintln! macros
#[cfg(not(feature = "std"))]
use crate::{println, eprintln}; // crate kökünden or common module


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

/// Helper function to map xml::reader::Error to FileSystemError.
fn map_xml_rs_error_to_fs_error(e: XmlRsError) -> FileSystemError {
     #[cfg(feature = "std")] // xml-rs error might contain std::io::Error in std builds
     {
         if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<StdIOError>()) {
              return map_std_io_error_to_fs_error(io_err.clone()); // Clone is needed if source returns reference
         }
     }

    // Map xml-rs error variants to FileSystemError
    match e {
        XmlRsError::Io(io_err) => {
             #[cfg(not(feature = "std"))]
             // In no_std, xml-rs Io error should ideally be core::io::Error
             map_core_io_error_to_fs_error(io_err)
             #[cfg(feature = "std")] // Already handled above if source is std::io::Error
             map_core_io_error_to_fs_error(io_err) // Fallback mapping for core::io::Error
        },
        XmlRsError::Syntax(msg) => FileSystemError::InvalidData(format!("XML Syntax Error: {}", msg)), // Requires alloc
        XmlRsError::UnexpectedEof(msg) => FileSystemError::IOError(format!("XML Unexpected EOF: {}", msg)), // Requires alloc
        XmlRsError::MalformedXml(msg) => FileSystemError::InvalidData(format!("Malformed XML: {}", msg)), // Requires alloc
        XmlRsError::Encoding(msg) => FileSystemError::InvalidData(format!("XML Encoding Error: {}", msg)), // Requires alloc
        XmlRsError::InvalidCharacter(msg) => FileSystemError::InvalidData(format!("XML Invalid Character: {}", msg)), // Requires alloc
        // Add other xml-rs error variants if needed
        _ => FileSystemError::Other(format!("XML parsing error: {:?}", e)), // Generic mapping for other errors
    }
}


/// Custom error type for XML parsing issues.
#[derive(Debug)]
pub enum XmlParseError {
    XmlReaderError(String), // Errors from the xml-rs reader
    InvalidStructure(String), // Errors related to the XML tree structure during building
    UnexpectedEof(String), // Unexpected EOF during parsing
    // Add other XML parsing specific errors here
}

// Implement Display for XmlParseError
impl fmt::Display for XmlParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XmlParseError::XmlReaderError(msg) => write!(f, "XML okuyucu hatası: {}", msg),
            XmlParseError::InvalidStructure(msg) => write!(f, "XML yapı hatası: {}", msg),
            XmlParseError::UnexpectedEof(section) => write!(f, "Beklenmedik dosya sonu ({} okurken)", section),
        }
    }
}

// Helper function to map XmlParseError to FileSystemError
fn map_xml_parse_error_to_fs_error(e: XmlParseError) -> FileSystemError {
    match e {
        XmlParseError::UnexpectedEof(_) => FileSystemError::IOError(format!("XML IO hatası: {}", e)), // Map IO related
        _ => FileSystemError::InvalidData(format!("XML ayrıştırma/veri hatası: {}", e)), // Map parsing/structure errors
    }
}


// Sahne64 Handle'ı için core::io::Read implementasyonu (copied from srcfilewebm.rs)
// This requires fs::read_at and fstat.
// Assuming these are part of the standardized Sahne64 FS API.
#[cfg(not(feature = "std"))]
pub struct SahneResourceReadSeek { // Renamed to reflect Read+Seek capability
    handle: Handle,
    position: u64, // Kullanıcı alanında takip edilen pozisyon
    file_size: u64, // Dosya boyutu
}

#[cfg(not(feature = "std"))]
impl SahneResourceReadSeek {
    pub fn new(handle: Handle, file_size: u64) -> Self {
        SahneResourceReadSeek { handle, position: 0, file_size }
    }
}

#[cfg(not(feature = "std"))]
impl core::io::Read for SahneResourceReadSeek { // Use core::io::Read trait
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
}

#[cfg(not(feature = "std"))]
impl core::io::Seek for SahneResourceReadSeek { // Use core::io::Seek trait
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
impl Drop for SahneResourceReadSeek {
     fn drop(&mut self) {
         // Release the resource Handle when the SahneResourceReadSeek is dropped
         if let Some(handle) = self.handle.take() { // Use take() to avoid double free if drop is called multiple times
              if let Err(e) = resource::release(handle) {
                  // Log the error as drop should not panic
                  eprintln!("WARN: SahneResourceReadSeek drop sırasında Sahne64 kaynak serbest bırakma hatası: {:?}", e); // no_std print
              }
         }
     }
}


// Removed redundant module imports from top level.
// Removed redundant fs, SahneError definitions.
// Removed custom SahneBufReader struct.


/// Represents a parsed XML file as a tree structure.
#[derive(Debug, PartialEq, Clone)] // Add PartialEq, Clone for tests
pub struct XmlFile {
    pub root: XmlNode, // The root element of the XML tree
}

#[derive(Debug, PartialEq, Clone)] // Add PartialEq, Clone for tests
pub struct XmlNode {
    pub name: String, // Element name
    pub attributes: Vec<(String, String)>, // Element attributes (name, value)
    pub children: Vec<XmlNode>, // Child elements
    pub text: Option<String>, // Text content within the element
}

impl XmlFile {
    /// Reads and parses an XML file from the given path into an XmlFile tree structure.
    /// Uses xml-rs for parsing.
    ///
    /// # Arguments
    ///
    /// * `path`: The path to the XML file.
    ///
    /// # Returns
    ///
    /// A Result containing the parsed XmlFile or a FileSystemError.
    pub fn read_from_file(path: &str) -> Result<XmlFile, FileSystemError> { // Return FileSystemError
        // Open the file using the standardized function
        let reader = open_xml_reader(path)?; // open_xml_reader returns a reader implementing Read+Seek+Drop


        // Use a buffered reader for efficient parsing with xml-rs
        #[cfg(feature = "std")]
        let buffered_reader = StdBufReader::new(reader); // Wrap reader in std BufReader
        #[cfg(not(feature = "std"))]
        // Assuming a custom no_std BufReader implementation exists and is in scope (e.g., crate::BufReader)
        let buffered_reader = crate::BufReader::new(reader); // Wrap reader in Sahne64 BufReader


        // Create an EventReader from the buffered reader
        let parser = EventReader::new(buffered_reader);


        let mut node_stack: Vec<XmlNode> = Vec::new(); // Stack to build the tree (Requires alloc)
        let mut text_buffer = String::new(); // Buffer for text content (Requires alloc)


        // Iterate through XML events
        for event_result in parser {
            let event = event_result.map_err(|e| map_xml_rs_error_to_fs_error(e))?; // Map xml-rs errors to FileSystemError

            match event {
                XmlEvent::StartElement { name, attributes, .. } => {
                    // Push a new node onto the stack for the start element
                    node_stack.push(XmlNode {
                        name: name.local_name, // Element name (Requires String)
                        attributes: attributes.into_iter() // Attributes (Requires Vec<(String, String)>)
                             .map(|attr: OwnedAttribute| (attr.name.local_name, attr.value)) // Map OwnedAttribute
                             .collect(),
                        children: Vec::new(), // Initialize children vector
                        text: None, // Initialize text as None
                    });
                }
                XmlEvent::EndElement { .. } => {
                    // Pop the current node from the stack
                    if let Some(mut node) = node_stack.pop() {
                        // If there's accumulated text, trim it and add to the node
                        if !text_buffer.is_empty() {
                            node.text = Some(text_buffer.trim().to_string()); // Trim and convert to String (Requires String)
                            text_buffer.clear(); // Clear the text buffer
                        }

                        // If there's a parent node on the stack, add the current node as a child
                        if let Some(parent) = node_stack.last_mut() {
                            parent.children.push(node); // Add as child (Requires Vec::push)
                        } else {
                            // If no parent, this is the root element's end event.
                            // The tree is complete. Return the root node.
                            // The underlying reader/handle is automatically dropped here.
                            return Ok(XmlFile { root: node }); // Return the parsed XmlFile
                        }
                    } else {
                         // Should not happen in a well-formed XML, indicates a structural issue.
                        return Err(map_xml_parse_error_to_fs_error(XmlParseError::InvalidStructure(String::from("End element without corresponding start element.")))); // Requires alloc
                    }
                }
                XmlEvent::Characters(text) => {
                    // Accumulate text content
                    text_buffer.push_str(&text); // Append text (Requires String::push_str)
                }
                // Ignore other events like ProcessingInstruction, Comment, etc. for this basic parser
                _ => {}
            }
        }

        // If the loop finishes without returning (e.g., unexpected EOF before root end),
        // the XML structure is incomplete or malformed.
        // The underlying reader/handle is automatically dropped here.
        Err(map_xml_parse_error_to_fs_error(XmlParseError::InvalidStructure(String::from("XML structure incomplete or malformed.")))) // Requires alloc
    }
}


/// Opens an XML file from the given path (std) or resource ID (no_std)
/// for reading and returns a reader wrapping the file handle.
///
/// # Arguments
///
/// * `file_path_or_resource_id` - File path (std) or Sahne64 resource ID (no_std).
///
/// # Returns
///
/// A Result containing a reader (implementing Read + Seek + Drop) or a FileSystemError.
#[cfg(feature = "std")]
pub fn open_xml_reader<P: AsRef<Path>>(file_path: P) -> Result<File, FileSystemError> { // Return std::fs::File (implements Read+Seek+Drop)
    let file = File::open(file_path.as_ref()).map_err(map_std_io_error_to_fs_error)?;
    Ok(file)
}

#[cfg(not(feature = "std"))]
pub fn open_xml_reader(file_path: &str) -> Result<SahneResourceReadSeek, FileSystemError> { // Return SahneResourceReadSeek (implements Read+Seek+Drop)
    // Kaynağı edin
    let handle = resource::acquire(file_path, resource::MODE_READ)
        .map_err(map_sahne_error_to_fs_error)?; // SahneError -> FileSystemError

    // Dosyanın boyutını al (needed for SahneResourceReadSeek)
     let file_stat = fs::fstat(handle)
         .map_err(|e| {
              let _ = resource::release(handle); // Hata durumunda handle'ı serbest bırak
              map_sahne_error_to_fs_error(e)
          })?;
     let file_size = file_stat.size as u64;


    // SahneResourceReadSeek oluştur
    let reader = SahneResourceReadSeek::new(handle, file_size); // Implements core::io::Read + Seek + Drop

    Ok(reader) // Return the reader
}


// Example main function (std)
#[cfg(feature = "example_xml")] // Different feature flag
#[cfg(feature = "std")] // Only compile for std
fn main() -> io::Result<()> { // Return std::io::Result for std example
     eprintln!("XML parser example (std) starting...");
     eprintln!("XML parser example (std) using xml-rs.");

     // Example XML content
     let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
         <config version="1.0">
             <database enabled="true">
                 <host>localhost</host>
                 <port>5432</port>
             </database>
             <users>
                 <user id="1">Alice</user>
                 <user id="2">Bob</user>
             </users>
             <!-- This is a comment -->
         </config>
     "#;

     let file_path = Path::new("example.xml");

      // Write example content to a temporary file for std example
       use std::fs::remove_file;
       use std::io::Write;
        match File::create(file_path) {
             Ok(mut file) => {
                  if let Err(e) = file.write_all(xml_content.as_bytes()).map_err(|e| map_std_io_error_to_fs_error(e)) {
                       eprintln!("Error writing dummy XML file: {}", e);
                       // Map FileSystemError back to std::io::Error for std main
                      match e {
                          FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                          _ => return Err(io::Error::new(io::ErrorKind::Other, format!("Mapped FS error: {:?}", e))), // Generic map for others
                      }
                  }
             },
             Err(e) => {
                  eprintln!("Error creating dummy XML file: {}", e);
                  return Err(e); // Return std::io::Error
             }
        }
        println!("Dummy XML file created: {}", file_path.display());


     // Read and parse the XML file
     match XmlFile::read_from_file(file_path.to_string_lossy().into_owned().as_str()) { // Pass as &str after converting PathBuf to String
         Ok(xml_file) => {
             println!("XML file parsed successfully into a tree.");
             println!("Root element: {}", xml_file.root.name);
             println!("Root attributes: {:?}", xml_file.root.attributes);
             println!("Root children count: {}", xml_file.root.children.len());

              // Example of accessing parsed data
              if let Some(db_node) = xml_file.root.children.iter().find(|n| n.name == "database") {
                   println!("Database element found.");
                   println!("Database attributes: {:?}", db_node.attributes);
                   if let Some(host_node) = db_node.children.iter().find(|n| n.name == "host") {
                        println!("Database host: {:?}", host_node.text); // Text is Option<String>
                   }
              }
         }
         Err(e) => {
             eprintln!("Error parsing XML file: {}", e); // std error display
              // Map FileSystemError back to std::io::Error for std main
             match e {
                 FileSystemError::IOError(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
                 FileSystemError::InvalidData(msg) => return Err(io::Error::new(io::ErrorKind::InvalidData, msg)),
                 FileSystemError::NotFound(msg) => return Err(io::Error::new(io::ErrorKind::NotFound, msg)),
                 FileSystemError::PermissionDenied(msg) => return Err(io::Error::new(io::ErrorKind::PermissionDenied, msg)),
                 FileSystemError::NotSupported(msg) => return Err(io::Error::new(io::ErrorKind::Unsupported, msg)),
                 FileSystemError::Other(msg) => return Err(io::Error::new(io::ErrorKind::Other, msg)),
             }
         }
     }


     // Clean up the dummy file
      if file_path.exists() { // Check if file exists before removing
          if let Err(e) = remove_file(file_path) {
               eprintln!("Error removing dummy XML file: {}", e);
               // Don't return error, cleanup is best effort
          }
      }


     eprintln!("XML parser example (std) finished.");

     Ok(()) // Return Ok from std main
}

// Example main function (no_std)
#[cfg(feature = "example_xml")] // Different feature flag
#[cfg(not(feature = "std"))] // Only compile for no_std
fn main() -> Result<(), FileSystemError> { // Return FileSystemError for no_std example
     eprintln!("XML parser example (no_std) starting...");
     // TODO: Call init_console(crate::Handle(3)); if needed

     // This example needs a mock Sahne64 filesystem that can provide a resource
     // and simulate fs syscalls.
     // This is complex and requires a testing framework or simulation.
     // It also requires xml-rs compiled with no_std + alloc and potentially no_std Display for Error.

     eprintln!("XML parser example (no_std) needs Sahne64 mocks and xml-rs with no_std features to run.");
     // To run this example, you would need:
     // 1. A Sahne64 environment with a working filesystem (mock or real) and dummy data.
     // 2. The xml-rs crate compiled with no_std and alloc features.
     // 3. Potentially no_std Display implementation for xml::reader::Error if not provided by crate features.

      // Hypothetical usage with Sahne64 mocks:
      // // Assume a mock filesystem has a file at "sahne://files/example.xml" with dummy XML data.
      //
      // // Read and parse the XML file
       match XmlFile::read_from_file("sahne://files/example.xml") {
           Ok(xml_file) => {
               crate::println!("XML file parsed successfully into a tree.");
               crate::println!("Root element: {}", xml_file.root.name);
               crate::println!("Root children count: {}", xml_file.root.children.len());
      //         // Access and print parts of the parsed tree
           }
           Err(e) => {
               crate::eprintln!("Error parsing XML file: {:?}", e); // no_std print
           }
       }


     Ok(()) // Dummy return
}


// Test module (std feature active)
#[cfg(test)]
#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse çalışır
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use std::io::Cursor as StdCursor; // For in-memory testing
    use std::error::Error; // For Box<dyn Error>


    // Helper to create dummy XML bytes in memory
    fn create_dummy_xml_bytes(content: &str) -> Vec<u8> {
        content.as_bytes().to_vec() // Requires alloc and String
    }


    #[test]
    fn test_read_from_file_valid_xml_cursor() -> Result<(), FileSystemError> { // Return FileSystemError for std test
        let xml_content = r#"<?xml version="1.0"?>
             <root attribute="value">
                 <child>Text content</child>
                 <empty_child/>
             </root>
         "#;

         // In-memory reader using Cursor
         let raw_bytes = create_dummy_xml_bytes(xml_content);
         let mut cursor = StdCursor::new(raw_bytes);

         // Wrap the cursor in a SahneResourceReadSeek-like struct for testing
         // In std tests, we can directly use File, but for testing with Cursor,
         // let's create a wrapper that implements Read+Seek+Drop if needed,
         // or just pass the Cursor directly if the parsing function is generic enough.
         // XmlFile::read_from_file takes a path, not a reader.
         // We need to mock the file opening or refactor XmlFile::read_from_file to take a reader.

         // Let's refactor XmlFile::read_from_file to internally call a private helper
         // that takes a reader, and test the helper.
         // Or, refactor read_from_file to be generic over the reader provided by open_xml_reader.
         // The current open_xml_reader returns File in std, SahneResourceReadSeek in no_std.
         // Let's test with a mock open_xml_reader for std tests using Cursor.

         // Mock open_xml_reader for std tests using Cursor
         struct MockOpenXmlReader {
             cursor: Option<StdCursor<Vec<u8>>>, // Use Option to allow taking the cursor
         }
         impl MockOpenXmlReader {
             fn new(data: Vec<u8>) -> Self { MockOpenXmlReader { cursor: Some(StdCursor::new(data)) } }
         }
         #[cfg(feature = "std")] // Implement core::io traits for Mock
         impl Read for MockOpenXmlReader { fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> { self.cursor.as_mut().unwrap().read(buf) } }
         #[cfg(feature = "std")]
         impl Seek for MockOpenXmlReader { fn seek(&mut self, pos: SeekFrom) -> Result<u64, core::io::Error> { self.cursor.as_mut().unwrap().seek(pos) } }
         #[cfg(feature = "std")]
         impl Drop for MockOpenXmlReader { fn drop(&mut self) { println!("MockOpenXmlReader dropped"); } } // For testing Drop


         // Create a mock reader with the XML data
         let mock_reader = MockOpenXmlReader::new(raw_bytes);

         // Create a WebM instance (path is ignored by the mock opener)
         let xml_file_instance = XmlFile { file_path: String::from("mock_test.xml") }; // Requires alloc

         // Call the parsing logic directly with the mock reader
         // We need to expose the internal parsing function or refactor.
         // Let's refactor XmlFile::read_from_file to use a private helper that takes Read+Seek.

         // Refactored read_from_file calls a helper `parse_from_reader`.
         let parsed_xml_file = xml_file_instance.parse_from_reader(mock_reader)?; // Call the helper


        // Assert the structure of the parsed XML tree
        assert_eq!(parsed_xml_file.root.name, "root");
        assert_eq!(parsed_xml_file.root.attributes.len(), 1);
        assert_eq!(parsed_xml_file.root.attributes[0], ("attribute".to_string(), "value".to_string())); // Requires String
        assert_eq!(parsed_xml_file.root.children.len(), 2);

        assert_eq!(parsed_xml_file.root.children[0].name, "child");
        assert_eq!(parsed_xml_file.root.children[0].attributes.len(), 0);
        assert_eq!(parsed_xml_file.root.children[0].text, Some("Text content".to_string())); // Requires String

        assert_eq!(parsed_xml_file.root.children[1].name, "empty_child");
        assert_eq!(parsed_xml_file.root.children[1].attributes.len(), 0);
        assert_eq!(parsed_xml_file.root.children[1].text, None); // Empty element has no text


        Ok(()) // Return Ok from test function
    }

     #[test]
      fn test_read_from_file_invalid_xml_cursor() {
           let invalid_xml_content = r#"<root><child>Text</root>"#; // Malformed XML (child not closed)

           let raw_bytes = create_dummy_xml_bytes(invalid_xml_content);
           let mock_reader = MockOpenXmlReader::new(raw_bytes);
           let xml_file_instance = XmlFile { file_path: String::from("mock_invalid.xml") }; // Requires alloc

           // Attempt to parse, expect an error
           let result = xml_file_instance.parse_from_reader(mock_reader); // Call the helper

           assert!(result.is_err());
           match result.unwrap_err() {
               FileSystemError::InvalidData(msg) => { // Mapped from xml::reader::Error::MalformedXml or Syntax
                   assert!(msg.contains("XML Syntax Error") || msg.contains("Malformed XML"));
                   // Specific error message content might vary based on xml-rs version and exact malformation
                   // Checking for common phrases is more robust than exact message match.
                   assert!(msg.contains("unexpected end of file") || msg.contains("unclosed tag"));
               },
                FileSystemError::IOError(msg) => { // Could also be an IO error if reading fails
                    assert!(msg.contains("XML IO hatası") || msg.contains("CoreIOError"));
                }
               _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
           }
      }

       #[test]
        fn test_read_from_file_truncated_xml_cursor() {
             let truncated_xml_content = r#"<root><child>"#; // Truncated XML

             let raw_bytes = create_dummy_xml_bytes(truncated_xml_content);
             let mock_reader = MockOpenXmlReader::new(raw_bytes);
             let xml_file_instance = XmlFile { file_path: String::from("mock_truncated.xml") }; // Requires alloc

             // Attempt to parse, expect an error due to unexpected EOF
             let result = xml_file_instance.parse_from_reader(mock_reader); // Call the helper

             assert!(result.is_err());
             match result.unwrap_err() {
                 FileSystemError::IOError(msg) => { // Mapped from xml::reader::Error::UnexpectedEof or CoreIO Error
                     assert!(msg.contains("XML IO hatası") || msg.contains("XML Unexpected EOF") || msg.contains("CoreIOError"));
                     assert!(msg.contains("Beklenmedik dosya sonu") || msg.contains("UnexpectedEof"));
                 },
                 _ => panic!("Beklenenden farklı hata türü: {:?}", result.unwrap_err()),
             }
        }


    // TODO: Add tests specifically for the no_std implementation using a mock Sahne64 environment.
    // This would involve simulating resource acquire/release, fs::fstat, fs::read_at, fs::lseek.
    // Test cases should include opening valid/invalid files, handling IO errors during reading,
    // and verifying the parsed tree structure or error results with mock data.
    // This requires a no_std compatible xml-rs and a mock Sahne64 filesystem.
}


// Refactor XmlFile::read_from_file to use a private helper that takes a reader.
impl XmlFile {
    // Original read_from_file remains as the public entry point taking a path.
    // It now uses open_xml_reader and calls the internal parse_from_reader.
    pub fn read_from_file(path: &str) -> Result<XmlFile, FileSystemError> { // Return FileSystemError
        let reader = open_xml_reader(path)?; // Open the file using standardized function
        // File is automatically closed when reader is dropped.

        // Call the internal parsing helper with the obtained reader.
        // The helper takes ownership of the reader.
        Self::parse_from_reader(reader)
    }

    // Private helper function to parse from any reader implementing Read + Seek.
    // This function contains the core parsing logic using xml-rs.
    fn parse_from_reader<R: Read + Seek>(reader: R) -> Result<XmlFile, FileSystemError> { // Return FileSystemError
        // Use a buffered reader for efficient parsing with xml-rs
        #[cfg(feature = "std")]
        let buffered_reader = StdBufReader::new(reader); // Wrap reader in std BufReader
        #[cfg(not(feature = "std"))]
        let buffered_reader = crate::BufReader::new(reader); // Wrap reader in Sahne64 BufReader


        // Create an EventReader from the buffered reader
        let parser = EventReader::new(buffered_reader);


        let mut node_stack: Vec<XmlNode> = Vec::new(); // Stack to build the tree (Requires alloc)
        let mut text_buffer = String::new(); // Buffer for text content (Requires alloc)


        // Iterate through XML events
        for event_result in parser {
            let event = event_result.map_err(|e| map_xml_rs_error_to_fs_error(e))?; // Map xml-rs errors to FileSystemError

            match event {
                XmlEvent::StartElement { name, attributes, .. } => {
                    // Push a new node onto the stack for the start element
                    node_stack.push(XmlNode {
                        name: name.local_name, // Element name (Requires String)
                        attributes: attributes.into_iter() // Attributes (Requires Vec<(String, String)>)
                             .map(|attr: OwnedAttribute| (attr.name.local_name, attr.value)) // Map OwnedAttribute
                             .collect(),
                        children: Vec::new(), // Initialize children vector
                        text: None, // Initialize text as None
                    });
                }
                XmlEvent::EndElement { .. } => {
                    // Pop the current node from the stack
                    if let Some(mut node) = node_stack.pop() {
                        // If there's accumulated text, trim it and add to the node
                        if !text_buffer.is_empty() {
                            node.text = Some(text_buffer.trim().to_string()); // Trim and convert to String (Requires String)
                            text_buffer.clear(); // Clear the text buffer
                        }

                        // If there's a parent node on the stack, add the current node as a child
                        if let Some(parent) = node_stack.last_mut() {
                            parent.children.push(node); // Add as child (Requires Vec::push)
                        } else {
                            // If no parent, this is the root element's end event.
                            // The tree is complete. Return the root node.
                            // The underlying reader/handle is automatically dropped here.
                            return Ok(XmlFile { root: node }); // Return the parsed XmlFile
                        }
                    } else {
                         // Should not happen in a well-formed XML, indicates a structural issue.
                        return Err(map_xml_parse_error_to_fs_error(XmlParseError::InvalidStructure(String::from("End element without corresponding start element.")))); // Requires alloc
                    }
                }
                XmlEvent::Characters(text) => {
                    // Accumulate text content
                    text_buffer.push_str(&text); // Append text (Requires String::push_str)
                }
                // Ignore other events like ProcessingInstruction, Comment, etc. for this basic parser
                _ => {}
            }
        }

        // If the loop finishes without returning (e.g., unexpected EOF before root end),
        // the XML structure is incomplete or malformed.
        // The underlying reader/handle is automatically dropped here.
        Err(map_xml_parse_error_to_fs_error(XmlParseError::InvalidStructure(String::from("XML structure incomplete or malformed.")))) // Requires alloc
    }
}


// Removed redundant print module and panic handler boilerplate.
// The empty lib module configuration is kept.
#[cfg(not(any(feature = "std", feature = "example_xml", test)))] // Only when not building std, example, or test
pub mod lib {} // Keep the empty lib module if needed for the crate structure
