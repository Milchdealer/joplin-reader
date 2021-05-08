use crate::key::{load_master_key, MasterKey};
use crate::note::NoteInfo;
use crate::JoplinReaderError;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

/// Container `struct` which contains the references (and contents) to the
/// [`NoteInfo`]s as well as the [`MasterKey`]s.
#[derive(Debug, Serialize)]
pub struct JoplinNotebook {
    notes: HashMap<String, NoteInfo>,
    master_keys: HashMap<String, MasterKey>,
}

impl JoplinNotebook {
    /// Read a Joplin data folder. `passwords` need to be passed as comma-separated
    /// key-value (master_key_id,passphrase) pairs.
    pub fn new<'a, P: AsRef<Path>, I>(
        joplin_folder: P,
        passwords: I,
    ) -> Result<JoplinNotebook, JoplinReaderError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut master_keys: HashMap<String, MasterKey> = HashMap::new();
        for password in passwords.into_iter() {
            let mut iter = password.splitn(2, ",");
            let master_key_id = iter.next();
            let key = iter.next();
            if let (Some(master_key_id), Some(key)) = (master_key_id, key) {
                let mut key_filename = master_key_id.to_string();
                key_filename.push_str(".md");
                let key_path = joplin_folder.as_ref().join(key_filename);
                if key_path.is_file() {
                    let mk = load_master_key(&key_path, master_key_id.to_string(), key.to_string());
                    if let Ok(mk) = mk {
                        master_keys.insert(master_key_id.to_string(), mk);
                    }
                } else {
                    return Err(JoplinReaderError::NoEncryptionKey { key: format!("{:?}", key_path)});
                }
            }
        }

        let note_paths = match fs::read_dir(joplin_folder) {
            Ok(d) => d,
            Err(_) => return Err(JoplinReaderError::FolderReadError),
        };
        let mut notes: HashMap<String, NoteInfo> = HashMap::new();
        for note_path in note_paths {
            let note_path = note_path.expect("Unable to read path").path();
            let note_path = Path::new(&note_path);

            if note_path.is_file() {
                let item_id = note_path.file_stem().unwrap_or_default();
                if !master_keys.contains_key(item_id.to_str().unwrap_or_default()) {
                    if let Ok(note) = NoteInfo::new(note_path) {
                        match item_id.to_str() {
                            Some(note_id) => {
                                notes.insert(note_id.to_string(), note);
                            }
                            None => {}
                        }
                    }
                }
            }
        }

        Ok(JoplinNotebook { notes, master_keys })
    }

    /// Returns the content of a note.
    pub fn read_note(&mut self, note_id: &str) -> Result<&str, JoplinReaderError> {
        let note = match self.notes.get_mut(note_id) {
            Some(note) => note,
            None => {
                return Err(JoplinReaderError::NoteIdNotFound {
                    note_id: note_id.to_string(),
                })
            }
        };
        let mut encryption_key: Option<&str> = None;
        if note.is_encrypted() {
            let master_key_id = match note.get_encryption_key_id() {
                Some(key_id) => key_id.to_string(),
                None => {
                    return Err(JoplinReaderError::NoEncryptionKey {key: format!("{:?}", note.get_encryption_key_id())});
                }
            };

            encryption_key = match self.master_keys.get(&master_key_id) {
                Some(master_key) => Some(master_key.as_str()),
                None => {
                    return Err(JoplinReaderError::NoEncryptionKey {key: format!("{:?}", master_key_id)});
                }
            }
        }

        note.read(encryption_key)
    }

    /// Returns a [`NoteInfo`]
    pub fn get_note(&self, note_id: &str) -> Result<&NoteInfo, JoplinReaderError> {
        match self.notes.get(note_id) {
            Some(note) => Ok(note),
            None => Err(JoplinReaderError::NoteIdNotFound {
                note_id: note_id.to_string(),
            }),
        }
    }
}
