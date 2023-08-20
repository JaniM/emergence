use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::Hash;
use std::rc::Rc;

use dioxus::prelude::{use_context, use_context_provider, ScopeState};
use dioxus_signals::*;

use super::notes::{NoteBuilder, NoteData, NoteSearch};
use super::search::SearchWorker;
use super::subjects::{Subject, SubjectId};
use super::ConnectionType;
use super::{
    notes::{Note, NoteId},
    Store,
};

#[derive(Debug, Clone)]
struct Cache<K, V> {
    size: usize,
    map: HashMap<K, V, ahash::RandomState>,
    drop_order: VecDeque<K>,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn new(size: usize) -> Self {
        Self {
            size,
            map: HashMap::default(),
            drop_order: VecDeque::new(),
        }
    }

    fn get(&self, key: &K) -> Option<V> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: K, value: V) {
        match self.map.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                self.drop_order.retain(|k| k != &key);
            }
            Entry::Vacant(entry) => {
                entry.insert(value);
                if self.drop_order.len() >= self.size {
                    self.remove_oldest();
                }
            }
        }

        self.drop_order.push_back(key);
    }

    fn get_or_insert_with<F>(&mut self, key: K, f: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(value) = self.get(&key) {
            return value;
        }

        let value = f();
        self.insert(key, value.clone());
        value
    }

    fn invalidate_key(&mut self, key: &K) -> Option<V> {
        self.drop_order.retain(|k| k != key);
        self.map.remove(key)
    }

    fn remove_oldest(&mut self) -> Option<V> {
        if let Some(key) = self.drop_order.pop_front() {
            self.map.remove(&key)
        } else {
            None
        }
    }

    fn clear(&mut self) {
        self.map.clear();
        self.drop_order.clear();
    }
}

type Notes = Signal<Vec<Note>>;
type Subjects = Signal<Rc<BTreeMap<SubjectId, Subject>>>;
type SubjectChildren = Signal<BTreeMap<SubjectId, Vec<SubjectId>>>;

/// Layer provides an abstraction layer over the store to provide a
/// consistent interface for the rest of the application.
pub struct Layer {
    store: Rc<Store>,
    note_cache: Cache<NoteId, Note>,
    query_cache: Cache<NoteSearch, Vec<NoteId>>,
    event_count: usize,

    query: NoteSearch,
    notes: Notes,
    subjects: Subjects,
    subject_children: SubjectChildren,
}

impl Layer {
    pub fn new(
        store: Rc<Store>,
        notes: Notes,
        subjects: Subjects,
        subject_children: SubjectChildren,
    ) -> Self {
        Self {
            store,
            note_cache: Cache::new(1024),
            query_cache: Cache::new(16),
            event_count: 0,
            query: Default::default(),
            notes,
            subjects,
            subject_children,
        }
    }

    fn update_notes(&mut self) {
        let search = self.query.clone();
        let notes = self
            .get_note_ids_for_search(search)
            .into_iter()
            .map(|id| self.get_note_by_id(id))
            .collect();

        *self.notes.write() = notes;
    }

    pub fn create_note(&mut self, builder: NoteBuilder) {
        self.store.add_note(builder).unwrap();
        self.event();
        self.invalidate_note_queries();
        self.update_notes();
    }

    pub fn delete_note_by_id(&mut self, id: NoteId) {
        self.store.delete_note(id).unwrap();
        self.event();
        self.invalidate_note(id);
        self.invalidate_note_queries();
        self.update_notes();
    }

    pub fn edit_note_with(&mut self, id: NoteId, f: impl FnOnce(&mut NoteData)) {
        let old_note = self.store.get_note(id).unwrap();
        let mut note = (&*old_note).clone();
        f(&mut note);
        self.store.update_note(Rc::new(note)).unwrap();

        self.event();
        self.invalidate_note(id);
        self.invalidate_note_queries();
        self.update_notes();
    }

    fn invalidate_note_queries(&mut self) {
        self.query_cache.clear();
    }

    fn invalidate_note(&mut self, id: NoteId) {
        self.note_cache.invalidate_key(&id);
    }

    pub fn search(&self) -> SearchWorker {
        self.store.search.clone()
    }

    pub fn set_search(&mut self, search: NoteSearch) {
        if self.query == search {
            return;
        }

        self.query = search;
        self.update_notes();
    }

    fn get_note_ids_for_search(&mut self, search: NoteSearch) -> Vec<NoteId> {
        self.query_cache
            .get_or_insert_with(search.clone(), || self.store.find_notes(search).unwrap())
    }

    fn get_note_by_id(&mut self, id: NoteId) -> Note {
        self.note_cache
            .get_or_insert_with(id, || self.store.get_note(id).unwrap())
    }

    fn event(&mut self) {
        self.event_count += 1;
    }

    pub fn event_count(&self) -> usize {
        self.event_count
    }

    fn update_subjects(&mut self) {
        let subject_list = self.store.get_subjects().unwrap();
        let map = subject_list
            .iter()
            .map(|s| (s.id, s.clone()))
            .collect::<BTreeMap<_, _>>();
        *self.subjects.write() = Rc::new(map);

        *self.subject_children.write() = subject_list
            .iter()
            .map(|subject| {
                (
                    subject.id,
                    self.store.get_subject_children(subject.id).unwrap(),
                )
            })
            .collect();
    }

    pub fn create_subject(&mut self, name: String) -> Subject {
        let subject = self.store.add_subject(name).unwrap();
        self.update_subjects();
        subject
    }

    pub fn set_subject_parent(&mut self, subject: SubjectId, parent: Option<SubjectId>) {
        self.store.set_subject_parent(subject, parent).unwrap();
        self.update_subjects();
    }
}

pub fn use_layer_provider(cx: &ScopeState, conn: ConnectionType) -> Signal<Layer> {
    let notes = *use_context_provider(cx, Default::default);
    let subjects = *use_context_provider(cx, Default::default);
    let subject_children = *use_context_provider(cx, Default::default);
    *use_context_provider(cx, || {
        let store = Store::new(conn);
        let mut layer = Layer::new(Rc::new(store), notes, subjects, subject_children);
        layer.update_subjects();
        layer.update_notes();
        Signal::new(layer)
    })
}

pub fn use_layer(cx: &ScopeState) -> Signal<Layer> {
    *use_context(cx).expect("Layer should be provided")
}

pub fn use_notes(cx: &ScopeState) -> Notes {
    *use_context(cx).expect("Layer should be provided")
}

pub fn use_subjects(cx: &ScopeState) -> Subjects {
    *use_context(cx).expect("Layer should be provided")
}

pub fn use_subject_children(cx: &ScopeState) -> SubjectChildren {
    *use_context(cx).expect("Layer should be provided")
}
