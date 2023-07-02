pub mod subjects;
pub mod notes;

pub struct Store {
    pub notes: notes::Notes,
    pub subjects: subjects::Subjects,
}

impl Store {
    pub fn new() -> Self {
        Self {
            notes: notes::Notes::new(),
            subjects: subjects::Subjects::new(),
        }
    }
}
