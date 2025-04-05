use std::fs::File;
use std::io::Read;
use mlua::prelude::*;

// İyileştirilmiş load_lua_file fonksiyonu:
// - Hata türü daha spesifik hale getirildi (LuaError).
// - Gereksiz ara değişkenler kaldırıldı.
pub fn load_lua_file_optimized(file_path: &str) -> Result<LuaValue, LuaError> {
    let lua = Lua::new();
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Lua kodunu doğrudan yükleyip değerlendir.
    lua.load(&buffer).eval()
}

// Örnek kullanım (optimize edilmiş fonksiyon ile):
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Optimize edilmiş fonksiyonu kullan
    let lua_value = load_lua_file_optimized("ornek.lua")?;

    // Lua değerini kullanma (aynı kalır)
    match lua_value {
        LuaValue::Table(table) => {
            for pair in table.pairs::<LuaValue, LuaValue>() {
                let (key, value) = pair?;
                println!("{:?} => {:?}", key, value);
            }
        }
        _ => println!("Lua dosyasından yüklenen değer: {:?}", lua_value),
    }

    Ok(())
}