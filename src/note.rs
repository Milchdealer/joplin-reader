use crate::JoplinReaderError;

use regex::{Captures, Regex};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::iter::DoubleEndedIterator;
use std::path::{Path, PathBuf};
use std::str::Chars;
use std::time::SystemTime;

use chrono::NaiveDateTime;
use percent_encoding::percent_decode_str;
use sjcl::decrypt_raw;

/// How often encrypted notes should be refreshed in seconds
const REFRESH_INTERVAL: u64 = 60 * 60 * 12;
/// Size of the full encryption header
const HEADER_SIZE: u32 = 45;

/// Various types of items a joplin file can be.
/// See: https://joplinapp.org/api/references/rest_api/#item-type-ids
#[derive(Debug, PartialEq)]
pub enum JoplinItemType {
    Undefined = 0,
    Note = 1,
    Folder = 2,
    Setting = 3,
    Resource = 4,
    Tag = 5,
    NoteTag = 6,
    Search = 7,
    Alarm = 8,
    MasterKey = 9,
    ItemChange = 10,
    NoteResource = 11,
    ResourceLocalState = 12,
    Revision = 13,
    Migration = 14,
    SmartFilter = 15,
    Command = 16,
}

impl From<i32> for JoplinItemType {
    fn from(v: i32) -> Self {
        match v {
            1 => JoplinItemType::Note,
            2 => JoplinItemType::Folder,
            3 => JoplinItemType::Setting,
            4 => JoplinItemType::Resource,
            5 => JoplinItemType::Tag,
            6 => JoplinItemType::NoteTag,
            7 => JoplinItemType::Search,
            8 => JoplinItemType::Alarm,
            9 => JoplinItemType::MasterKey,
            10 => JoplinItemType::ItemChange,
            11 => JoplinItemType::NoteResource,
            12 => JoplinItemType::ResourceLocalState,
            13 => JoplinItemType::Revision,
            14 => JoplinItemType::Migration,
            15 => JoplinItemType::SmartFilter,
            16 => JoplinItemType::Command,
            _ => JoplinItemType::Undefined,
        }
    }
}

/// Contains general information about a note, and reads a part of the header
/// when created to check if the note needs to be decrypted (and with which
/// key).
#[derive(Debug)]
pub struct NoteInfo {
    path: PathBuf,
    id: String,
    type_: JoplinItemType,
    encryption_applied: bool,
    parent_id: Option<String>,
    encryption_key_id: Option<String>,
    updated_time: Option<NaiveDateTime>,
    // `read_time` is when it was read into by **us**
    read_time: Option<SystemTime>,
    content: NoteProperties,
}

/// Contains the actual properties and content of a note. This follows the
/// general structure of the note properties from Joplin minus the ones already
/// read into [`NoteInfo`].
/// See: https://joplinapp.org/api/references/rest_api/#properties
#[derive(Debug)]
pub struct NoteProperties {
    title: Option<String>,
    body: Option<String>,
    created_time: Option<NaiveDateTime>,
    altitude: Option<f32>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    author: Option<String>,
    source_url: Option<String>,
    is_todo: Option<bool>,
    todo_due: Option<bool>,
    todo_completed: Option<bool>,
    source: Option<String>,
    source_application: Option<String>,
    application_data: Option<String>,
    order: Option<i32>,
    user_created_time: Option<NaiveDateTime>,
    user_updated_time: Option<NaiveDateTime>,
    markup_language: Option<String>,
    is_shared: Option<bool>,
}
impl Default for NoteProperties {
    fn default() -> Self {
        Self {
            title: None,
            body: None,
            created_time: None,
            altitude: None,
            latitude: None,
            longitude: None,
            author: None,
            source_url: None,
            is_todo: None,
            todo_due: None,
            todo_completed: None,
            source: None,
            source_application: None,
            application_data: None,
            order: None,
            user_created_time: None,
            user_updated_time: None,
            markup_language: None,
            is_shared: None,
        }
    }
}
impl From<HashMap<String, String>> for NoteProperties {
    fn from(mut kv_store: HashMap<String, String>) -> Self {
        let mut title: Option<String> = None;
        let mut body: Option<String> = None;
        let mut created_time: Option<NaiveDateTime> = None;
        let mut altitude: Option<f32> = None;
        let mut latitude: Option<f64> = None;
        let mut longitude: Option<f64> = None;
        let mut author: Option<String> = None;
        let mut source_url: Option<String> = None;
        let mut is_todo: Option<bool> = None;
        let mut todo_due: Option<bool> = None;
        let mut todo_completed: Option<bool> = None;
        let mut source: Option<String> = None;
        let mut source_application: Option<String> = None;
        let mut application_data: Option<String> = None;
        let mut order: Option<i32> = None;
        let mut user_created_time: Option<NaiveDateTime> = None;
        let mut user_updated_time: Option<NaiveDateTime> = None;
        let mut markup_language: Option<String> = None;
        let mut is_shared: Option<bool> = None;

        for (k, v) in kv_store.drain() {
            match k.as_str() {
                "title" => title = Some(v),
                "body" => body = Some(v),
                "created_time" => {
                    created_time = match NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S%.fZ")
                    {
                        Ok(ut) => Some(ut),
                        Err(_) => None,
                    }
                }
                "altitude" => {
                    altitude = match v.trim().parse::<f32>() {
                        Ok(l) => Some(l),
                        _ => None,
                    }
                }
                "latitude" => {
                    latitude = match v.trim().parse::<f64>() {
                        Ok(l) => Some(l),
                        _ => None,
                    }
                }
                "longitude" => {
                    longitude = match v.trim().parse::<f64>() {
                        Ok(l) => Some(l),
                        _ => None,
                    }
                }
                "author" => author = Some(v),
                "source_url" => source_url = Some(v),
                "is_todo" => {
                    is_todo = match v.trim().parse::<i8>() {
                        Ok(b) => Some(b == 1),
                        _ => None,
                    }
                }
                "todo_due" => {
                    todo_due = match v.trim().parse::<i8>() {
                        Ok(b) => Some(b == 1),
                        _ => None,
                    }
                }
                "todo_completed" => {
                    todo_completed = match v.trim().parse::<i8>() {
                        Ok(b) => Some(b == 1),
                        _ => None,
                    }
                }
                "source" => source = Some(v),
                "source_application" => source_application = Some(v),
                "application_data" => application_data = Some(v),
                "order" => {
                    order = match v.trim().parse::<i32>() {
                        Ok(o) => Some(o),
                        _ => None,
                    }
                }
                "user_created_time" => {
                    user_created_time =
                        match NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S%.fZ") {
                            Ok(ut) => Some(ut),
                            Err(_) => None,
                        }
                }
                "user_updated_time" => {
                    user_updated_time =
                        match NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S%.fZ") {
                            Ok(ut) => Some(ut),
                            Err(_) => None,
                        }
                }
                "markup_language" => markup_language = Some(v),
                "is_shared" => {
                    is_shared = match v.trim().parse::<i8>() {
                        Ok(b) => Some(b == 1),
                        _ => None,
                    }
                }
                _ => { /* unknown key */ }
            }
        }

        Self {
            title,
            body,
            created_time,
            altitude,
            latitude,
            longitude,
            author,
            source_url,
            is_todo,
            todo_due,
            todo_completed,
            source,
            source_application,
            application_data,
            order,
            user_created_time,
            user_updated_time,
            markup_language,
            is_shared,
        }
    }
}

/// Leading header of the `encryption_cipher_text` in an item
#[derive(Debug)]
struct JoplinEncryptionHeader {
    version: u8,
    length: u32,
    encryption_method: JoplinEncryptionMethod,
    master_key_id: String,
}

/// Joplin defines the various cipher suits and key lengths SJCL provides as
/// methods in an enumerated fashion.
/// Method 4 is used for key encryption, and method 1a for notes.
/// Everything else is deprecated (and also considered unsecure).
#[derive(Debug, PartialEq)]
pub enum JoplinEncryptionMethod {
    MethodUndefined = 0x0,
    MethodSjcl = 0x1,
    MethodSjcl2 = 0x2,
    MethodSjcl3 = 0x3,
    MethodSjcl4 = 0x4,
    MethodSjcl1a = 0x5,
}

impl From<u8> for JoplinEncryptionMethod {
    fn from(v: u8) -> Self {
        match v {
            0x1 => JoplinEncryptionMethod::MethodSjcl,
            0x2 => JoplinEncryptionMethod::MethodSjcl2,
            0x3 => JoplinEncryptionMethod::MethodSjcl3,
            0x4 => JoplinEncryptionMethod::MethodSjcl4,
            0x5 => JoplinEncryptionMethod::MethodSjcl1a,
            _ => JoplinEncryptionMethod::MethodUndefined,
        }
    }
}

impl NoteInfo {
    /// Reads an encrypted file, which has some unencrypted keys as well as the
    /// ciphertext. List of all keys which are stored unencrypted:
    /// https://github.com/laurent22/joplin/blob/bfacf71397e21fda5c7c1675365c4199d29de9e7/packages/lib/models/BaseItem.ts#L418
    fn parse_encrypted_file<R: BufRead>(
        reader: &mut R,
    ) -> Result<HashMap<String, String>, JoplinReaderError> {
        let mut kv_store: HashMap<String, String> = HashMap::new();
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
                // This will update&succeed in case of duplicate keys:
                kv_store.insert(
                    key.to_string().trim().to_string(),
                    value.to_string().trim().to_string(),
                );
            }
        }

        Ok(kv_store)
    }

    /// So the general format for notes is:
    /// Title\n\nBody\n\n[Prop: PropValue\n,...]
    /// But if they are encrypted, instead some unencrypted properties may be
    /// stored at first, and then this general format is encrypted entirely.
    /// See [`NoteInfo::parse_encrypted_file`].
    /// Serialization:
    /// https://github.com/laurent22/joplin/blob/bfacf71397e21fda5c7c1675365c4199d29de9e7/packages/lib/models/BaseItem.ts#L330
    fn deserialize(
        text: impl DoubleEndedIterator<Item = impl AsRef<str>>,
    ) -> Result<HashMap<String, String>, JoplinReaderError> {
        let mut kv_store: HashMap<String, String> = HashMap::new();
        let mut body: Vec<String> = Vec::new();

        enum ReadingState {
            Props,
            Body,
        }
        let mut state: ReadingState = ReadingState::Props;
        // Because \n\n is used for splitting, the content has to be read backwards
        // See: https://github.com/laurent22/joplin/blob/bfacf71397e21fda5c7c1675365c4199d29de9e7/packages/lib/models/BaseItem.ts#L446
        for line in text.rev() {
            let line = line.as_ref().trim().to_string();
            match state {
                ReadingState::Props => {
                    if line.is_empty() {
                        state = ReadingState::Body;
                        continue;
                    }

                    let mut iter = line.splitn(2, ":");
                    let key = iter.next();
                    let value = iter.next();
                    if let (Some(key), Some(value)) = (key, value) {
                        // This will update&succeed in case of duplicate keys:
                        kv_store.insert(
                            key.to_string().trim().to_string(),
                            value.to_string().trim().to_string(),
                        );
                    } else {
                        return Err(JoplinReaderError::InvalidFormat {
                            message: "Invalid property format".to_string(),
                        });
                    }
                }
                ReadingState::Body => {
                    // Since we read backwards, we insert the lines into the beginning
                    body.insert(0, line);
                }
            }
        }

        let type_ = match kv_store.get(&"type_".to_string()) {
            Some(t) => match t.parse::<i32>() {
                Ok(t) => JoplinItemType::from(t),
                Err(_) => {
                    return Err(JoplinReaderError::InvalidFormat {
                        message: "Missing required property: `type_`".to_string(),
                    });
                }
            },
            None => {
                return Err(JoplinReaderError::InvalidFormat {
                    message: "Missing required property: `type_`".to_string(),
                });
            }
        };

        if !body.is_empty() {
            kv_store.insert("title".to_string(), body.remove(0));
            body.remove(0); // Because it is title\n\n
        }
        if type_ == JoplinItemType::Note {
            kv_store.insert("body".to_string(), body.join("\n"));
        }

        Ok(kv_store)
    }

    /// Reads in a new from a `Path`.
    pub fn new(note_path: &Path) -> Result<NoteInfo, JoplinReaderError> {
        let file = match fs::File::open(note_path) {
            Ok(file) => file,
            Err(_) => {
                return Err(JoplinReaderError::FileReadError {
                    message: "Failed to open file".to_string(),
                })
            }
        };
        let reader = BufReader::new(file);

        let mut id: Option<String> = None;
        let mut parent_id: Option<String> = None;
        let mut type_: Option<JoplinItemType> = None;
        let mut encryption_cipher_text: Option<String> = None;
        let mut encryption_applied: Option<i8> = None;
        let mut updated_time: Option<NaiveDateTime> = None;

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
                    "parent_id" => parent_id = Some(value.to_string().trim().to_string()),
                    "type_" => {
                        if let Ok(t) = value.to_string().trim().parse::<i32>() {
                            type_ = Some(JoplinItemType::from(t))
                        } else {
                            return Err(JoplinReaderError::FileReadError {
                                message: "Invalid value specified for `type_`".to_string(),
                            });
                        }
                    }
                    "encryption_applied" => {
                        if let Ok(ea) = value.to_string().trim().parse::<i8>() {
                            encryption_applied = Some(ea)
                        } else {
                            return Err(JoplinReaderError::FileReadError {
                                message: "Invalid value specified for `encryption_applied`"
                                    .to_string(),
                            });
                        }
                    }
                    "encryption_cipher_text" => {
                        encryption_cipher_text = Some(value.to_string().trim().to_string())
                    }
                    "updated_time" => {
                        let ut = value.to_string().trim().to_string();
                        updated_time =
                            match NaiveDateTime::parse_from_str(&ut, "%Y-%m-%dT%H:%M:%S%.fZ") {
                                Ok(ut) => Some(ut),
                                Err(_) => None,
                            }
                    }
                    _ => { /*println!("Unsupported key: {}", key);*/ }
                };
            }
        }

        // Mandatory attributes:
        if let None = id {
            return Err(JoplinReaderError::FileReadError {
                message: "No `id` specified in note".to_string(),
            });
        }
        if let None = encryption_applied {
            return Err(JoplinReaderError::FileReadError {
                message: "No `encryption_applied` attribute specified in note".to_string(),
            });
        }
        let encryption_applied = encryption_applied.unwrap();
        let encryption_applied = match encryption_applied {
            1 => true,
            _ => false,
        };
        let encryption_key_id = match encryption_applied {
            true => match NoteInfo::parse_encrypted_header(
                encryption_cipher_text.clone().unwrap().chars(),
            ) {
                Ok(header) => Some(header.master_key_id),
                Err(_) => {
                    return Err(JoplinReaderError::FileReadError {
                        message: "Failed to read the encryption header".to_string(),
                    });
                }
            },
            _ => None,
        };

        Ok(NoteInfo {
            path: note_path.to_path_buf(),
            id: id.unwrap(),
            type_: type_.unwrap(),
            encryption_applied,
            parent_id,
            encryption_key_id,
            updated_time,
            read_time: None,
            content: NoteProperties::default(),
        })
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn is_encrypted(&self) -> bool {
        self.encryption_applied
    }

    pub fn get_type_(&self) -> &JoplinItemType {
        &self.type_
    }

    pub fn get_parent_id(&self) -> Option<&str> {
        match &self.parent_id {
            Some(parent_id) => Some(&parent_id),
            None => None,
        }
    }

    pub fn get_encryption_key_id(&self) -> Option<&str> {
        match &self.encryption_key_id {
            Some(encryption_key_id) => Some(&encryption_key_id),
            None => None,
        }
    }

    /// Parses the [`JoplinEncryptionHeader`].
    /// Spec: https://joplinapp.org/spec/e2ee/
    fn parse_encrypted_header(
        mut chars: Chars<'_>,
    ) -> Result<JoplinEncryptionHeader, JoplinReaderError> {
        // Header (3 chars): Always 'JED'
        let mut identifier = String::from("");
        for _ in 0..3 {
            if let Some(v) = chars.next() {
                identifier.push(v);
            }
        }
        if identifier.is_empty() || identifier.len() != 3 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Header has invalid size".to_string(),
            });
        }
        if identifier != "JED" {
            return Err(JoplinReaderError::DecryptionError {
                message: "Identifier is not 'JED'".to_string(),
            });
        }
        // Version number (2 chars)
        let mut version = String::from("");
        for _ in 0..2 {
            if let Some(v) = chars.next() {
                version.push(v);
            }
        }
        if version.is_empty() || version.len() != 2 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Header has invalid size".to_string(),
            });
        }
        let version = match u8::from_str_radix(&version, 16) {
            Ok(v) => v,
            Err(_) => {
                return Err(JoplinReaderError::DecryptionError {
                    message: "Version is not a number".to_string(),
                });
            }
        };
        if version != 1 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Invalid version. Needs to be '01'".to_string(),
            });
        }
        // Length (6 chars)
        let mut length = String::from("");
        for _ in 0..6 {
            if let Some(v) = chars.next() {
                length.push(v);
            }
        }
        if length.is_empty() || length.len() != 6 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Header has invalid size".to_string(),
            });
        }
        let length = match u32::from_str_radix(&length, 16) {
            Ok(v) => v,
            Err(_) => {
                return Err(JoplinReaderError::DecryptionError {
                    message: "Length is not a number".to_string(),
                });
            }
        };
        if length != 34 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Expected length 34: Method + master key id".to_string(),
            });
        }
        // Encryption Method (2 chars)
        let mut encryption_method = String::from("");
        for _ in 0..2 {
            if let Some(v) = chars.next() {
                encryption_method.push(v);
            }
        }
        if encryption_method.is_empty() || encryption_method.len() != 2 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Header has invalid size".to_string(),
            });
        }
        let encryption_method = match u8::from_str_radix(&encryption_method, 16) {
            Ok(v) => JoplinEncryptionMethod::from(v),
            Err(_) => {
                return Err(JoplinReaderError::DecryptionError {
                    message: "Encryption Method is not a number".to_string(),
                });
            }
        };
        if encryption_method == JoplinEncryptionMethod::MethodUndefined {
            return Err(JoplinReaderError::DecryptionError {
                message: "Unknown decryption method".to_string(),
            });
        }
        // Master key ID (32 chars)
        let mut master_key_id = String::from("");
        for _ in 0..32 {
            if let Some(v) = chars.next() {
                master_key_id.push(v);
            }
        }
        if master_key_id.is_empty() || master_key_id.len() != 32 {
            return Err(JoplinReaderError::DecryptionError {
                message: "Header has invalid size".to_string(),
            });
        }

        Ok(JoplinEncryptionHeader {
            version,
            length,
            encryption_method,
            master_key_id,
        })
    }

    fn clean_encoded_ascii(text: String) -> String {
        let re = Regex::new(r"%([0-9a-fA-F]{2})").unwrap();

        let text = re.replace_all(&text, |caps: &Captures| {
            let value = caps[0].strip_prefix("%").unwrap();
            let value = u8::from_str_radix(value, 16).unwrap();
            let value = value as char;
            value.to_string()
        });

        text.to_string()
    }

    fn clean_encoded_unicode(text: String) -> String {
        let re = Regex::new(r"%u([0-9a-fA-F]{4})").unwrap();

        let text = re.replace_all(&text, |_caps: &Captures| {
            // We should do this properly, but it's UTF-16 which gets inserted
            // by my kindle and I do not really need these values.
            // The text is more important
            // let value = caps[0].strip_prefix("%u").unwrap();
            // let value = u32::from_str_radix(value, 16).unwrap();
            // let value = char::try_from(value).unwrap();
            "".to_string()
        });

        text.to_string()
    }

    /// Decrypts all chunks one after another and returns the whole `String`
    /// or breaks on an error.
    fn decrypt(mut chars: Chars<'_>, encryption_key: &str) -> Result<String, JoplinReaderError> {
        let mut _chunks_read: u32 = 0;
        let mut _bytes_read: u32 = 0;
        let mut body = String::from("");
        loop {
            let mut length = String::from("");
            for _ in 0..6 {
                if let Some(v) = chars.next() {
                    length.push(v);
                }
            }
            if length.is_empty() || length.len() != 6 {
                break;
            }
            let length = match u32::from_str_radix(&length, 16) {
                Ok(v) => v,
                Err(_) => {
                    return Err(JoplinReaderError::DecryptionError {
                        message: "Length is not a number".to_string(),
                    });
                }
            };

            let mut data = String::from("");
            for _ in 0..length {
                if let Some(v) = chars.next() {
                    data.push(v);
                }
            }
            if data.is_empty() || data.len() != length as usize {
                return Err(JoplinReaderError::UnexpectedEndOfNote);
            }
            match decrypt_raw(data, encryption_key.to_string()) {
                Ok(data) => {
                    let data = match String::from_utf8(data) {
                        Ok(data) => data,
                        Err(_) => {
                            return Err(JoplinReaderError::DecryptionError {
                                message: "Message did not contain valid ascii".to_string(),
                            })
                        }
                    };
                    let data = NoteInfo::clean_encoded_ascii(data);
                    let data = NoteInfo::clean_encoded_unicode(data);
                    body.push_str(&data)
                }
                Err(_) => {
                    return Err(JoplinReaderError::DecryptionError {
                        message: "Error decrypting".to_string(),
                    })
                }
            };

            _bytes_read += length;
            _chunks_read += 1;
        }
        let body = percent_decode_str(&body).decode_utf8_lossy();
        Ok(body.to_string())
    }

    /// Reads the content into the `content` attribute of `self`
    fn read_content(&mut self, encryption_key: Option<&str>) -> Result<(), JoplinReaderError> {
        let content = match self.is_encrypted() {
            true => self.read_decrypted(encryption_key),
            false => self.read_unencrypted(),
        };

        match content {
            Ok(content) => {
                self.content = NoteProperties::from(content);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Read an unencrypted item and return a [`std::collection::HashMap`]
    /// with the key value pairs
    fn read_unencrypted(&self) -> Result<HashMap<String, String>, JoplinReaderError> {
        let file = match fs::File::open(self.path.clone()) {
            Ok(file) => file,
            Err(_) => {
                return Err(JoplinReaderError::FileReadError {
                    message: "Failed to open file".to_string(),
                })
            }
        };
        let reader = BufReader::new(file);
        let mut text: Vec<String> = Vec::new();
        // Reverse the order of the lines
        for line in reader.lines() {
            let line = line.unwrap();
            text.insert(0, line);
        }

        NoteInfo::deserialize(text.iter())
    }

    /// Read and decrypt an encrypted item and return a
    /// [`std::collection::HashMap`] with the key value pairs
    fn read_decrypted(
        &self,
        encryption_key: Option<&str>,
    ) -> Result<HashMap<String, String>, JoplinReaderError> {
        let encryption_key = match encryption_key {
            Some(ek) => ek,
            _ => {
                return Err(JoplinReaderError::NoEncryptionKey);
            }
        };

        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(_) => {
                return Err(JoplinReaderError::FileReadError {
                    message: "Failed to open file".to_string(),
                })
            }
        };
        let mut reader = BufReader::new(file);
        let content = match NoteInfo::parse_encrypted_file(&mut reader) {
            Ok(content) => content,
            Err(e) => return Err(e),
        };

        if let Some(text) = content.get(&"encryption_cipher_text".to_string()) {
            if !text.is_ascii() {
                return Err(JoplinReaderError::DecryptionError {
                    message: "Encrypted text is not ascii".to_string(),
                });
            }
            let mut chars = text.chars();
            // Skip header
            for _ in 0..HEADER_SIZE {
                chars.next();
            }
            let plaintext = match NoteInfo::decrypt(chars, encryption_key) {
                Ok(plaintext) => plaintext,
                Err(_e) => {
                    println!("{:?}", _e);
                    return Err(JoplinReaderError::DecryptionError {
                        message: "Failed to decrypt SJCL chunks".to_string(),
                    });
                }
            };

            NoteInfo::deserialize(plaintext.lines())
        } else {
            Err(JoplinReaderError::NoEncryptionText)
        }
    }

    /// The content is only read when not existant or after a certain amount of
    /// time has passed. That is written into the attributes of `self` and
    /// returned directly from the body.
    pub fn read(&mut self, encryption_key: Option<&str>) -> Result<&str, JoplinReaderError> {
        let reading = match self.read_time {
            None => self.read_content(encryption_key),
            Some(t) => {
                let since_last_refresh = SystemTime::now()
                    .duration_since(t)
                    .expect("Time went backwards!")
                    .as_secs();
                if since_last_refresh >= REFRESH_INTERVAL {
                    self.read_content(encryption_key)
                } else {
                    Ok(())
                }
            }
        };

        match reading {
            Ok(_) => match &self.content.body {
                Some(body) => Ok(body),
                None => Err(JoplinReaderError::NoText),
            },
            Err(e) => Err(e),
        }
    }
}
