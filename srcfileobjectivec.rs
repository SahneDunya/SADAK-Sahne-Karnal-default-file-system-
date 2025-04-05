use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use plist::{Dictionary, Value};

pub fn read_objectivec_plist_optimized(file_path: &str) -> Result<HashMap<String, Value>, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let dictionary: Dictionary = plist::from_reader(reader)?;

    // İyileştirme: Doğrudan Dictionary'den HashMap'e dönüştürülüyor.
    let map: HashMap<String, Value> = dictionary.into_iter().collect();

    Ok(map)
}

pub fn write_objectivec_plist_optimized(
    file_path: &str,
    data: &HashMap<String, Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(file_path)?;
    let writer = BufWriter::new(file);

    // İyileştirme: Doğrudan HashMap'den Dictionary'e dönüştürülüyor.
    let dictionary: Dictionary = data.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    plist::to_writer_xml(writer, &dictionary)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plist::Value;
    use std::fs;

    #[test]
    fn test_read_write_plist_optimized() {
        let file_path = "test_optimized.plist";
        let mut data = HashMap::new();
        data.insert("name".to_string(), Value::String("Test Name".to_string()));
        data.insert("age".to_string(), Value::Integer(30));

        write_objectivec_plist_optimized(file_path, &data).unwrap();
        let read_data = read_objectivec_plist_optimized(file_path).unwrap();

        assert_eq!(read_data.get("name"), Some(&Value::String("Test Name".to_string())));
        assert_eq!(read_data.get("age"), Some(&Value::Integer(30)));

        fs::remove_file(file_path).unwrap();
    }
}