use std::rc::Rc;

use rusqlite::ToSql;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SubjectId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectData {
    pub id: SubjectId,
    pub name: String,
}

pub type Subject = Rc<SubjectData>;

impl ToSql for SubjectId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl std::fmt::Display for SubjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
