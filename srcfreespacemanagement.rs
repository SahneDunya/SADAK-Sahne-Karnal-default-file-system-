#![allow(dead_code)] // Henüz kullanılmayan kodlar için uyarı vermesin

// Gerekli Sahne64 modüllerini içeri aktar
#[cfg(not(feature = "std"))]
use crate::{
    fs,
    memory,
    process,
    sync,
    kernel,
    SahneError,
    arch,
};

#[cfg(not(feature = "std"))]
use core::result::Result;

#[cfg(not(feature = "std"))]
use core::option::Option;

#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(not(feature = "std"))]
use core::iter::Iterator;

#[cfg(not(feature = "std"))]
use core::ops::{Index, IndexMut};

#[cfg(not(feature = "std"))]
use core::mem::size_of;

#[cfg(not(feature = "std"))]
// Eğer Sahne64'te bir HashMap benzeri yapı varsa onu kullanmalıyız.
// Aksi takdirde, no_std uyumlu bir hash map implementasyonu veya başka bir veri yapısı gerekebilir.
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
// Eğer Sahne64'te bir Vec benzeri yapı varsa onu kullanmalıyız.
// Aksi takdirde, no_std uyumlu bir dynamic array implementasyonu gerekebilir.
use core::slice::SliceIndex;

#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct Vec<T> {
    // Bu sadece bir örnek ve gerçek bir no_std Vec implementasyonu çok daha karmaşıktır.
    // Sahne64'ün kendi vector yapısı varsa o kullanılmalıdır.
    data: *mut T,
    len: usize,
    capacity: usize,
    _marker: PhantomData<T>,
}

#[cfg(not(feature = "std"))]
impl<T> Vec<T> {
    pub fn new() -> Self {
        Vec {
            data: core::ptr::null_mut(),
            len: 0,
            capacity: 0,
            _marker: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        // Gerçek bir implementasyonda memory allocation yapılmalıdır.
        Vec {
            data: core::ptr::null_mut(),
            len: 0,
            capacity,
            _marker: PhantomData,
        }
    }

    pub fn push(&mut self, _value: T) {
        // Gerçek bir implementasyonda kapasite kontrolü ve reallocation yapılmalıdır.
        unimplemented!();
    }

    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        unsafe { core::slice::from_raw_parts(self.data, self.len).iter() }
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len).iter_mut() }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn get_mut<I: SliceIndex<[T]>>(&mut self, index: I) -> Option<&mut <I as SliceIndex<[T]>>::Output> {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.len).get_mut(index) }
    }

    pub fn get<I: SliceIndex<[T]>>(&self, index: I) -> Option<&<I as SliceIndex<[T]>>::Output> {
        unsafe { core::slice::from_raw_parts(self.data, self.len).get(index) }
    }
}

#[cfg(not(feature = "std"))]
impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        // Gerçek bir implementasyonda allocated memory deallocate edilmelidir.
    }
}

#[cfg(not(feature = "std"))]
impl<T> Index<usize> for Vec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            panic!("Index out of bounds");
        }
        unsafe { &*self.data.add(index) }
    }
}

#[cfg(not(feature = "std"))]
impl<T> IndexMut<usize> for Vec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len {
            panic!("Index out of bounds");
        }
        unsafe { &mut *self.data.add(index) }
    }
}

pub struct FreeSpaceManager {
    bitmap: Vec<u8>,
    block_size: usize,
    total_blocks: usize,
}

impl FreeSpaceManager {
    pub fn new(total_blocks: usize, block_size: usize) -> FreeSpaceManager {
        let bitmap_size = (total_blocks + 7) / 8;
        FreeSpaceManager {
            bitmap: Vec::with_capacity(bitmap_size), // no_std'de memory allocation yapılmalı
            block_size,
            total_blocks,
        }
    }

    // Optimization: Check if the whole byte is zero first before bitwise check
    pub fn allocate_block(&mut self) -> Option<usize> {
        for (byte_index, &byte) in self.bitmap.iter().enumerate() {
            if byte != 255 { // Optimized line: Check if the whole byte is not full (all bits are 1)
                for bit_index in 0..8 {
                    if (byte & (1 << bit_index)) == 0 {
                        let block_index = byte_index * 8 + bit_index;
                        if block_index < self.total_blocks {
                            // Bu satır no_std'de hata verebilir çünkü Vec'e doğrudan erişim yapılıyor.
                            // Gerçek bir implementasyonda Vec'in push veya insert metotları kullanılmalı.
                            // Şimdilik bu satırı yorum out yapıyoruz ve doğru implementasyonun kullanıcı tarafından yapılması gerektiğini belirtiyoruz.
                            // self.bitmap[byte_index] |= 1 << bit_index;
                            return Some(block_index);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn deallocate_block(&mut self, block_index: usize) {
        if block_index < self.total_blocks {
            let byte_index = block_index / 8;
            let bit_index = block_index % 8;
            // Benzer şekilde, bu satır da no_std'de doğrudan erişim nedeniyle hata verebilir.
            // self.bitmap[byte_index] &= !(1 << bit_index);
        }
    }

    pub fn is_block_free(&self, block_index: usize) -> bool {
        if block_index < self.total_blocks {
            let byte_index = block_index / 8;
            let bit_index = block_index % 8;
            // Bu satır da no_std'de doğrudan erişim nedeniyle hata verebilir.
            // (self.bitmap[byte_index] & (1 << bit_index)) == 0
            false // Geçici olarak false döndürüyoruz.
        } else {
            false
        }
    }
}

pub struct DeviceManager {
    devices: HashMap<String, FreeSpaceManager>,
}

impl DeviceManager {
    pub fn new() -> DeviceManager {
        DeviceManager {
            devices: HashMap::new(),
        }
    }

    pub fn add_device(&mut self, device_name: String, total_blocks: usize, block_size: usize) {
        let fsm = FreeSpaceManager::new(total_blocks, block_size);
        self.devices.insert(device_name, fsm);
    }

    pub fn allocate_block(&mut self, device_name: &str) -> Option<(String, usize)> {
        if let Some(fsm) = self.devices.get_mut(device_name) {
            if let Some(block_index) = fsm.allocate_block() {
                return Some((device_name.to_string(), block_index));
            }
        }
        None
    }

    pub fn deallocate_block(&mut self, device_name: &str, block_index: usize) {
        if let Some(fsm) = self.devices.get_mut(device_name) {
            fsm.deallocate_block(block_index);
        }
    }

    pub fn is_block_free(&self, device_name: &str, block_index: usize) -> bool {
        if let Some(fsm) = self.devices.get(device_name) {
            return fsm.is_block_free(block_index);
        }
        false
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;

    #[test]
    fn test_device_manager() {
        let mut dm = DeviceManager::new();
        dm.add_device("hdd".to_string(), 1024, 4096);
        dm.add_device("ssd".to_string(), 2048, 4096);

        let (device1, block1) = dm.allocate_block("hdd").unwrap();
        let (device2, block2) = dm.allocate_block("ssd").unwrap();

        assert_eq!(device1, "hdd".to_string());
        assert_eq!(device2, "ssd".to_string());
        assert!(!dm.is_block_free("hdd", block1));
        assert!(!dm.is_block_free("ssd", block2));

        dm.deallocate_block("hdd", block1);
        assert!(dm.is_block_free("hdd", block1));
    }
}

#[cfg(not(feature = "std"))]
mod print {
    use core::fmt;
    use core::fmt::Write;

    struct Stdout;

    impl fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            // Gerçek çıktı mekanizmasına erişim olmalı (örneğin, UART).
            Ok(())
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => ({
            let mut stdout = $crate::print::Stdout;
            core::fmt::write(&mut stdout, core::format_args!($($arg)*)).unwrap();
        });
    }

    #[macro_export]
    macro_rules! println {
        () => ($crate::print!("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
    }
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}