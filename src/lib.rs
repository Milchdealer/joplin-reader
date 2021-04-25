//! # joplin-reader
//! Read-only library for joplin data folders.
//!
//! ## Usage
//!
//! Decrypt a file loaded into a string:
//! ```rust
//! use joplin_reader::notebook::JoplinNotebook;
//! 
//! # fn main() -> Result<(), SjclError> {
//! let joplin_folder = "./Joplin";
//! let passwords = "3336eb7a2472d9ae4a690a978fa8a46f,plaintext_password";
//! let notebooks = JoplinNotebook::new(joplin_folder, passwords)?;
//! println!("{:?}", notebooks.read_note("9a20a9e4d336de70cb6d22a58a3e673c"));
//! # Ok(())
//! # }
//! ```
//!

pub mod key;
pub mod note;
pub mod notebook;

use thiserror::Error;
#[derive(Error, Debug)]
pub enum JoplinReaderError {
    #[error("Failed to read joplin folder")]
    FolderReadError,
    #[error("Failed to read file: {message:?}")]
    FileReadError { message: String },
    #[error("Failed to decrypt: {message:?}")]
    DecryptionError { message: String },
    #[error("Note `{note_id:?}` not found")]
    NoteIdNotFound { note_id: String },
    #[error("No note with text `{search_text:?}` found")]
    NoteNotFound { search_text: String },
    #[error("Invalid format: {message:?}")]
    InvalidFormat { message: String },
    #[error("Encryption key `{key:?}` not found")]
    NoEncryptionKey { key: String },
    #[error("No encryption text provided")]
    NoEncryptionText,
    #[error("No text found")]
    NoText,
    #[error("Unexpected end of note")]
    UnexpectedEndOfNote,
    #[error("Unknown encryption method")]
    UnknownEncryptionMethod,
    #[error("Key id mismatch")]
    KeyIdMismatch,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
