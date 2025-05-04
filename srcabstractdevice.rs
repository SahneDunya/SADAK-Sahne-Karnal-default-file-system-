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

/// Trait defining the interface for abstract block devices.
pub trait AbstractDevice {
    /// Reads a block of data from the device.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The logical block number to read.
    /// * `buffer` - The buffer to store the read data. Must be the size of a block.
    ///
    /// # Returns
    ///
    /// Returns `Ok(buffer.len())` if the block was successfully read, or a `SahneError` if an error occurred.
    fn read_block(&mut self, block_number: u64, buffer: &mut [u8]) -> Result<usize, SahneError>;

    /// Writes a block of data to the device.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The logical block number to write to.
    /// * `buffer` - The buffer containing the data to write. Must be the size of a block.
    ///
    /// # Returns
    ///
    /// Returns `Ok(buffer.len())` if the block was successfully written, or a `SahneError` if an error occurred.
    fn write_block(&mut self, block_number: u64, buffer: &[u8]) -> Result<usize, SahneError>;

    /// Returns the size of a block in bytes.
    fn block_size(&self) -> u64;

    /// Returns the total number of blocks in the device.
    fn block_count(&self) -> u64;
}

/// Enum to handle different types of devices.
pub enum DeviceHandle {
    /// Represents a resource-backed device, using a Sahne64 Handle.
    Resource(Handle), // File(u64) yerine Resource(Handle) kullanıldı
    // Future device types can be added here, e.g.,
    UsbDevice(UsbDeviceImpl),
    NvmeDevice(NvmeDeviceImpl),
}

/// Represents a block device built on top of a DeviceHandle.
pub struct BlockDevice {
    device: DeviceHandle,
    block_size: u64,
    block_count: u64,
}

impl BlockDevice {
    /// Creates a new BlockDevice instance.
    ///
    /// # Arguments
    ///
    /// * `device` - The underlying device handle (Sahne64 resource Handle).
    /// * `block_size` - The size of each block in bytes.
    /// * `block_count` - The total number of blocks.
    pub fn new(device: DeviceHandle, block_size: u64, block_count: u64) -> Self {
        BlockDevice {
            device,
            block_size,
            block_count,
        }
    }
}

impl AbstractDevice for BlockDevice {
    fn read_block(&mut self, block_number: u64, buffer: &mut [u8]) -> Result<usize, SahneError> {
        match &mut self.device {
            DeviceHandle::Resource(handle) => { // File(fd) yerine Resource(handle) kullanıldı
                let offset = block_number * self.block_size;
                // Sahne64'te doğrudan offsetli okuma/yazma syscall'ları yok (en azından paylaşılan kodda).
                // Resource::read ve Resource::write muhtemelen kaynağın mevcut konumundan okur/yazar.
                // Gerçek bir blok cihazı için, okumadan önce kaynağın konumunu 'seek' benzeri
                // bir mekanizma (muhtemelen resource::control ile) ayarlamak gerekebilir.
                // Şu anki implementasyon, bu soyutlamanın eksik bir parçasıdır ve
                // her okuma/yazma çağrısının kaynağın başından başladığını varsayarsa
                // veya kaynağın durumunu (offsetini) dahili olarak yönettiğini varsayarsa
                // doğru çalışmaz. Bu, Sahne64 API'sında çözülmesi gereken bir konudur.
                // Şimdilik, sadece mevcut resource::read fonksiyonunu kullanıyoruz ve
                // offset yönetiminin API'nin daha alt katmanında (veya resource::control
                // gibi bir yolla) ele alınması gerektiğini not ediyoruz.

                if buffer.len() as u66 != self.block_size { // buffer.len() bir usize, u64 ile karşılaştırırken dikkatli olmalı
                    // Veya SahneError::InvalidParameter
                    return Err(SahneError::InvalidOperation); // Buffer boyutu blok boyutuna eşit olmalı
                }

                // Burada ideal olarak bir seek işlemi olurdu:
                resource::control(*handle, resource::CONTROL_SEEK, offset)?;

                let read_result = resource::read(*handle, buffer); // fs::read yerine resource::read kullanıldı
                match read_result {
                    Ok(bytes_read) => {
                         // resource::read kaç byte okuduğunu dönmeli.
                         // Blok cihaz trait'i tam blok okumayı bekler.
                        if bytes_read == buffer.len() {
                             Ok(bytes_read)
                         } else {
                             // Tam blok okuyamadık, bu bir hata veya dosya sonu olabilir.
                             // Dosya sistemi mantığına göre bu durum ele alınmalı.
                             // Örneğin, eğer bytes_read 0 ise dosya sonu.
                             // Eğer 0 < bytes_read < buffer.len() ise kısmi okuma (genellikle hata).
                            Err(SahneError::CommunicationError) // Veya daha spesifik bir hata
                         }
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn write_block(&mut self, block_number: u64, buffer: &[u8]) -> Result<usize, SahneError> {
        match &mut self.device {
            DeviceHandle::Resource(handle) => { // File(fd) yerine Resource(handle) kullanıldı
                let offset = block_number * self.block_size;
                 // Offset yönetimi notu burada da geçerli.

                if buffer.len() as u64 != self.block_size {
                    return Err(SahneError::InvalidOperation); // Buffer boyutu blok boyutuna eşit olmalı
                }

                 // Burada ideal olarak bir seek işlemi olurdu:
                 resource::control(*handle, resource::CONTROL_SEEK, offset)?;


                let write_result = resource::write(*handle, buffer); // fs::write yerine resource::write kullanıldı
                match write_result {
                    Ok(bytes_written) => {
                         // resource::write kaç byte yazdığını dönmeli.
                         // Blok cihaz trait'i tam blok yazmayı bekler.
                        if bytes_written == buffer.len() {
                             Ok(bytes_written)
                         } else {
                             // Tam blok yazamadık, bu bir hata.
                             Err(SahneError::CommunicationError) // Veya daha spesifik bir hata
                         }
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn block_count(&self) -> u64 {
        self.block_count
    }
}

// #[cfg(feature = "std")] // main fonksiyonu sadece standart kütüphane varsa derlensin
// Örnek kullanım, Sahne64 API'sına göre güncellendi.
fn main() -> Result<(), SahneError> {
    let resource_id = "sahne://devices/disk0"; // Sahne64 kaynak tanımlayıcısı

    // Kaynak açma modları için Sahne64 sabitlerini kullanıyoruz
    let flags = resource::MODE_READ | resource::MODE_WRITE | resource::MODE_CREATE | resource::MODE_TRUNCATE; // fs::O_* yerine resource::MODE_* kullanıldı
    let acquire_result = resource::acquire(resource_id, flags); // fs::open yerine resource::acquire kullanıldı

    let device_handle_val = match acquire_result {
        Ok(handle) => {
            println!("Kaynak edinildi ('{}'), Handle: {:?}", resource_id, handle);
            handle
        },
        Err(e) => {
            eprintln!("Kaynak edinme hatası: {:?}", e);
            return Err(e);
        }
    };

    let block_size = 512;
    let block_count = 1024;
    let device_size = block_size * block_count;

    // Sahne64'te kaynağın boyutunu ayarlamak için bir sistem çağrısı gerekebilir (örn. resource::control ile bir SET_SIZE komutu).
    // acquire modlarındaki TRUNCATE belki yeterlidir, ama kaynağın istenen boyuta ulaştığından emin olmak gerekir.
    resource::control(device_handle_val, resource::CONTROL_SET_SIZE, device_size)?;


    let device_handle = DeviceHandle::Resource(device_handle_val); // Handle kullanıldı
    let mut block_device = BlockDevice::new(device_handle, block_size, block_count);

    let mut read_buffer = [0u8; 512]; // Blok boyutunda okuma arabelleği
    let write_buffer = [42u8; 512];    // Blok boyutunda yazma arabelleği

    // Blok 10'a yazma
    // Not: Bu yazma işlemi, resource::write'ın offset'i otomatik yönettiğini veya
    // yukarıdaki yorumda belirtildiği gibi seek benzeri bir mekanizma kullanıldığını varsayar.
    let write_result = block_device.write_block(10, &write_buffer);
    match write_result {
        Ok(_) => println!("Blok 10'a yazma başarılı."),
        Err(e) => eprintln!("Blok 10'a yazma hatası: {:?}", e),
    }

    // Blok 10'dan okuma
    // Not: Benzer şekilde, bu okuma işlemi de offset yönetimini varsayar.
    let read_result = block_device.read_block(10, &mut read_buffer);
    match read_result {
        Ok(bytes_read) => {
            println!("Blok 10'dan {} byte okundu.", bytes_read);
            // Burada okunan veriyi kontrol etmek isteyebilirsiniz.
            assert_eq!(read_buffer, write_buffer);
        }
        Err(e) => eprintln!("Blok 10'dan okuma hatası: {:?}", e),
    }

    // Kaynağı serbest bırakma
    let release_result = resource::release(device_handle_val); // fs::close yerine resource::release kullanıldı
    match release_result {
        Ok(_) => println!("Kaynak serbest bırakıldı."),
        Err(e) => eprintln!("Kaynak serbest bırakma hatası: {:?}", e),
    }

    Ok(())
}

// Bu kısım, no_std ortamında çalışabilmek için gereklidir.
// Eğer bu kodu standart bir Rust ortamında derlemek isterseniz,
// yukarıdaki 'main' fonksiyonunun başındaki #[cfg(feature = "std")]'i aktif hale getirebilirsiniz.
#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;
    use crate::resource; // resource modülünü kullanmak için import edildi
    use crate::SahneError; // Hata tipini kullanmak için import edildi
    use crate::Handle; // Handle tipini kullanmak için import edildi

    // TODO: Konsol Handle'ı bir yerden alınmalı.
    // Bu, görevin başlatılması sırasında argüman olarak verilebilir veya
    // resource::acquire("sahne://devices/console", ...) gibi bir çağrı ile edinilebilir.
    // Şimdilik dummy bir Handle kullanıyoruz.
    // Gerçek sistemde bu global mutable static bir değişken olabilir (unsafe gerektirir)
    // veya thread-local depolama kullanılabilir.
    static mut CONSOLE_HANDLE: Option<Handle> = None;

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
                    let write_result = resource::write(handle, s.as_bytes());
                     // println!/print! içinde hata yönetimi genellikle istenmez,
                     // bu yüzden unwrap() veya basit hata yoksayma yaygındır.
                    if write_result.is_err() {
                         // Hata durumunu bir yere kaydetmek isteyebiliriz, ama burası kısıtlı bir ortam.
                         // Belki de daha ilkel bir çıktı mekanizması kullanılır.
                    }
                    Ok(())
                } else {
                    // Konsol handle'ı ayarlanmamışsa, çıktıyı yoksay.
                    Err(fmt::Error) // Veya başka bir hata
                }
            }
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => ({
            use core::fmt::Write;
            let mut stdout = $crate::print::Stdout; // $crate kullanımı modül yolunu belirtir
            let _ = core::fmt::write(&mut stdout, core::format_args!($($arg)*)); // Hata durumunu yoksay
        });
    }

    #[macro_export]
    macro_rules! println {
        () => ($crate::print!("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
    }

    #[macro_export]
    macro_rules! eprintln {
        () => ($crate::print!("\n")); // Şimdilik stderr yok, stdout'a yaz
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
    }
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // no_std panic'te çıktı alabilmek için print! makrosunu kullan
    #[cfg(not(feature = "std"))] // print makrosu sadece no_std ortamında aktif
    {
        println!("{}", info);
    }
    // Gerçek sistemde burada bir hata kaydı, sistem reset veya sonsuz döngü olabilir.
    loop {
        core::hint::spin_loop(); // İşlemciyi meşgul etmeden bekle
    }
}

// Re-export macros in no_std mode
#[cfg(not(feature = "std"))]
pub use print::{print, println, eprintln};
