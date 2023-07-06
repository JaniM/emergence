use crate::data::notes::Note;
use crate::use_store;
use dioxus::prelude::*;
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

use super::subjects::{Subject, SubjectId};

pub(super) struct NoteQuerySource {
    pub note_data: Vec<Note>,
    pub update_callback: Arc<dyn Fn()>,
    pub alive: bool,
}

#[derive(Debug)]
pub struct NoteQuery {
    source: Rc<RefCell<NoteQuerySource>>,
}

impl Debug for NoteQuerySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoteQuerySource")
            .field("note_data", &self.note_data.len())
            .field("alive", &self.alive)
            .finish()
    }
}

pub fn use_note_query<'a, 'b>(cx: &'a ScopeState) -> &'a NoteQuery {
    let store = use_store(cx).read();
    cx.use_hook(|| {
        let update_callback = cx.schedule_update();
        let source = Rc::new(RefCell::new(NoteQuerySource {
            note_data: Vec::new(),
            update_callback,
            alive: true,
        }));
        store.add_source(source.clone());
        NoteQuery { source }
    })
}

impl NoteQuery {
    pub fn notes(&self) -> Ref<Vec<Note>> {
        let source = self.source.borrow();
        Ref::map(source, |s| &s.note_data)
    }
}

impl Drop for NoteQuery {
    fn drop(&mut self) {
        self.source.borrow_mut().alive = false;
    }
}

pub(super) struct SubjectQuerySource {
    pub subjects: HashMap<SubjectId, Subject>,
    pub update_callback: Vec<Arc<dyn Fn()>>,
}

pub struct SubjectQuery {
    source: Rc<RefCell<SubjectQuerySource>>,
    update_callback: Arc<dyn Fn()>,
}

impl Debug for SubjectQuerySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubjectQuerySource")
            .field("subjects", &self.subjects.len())
            .finish()
    }
}

impl Debug for SubjectQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubjectQuery")
            .field("source", &self.source)
            .finish()
    }
}

pub fn use_subject_query<'a, 'b>(cx: &'a ScopeState) -> &'a SubjectQuery {
    let store = use_store(cx).read();
    cx.use_hook(|| {
        let update_callback = cx.schedule_update();
        let source = store.subject_source.clone();
        source
            .borrow_mut()
            .update_callback
            .push(update_callback.clone());
        SubjectQuery {
            source,
            update_callback,
        }
    })
}

impl SubjectQuery {
    pub fn subjects(&self) -> Ref<HashMap<SubjectId, Subject>> {
        let source = self.source.borrow();
        Ref::map(source, |s| &s.subjects)
    }
}

impl Drop for SubjectQuery {
    fn drop(&mut self) {
        let mut source = self.source.borrow_mut();
        source
            .update_callback
            .retain(|cb| Arc::ptr_eq(cb, &self.update_callback));
    }
}
