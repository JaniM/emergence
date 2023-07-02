mod subjects;

use chrono::prelude::*;
use std::{collections::HashMap, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoteData {
    pub id: NodeId,
    pub text: String,
    pub created_at: DateTime<Local>,
}

pub type Note = Rc<NoteData>;

pub struct Store {
    notes: HashMap<NodeId, Note>,
    subjects: subjects::Subjects,
    id_counter: u64,
}

impl Store {
    pub fn new() -> Self {
        let mut s = Self {
            notes: HashMap::new(),
            subjects: subjects::Subjects::new(),
            id_counter: 0,
        };
        s.add_test_nodes();
        s
    }

    pub fn subjects(&self) -> &subjects::Subjects {
        &self.subjects
    }

    pub fn subjects_mut(&mut self) -> &mut subjects::Subjects {
        &mut self.subjects
    }

    pub fn add_node(&mut self, text: String) -> NodeId {
        self.id_counter += 1;
        let id = NodeId(self.id_counter);
        self.notes.insert(
            id,
            Rc::new(NoteData {
                id,
                text,
                created_at: Local::now(),
            }),
        );
        id
    }

    pub fn get_notes(&self) -> Vec<Note> {
        self.notes.values().cloned().collect()
    }

    pub fn add_test_nodes(&mut self) {
        self.notes.extend(
            vec![
                NoteData {
                    id: NodeId(self.id_counter + 1),
                    text: "Hello, world!".to_string(),
                    created_at: Local::now() - chrono::Duration::days(1),
                },
                NoteData {
                    id: NodeId(self.id_counter + 2),
                    text: "Old message".to_string(),
                    created_at: Local::now() - chrono::Duration::days(2),
                },
            ]
            .into_iter()
            .map(Rc::new)
            .map(|n| (n.id, n)),
        );
        self.id_counter += 2;
    }
}
