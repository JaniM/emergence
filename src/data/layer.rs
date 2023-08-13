use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::Hash;
use std::rc::Rc;

use dioxus::prelude::{
    use_shared_state, use_shared_state_provider, 
    UseSharedState, ScopeState,
};

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

/// Layer provides an abstraction layer over the store to provide a
/// consistent interface for the rest of the application.
pub struct Layer {
    store: Rc<Store>,
    note_cache: RefCell<Cache<NoteId, Note>>,
    query_cache: RefCell<Cache<NoteSearch, Vec<NoteId>>>,
    event_count: usize,
}

impl Layer {
    pub fn new(store: Rc<Store>) -> Self {
        Self {
            store,
            note_cache: RefCell::new(Cache::new(1024)),
            query_cache: RefCell::new(Cache::new(16)),
            event_count: 0,
        }
    }

    pub fn create_note(&mut self, builder: NoteBuilder) -> Note {
        self.event();
        self.invalidate_note_queries();

        let note = self.store.add_note(builder).unwrap();
        note
    }

    pub fn delete_note_by_id(&mut self, id: NoteId) {
        self.event();
        self.invalidate_note(id);
        self.invalidate_note_queries();

        self.store.delete_note(id).unwrap();
    }

    pub fn edit_note_with(&mut self, id: NoteId, f: impl FnOnce(&mut NoteData)) {
        self.event();
        self.invalidate_note(id);
        self.invalidate_note_queries();

        let old_note = self.store.get_note(id).unwrap();
        let mut note = (&*old_note).clone();
        f(&mut note);
        self.store.update_note(Rc::new(note)).unwrap();
    }

    fn invalidate_note_queries(&mut self) {
        self.query_cache.borrow_mut().clear();
    }

    fn invalidate_note(&mut self, id: NoteId) {
        self.note_cache.borrow_mut().invalidate_key(&id);
    }

    pub fn search(&self) -> SearchWorker {
        self.store.search.clone()
    }

    pub fn get_notes_for_search(&self, search: NoteSearch) -> Vec<Note> {
        self.get_note_ids_for_search(search)
            .into_iter()
            .map(|id| self.get_note_by_id(id))
            .collect()
    }

    fn get_note_ids_for_search(&self, search: NoteSearch) -> Vec<NoteId> {
        self.query_cache
            .borrow_mut()
            .get_or_insert_with(search.clone(), || self.store.find_notes(search).unwrap())
    }

    fn get_note_by_id(&self, id: NoteId) -> Note {
        self.note_cache
            .borrow_mut()
            .get_or_insert_with(id, || self.store.get_note(id).unwrap())
    }

    fn event(&mut self) {
        self.event_count += 1;
    }

    pub fn event_count(&self) -> usize {
        self.event_count
    }
}

pub struct SubjectLayer {
    store: Rc<Store>,
    subjects: RefCell<Option<Rc<BTreeMap<SubjectId, Subject>>>>,
    subject_children: RefCell<Cache<SubjectId, Vec<SubjectId>>>,
}

impl SubjectLayer {
    pub fn new(store: Rc<Store>) -> Self {
        Self {
            store,
            subjects: Default::default(),
            subject_children: RefCell::new(Cache::new(16)),
        }
    }

    fn invalidate_subjects(&mut self) {
        *self.subjects.borrow_mut() = None;
        self.subject_children.borrow_mut().clear();
    }

    pub fn get_subjects(&self) -> Rc<BTreeMap<SubjectId, Subject>> {
        let mut subjects = self.subjects.borrow_mut();
        if subjects.is_none() {
            let subject_list = self.store.get_subjects().unwrap();
            let map = subject_list
                .iter()
                .map(|s| (s.id, s.clone()))
                .collect::<BTreeMap<_, _>>();
            *subjects = Some(Rc::new(map));
        }

        subjects.as_ref().unwrap().clone()
    }

    pub fn create_subject(&mut self, name: String) -> Subject {
        self.invalidate_subjects();
        let subject = self.store.add_subject(name).unwrap();
        subject
    }

    pub fn set_subject_parent(&mut self, subject: SubjectId, parent: Option<SubjectId>) {
        self.invalidate_subjects();
        self.store.set_subject_parent(subject, parent).unwrap();
    }

    pub fn get_subject_children(&self, subject: SubjectId) -> Vec<Subject> {
        self.subject_children
            .borrow_mut()
            .get_or_insert_with(subject, || {
                self.store.get_subject_children(subject).unwrap()
            })
            .iter()
            .map(|id| self.get_subject_by_id(*id))
            .collect()
    }

    pub fn get_subject_by_id(&self, id: SubjectId) -> Subject {
        self.get_subjects().get(&id).unwrap().clone()
    }
}

pub fn use_layer_provider(cx: &ScopeState, conn: ConnectionType) -> &UseSharedState<Layer> {
    use_shared_state_provider(cx, || {
        let store = Store::new(conn);
        Layer::new(Rc::new(store))
    });
    let layer = use_shared_state::<Layer>(cx).unwrap();
    use_shared_state_provider(cx, || SubjectLayer::new(layer.read().store.clone()));
    layer
}

pub fn use_layer(cx: &ScopeState) -> &UseSharedState<Layer> {
    use_shared_state(cx).unwrap()
}

pub fn use_subject_layer(cx: &ScopeState) -> &UseSharedState<SubjectLayer> {
    use_shared_state(cx).unwrap()
}