use crate::fs::vfs::{FileType, VfsNode};
use crate::mm::alloc::kalloc;
use crate::sync::spinlock::Spinlock;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::size_of;
use lazy_static::lazy_static;
use spin::Mutex;

// Basit bir XLSX ayrıştırıcısı (gerçek bir ayrıştırıcı çok daha karmaşık olacaktır)
// Optimizasyon: Bu basit ayrıştırıcı, CSV benzeri yapıyı olabildiğince verimli işleyecek şekilde tasarlandı.
// Gerçek bir XLSX ayrıştırıcısı çok daha karmaşık olurdu ve harici bir kütüphane kullanmak gerekirdi.
fn parse_xlsx(data: &[u8]) -> Result<Vec<Vec<String>>, &'static str> {
    // UTF-8 kaybıyla dizeye dönüştürme işlemini optimize edin:
    // Bu, veriyi bir String'e dönüştürmenin en hızlı yoludur, ancak geçersiz UTF-8 verisi durumunda veri kaybına neden olabilir.
    // Eğer performans kritikse ve verinin genellikle UTF-8 olduğundan eminseniz bu kabul edilebilir.
    let content = String::from_utf8_lossy(data);

    // Satır ve hücre ayırma işlemlerini optimize edin:
    // `lines()` ve `split(',')` yineleyicileri bellek verimliliği için kullanılır.
    // `trim()` ve `to_string()` hücre değerlerini temizler ve yeni String'ler oluşturur.
    let rows: Vec<Vec<String>> = content
        .lines()
        .map(|line| {
            line.split(',')
                .map(|cell| cell.trim().to_string())
                .collect()
        })
        .collect();
    Ok(rows)
}

pub struct XlsxFile {
    data: Vec<u8>,
    // Mutex ile korunan ayrıştırılmış veri önbelleği:
    // `Mutex` kullanmak, ayrıştırılmış verilere eşzamanlı erişimi güvenli hale getirir.
    // `Option` kullanmak, verinin henüz ayrıştırılmadığını belirtmek için kullanılır.
    parsed_data: Mutex<Option<Vec<Vec<String>>>>,
}

impl XlsxFile {
    pub fn new(data: Vec<u8>) -> Self {
        XlsxFile {
            data,
            parsed_data: Mutex::new(None), // Başlangıçta ayrıştırılmış veri yok
        }
    }

    // Veri okuma fonksiyonunu optimize edin:
    // Bu fonksiyon, ayrıştırılmış veriyi önbellekten döndürmek veya gerekirse ayrıştırmak için tasarlanmıştır.
    pub fn read(&self) -> Result<Vec<Vec<String>>, &'static str> {
        // Kilitlenme süresini en aza indirin:
        // `lock()` çağrısı sadece ayrıştırılmış veriye erişirken yapılır.
        let mut parsed_data = self.parsed_data.lock();

        // Ayrıştırılmış veri zaten varsa, onu klonlayarak döndürün:
        // `clone()` kullanmak, verinin bir kopyasını oluşturur ve böylece iç veriyi değiştirmeden güvenli erişim sağlar.
        if let Some(ref data) = *parsed_data {
            return Ok(data.clone()); // Önbellekten klonlanmış veriyi döndür
        }

        // Ayrıştırılmış veri yoksa, ayrıştırma işlemini gerçekleştirin:
        // `parse_xlsx` fonksiyonu sadece bir kez çağrılır (ilk okuma işleminde).
        let parsed = parse_xlsx(&self.data)?;

        // Ayrıştırılmış veriyi önbelleğe kaydedin:
        // `parsed.clone()` kullanarak verinin bir kopyasını önbelleğe kaydederiz,
        // böylece orijinal `parsed` verisi başka yerlerde kullanılabilir.
        *parsed_data = Some(parsed.clone()); // Ayrıştırılmış veriyi önbelleğe kaydet

        Ok(parsed) // Yeni ayrıştırılmış veriyi döndür
    }
}

// XLSX düğümü oluşturma fonksiyonunu optimize edin:
// Bu fonksiyon, `XlsxFile` yapısını ve VFS düğümünü oluşturur ve Arc ile sarmalar.
pub fn create_xlsx_node(name: String, data: Vec<u8>) -> Arc<Spinlock<VfsNode>> {
    // `XlsxFile` örneğini oluşturun:
    // Veriyi ve Mutex ile korunan ayrıştırılmış veri önbelleğini içerir.
    let xlsx_file = Arc::new(XlsxFile::new(data));

    // VFS düğümünü oluşturun:
    // Düğüm adı, dosya tipi ve XLSX dosyası örneği ile oluşturulur.
    let node = VfsNode::new(name, FileType::File, Some(xlsx_file), None);

    // VFS düğümünü Spinlock ve Arc ile sarın ve döndürün:
    // `Spinlock` ve `Arc` kullanarak, düğümün eşzamanlı ve paylaşımlı erişimini güvenli hale getiririz.
    Arc::new(Spinlock::new(node))
}