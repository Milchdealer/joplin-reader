pub mod key;
pub mod note;
pub mod notebook;

use snafu::Snafu;
#[derive(Debug, Snafu)]
pub enum JoplinReaderError {
    #[snafu(display("Failed to read joplin folder"))]
    FolderReadError,
    #[snafu(display("Failed to read file: {}", message))]
    FileReadError { message: String },
    #[snafu(display("Failed to decrypt: {}", message))]
    DecryptionError { message: String },
    #[snafu(display("Note `{}` not found", note_id))]
    NoteIdNotFound { note_id: String },
    #[snafu(display("No note with text `{}` found", search_text))]
    NoteNotFound { search_text: String },
    #[snafu(display("Invalid format: {}", message))]
    InvalidFormat { message: String },
    #[snafu(display("No encryption key found"))]
    NoEncryptionKey,
    #[snafu(display("No encryption text provided"))]
    NoEncryptionText,
    #[snafu(display("No text found"))]
    NoText,
    #[snafu(display("Unexpected end of note"))]
    UnexpectedEndOfNote,
    #[snafu(display("Unknown encryption method"))]
    UnknownEncryptionMethod,
    #[snafu(display("Key id mismatch"))]
    KeyIdMismatch,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
