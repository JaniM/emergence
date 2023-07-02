use chrono::prelude::*;
use std::{collections::HashMap, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SubjectId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectData {
    pub id: SubjectId,
    pub text: String,
    pub created_at: DateTime<Local>,
}

pub type Subject = Rc<SubjectData>;

pub struct Subjects {
    subjects: HashMap<SubjectId, Subject>,
    subjects_by_name: HashMap<String, SubjectId>,
    id_counter: u64,
}

impl Subjects {
    pub fn new() -> Self {
        let mut s = Self {
            subjects: HashMap::new(),
            subjects_by_name: HashMap::new(),
            id_counter: 0,
        };
        s.add_test_nodes();
        s
    }

    fn add(&mut self, text: String) -> SubjectId {
        self.id_counter += 1;
        let id = SubjectId(self.id_counter);
        self.subjects.insert(
            id,
            Rc::new(SubjectData {
                id,
                text: text.clone(),
                created_at: Local::now(),
            }),
        );
        self.subjects_by_name.insert(text, id);
        id
    }

    pub fn get_or_add(&mut self, text: &str) -> SubjectId {
        if let Some(id) = self.subjects_by_name.get(text) {
            *id
        } else {
            self.add(text.to_owned())
        }
    }

    pub fn find(&self, search: &str) -> Vec<Subject> {
        self.subjects
            .values()
            .filter(|s| s.text.contains(search))
            .cloned()
            .collect()
    }

    pub fn get(&self, id: SubjectId) -> Subject {
        self.subjects.get(&id).unwrap().clone()
    }

    fn add_test_nodes(&mut self) {
        self.subjects.extend(
            vec![
                SubjectData {
                    id: SubjectId(self.id_counter + 1),
                    text: "Test".to_string(),
                    created_at: Local::now() - chrono::Duration::days(1),
                },
                SubjectData {
                    id: SubjectId(self.id_counter + 2),
                    text: "groceries".to_string(),
                    created_at: Local::now() - chrono::Duration::days(1),
                },
                SubjectData {
                    id: SubjectId(self.id_counter + 3),
                    text: "work project".to_string(),
                    created_at: Local::now() - chrono::Duration::days(1),
                },
                SubjectData {
                    id: SubjectId(self.id_counter + 4),
                    text: "taking notes".to_string(),
                    created_at: Local::now() - chrono::Duration::days(1),
                },
                SubjectData {
                    id: SubjectId(self.id_counter + 5),
                    text: "Hello, world!".to_string(),
                    created_at: Local::now() - chrono::Duration::days(1),
                },
            ]
            .into_iter()
            .map(|n| (n.id, Rc::new(n))),
        );
        self.id_counter += 5;
    }
}
