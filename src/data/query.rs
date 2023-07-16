use crate::data::notes::Note;
use dioxus::prelude::*;
use std::cell::{Cell, Ref, RefCell};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{instrument, trace};

use super::notes::NoteSearch;
use super::subjects::{Subject, SubjectId};
use super::Store;

pub fn use_store(cx: &ScopeState) -> &UseSharedState<super::Store> {
    use_shared_state(cx).expect("Store context not set")
}

pub(super) struct NoteQuerySource {
    pub note_data: Vec<Note>,
    pub search: NoteSearch,
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

pub fn use_note_query<'a, 'b>(cx: &'a ScopeState, search: NoteSearch) -> &'a NoteQuery {
    let store = use_store(cx).read();
    let query = cx.use_hook(|| {
        let update_callback = cx.schedule_update();
        let source = Rc::new(RefCell::new(NoteQuerySource {
            note_data: Vec::new(),
            search,
            update_callback,
            alive: true,
        }));
        store.add_note_source(source.clone());
        NoteQuery { source }
    });

    if query.source.borrow().search != search {
        query.source.borrow_mut().search = search;
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
    pub subjects: BTreeMap<SubjectId, Subject>,
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
    pub fn subjects(&self) -> Ref<BTreeMap<SubjectId, Subject>> {
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

pub struct StoreEventSource {
    update_callback: Arc<dyn Fn()>,
    pub counter: Cell<usize>,
    pub(super) alive: bool,
}

pub struct StoreEventQuery {
    source: Rc<RefCell<StoreEventSource>>,
}

pub fn use_store_event_query<'a>(cx: &'a ScopeState) -> &'a StoreEventQuery {
    let store = use_store(cx).read();
    cx.use_hook(|| {
        let update_callback = cx.schedule_update();
        let source = StoreEventSource {
            update_callback,
            counter: Cell::new(0),
            alive: true,
        };
        let source = Rc::new(RefCell::new(source));
        store.update_targets.borrow_mut().push(source.clone());
        StoreEventQuery { source }
    })
}

impl StoreEventQuery {
    pub fn count(&self) -> usize {
        let source = self.source.borrow();
        source.counter.get()
    }
}

impl Drop for StoreEventQuery {
    fn drop(&mut self) {
        self.source.borrow_mut().alive = false;
    }
}

impl Store {
    #[instrument(skip_all)]
    fn add_note_source(&self, source: Rc<RefCell<NoteQuerySource>>) {
        trace!("Adding note source");
        let subject = source.borrow().search;
        source.borrow_mut().note_data = self.get_notes(subject).unwrap();
        self.note_sources.borrow_mut().push(source);
    }

    #[instrument(skip(self))]
    pub(super) fn update_note_sources(&self) {
        let mut events = self.update_targets.borrow_mut();
        events.retain(|s| s.borrow().alive);
        for event in events.iter() {
            let event = event.borrow_mut();
            event.counter.set(event.counter.get() + 1);
            (event.update_callback)();
        }

        let mut cache = BTreeMap::<NoteSearch, Vec<Note>>::new();
        let mut sources = self.note_sources.borrow_mut();
        sources.retain(|s| s.borrow().alive);
        for source in sources.iter() {
            let mut source = source.borrow_mut();
            let notes = if let Some(notes) = cache.get(&source.search) {
                notes.clone()
            } else {
                let notes = self.get_notes(source.search).unwrap();
                cache.insert(source.search.clone(), notes.clone());
                notes
            };
            source.note_data = notes;
            (source.update_callback)();
        }
    }

    #[instrument(skip(self))]
    pub(super) fn update_subject_sources(&self) {
        let subjects = self.get_subjects().unwrap();
        let subjects = subjects.into_iter().map(|s| (s.id, s)).collect();
        let mut source = self.subject_source.borrow_mut();
        source.subjects = subjects;
        for callback in source.update_callback.iter() {
            callback();
        }
    }
}
