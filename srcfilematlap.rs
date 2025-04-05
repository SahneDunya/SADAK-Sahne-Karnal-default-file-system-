use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write, ErrorKind};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

// Basit bir MATLAB değişkeni yapısı (örnek olarak)
struct MatVariable {
    name: String,
    data: Vec<f64>,
}

// .mat dosyasından değişken okuma fonksiyonu (İyileştirilmiş hata yönetimi)
fn read_mat_variable<R: Read>(reader: &mut R) -> io::Result<MatVariable> {
    // Değişken adını oku (örnek olarak, 64 baytlık bir alan)
    let mut name_buffer = [0u8; 64];
    match reader.read_exact(&mut name_buffer) {
        Ok(_) => {},
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
            // Dosya sonuna ulaşıldı, değişken okumayı durdur
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "EOF while reading variable name"));
        }
        Err(e) => return Err(e), // Diğer hataları yukarıya ilet
    }
    let name = String::from_utf8_lossy(&name_buffer).trim_end_matches('\0').to_string();

    // Veri boyutunu oku (örnek olarak, f64 dizisi)
    let data_size = match reader.read_u64::<LittleEndian>() {
        Ok(size) => size,
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "EOF while reading data size"));
        }
        Err(e) => return Err(e),
    };
    let mut data = Vec::with_capacity(data_size as usize);

    // Verileri oku
    for _ in 0..data_size {
        match reader.read_f64::<LittleEndian>() {
            Ok(value) => data.push(value),
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                return Err(io::Error::new(ErrorKind::UnexpectedEof, "EOF while reading data"));
            }
            Err(e) => return Err(e),
        }
    }

    Ok(MatVariable { name, data })
}

// .mat dosyasına değişken yazma fonksiyonu (Değişiklik yok, zaten iyi)
fn write_mat_variable<W: Write>(writer: &mut W, variable: &MatVariable) -> io::Result<()> {
    // Değişken adını yaz (64 baytlık alan)
    let mut name_buffer = [0u8; 64];
    let name_bytes = variable.name.as_bytes();
    name_buffer[..name_bytes.len()].copy_from_slice(name_bytes);
    writer.write_all(&name_buffer)?;

    // Veri boyutunu yaz
    writer.write_u64::<LittleEndian>(variable.data.len() as u64)?;

    // Verileri yaz
    for value in &variable.data {
        writer.write_f64::<LittleEndian>(*value)?;
    }

    Ok(())
}

// .mat dosyasını okuma fonksiyonu (İyileştirilmiş döngü ve hata işleme)
pub fn read_mat_file_optimized(file_path: &str) -> io::Result<Vec<MatVariable>> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut variables = Vec::new();

    loop {
        match read_mat_variable(&mut reader) {
            Ok(variable) => variables.push(variable),
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                // Dosya sonuna ulaşıldı, döngüden çık
                break;
            }
            Err(e) => return Err(e), // Diğer hataları yukarıya ilet
        }
    }

    Ok(variables)
}

// .mat dosyasına yazma fonksiyonu (Değişiklik yok)
pub fn write_mat_file(file_path: &str, variables: &[MatVariable]) -> io::Result<()> {
    let file = File::create(file_path)?;
    let mut writer = BufWriter::new(file);

    for variable in variables {
        write_mat_variable(&mut writer, variable)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn test_mat_rw_optimized() {
        let variables = vec![
            MatVariable {
                name: "var1".to_string(),
                data: vec![1.0, 2.0, 3.0],
            },
            MatVariable {
                name: "var2".to_string(),
                data: vec![4.0, 5.0, 6.0, 7.0],
            },
        ];

        let file_path = "test_optimized.mat";
        write_mat_file(file_path, &variables).unwrap();

        let read_variables = read_mat_file_optimized(file_path).unwrap();
        assert_eq!(variables.len(), read_variables.len());
        for i in 0..variables.len() {
            assert_eq!(variables[i].name, read_variables[i].name);
            assert_eq!(variables[i].data, read_variables[i].data);
        }
        std::fs::remove_file(file_path).unwrap(); // Test dosyasını temizle
    }

    #[test]
    fn test_mat_rw_empty_file_optimized() {
        let file_path = "test_empty_optimized.mat";
        File::create(file_path).unwrap(); // Boş dosya oluştur

        let read_variables = read_mat_file_optimized(file_path).unwrap();
        assert_eq!(read_variables.len(), 0); // Boş dosyadan değişken okunmamalı
        std::fs::remove_file(file_path).unwrap(); // Test dosyasını temizle
    }

    #[test]
    fn test_mat_rw_truncated_data_optimized() {
        let file_path = "test_truncated_optimized.mat";
        let mut file = File::create(file_path).unwrap();

        // Kısmi veri yaz (Sadece değişken adı ve boyut, verinin kendisi yok)
        let variable = MatVariable {
            name: "truncated_var".to_string(),
            data: vec![1.0, 2.0, 3.0],
        };
        let mut name_buffer = [0u8; 64];
        let name_bytes = variable.name.as_bytes();
        name_buffer[..name_bytes.len()].copy_from_slice(name_bytes);
        file.write_all(&name_buffer).unwrap();
        file.write_u64::<LittleEndian>(variable.data.len() as u64).unwrap();
        // Veriler yazılmıyor, dosya kesik

        let result = read_mat_file_optimized(file_path);
        match result {
            Err(e) => assert_eq!(e.kind(), ErrorKind::UnexpectedEof), // Kesik dosya hatası bekleniyor
            Ok(_) => panic!("Beklenen hata oluşmadı: UnexpectedEof"),
        }
        std::fs::remove_file(file_path).unwrap(); // Test dosyasını temizle
    }
}