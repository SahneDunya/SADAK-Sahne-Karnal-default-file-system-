#![no_std] // Standart kütüphaneye ihtiyaç duymuyoruz

use core::fmt; // Hata mesajlarını formatlamak için

/// Sahne64 ve SADAK işlemlerinden dönebilecek hata türleri.
///
/// Bu enum, sistem çağrısı hatalarını, kaynak yönetimi hatalarını,
/// dosya sistemi operasyon hatalarını vb. kapsar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SahneError {
    /// Yetersiz bellek.
    OutOfMemory,
    /// Geçersiz bellek adresi veya aralık.
    InvalidAddress,
    /// Fonksiyona geçersiz veya uygun olmayan parametre verildi.
    InvalidParameter,
    /// Belirtilen kaynak (dosya, aygıt, görev vb.) bulunamadı.
    ResourceNotFound,
    /// İşlem için yeterli yetki yok.
    PermissionDenied,
    /// Kaynak şu anda başka bir görev/iş parçacığı tarafından meşgul.
    ResourceBusy,
    /// İşlem bir sinyal veya başka bir olayla kesildi.
    Interrupted,
    /// Beklenen mesaj kuyrukta yok (non-blocking operasyonlar için).
    NoMessage,
    /// Kaynak üzerinde geçersiz bir işlem denendi (örn. okunamaz kaynağı okumak, yanlış seek kullanımı).
    InvalidOperation,
    /// İstenen işlem veya özellik çekirdek/sistem tarafından desteklenmiyor.
    NotSupported,
    /// Çekirdek tarafından bilinmeyen bir sistem çağrısı numarası alındı.
    UnknownSystemCall,
    /// Yeni bir görev (task) oluşturulamadı.
    TaskCreationFailed,
    /// Geçersiz veya süresi dolmuş bir Handle kullanıldı.
    InvalidHandle,
    /// Görev başına düşen Handle limiti aşıldı.
    HandleLimitExceeded,
    /// Kaynak isimlendirme veya yol çözümleme ile ilgili hata.
    NamingError,
    /// Görevler arası iletişim (IPC) sırasında hata oluştu.
    CommunicationError,
    /// Genel Giriş/Çıkış (I/O) hatası (belirli bir SahneError türüne uymayan).
    Io,
    // SADAK dosya sistemine özel hatalar buraya eklenebilir, örneğin:
    CorruptedFileSystem,       // Dosya sistemi yapısı bozuk
    InodeNotFound(u64),        // Belirtilen inode bulunamadı (Bu durumda Copy türetilemez veya u64 kaldırılmalı)
    DirectoryNotEmpty,         // Dizin boş değilken silme denemesi
    AlreadyExists,             // Kaynak zaten mevcutken oluşturma denemesi
}

// Hata mesajlarını formatlamak için Display trait'ini implemente edelim.
impl fmt::Display for SahneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SahneError::OutOfMemory => write!(f, "Out of memory"),
            SahneError::InvalidAddress => write!(f, "Invalid memory address"),
            SahneError::InvalidParameter => write!(f, "Invalid parameter"),
            SahneError::ResourceNotFound => write!(f, "Resource not found"),
            SahneError::PermissionDenied => write!(f, "Permission denied"),
            SahneError::ResourceBusy => write!(f, "Resource busy"),
            SahneError::Interrupted => write!(f, "Operation interrupted"),
            SahneError::NoMessage => write!(f, "No message available"),
            SahneError::InvalidOperation => write!(f, "Invalid operation"),
            SahneError::NotSupported => write!(f, "Operation not supported"),
            SahneError::UnknownSystemCall => write!(f, "Unknown system call"),
            SahneError::TaskCreationFailed => write!(f, "Task creation failed"),
            SahneError::InvalidHandle => write!(f, "Invalid handle"),
            SahneError::HandleLimitExceeded => write!(f, "Handle limit exceeded"),
            SahneError::NamingError => write!(f, "Naming error"),
            SahneError::CommunicationError => write!(f, "Communication error"),
            SahneError::Io => write!(f, "I/O error"),
            // SADAK'a özel hataların Display implementasyonları
            SahneError::CorruptedFileSystem => write!(f, "Corrupted file system"),
            SahneError::InodeNotFound(inode) => write!(f, "Inode {} not found", inode),
            SahneError::DirectoryNotEmpty => write!(f, "Directory not empty"),
            SahneError::AlreadyExists => write!(f, "Resource already exists"),
        }
    }
}

// Rust'ın standart Error trait'ini implemente etmek isteyebilirsiniz.
// Ancak core::error::Error trait'i genellikle nightly veya özel derleme
// ayarları gerektirir ve hata zincirleme gibi özellikler için kullanılır.
// no_std ortamında Display ve Debug genellikle yeterlidir.
#[cfg(feature = "std")] // Eğer std varsa std::error::Error implementasyonu
impl std::error::Error for SahneError {}


/// C uyumlu hata kodları.
/// Sahne64 API'sını C'den çağırırken kullanılır.
#[repr(i32)] // Bellekte 32-bit tam sayı olarak temsil edilmesini sağlar
#[allow(non_camel_case_types)] // C stilinde isimlere izin verir
pub enum sahne_error_t {
    SAHNE_SUCCESS = 0,
    SAHNE_ERROR_OUT_OF_MEMORY = 1,
    SAHNE_ERROR_INVALID_ADDRESS = 2,
    SAHNE_ERROR_INVALID_PARAMETER = 3,
    SAHNE_ERROR_RESOURCE_NOT_FOUND = 4,
    SAHNE_ERROR_PERMISSION_DENIED = 5,
    SAHNE_ERROR_RESOURCE_BUSY = 6,
    SAHNE_ERROR_INTERRUPTED = 7,
    SAHNE_ERROR_NO_MESSAGE = 8,
    SAHNE_ERROR_INVALID_OPERATION = 9,
    SAHNE_ERROR_NOT_SUPPORTED = 10,
    SAHNE_ERROR_UNKNOWN_SYSCALL = 11,
    SAHNE_ERROR_TASK_CREATION_FAILED = 12,
    SAHNE_ERROR_INVALID_HANDLE = 13,
    SAHNE_ERROR_HANDLE_LIMIT_EXCEEDED = 14,
    SAHNE_ERROR_NAMING_ERROR = 15,
    SAHNE_ERROR_COMMUNICATION_ERROR = 16,
    SAHNE_ERROR_IO = 17, // Genel I/O hatası için C kodu
    // SADAK'a özel C hata kodları buraya eklenebilir
    SAHNE_ERROR_FS_CORRUPTED = 50,
    SAHNE_ERROR_INODE_NOT_FOUND = 51,
    SAHNE_ERROR_DIR_NOT_EMPTY = 52,
    SAHNE_ERROR_ALREADY_EXISTS = 53,

    SAHNE_ERROR_OTHER = 255, // Eşleşmeyen diğer hatalar için
}

/// SahneError enum'unu C uyumlu hata koduna çevirir.
pub fn map_sahne_error_to_c(err: SahneError) -> sahne_error_t {
    match err {
        SahneError::OutOfMemory => sahne_error_t::SAHNE_ERROR_OUT_OF_MEMORY,
        SahneError::InvalidAddress => sahne_error_t::SAHNE_ERROR_INVALID_ADDRESS,
        SahneError::InvalidParameter => sahne_error_t::SAHNE_ERROR_INVALID_PARAMETER,
        SahneError::ResourceNotFound => sahne_error_t::SAHNE_ERROR_RESOURCE_NOT_FOUND,
        SahneError::PermissionDenied => sahne_error_t::SAHNE_ERROR_PERMISSION_DENIED,
        SahneError::ResourceBusy => sahne_error_t::SAHNE_ERROR_RESOURCE_BUSY,
        SahneError::Interrupted => sahne_error_t::SAHNE_ERROR_INTERRUPTED,
        SahneError::NoMessage => sahne_error_t::SAHNE_ERROR_NO_MESSAGE,
        SahneError::InvalidOperation => sahne_error_t::SAHNE_ERROR_INVALID_OPERATION,
        SahneError::NotSupported => sahne_error_t::SAHNE_ERROR_NOT_SUPPORTED,
        SahneError::UnknownSystemCall => sahne_error_t::SAHNE_ERROR_UNKNOWN_SYSCALL,
        SahneError::TaskCreationFailed => sahne_error_t::SAHNE_ERROR_TASK_CREATION_FAILED,
        SahneError::InvalidHandle => sahne_error_t::SAHNE_ERROR_INVALID_HANDLE,
        SahneError::HandleLimitExceeded => sahne_error_t::SAHNE_ERROR_HANDLE_LIMIT_EXCEEDED,
        SahneError::NamingError => sahne_error_t::SAHNE_ERROR_NAMING_ERROR,
        SahneError::CommunicationError => sahne_error_t::SAHNE_ERROR_COMMUNICATION_ERROR,
        SahneError::Io => sahne_error_t::SAHNE_ERROR_IO,
        // SADAK'a özel hataları C kodlarına çevir
        SahneError::CorruptedFileSystem => sahne_error_t::SAHNE_ERROR_FS_CORRUPTED,
        SahneError::InodeNotFound(_) => sahne_error_t::SAHNE_ERROR_INODE_NOT_FOUND,
        SahneError::DirectoryNotEmpty => sahne_error_t::SAHNE_ERROR_DIR_NOT_EMPTY,
        SahneError::AlreadyExists => sahne_error_t::SAHNE_ERROR_ALREADY_EXISTS,
    }
}


/// Çekirdekten dönen ham i64 hata kodlarını SahneError enum'una çevirir.
/// Bu fonksiyon genellikle düşük seviye sistem çağrısı sarmalayıcılarında kullanılır.
/// NOT: Gerçek Sahne64 çekirdeği kendi hata kodlarını tanımlamalı ve bu eşleme buna göre güncellenmelidir.
#[cfg(not(feature = "std"))] // Sadece no_std ortamında (syscall katmanında) kullanılır.
pub fn map_kernel_error(code: i64) -> SahneError {
    // Başarı durumları (>= 0) buraya gelmemeli, çağıran onları ele almalı.
    // Bu fonksiyon sadece negatif hata kodlarını çevirir.
    match code {
        -1 => SahneError::PermissionDenied,     // EPERM gibi (POSIX)
        -2 => SahneError::ResourceNotFound,     // ENOENT gibi
        -3 => SahneError::TaskCreationFailed,  // ESRCH gibi (işlem/görev bulunamadı)
        -4 => SahneError::Interrupted,         // EINTR gibi
        -5 => SahneError::Io,                  // EIO gibi (Genel G/Ç hatası)
        -6 => SahneError::InvalidAddress,      // ENXIO gibi (Cihaz/Adres yok)
        -9 => SahneError::InvalidHandle,       // EBADF gibi (Geçersiz dosya tanımlayıcısı/Handle)
        -11 => SahneError::ResourceBusy,       // EAGAIN veya EWOULDBLOCK gibi
        -12 => SahneError::OutOfMemory,        // ENOMEM gibi
        -13 => SahneError::PermissionDenied,     // EACCES gibi (Erişim reddedildi)
        -14 => SahneError::InvalidAddress,     // EFAULT gibi (Hatalı adres)
        -16 => SahneError::ResourceBusy,       // EBUSY gibi
        -17 => SahneError::NamingError,        // EEXIST gibi (Kaynak zaten mevcut)
        -19 => SahneError::NotSupported,       // ENODEV gibi (Aygıt yok)
        -22 => SahneError::InvalidParameter,   // EINVAL gibi (Geçersiz argüman)
        -28 => SahneError::Io,                 // ENOSPC gibi (Disk/Cihazda yer kalmadı - G/Ç hatası türü)
        -30 => SahneError::Io,                 // EROFS gibi (Salt okunur dosya sistemi - G/Ç hatası türü)
        -38 => SahneError::NotSupported,       // ENOSYS gibi (Fonksiyon/Syscall desteklenmiyor)
        -61 => SahneError::NoMessage,          // ENOMSG gibi
        // ... diğer Sahne64'e özel veya POSIX'ten esinlenen hata kodları ...

        // Belirli bir SahneError'a eşleşmeyen tüm diğer negatif çekirdek kodları
        _ => {
             println!("WARN: Eşleşmeyen çekirdek hata kodu: {}", code); // no_std print makrosu kullanılabilir.
             SahneError::UnknownSystemCall // Bilinmeyen syscall veya eşlenmemiş hata
        }
    }
}

/// Çekirdekten dönen ham i64 sonucu C uyumlu hata koduna çevirir.
/// Başarı durumunda SAHNE_SUCCESS (0) döner.
#[cfg(not(feature = "std"))] // Sadece no_std ortamında (C API katmanında) kullanılır.
pub fn map_raw_result_to_c_error(result: i64) -> sahne_error_t {
    if result >= 0 {
        sahne_error_t::SAHNE_SUCCESS
    } else {
        // Negatif çekirdek kodunu önce SahneError'a, sonra C hata koduna çevir.
        let sahne_err = map_kernel_error(result);
        map_sahne_error_to_c(sahne_err)
    }
}
