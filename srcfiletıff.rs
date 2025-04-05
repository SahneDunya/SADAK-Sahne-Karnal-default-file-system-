#![allow(unused_imports)] // Gerekli olmayan importlar için uyarı vermesin

use super::{fs, SahneError};
use super::SahneError as MetadataError; // Kendi hata türümüzü MetadataError olarak da kullanabiliriz

use core::fmt;

#[cfg(feature = "std")] // Standart kütüphane özelliği aktifse derlenecek
pub mod srcfiletıff {
    use super::*;
    use std::fs::File;
    use std::io::{BufReader, Read};
    use tiff::decoder::{Decoder, DecodingError};
    use tiff::tags::{Tag, Type};
    use std::fmt;

    // Daha okunabilir bir çıktı için özel hata türü
    #[derive(Debug)]
    pub enum MetadataError {
        TiffError(DecodingError),
        TagNotFound(Tag),
        InvalidTagType(Tag, Type),
        ConversionError(Tag, String), // Genel dönüşüm hatası için
        SahneFsError(SahneError), // Sahne64 dosya sistemi hataları için
    }

    impl fmt::Display for MetadataError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MetadataError::TiffError(e) => write!(f, "TIFF Hatası: {}", e),
                MetadataError::TagNotFound(tag) => write!(f, "Etiket bulunamadı: {:?}", tag),
                MetadataError::InvalidTagType(tag, type_enum) => write!(f, "Geçersiz etiket türü: {:?} için beklenen tür: {:?}", tag, type_enum),
                MetadataError::ConversionError(tag, message) => write!(f, "{:?} etiketi dönüştürme hatası: {}", tag, message),
                MetadataError::SahneFsError(e) => write!(f, "Sahne64 Dosya Sistemi Hatası: {:?}", e),
            }
        }
    }

    impl From<DecodingError> for MetadataError {
        fn from(err: DecodingError) -> Self {
            MetadataError::TiffError(err)
        }
    }

    impl From<SahneError> for MetadataError {
        fn from(err: SahneError) -> Self {
            MetadataError::SahneFsError(err)
        }
    }


    pub fn read_tiff_metadata(file_path: &str) -> Result<(), MetadataError> {
        // Sahne64 dosya sistemini kullanarak dosyayı aç
        let fd = fs::open(file_path, fs::O_RDONLY)?;

        // Dosyanın boyutunu almamız gerekebilir (Sahne64'te böyle bir fonksiyon varsa)
        // Şimdilik dosyanın tamamını okuyacağımızı varsayalım.
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let bytes_read = match fs::read(fd, &mut chunk) {
                Ok(size) => size,
                Err(e) => {
                    fs::close(fd).unwrap_or(()); // Hata durumunda dosyayı kapat
                    return Err(e.into());
                }
            };
            if bytes_read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..bytes_read]);
        }

        // Dosyayı okuduktan sonra kapat
        fs::close(fd)?;

        // TIFF decoder'ı için bir BufReader oluşturalım (std kütüphanesini varsayıyoruz)
        let reader = BufReader::new(buffer.as_slice());
        let mut decoder = Decoder::new(reader)?;

        println!("TIFF Dosyası Meta Verileri ({:?}):", file_path);

        // Yardımcı fonksiyon etiket değerini güvenli bir şekilde okumak ve yazdırmak için
        let get_and_print_tag = |decoder: &mut Decoder<BufReader<&[u8]>>, tag: Tag, tag_name: &str| -> Result<(), MetadataError> {
            match decoder.get_tag(tag) {
                Ok(value) => {
                    match value {
                        Type::U32(v) => {
                            if let Some(val) = v.first() {
                                println!("{}: {}", tag_name, val);
                            } else {
                                println!("{}: Değer bulunamadı", tag_name);
                            }
                        },
                        Type::U16(v) => {
                            if let Some(val) = v.first() {
                                println!("{}: {}", tag_name, val);
                            } else {
                                println!("{}: Değer bulunamadı", tag_name);
                            }
                        },
                        Type::Ascii(v) => {
                            if let Some(val) = v.first() {
                                println!("{}: {}", tag_name, val);
                            } else {
                                println!("{}: Değer bulunamadı", tag_name);
                            }
                        },
                        Type::Rational(v) => {
                            if let Some(val) = v.first() {
                                println!("{}: {}/{}", tag_name, val.n, val.d);
                            } else {
                                println!("{}: Değer bulunamadı", tag_name);
                            }
                        },
                        _ => {
                            println!("{}: {:?} (Yazdırılamayan tür)", tag_name, value);
                        }
                    }
                    Ok(())
                },
                Err(DecodingError::TagNotFound) => {
                    println!("{}: Bulunamadı", tag_name); // Etiket bulunamadığında bilgi ver
                    Ok(()) // Bulunamaması hata değil, devam et
                },
                Err(e) => Err(e.into()), // Diğer TIFF hatalarını işle
            }
        };

        get_and_print_tag(&mut decoder, Tag::ImageWidth, "Genişlik")?;
        get_and_print_tag(&mut decoder, Tag::ImageLength, "Yükseklik")?;
        get_and_print_tag(&mut decoder, Tag::BitsPerSample, "Bit/Örnek")?;
        get_and_print_tag(&mut decoder, Tag::PhotometricInterpretation, "Fotometrik Yorumlama")?;
        get_and_print_tag(&mut decoder, Tag::ImageDescription, "Dosya Açıklaması")?;
        get_and_print_tag(&mut decoder, Tag::Make, "Üretici")?;
        get_and_print_tag(&mut decoder, Tag::Model, "Model")?;
        get_and_print_tag(&mut decoder, Tag::Software, "Yazılım")?;
        get_and_print_tag(&mut decoder, Tag::DateTime, "Tarih ve Saat")?;
        get_and_print_tag(&mut decoder, Tag::Artist, "Sanatçı")?;
        get_and_print_tag(&mut decoder, Tag::Copyright, "Telif Hakkı")?;
        get_and_print_tag(&mut decoder, Tag::ResolutionUnit, "Çözünürlük Birimi")?;
        get_and_print_tag(&mut decoder, Tag::XResolution, "X Çözünürlüğü")?;
        get_and_print_tag(&mut decoder, Tag::YResolution, "Y Çözünürlüğü")?;


        Ok(())
    }


    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Write;
        use tiff::encoder::{TiffEncoder, colortype::ColorType};
        use tiff::ImageBuffer;

        // Test TIFF dosyası oluşturma fonksiyonu
        fn create_test_tiff(file_path: &str) -> std::io::Result<()> {
            let mut file = File::create(file_path)?;
            let encoder = TiffEncoder::new(&mut file)?;

            let width: u32 = 100;
            let height: u32 = 100;
            let mut image_buffer: ImageBuffer<ColorType::Gray(8), Vec<u8>> = ImageBuffer::new(width, height);

            // Basit bir desen oluştur
            for x in 0..width {
                for y in 0..height {
                    image_buffer.put_pixel(x, y, tiff::ColorValue::Gray( (x % 255) as u8));
                }
            }

            encoder.encode_image(image_buffer.as_raw(), width, height, ColorType::Gray(8))?;
            Ok(())
        }


        #[test]
        fn test_read_tiff_metadata() {
            let test_file_path = "test.tiff";

            // Test dosyası yoksa oluştur
            if !std::path::Path::new(test_file_path).exists() {
                create_test_tiff(test_file_path).expect("Test TIFF dosyası oluşturulamadı");
            }

            println!("Test dosyası oluşturuldu veya bulundu: {}", test_file_path);
            let result = read_tiff_metadata(test_file_path);
            assert!(result.is_ok());

            if result.is_err() {
                match result.unwrap_err() {
                    MetadataError::TiffError(e) => println!("TIFF Hatası: {}", e),
                    MetadataError::TagNotFound(tag) => println!("Etiket bulunamadı: {:?}", tag),
                    MetadataError::InvalidTagType(tag, type_enum) => println!("Geçersiz etiket türü: {:?} için beklenen tür: {:?}", tag, type_enum),
                    MetadataError::ConversionError(tag, message) => println!("{:?} etiketi dönüştürme hatası: {}", tag, message),
                    MetadataError::SahneFsError(e) => println!("Sahne64 Dosya Sistemi Hatası: {:?}", e),
                }
            }
        }
    }
}

#[cfg(not(feature = "std"))]
pub mod srcfiletıff {
    // no_std ortamı için TIFF desteği şu anki Sahne64 API'si ile mümkün değil.
    // Eğer TIFF desteği isteniyorsa, ya no_std uyumlu bir TIFF kütüphanesi bulunmalı
    // ya da TIFF formatını ayrıştırmak için özel kod yazılmalıdır.

    use super::{SahneError};
    use core::fmt;
    use tiff::tags::{Tag};
    use tiff::decoder::DecodingError;
    use tiff::tags::Type;

    #[derive(Debug)]
    pub enum MetadataError {
        TiffError(DecodingError),
        TagNotFound(Tag),
        InvalidTagType(Tag, Type),
        ConversionError(Tag, String), // Genel dönüşüm hatası için
        SahneFsError(SahneError), // Sahne64 dosya sistemi hataları için
        NotSupported, // no_std ortamında TIFF desteği yok
    }

    impl fmt::Display for MetadataError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MetadataError::TiffError(e) => write!(f, "TIFF Hatası: {}", e),
                MetadataError::TagNotFound(tag) => write!(f, "Etiket bulunamadı: {:?}", tag),
                MetadataError::InvalidTagType(tag, type_enum) => write!(f, "Geçersiz etiket türü: {:?} için beklenen tür: {:?}", tag, type_enum),
                MetadataError::ConversionError(tag, message) => write!(f, "{:?} etiketi dönüştürme hatası: {}", tag, message),
                MetadataError::SahneFsError(e) => write!(f, "Sahne64 Dosya Sistemi Hatası: {:?}", e),
                MetadataError::NotSupported => write!(f, "no_std ortamında TIFF desteği bulunmuyor"),
            }
        }
    }

    impl From<SahneError> for MetadataError {
        fn from(err: SahneError) -> Self {
            MetadataError::SahneFsError(err)
        }
    }

    // no_std ortamında TIFF okuma şu anki API ile desteklenmiyor.
    pub fn read_tiff_metadata(file_path: &str) -> Result<(), MetadataError> {
        Err(MetadataError::NotSupported)
    }

    #[cfg(test)]
    mod tests {
    }
}