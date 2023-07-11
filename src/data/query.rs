use crate::data::notes::Note;
use dioxus::prelude::*;
use tracing::{instrument, trace};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

use super::Store;
use super::subjects::{Subject, SubjectId};

pub fn use_store(cx: &ScopeState) -> &UseSharedState<super::Store> {
    use_shared_state(cx).expect("Store context not set")
}

pub(super) struct NoteQuerySource {
    pub note_data: Vec<Note>,
    pub subject: Option<SubjectId>,
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

pub fn use_note_query<'a, 'b>(cx: &'a ScopeState, subject: Option<SubjectId>) -> &'a NoteQuery {
    let store = use_store(cx).read();
    let query = cx.use_hook(|| {
        let update_callback = cx.schedule_update();
        let source = Rc::new(RefCell::new(NoteQuerySource {
            note_data: Vec::new(),
            subject,
            update_callback,
            alive: true,
        }));
        store.add_note_source(source.clone());
        NoteQuery { source }
    });

    if query.source.borrow().subject != subject {
        query.source.borrow_mut().subject = subject;
        // TODO: Use a finer-grained update
        store.update_note_sources();
    }

    query
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

impl Store {
    #[instrument(skip_all)]
    fn add_note_source(&self, source: Rc<RefCell<NoteQuerySource>>) {
        trace!("Adding note source");
        let subject = source.borrow().subject;
        source.borrow_mut().note_data = self.get_notes(subject).unwrap();
        self.note_sources.borrow_mut().push(source);
    }

    #[instrument(skip(self))]
    pub(super) fn update_note_sources(&self) {
        let mut sources = self.note_sources.borrow_mut();
        sources.retain(|s| s.borrow().alive);
        for source in sources.iter() {
            let mut source = source.borrow_mut();
            source.note_data = self.get_notes(source.subject).unwrap();
            (source.update_callback)();
        }
    }

    #[instrument(skip(self))]
    pub(super) fn update_subject_sources(&self) {
        let subjects = self.get_subjects().unwrap();
        let subjects = subjects
            .into_iter()
            .map(|s| (s.id, s))
            .collect::<HashMap<_, _>>();
        let mut source = self.subject_source.borrow_mut();
        source.subjects = subjects;
        for callback in source.update_callback.iter() {
            callback();
        }
    }
}