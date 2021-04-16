use crate::JoplinReaderError;

use std::fs;
use std::io::{prelude::*, BufReader};
use std::path::Path;

use sjcl::decrypt_raw;

pub type MasterKey = String;

/// A passphrase is only used to decrypt the actual master key.
/// This function uses a `key_id` and `passphrase` pair to read the key file
/// and return the actual master key.
pub fn load_master_key(
    key_path: &Path,
    key_id: String,
    passphrase: String,
) -> Result<MasterKey, JoplinReaderError> {
    let file = match fs::File::open(key_path) {
        Ok(file) => file,
        Err(_) => {
            return Err(JoplinReaderError::FileReadError {
                message: "Failed to open file".to_string(),
            })
        }
    };
    let reader = BufReader::new(file);

    let mut id: Option<String> = None;
    let mut content: Option<String> = None;
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                return Err(JoplinReaderError::FileReadError {
                    message: "Failed to read file".to_string(),
                })
            }
        };
        let mut iter = line.splitn(2, ":");
        let key = iter.next();
        let value = iter.next();
        if let (Some(key), Some(value)) = (key, value) {
            match key {
                "id" => id = Some(value.to_string().trim().to_string()),
                "content" => content = Some(value.to_string()),
                _ => { /*println!("Unsupported key: {}", key);*/ }
            };
        }
    }
    if let None = id {
        return Err(JoplinReaderError::FileReadError {
            message: "No `id` specified in key".to_string(),
        });
    }
    if let None = content {
        return Err(JoplinReaderError::FileReadError {
            message: "No `content` specified in key".to_string(),
        });
    }
    let id = id.unwrap();
    let content = content.unwrap();
    if id != key_id {
        return Err(JoplinReaderError::KeyIdMismatch);
    }

    let plaintext = match decrypt_raw(content, passphrase) {
        Ok(pt) => pt,
        Err(_) => {
            return Err(JoplinReaderError::DecryptionError {
                message: "Failed to load master key".to_string(),
            });
        }
    };
    Ok(String::from_utf8(plaintext).unwrap())
}
