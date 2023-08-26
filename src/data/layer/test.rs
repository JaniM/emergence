#![cfg(test)]

use std::rc::Rc;

use crate::data::{
    notes::{NoteBuilder, NoteSearch},
    ConnectionType, Store,
};

use super::{DbActions, LayerAction::*};

fn setup() -> DbActions {
    let store = Store::new(ConnectionType::InMemory);
    DbActions::new(Rc::new(store))
}

#[test]
fn create_note() {
    let mut actions = setup();
    let builder = NoteBuilder::new().text("Test Note");
    actions.perform(CreateNote(builder.clone()));

    let note_ids = actions.get_note_ids_for_search(NoteSearch::default());
    assert_eq!(note_ids.len(), 1);

    let note = actions.get_note_by_id(note_ids[0]);
    builder.assert_matches_note(&note);
}

#[test]
fn deleted_note_listing() {
    let mut actions = setup();
    let builder1 = NoteBuilder::new().text("Test Note 1").decide_id();
    let builder2 = NoteBuilder::new().text("Test Note 2").decide_id();
    actions.perform(CreateNote(builder1.clone()));
    actions.perform(CreateNote(builder2.clone()));

    let note_ids = actions.get_note_ids_for_search(NoteSearch::default());
    assert_eq!(note_ids, vec![builder2.id(), builder1.id()]);

    actions.perform(DeleteNote(builder1.id()));
    let note_ids = actions.get_note_ids_for_search(NoteSearch::default());
    assert_eq!(note_ids, vec![builder2.id()]);

    actions.perform(DeleteNote(builder2.id()));
    let note_ids = actions.get_note_ids_for_search(NoteSearch::default());
    assert_eq!(note_ids, vec![]);
}

#[test]
#[should_panic]
fn reading_deleted_note_fails() {
    let mut actions = setup();
    let builder1 = NoteBuilder::new().text("Test Note 1").decide_id();
    actions.perform(CreateNote(builder1.clone()));
    actions.perform(DeleteNote(builder1.id()));
    actions.get_note_by_id(builder1.id());
}

#[test]
pub fn edit_note_with() {
    let mut actions = setup();
    let builder1 = NoteBuilder::new().text("Test Note 1").decide_id();
    let modify = NoteBuilder::new().text("Modified Test Note 1");
    actions.perform(CreateNote(builder1.clone()));
    actions.perform(EditNote(builder1.id(), modify.clone()));
    let note = actions.get_note_by_id(builder1.id());
    modify.assert_matches_note(&note);
}

#[test]
pub fn subject_search() {
    let mut actions = setup();
    actions.perform(AddSubject("Subject".to_string()));
    let subject = actions.last_added_subject.clone().unwrap();

    let builder1 = NoteBuilder::new().text("Test Note 1").decide_id();
    let builder2 = NoteBuilder::new()
        .text("Test Note 2")
        .subject(subject.id)
        .decide_id();
    actions.perform(CreateNote(builder1.clone()));
    actions.perform(CreateNote(builder2.clone()));

    let note_ids = actions.get_note_ids_for_search(NoteSearch::default().subject(subject.id));
    assert_eq!(note_ids, vec![builder2.id()])
}
