use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SubjectId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectData {
    pub id: SubjectId,
    pub name: String,
}

pub type Subject = Rc<SubjectData>;