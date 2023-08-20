use std::path::PathBuf;

use super::{notes::NoteData, subjects::SubjectData, ConnectionType, Store};

#[derive(serde::Serialize, serde::Deserialize)]
struct SerializedStore {
    subjects: Vec<SubjectData>,
    notes: Vec<NoteData>,
}

pub fn export(db_path: PathBuf, export_path: PathBuf) {
    let store = Store::new(ConnectionType::File(db_path));
    let subjects = store
        .get_subjects()
        .unwrap()
        .into_iter()
        .map(|s| (*s).clone())
        .collect();
    let notes = store
        .get_all_notes()
        .unwrap()
        .into_iter()
        .map(|n| (*n).clone())
        .collect();

    let serialized = SerializedStore { subjects, notes };

    // write to file
    let file = std::fs::File::create(export_path).unwrap();
    serde_json::to_writer_pretty(file, &serialized).unwrap();
}

pub fn import(db_path: PathBuf, import_path: PathBuf) {
    // if db exists, confirm overwrite and delete
    if db_path.exists() {
        println!("Database already exists. Overwrite? (y/n)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        if input.trim() != "y" {
            println!("Aborting");
            return;
        }
        std::fs::remove_file(&db_path).unwrap();
    }

    let store = Store::new(ConnectionType::File(db_path));

    // read from file
    let file = std::fs::File::open(import_path).unwrap();
    let serialized: SerializedStore = serde_json::from_reader(file).unwrap();

    // add subjects
    for subject in serialized.subjects {
        store.import_subject(&subject).unwrap();
    }

    // add notes
    for note in serialized.notes {
        store.import_note(&note).unwrap();
    }
}
