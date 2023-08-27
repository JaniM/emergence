mod test;

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;

use dioxus::prelude::{use_context, use_context_provider, ScopeState};
use dioxus_signals::*;
use uuid::Uuid;

use super::notes::{NoteBuilder, NoteSearch};
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

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub(crate) enum ApplyDirection {
    Forward,
    Backward,
}

pub struct DbActions {
    store: Rc<Store>,
    note_cache: Cache<NoteId, Note>,
    query_cache: Cache<NoteSearch, Vec<NoteId>>,
    last_added_subject: Option<Subject>,
    undo_queue: VecDeque<LayerAction>,
    redo_queue: VecDeque<LayerAction>,
    direction: ApplyDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerEffect {
    InvalidateQuery,
    InvalidateNote(NoteId),
    InvalidateSubjects,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerAction {
    CreateNote(NoteBuilder),
    DeleteNote(NoteId),
    EditNote(NoteId, NoteBuilder),
    AddSubject(Option<SubjectId>, String),
    RemoveSubject(SubjectId),
    SetSubjectParent {
        subject: SubjectId,
        parent: Option<SubjectId>,
    },
}

impl DbActions {
    pub(crate) fn new(store: Rc<Store>) -> Self {
        Self {
            store,
            note_cache: Cache::new(1024),
            query_cache: Cache::new(16),
            last_added_subject: None,
            undo_queue: VecDeque::new(),
            redo_queue: VecDeque::new(),
            direction: ApplyDirection::Forward,
        }
    }

    fn add_undo_action(&mut self, action: LayerAction) {
        if self.undo_queue.len() >= 64 {
            self.undo_queue.pop_front();
        }
        self.undo_queue.push_back(action);
    }

    fn add_redo_action(&mut self, action: LayerAction) {
        if self.redo_queue.len() >= 64 {
            self.redo_queue.pop_front();
        }
        self.redo_queue.push_back(action);
    }

    fn add_backwards(&mut self, action: LayerAction) {
        match self.direction {
            ApplyDirection::Forward => self.add_undo_action(action),
            ApplyDirection::Backward => self.add_redo_action(action),
        }
    }

    pub(crate) fn undo(&mut self) -> Option<LayerEffect> {
        let Some(action) = self.undo_queue.pop_back() else { return None };
        let effect = self.perform_direction(action, ApplyDirection::Backward);
        Some(effect)
    }

    pub(crate) fn redo(&mut self) -> Option<LayerEffect> {
        let Some(action) = self.redo_queue.pop_back() else { return None };
        let effect = self.perform_direction(action, ApplyDirection::Forward);
        Some(effect)
    }

    pub(crate) fn perform(&mut self, action: LayerAction) -> LayerEffect {
        self.redo_queue.clear();
        self.perform_direction(action, ApplyDirection::Forward)
    }

    pub(crate) fn perform_direction(
        &mut self,
        action: LayerAction,
        direction: ApplyDirection,
    ) -> LayerEffect {
        let old_direction = self.direction;
        self.direction = direction;
        let effect = match action {
            LayerAction::CreateNote(builder) => self.create_note(builder),
            LayerAction::DeleteNote(id) => self.delete_note_by_id(id),
            LayerAction::EditNote(id, builder) => self.edit_note_with(id, builder),
            LayerAction::AddSubject(id, name) => self.add_subject(id, name),
            LayerAction::RemoveSubject(id) => self.remove_subject(id),
            LayerAction::SetSubjectParent { subject, parent } => {
                self.set_subject_parent(subject, parent)
            }
        };
        self.apply_effect(&effect);
        self.direction = old_direction;
        effect
    }

    fn apply_effect(&mut self, eff: &LayerEffect) {
        match eff {
            LayerEffect::InvalidateQuery => {
                self.invalidate_note_queries();
            }
            &LayerEffect::InvalidateNote(id) => {
                self.invalidate_note_queries();
                self.invalidate_note(id);
            }
            LayerEffect::InvalidateSubjects => {}
        }
    }

    fn invalidate_note_queries(&mut self) {
        self.query_cache.clear();
    }

    fn invalidate_note(&mut self, id: NoteId) {
        self.note_cache.invalidate_key(&id);
    }

    fn create_note(&mut self, builder: NoteBuilder) -> LayerEffect {
        let note = self.store.add_note(builder).unwrap();
        self.add_backwards(LayerAction::DeleteNote(note.id));
        LayerEffect::InvalidateQuery
    }

    fn delete_note_by_id(&mut self, id: NoteId) -> LayerEffect {
        let note = self.store.get_note(id).unwrap();
        self.store.delete_note(id).unwrap();
        self.add_backwards(LayerAction::CreateNote(note.to_builder()));
        LayerEffect::InvalidateNote(id)
    }

    fn edit_note_with(&mut self, id: NoteId, builder: NoteBuilder) -> LayerEffect {
        let mut builder = builder.with_id(id);
        if matches!(self.direction, ApplyDirection::Forward) && builder.modified_at.is_none() {
            builder = builder.modified_now();
        }

        let old_note = self.store.get_note(id).unwrap();
        let note = builder.apply_to_note(&old_note);
        self.store.update_note(note).unwrap();

        self.add_backwards(LayerAction::EditNote(id, old_note.to_builder()));
        LayerEffect::InvalidateNote(id)
    }

    fn add_subject(&mut self, id: Option<SubjectId>, name: String) -> LayerEffect {
        let id = id.unwrap_or_else(|| SubjectId(Uuid::new_v4()));
        let subject = self.store.add_subject_with_id(id, name).unwrap();
        self.add_backwards(LayerAction::RemoveSubject(subject.id));
        self.last_added_subject = Some(subject);
        LayerEffect::InvalidateSubjects
    }

    fn remove_subject(&mut self, subject_id: SubjectId) -> LayerEffect {
        let subject = self.store.get_subject(subject_id).unwrap();
        self.store.delete_subject(subject_id).unwrap();

        // TODO: Handle subject parent. Maybe a SubjectBuilder pattern?
        self.add_backwards(LayerAction::AddSubject(
            Some(subject_id),
            subject.name.clone(),
        ));
        self.last_added_subject = Some(subject);
        LayerEffect::InvalidateSubjects
    }

    fn set_subject_parent(
        &mut self,
        subject_id: SubjectId,
        parent: Option<SubjectId>,
    ) -> LayerEffect {
        let subject = self.store.get_subject(subject_id).unwrap();
        self.store.set_subject_parent(subject_id, parent).unwrap();
        self.add_backwards(LayerAction::SetSubjectParent {
            subject: subject_id,
            parent: subject.parent_id,
        });
        LayerEffect::InvalidateSubjects
    }

    fn get_subjects(&self) -> Vec<Subject> {
        self.store.get_subjects().unwrap()
    }

    fn get_subject_children(&self, subject: SubjectId) -> Vec<SubjectId> {
        self.store.get_subject_children(subject).unwrap()
    }

    fn get_note_ids_for_search(&mut self, search: NoteSearch) -> Vec<NoteId> {
        self.query_cache
            .get_or_insert_with(search, || self.store.find_notes(search).unwrap())
    }

    fn get_note_by_id(&mut self, id: NoteId) -> Note {
        self.note_cache
            .get_or_insert_with(id, || self.store.get_note(id).unwrap())
    }
}

type Notes = Signal<Vec<Note>>;
type Subjects = Signal<Rc<BTreeMap<SubjectId, Subject>>>;
type SubjectChildren = Signal<BTreeMap<SubjectId, Vec<SubjectId>>>;

/// Layer provides an abstraction layer over the store to provide a
/// consistent interface for the rest of the application.
pub struct Layer {
    actions: DbActions,
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
            actions: DbActions::new(store),
            event_count: 0,
            query: Default::default(),
            notes,
            subjects,
            subject_children,
        }
    }

    pub fn perform(&mut self, action: LayerAction) {
        self.with_action(|actions| actions.perform(action).into())
    }

    pub fn can_undo(&self) -> bool {
        !self.actions.undo_queue.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.actions.redo_queue.is_empty()
    }

    pub fn undo(&mut self) {
        self.with_action(|actions| actions.undo());
    }

    pub fn redo(&mut self) {
        self.with_action(|actions| actions.redo());
    }

    fn with_action(&mut self, f: impl FnOnce(&mut DbActions) -> Option<LayerEffect>) {
        self.event();
        let Some(eff) = f(&mut self.actions) else { return };
        match eff {
            LayerEffect::InvalidateQuery => {
                self.update_notes();
            }
            LayerEffect::InvalidateNote(_) => {
                self.update_notes();
            }
            LayerEffect::InvalidateSubjects => {
                self.update_subjects();
            }
        }
    }

    fn update_notes(&mut self) {
        let search = self.query;
        let notes = self
            .actions
            .get_note_ids_for_search(search)
            .into_iter()
            .map(|id| self.actions.get_note_by_id(id))
            .collect();

        *self.notes.write() = notes;
    }

    pub fn search(&self) -> SearchWorker {
        self.actions.store.search.clone()
    }

    pub fn set_search(&mut self, search: NoteSearch) {
        if self.query == search {
            return;
        }

        self.query = search;
        self.update_notes();
    }

    fn event(&mut self) {
        self.event_count += 1;
    }

    pub fn event_count(&self) -> usize {
        self.event_count
    }

    fn update_subjects(&mut self) {
        let subject_list = self.actions.get_subjects();
        let map = subject_list
            .iter()
            .map(|s| (s.id, s.clone()))
            .collect::<BTreeMap<_, _>>();
        *self.subjects.write() = Rc::new(map);

        *self.subject_children.write() = subject_list
            .iter()
            .map(|subject| (subject.id, self.actions.get_subject_children(subject.id)))
            .collect();
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

pub fn use_layer(cx: &ScopeState) -> LayerSignal {
    let layer = *use_context(cx).expect("Layer should be provided");
    LayerSignal { layer }
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

#[derive(Clone, Copy)]
pub struct LayerSignal {
    layer: Signal<Layer>,
}

impl Deref for LayerSignal {
    type Target = Signal<Layer>;

    fn deref(&self) -> &Self::Target {
        &self.layer
    }
}

impl LayerSignal {
    pub fn create_note(self, builder: NoteBuilder) {
        self.layer.write().perform(LayerAction::CreateNote(builder))
    }

    pub fn delete_note(self, id: NoteId) {
        self.layer.write().perform(LayerAction::DeleteNote(id))
    }

    pub fn edit_note(self, id: NoteId, builder: NoteBuilder) {
        self.layer
            .write()
            .perform(LayerAction::EditNote(id, builder))
    }

    pub fn create_subject(self, name: impl ToString) -> Subject {
        let mut layer = self.layer.write();
        layer.perform(LayerAction::AddSubject(None, name.to_string()));
        layer.actions.last_added_subject.clone().unwrap()
    }

    pub fn set_subject_parent(self, subject: SubjectId, parent: Option<SubjectId>) {
        self.layer
            .write()
            .perform(LayerAction::SetSubjectParent { subject, parent })
    }
}
