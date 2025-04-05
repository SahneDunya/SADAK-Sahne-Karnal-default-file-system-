#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz (eğer bu dosya tek başına derleniyorsa)
#![allow(dead_code)]

// Gerekli modülleri içe aktar
use crate::fs;
use crate::SahneError;
use xml::reader::{EventReader, XmlEvent};
use core::io::Read; // Read trait'ini kullanmamız gerekecek

pub struct XmlFile {
    pub root: XmlNode,
}

pub struct XmlNode {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
    pub text: Option<String>,
}

// Basit bir BufReader benzeri yapı (Sahne64'ün read fonksiyonunu kullanacak)
struct SahneBufReader {
    fd: u64,
    buffer: [u8; 4096], // Örnek bir arabellek boyutu
    position: usize,
    filled: usize,
}

impl SahneBufReader {
    fn new(fd: u64) -> Self {
        SahneBufReader {
            fd,
            buffer: [0; 4096],
            position: 0,
            filled: 0,
        }
    }

    fn fill_buffer(&mut self) -> Result<(), SahneError> {
        self.position = 0;
        self.filled = fs::read(self.fd, &mut self.buffer)?;
        Ok(())
    }
}

impl Read for SahneBufReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, core::io::Error> {
        if self.position >= self.filled {
            self.fill_buffer().map_err(|e| core::io::Error::new(core::io::ErrorKind::Other, e))?;
            if self.position >= self.filled {
                return Ok(0); // EOF
            }
        }

        let count = core::cmp::min(buf.len(), self.filled - self.position);
        buf[..count].copy_from_slice(&self.buffer[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

impl XmlFile {
    pub fn read_from_file(path: &str) -> Result<XmlFile, String> {
        let fd_result = fs::open(path, fs::O_RDONLY);
        let fd = match fd_result {
            Ok(file_descriptor) => file_descriptor,
            Err(e) => return Err(format!("Dosya açma hatası: {:?}", e)),
        };

        let reader = SahneBufReader::new(fd);
        let parser = EventReader::new(reader);
        let mut nodes = Vec::new();
        let mut text_buffer = String::new();

        for event in parser {
            match event {
                Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                    nodes.push(XmlNode {
                        name: name.local_name,
                        attributes: attributes.into_iter().map(|attr| (attr.name.local_name, attr.value)).collect(),
                        children: Vec::new(),
                        text: None,
                    });
                }
                Ok(XmlEvent::EndElement { .. }) => {
                    if let Some(mut node) = nodes.pop() {
                        if !text_buffer.is_empty() {
                            node.text = Some(text_buffer.trim().to_string());
                            text_buffer.clear();
                        }
                        if let Some(parent) = nodes.last_mut() {
                            parent.children.push(node);
                        } else {
                            // Dosyayı kapatmayı unutmayalım
                            let _ = fs::close(fd);
                            return Ok(XmlFile { root: node });
                        }
                    }
                }
                Ok(XmlEvent::Characters(text)) => {
                    text_buffer.push_str(&text);
                }
                Err(e) => {
                    // Dosyayı kapatmayı unutmayalım
                    let _ = fs::close(fd);
                    return Err(e.to_string());
                }
                _ => {}
            }
        }

        // Dosyayı kapatmayı unutmayalım (eğer döngüden çıkarsa)
        let _ = fs::close(fd);
        Err("XML yapısı hatalı".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write; // Testlerde standart kütüphaneyi kullanmaya devam edebiliriz (eğer bu testler Sahne64 üzerinde çalışmayacaksa)

    #[test]
    fn test_read_xml() {
        let xml_content = r#"
            <root>
                <child attr1="value1">Text</child>
                <child2>
                    <grandchild>More text</grandchild>
                </child2>
            </root>
        "#;
        write("test.xml", xml_content).expect("Dosya oluşturulamadı");

        let xml_file = XmlFile::read_from_file("test.xml").expect("XML okuma hatası");

        assert_eq!(xml_file.root.name, "root");
        assert_eq!(xml_file.root.children.len(), 2);
        assert_eq!(xml_file.root.children[0].name, "child");
        assert_eq!(xml_file.root.children[0].attributes[0].0, "attr1");
        assert_eq!(xml_file.root.children[0].attributes[0].1, "value1");
        assert_eq!(xml_file.root.children[0].text, Some("Text".to_string()));
        assert_eq!(xml_file.root.children[1].children[0].name, "grandchild");
        assert_eq!(xml_file.root.children[1].children[0].text, Some("More text".to_string()));
    }
}