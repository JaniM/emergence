
use chrono::prelude::*;
use std::rc::Rc;

use super::subjects::SubjectId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NoteId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoteData {
    pub id: NoteId,
    pub text: String,
    pub subjects: Vec<SubjectId>,
    pub created_at: DateTime<Utc>,
}

pub type Note = Rc<NoteData>;