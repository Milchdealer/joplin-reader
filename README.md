# joplin-reader
Library which provides an interface to read [Joplin](https://joplinapp.org)
notes.

## Features
- [x] Read notes from folder
- [x] Decrypt encrypted notes
- [ ] Allow for search of notes

## Usage
```rust
use joplin_reader::notebook::JoplinNotebook;

let joplin_folder = "./Joplin";
let passwords = "3336eb7a2472d9ae4a690a978fa8a46f,plaintext_password";
let notebooks = JoplinNotebook::new(joplin_folder, passwords)?;

println!("{:?}", notebooks.read_note("9a20a9e4d336de70cb6d22a58a3e673c"));
```


