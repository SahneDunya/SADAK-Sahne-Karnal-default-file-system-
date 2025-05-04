#![no_std]
 #![allow(dead_code)]

 #[cfg(not(feature = "std"))]
 use crate::{
  fs,
  SahneError,
 };

 pub struct OdfFile {
  pub content: String,
 }

 impl OdfFile {
  pub fn open<P: AsRef<str>>(path: P) -> Result<Self, SahneError> {
  let path_str = path.as_ref();
  let content = fs::read_file_as_string(path_str)?;
  Ok(OdfFile { content })
  }

  pub fn get_content(&self) -> &str {
  &self.content
  }
 }

 #[cfg(feature = "std")]
 #[cfg(test)]
 mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Write;
  use tempfile::tempdir;

  #[test]
  fn test_open_odf() {
  let dir = tempdir().unwrap();
  let file_path = dir.path().join("content.xml");
  let mut file = File::create(&file_path).unwrap();
  file.write_all(b"<office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"><office:body><office:text><text:p xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">Merhaba Dünya!</text:p></office:text></office:body></office:document-content>").unwrap();

  let odf_file = OdfFile::open(file_path.to_str().unwrap()).unwrap();
  assert_eq!(odf_file.get_content(), "<office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"><office:body><office:text><text:p xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">Merhaba Dünya!</text:p></office:text></office:body></office:document-content>");
  }
 }
