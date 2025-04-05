use crate::{fs, SahneError};
use zip::ZipArchive;

pub struct PptxFile {
    node: u64, // VfsNode yerine dosya tanımlayıcısını (file descriptor) kullanacağız
}

impl PptxFile {
    pub fn new(node_id: u64) -> Self {
        PptxFile { node: node_id }
    }

    pub fn read(&self) -> Result<String, SahneError> {
        // Dosyayı Sahne64'e özgü open fonksiyonu ile aç
        let fd = fs::open_by_id(self.node, fs::O_RDONLY)?; // VfsNode ID'sini kullanarak açtığımızı varsayıyoruz

        // Dosyanın tamamını okumak için bir döngü kullanacağız
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 4096]; // Okuma arabelleği boyutu

        loop {
            match fs::read(fd, &mut chunk) {
                Ok(0) => break, // Dosyanın sonuna gelindi
                Ok(bytes_read) => buffer.extend_from_slice(&chunk[..bytes_read]),
                Err(e) => {
                    fs::close(fd)?; // Hata durumunda dosyayı kapatmayı unutma
                    return Err(e);
                }
            }
        }

        // Dosyayı okuduktan sonra kapat
        fs::close(fd)?;

        let reader = std::io::Cursor::new(buffer);
        let mut archive = ZipArchive::new(reader).map_err(|e| {
            SahneError::InvalidParameter // ZIP arşivi açma hatasını farklı bir SahneError türüne eşleyebiliriz
        })?;

        // Basit bir örnek olarak, ilk XML dosyasının içeriğini okuyalım
        let zip_file_result = archive.by_index(0).map_err(|e| {
            SahneError::FileNotFound // XML dosyası bulunamadı hatasını farklı bir SahneError türüne eşleyebiliriz
        });

        match zip_file_result {
            Ok(mut zip_file) => {
                let mut xml_content = String::new();
                zip_file.read_to_string(&mut xml_content).map_err(|e| {
                    SahneError::InvalidParameter // XML içeriği okuma hatasını farklı bir SahneError türüne eşleyebiliriz
                })?;
                Ok(xml_content)
            }
            Err(e) => Err(e),
        }
    }
}