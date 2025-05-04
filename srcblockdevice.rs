#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Gerekli Sahne64 modüllerini içeri aktar
use crate::{
    resource, // fs modülü yerine resource modülü kullanıldı
    memory,
    task,     // process modülü yerine task modülü kullanıldı
    sync,
    kernel,
    SahneError,
    arch,
    Handle,   // Handle tipi eklendi
};

use core::result::Result;
use core::cmp::min; // core::cmp::min kullanıldı (std yerine)

// std::io::SeekFrom yerine kendi tanımımızı kullanıyoruz (no_std uyumluluğu için)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

// Blok aygıtı için temel arayüz
// Bu trait, blok aygıtlarının ofset ve seek gibi beklenen özelliklerini tanımlar.
// Ancak, alttaki Sahne64 API'sı bu özelliklerin tamamını doğrudan desteklemeyebilir.
pub trait BlockDevice {
    /// Blok boyutunu döndürür.
    fn block_size(&self) -> u64;

    /// Blok aygıtından belirtilen ofsetten başlayarak belirtilen boyutta veri okur.
    /// Okunan byte sayısını döner.
    /// Sahne64 resource API'sında doğrudan offsetli okuma yoksa, bu metodun
    /// doğru çalışması için Seek + Read kombinasyonu veya resource::control gibi
    /// özel bir mekanizma gereklidir. Mevcut implementasyon bu eksikliği yansıtacaktır.
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, SahneError>;

    /// Blok aygıtına belirtilen ofsetten başlayarak belirtilen boyutta veri yazar.
    /// Yazılan byte sayısını döner.
    /// Sahne64 resource API'sında doğrudan offsetli yazma yoksa, bu metodun
    /// doğru çalışması için Seek + Write kombinasyonu veya resource::control gibi
    /// özel bir mekanizma gereklidir. Mevcut implementasyon bu eksikliği yansıtacaktır.
    fn write(&mut self, offset: u64, buf: &[u8]) -> Result<usize, SahneError>;

    /// Blok aygıtının toplam boyutunu döndürür.
    /// Sahne64 resource API'sında boyutu almak için bir syscall yoksa, bu
    /// desteklenmiyor olarak işaretlenecektir.
    fn size(&self) -> Result<u64, SahneError>;

    /// Blok aygıtında belirtilen konuma (ofset) konumlanır.
    /// Dönüş değeri yeni konumu belirtir.
    /// Sahne64 resource API'sında seek için bir syscall yoksa, bu
    /// desteklenmiyor olarak işaretlenecektir.
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError>;
}

// Bellek tabanlı blok aygıtı (std veya alloc gerektirir)
// Bu implementasyon Sahne64 API'sından bağımsızdır ve test amaçlı kullanılabilir.
// Dikkat: 'vec!' ve 'std::cmp::min' kullanımı 'alloc' veya 'std' crate'ine bağlıdır.
#[cfg(any(feature = "std", feature = "alloc"))] // std veya alloc özelliği varsa derle
pub struct MemBlockDevice {
    data: alloc::vec::Vec<u8>, // std::vec::Vec yerine alloc::vec::Vec
    block_size: u64,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl MemBlockDevice {
    pub fn new(size: u64, block_size: u64) -> Self {
        MemBlockDevice {
            data: alloc::vec![0; size as usize],
            block_size,
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl BlockDevice for MemBlockDevice {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, SahneError> {
        let offset_usize = offset as usize;
        let len = buf.len();

        if offset_usize >= self.data.len() {
            return Ok(0); // Ofset cihaz boyutunun dışında
        }

        let read_len = min(len, self.data.len() - offset_usize); // core::cmp::min kullanıldı
        buf[..read_len].copy_from_slice(&self.data[offset_usize..offset_usize + read_len]);
        Ok(read_len)
    }

    fn write(&mut self, offset: u64, buf: &[u8]) -> Result<usize, SahneError> {
        let offset_usize = offset as usize;
        let len = buf.len();

        if offset_usize >= self.data.len() {
            return Ok(0); // Ofset cihaz boyutunun dışında
        }

        let write_len = min(len, self.data.len() - offset_usize); // core::cmp::min kullanıldı
        self.data[offset_usize..offset_usize + write_len].copy_from_slice(&buf[..write_len]);
        Ok(write_len)
    }

    fn size(&self) -> Result<u64, SahneError> {
        Ok(self.data.len() as u64)
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError> {
        // Bellek içi aygıtta seek işleminin anlamı sınırlıdır, her zaman 0'a döner.
        // Gerçek bir implementasyonda current_pos alanını tutmak gerekebilir.
        // Bu trait'teki seek tanımı, alttaki Sahne64 API'sının yetenekleriyle
        // tam olarak eşleşmeyebilir.
         match pos {
             SeekFrom::Start(offset) => {
                 if offset as usize > self.data.len() {
                     // Hata dönebilir veya cihaz boyutu kadar konumlanabilir
                     Err(SahneError::InvalidAddress) // Örnek hata
                 } else {
                     // Normalde burada iç durumdaki current_pos güncellenir.
                     Ok(offset)
                 }
             },
             SeekFrom::End(offset) => {
                  // Cihaz sonu + ofset hesaplanır
                 let new_pos = (self.data.len() as i64 + offset) as u64;
                  // Normalde burada iç durumdaki current_pos güncellenir.
                 Ok(new_pos) // Hata kontrolü gerekli
             },
             SeekFrom::Current(offset) => {
                  // Normalde burada iç durumdaki current_pos + ofset hesaplanır.
                  // Varsayılan olarak 0'da olduğumuzu varsayalım
                 let new_pos = offset as u64; // Hata kontrolü gerekli
                  // Normalde burada iç durumdaki current_pos güncellenir.
                 Ok(new_pos)
             },
         }
         // Bu MemBlockDevice seek implementasyonu tam doğru değil, sadece trait'i karşılamak için basit bir örnek.
         // Gerçek bir seek implementasyonu, struct içinde 'current_pos' gibi bir alan tutmalıdır.
         // Ancak trait implementasyonu offsetli read/write kullandığı için, seek fonksiyonu
         // bu MemBlockDevice implementasyonunda doğrudan kullanılmaz.
         // Bu kısım, BlockDevice trait'inin kendisinin Sahne64 API'sıyla tam uyumlu olmadığını göstermektedir.
    }
}


// Dosya tabanlı blok aygıtı (HDD, SSD, vb.) - Sahne64'e özel implementasyon
pub struct ResourceBlockDevice { // FileBlockDevice yerine ResourceBlockDevice
    handle: Handle, // Sahne64 kaynak Handle'ı
    block_size: u64,
    // Not: Sahne64 API'sında kaynağın boyutunu almak için doğrudan bir syscall yok gibi görünüyor.
    // Bu nedenle 'size' metodu doğru implemente edilemeyebilir.
    // Benzer şekilde, 'seek' metodu için de doğrudan bir syscall yok.
}

impl ResourceBlockDevice {
    /// Belirtilen Sahne64 kaynağını blok aygıt olarak açar.
    pub fn new(resource_id: &str, block_size: u64) -> Result<Self, SahneError> {
        // Sahne64 resource::acquire fonksiyonunu kullanıyoruz
        let flags = resource::MODE_READ | resource::MODE_WRITE | resource::MODE_CREATE; // MODE_TRUNCATE isteğe bağlı
        let acquire_result = resource::acquire(resource_id, flags); // fs::open yerine resource::acquire

        match acquire_result {
            Ok(handle) => Ok(ResourceBlockDevice { handle, block_size }), // fd yerine handle
            Err(e) => Err(e),
        }
    }

    /// Cihaz Handle'ını kapatır.
    pub fn close(&mut self) -> Result<(), SahneError> {
        resource::release(self.handle) // fs::close yerine resource::release
    }
}

impl BlockDevice for ResourceBlockDevice {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    // --- DİKKAT: Sahne64 API Kısıtlaması ---
    // Sahne64 resource::read syscall'ı doğrudan offset parametresi almaz.
    // resource::read muhtemelen kaynağın mevcut konumundan okur/yazar.
    // BlockDevice trait'indeki 'offset' parametresini doğrudan karşılayamaz.
    // Bu implementasyon, 'offset' parametresini GÖRMEZDEN GELİR ve her zaman
    // kaynağın mevcut konumundan (muhtemelen başlangıcından, API'ye bağlı) okur.
    // Gerçek bir blok aygıtı gibi çalışması için Sahne64 API'sında
    // offsetli okuma/yazma veya seek syscall'ı eklenmelidir.
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, SahneError> {
        // TODO: Eğer Sahne64'te seek benzeri bir resource::control komutu varsa,
        // burada önce o komut çağrılarak offset ayarlanmalıdır:
        // resource::control(self.handle, resource::CONTROL_SEEK, offset)?;
        // Ardından resource::read çağrılır.

        // Şimdilik, offset parametresini yoksayarak doğrudan okuyoruz.
        // BU YANLIŞ DAVRANIŞTIR, Sahne64 API'sındaki eksikliği yansıtır.
        println!("WARN: ResourceBlockDevice::read offset {} parametresini yoksayıyor!", offset); // no_std print makrosu
        resource::read(self.handle, buf) // resource::read kullanıldı
         // Okuma başarılıysa, okunan byte sayısını döndürür.
         // Trait gereksinimi buffer boyutunda okumak ise ek kontrol gerekir.
         // resource::read tam olarak buffer.len() okumayabilir (örn. dosya sonu).
    }

    // --- DİKKAT: Sahne64 API Kısıtlaması ---
    // Sahne64 resource::write syscall'ı doğrudan offset parametresi almaz.
    // resource::write muhtemelen kaynağın mevcut konumundan okur/yazar.
    // BlockDevice trait'indeki 'offset' parametresini doğrudan karşılayamaz.
    // Bu implementasyon, 'offset' parametresini GÖRMEZDEN GELİR ve her zaman
    // kaynağın mevcut konumundan (muhtemelen başlangıcından, API'ye bağlı) yazar.
    // Gerçek bir blok aygıtı gibi çalışması için Sahne64 API'sında
    // offsetli okuma/yazma veya seek syscall'ı eklenmelidir.
    fn write(&mut self, offset: u64, buf: &[u8]) -> Result<usize, SahneError> {
        // TODO: Eğer Sahne64'te seek benzeri bir resource::control komutu varsa,
        // burada önce o komut çağrılarak offset ayarlanmalıdır:
         resource::control(self.handle, resource::CONTROL_SEEK, offset)?;
        // Ardından resource::write çağrılır.

        // Şimdilik, offset parametresini yoksayarak doğrudan yazıyoruz.
        // BU YANLIŞ DAVRANIŞTIR, Sahne64 API'sındaki eksikliği yansıtır.
        println!("WARN: ResourceBlockDevice::write offset {} parametresini yoksayıyor!", offset); // no_std print makrosu
        resource::write(self.handle, buf) // resource::write kullanıldı
         // Yazma başarılıysa, yazılan byte sayısını döndürür.
         // Trait gereksinimi buffer boyutunda yazmak ise ek kontrol gerekir.
         // resource::write tam olarak buffer.len() yazmayabilir (örn. disk dolu).
    }

    // --- DİKKAT: Sahne64 API Kısıtlaması ---
    // Sahne64 API'sında kaynağın boyutunu almak için doğrudan bir syscall yok gibi.
    fn size(&self) -> Result<u64, SahneError> {
        // TODO: Sahne64'te resource::control ile size almak mümkünse, implemente et.
        // Örneğin: resource::control(self.handle, resource::CONTROL_GET_SIZE, 0) gibi.
        Err(SahneError::NotSupported) // Veya uygun hata
    }

    // --- DİKKAT: Sahne64 API Kısıtlaması ---
    // Sahne64 API'sında seek işlevi için doğrudan bir syscall yok gibi.
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SahneError> {
        // TODO: Sahne64'te seek için bir resource::control komutu varsa, implemente et.
        // Örneğin: resource::control(self.handle, resource::CONTROL_SEEK, offset_value) gibi.
        println!("WARN: ResourceBlockDevice::seek henüz desteklenmiyor!"); // no_std print makrosu
        Err(SahneError::NotSupported) // Veya uygun hata
    }
}

// Örnek kullanım (Bu fonksiyonun kendisi std veya alloc gerektirebilir,
// ancak ResourceBlockDevice'ın kullanımı no_std uyumludur)
// Gerçek bir Sahne64 uygulamasında entry point başka bir yerde olacaktır.
#[cfg(feature = "example")] // Sadece 'example' özelliği aktifse derle
fn main() -> Result<(), SahneError> {
    // MemBlockDevice örneği (eğer std/alloc varsa)
    #[cfg(any(feature = "std", feature = "alloc"))]
    {
        println!("MemBlockDevice örneği:");
        let mut mem_device = MemBlockDevice::new(1024, 512);
        let mut mem_buf = [0; 512];

        // MemBlockDevice trait'indeki read/write offset alıyor
        mem_device.write(0, b"Merhaba 1. blok!").unwrap();
        mem_device.write(512, b"Selam 2. blok!").unwrap();

        let bytes_read_1 = mem_device.read(0, &mut mem_buf).unwrap();
        println!("MemBlockDevice (Offset 0): Okunan {} byte: {:?}", bytes_read_1, &mem_buf[..bytes_read_1]);
        let bytes_read_2 = mem_device.read(512, &mut mem_buf).unwrap();
        println!("MemBlockDevice (Offset 512): Okunan {} byte: {:?}", bytes_read_2, &mem_buf[..bytes_read_2]);

        println!("MemBlockDevice boyutu: {:?}", mem_device.size().unwrap());
         // Seek MemBlockDevice için tam olarak implemente edilmediğini unutmayın.
          let new_pos = mem_device.seek(SeekFrom::Start(256)).unwrap();
          println!("MemBlockDevice seek sonucu: {}", new_pos);

        println!("---");
    }


    // ResourceBlockDevice örneği (Sahne64 API kullanır)
    println!("ResourceBlockDevice örneği (Sahne64 API):");
    let resource_id = "sahne://devices/disk0"; // Sahne64 kaynak tanımlayıcısı
    let block_size = 512;

    // Cihazı aç
    let mut resource_device = match ResourceBlockDevice::new(resource_id, block_size) {
        Ok(dev) => dev,
        Err(e) => {
            eprintln!("ResourceBlockDevice açma hatası: {:?}", e);
            return Err(e);
        }
    };

    let mut buf = [0u8; 512]; // Blok boyutunda arabelle
    let write_data = [42u8; 512]; // Yazılacak veri

    // --- Blok 10'a yazma denemesi ---
    // DİKKAT: Bu çağrı, ResourceBlockDevice::write içindeki Sahne64 API kısıtlaması nedeniyle
    // offset 10 * block_size'ı (yani 5120'yi) yoksayacak ve kaynağın mevcut konumuna yazacaktır!
    println!("Blok 10'a yazma denemesi (offset 5120)...");
    match resource_device.write(10 * block_size, &write_data) {
        Ok(bytes_written) => {
             println!("ResourceBlockDevice yazma başarılı ({} byte yazıldı).", bytes_written);
             if bytes_written != block_size as usize {
                 println!("UYARI: Tam blok yazılamadı!");
             }
        }
        Err(e) => eprintln!("ResourceBlockDevice yazma hatası: {:?}", e),
    }

    // --- Blok 10'dan okuma denemesi ---
    // DİKKAT: Bu çağrı, ResourceBlockDevice::read içindeki Sahne64 API kısıtlaması nedeniyle
    // offset 10 * block_size'ı (yani 5120'yi) yoksayacak ve kaynağın mevcut konumundan okuyacaktır!
    // Eğer bir önceki yazma işlemi kaynağın başına yazdıysa, burası 0. bloktan okur gibi davranır.
    println!("Blok 10'dan okuma denemesi (offset 5120)...");
     // Okuma öncesinde belki kaynağın konumunu başa almak mantıklı olabilir (seek desteklenseydi):
      resource_device.seek(SeekFrom::Start(0))?;
    match resource_device.read(10 * block_size, &mut buf) {
        Ok(bytes_read) => {
            println!("ResourceBlockDevice okuma başarılı ({} byte okundu).", bytes_read);
            if bytes_read > 0 {
                 println!("Okunan ilk 10 byte: {:?}", &buf[..min(10, bytes_read)]);
            }
        }
        Err(e) => eprintln!("ResourceBlockDevice okuma hatası: {:?}", e),
    }

    // Kaynak boyutunu alma denemesi
    match resource_device.size() {
        Ok(size) => println!("ResourceBlockDevice boyutu: {}", size),
        Err(e) => eprintln!("ResourceBlockDevice boyutu alma hatası: {:?}", e), // Muhtemelen NotSupported dönecektir
    }

    // Seek denemesi
      match resource_device.seek(SeekFrom::Start(5120)) {
          Ok(new_pos) => println!("ResourceBlockDevice seek başarılı, yeni konum: {}", new_pos),
          Err(e) => eprintln!("ResourceBlockDevice seek hatası: {:?}", e), // Muhtemelen NotSupported dönecektir
      }


    // Cihazı kapat
    match resource_device.close() {
        Ok(_) => println!("ResourceBlockDevice kapatıldı."),
        Err(e) => eprintln!("ResourceBlockDevice kapatma hatası: {:?}", e),
    }


    Ok(())
}


// Bu kısım, no_std ortamında çalışabilmek için gereklidir.
#[cfg(not(feature = "std"))] // Sadece std özelliği aktif değilse derle
mod print {
    use core::fmt;
    use core::fmt::Write;
    use crate::resource; // resource modülünü kullanmak için import edildi
    use crate::Handle; // Handle tipini kullanmak için import edildi
    use crate::SahneError; // Hata tipini kullanmak için import edildi

    // TODO: Konsol Handle'ı bir yerden alınmalı ve init_console ile ayarlanmalı.
    // Gerçek sistemde bu global mutable static bir değişken olabilir (unsafe gerektirir)
    // veya thread-local depolama kullanılabilir.
    static mut CONSOLE_HANDLE: Option<Handle> = None;

    /// Panik mesajları veya loglar için kullanılacak konsol Handle'ını ayarlar.
    /// Görevin başlangıcında çağrılmalıdır.
    pub fn init_console(handle: Handle) {
        unsafe {
            CONSOLE_HANDLE = Some(handle);
        }
    }

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            unsafe {
                if let Some(handle) = CONSOLE_HANDLE {
                    // Gerçek çıktı mekanizmasına (örneğin, bir konsol kaynağına) resource::write ile erişim.
                    // Hata durumunu basitçe yoksayıyoruz veya panic yapabiliriz.
                    // Panik handler içindeysek panic yapmak sonsuz döngüye yol açabilir.
                    let _ = resource::write(handle, s.as_bytes());
                    // resource::write(handle, b"\n"); // Her print sonrası yeni satır eklemek isteyebiliriz
                    Ok(())
                } else {
                    // Konsol handle'ı ayarlanmamışsa, çıktıyı yoksay veya ilkel bir çıktı kullan.
                    // Bu durumda çıktı kaybolur.
                    Err(fmt::Error) // fmt::Error herhangi bir çıktı hatasını belirtmek için kullanılabilir.
                }
            }
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => ({
            // Sadece no_std ortamında bu makroyu kullan
            #[cfg(not(feature = "std"))]
            {
                use core::fmt::Write;
                let mut stdout = $crate::print::Stdout; // $crate kullanımı modül yolunu belirtir
                // write! hata dönebilir, panik handler içindeysek unwrap() güvensiz olabilir.
                // Basitçe hatayı yoksayalım.
                let _ = core::fmt::write(&mut stdout, core::format_args!($($arg)*));
            }
            // Eğer std ortamındaysak ve stdio_impl yerine bu modülü kullanıyorsak,
            // std::print!/println! kullanmak daha iyi olabilir.
            // Bu kod yapısında stdio_impl modu std varken, print modu no_std varken aktif.
        });
    }

    #[macro_export]
    macro_rules! println {
        () => ($crate::print!("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", core::format_args!($($arg)*)));
    }

    #[macro_export]
    macro_rules! eprintln {
        () => ($crate::print!("\n")); // Şimdilik stderr yok, stdout'a yaz
        ($($arg:tt)*) => ($crate::print!("{}\n", core::format_args!($($arg)*)));
    }

    // no_std print modülünün içindeyiz, dışa aktarılması gerekiyorsa:
    pub use print::{print, println, eprintln}; // Bu satır modül dışına taşındı
}

// no_std print makrolarını dışa aktar (yalnızca no_std ortamında)
#[cfg(not(feature = "std"))]
pub use print::{print, println, eprintln};
// no_std print modülündeki init_console fonksiyonunu da dışa aktaralım
#[cfg(not(feature = "std"))]
pub use print::init_console;


#[cfg(not(feature = "std"))] // panic_handler her zaman no_std ortamında gereklidir
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // no_std panic'te çıktı alabilmek için print! makrosunu kullanıyoruz
    // CONSOLE_HANDLE ayarlanmamışsa çıktı görünmeyebilir.
    println!("PANIC: {}", info); // no_std print makrosu

    // Gerçek sistemde burada bir hata kaydı, sistem reset veya sonsuz döngü olabilir.
    loop {
        core::hint::spin_loop(); // İşlemciyi meşgul etmeden bekle
    }
}

// SeekFrom tanımı zaten yukarıda yapıldı, std gerektirmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom { ... }

// Bu dosya kütüphane olarak kullanılacaksa, gerekli tipleri dışa aktaralım:
pub use crate::BlockDevice;
#[cfg(any(feature = "std", feature = "alloc"))]
pub use crate::MemBlockDevice;
pub use crate::ResourceBlockDevice;
pub use crate::SeekFrom; // SeekFrom'u da dışa aktaralım
