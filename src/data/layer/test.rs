#![cfg(test)]

use chrono::{Local, TimeZone};
use rand::prelude::*;
use std::rc::Rc;

use crate::data::{
    notes::{Note, NoteBuilder, NoteId, NoteSearch},
    subjects::{Subject, SubjectId},
    ConnectionType, Store,
};

use super::{
    DbActions,
    LayerAction::{self, *},
};

fn setup() -> DbActions {
    let store = Store::new(ConnectionType::InMemory);
    DbActions::new(Rc::new(store))
}

fn random_action(
    actions: &mut DbActions,
    valid_notes: &mut Vec<NoteId>,
    valid_subjects: &mut Vec<SubjectId>,
) -> Option<()> {
    let mut r = rand::thread_rng();
    let mut choice = r.gen_range(0..=4);
    if choice == 3 && valid_notes.is_empty() {
        choice = 4;
    }
    if choice == 4 && valid_subjects.is_empty() {
        choice = r.gen_range(0..=2);
    }

    match choice {
        0 => {
            let builder = NoteBuilder::new().decide_id();
            valid_notes.push(builder.id());
            let action = LayerAction::CreateNote(builder);
            actions.perform(action);
        }
        1 => {
            let id = *valid_notes.choose(&mut r)?;
            let action = LayerAction::EditNote(
                id,
                NoteBuilder::new().modified_at(Local.timestamp_nanos(r.gen())),
            );
            actions.perform(action);
        }
        2 => {
            let n: u64 = r.gen();
            let name = format!("{n}");
            let action = LayerAction::AddSubject(None, name);
            actions.perform(action);
            valid_subjects.push(actions.last_added_subject.clone().unwrap().id);
        }
        3 => {
            let idx = r.gen_range(0..valid_notes.len());
            let id = valid_notes.remove(idx);
            let action = LayerAction::DeleteNote(id);
            actions.perform(action);
        }
        4 => {
            let idx = r.gen_range(0..valid_subjects.len());
            let id = valid_subjects.remove(idx);
            let action = LayerAction::RemoveSubject(id);
            actions.perform(action);
        }
        _ => unreachable!(),
    };
    Some(())
}

fn store_state(actions: &mut DbActions) -> (Vec<Note>, Vec<Subject>) {
    let notes = actions
        .get_note_ids_for_search(NoteSearch::new())
        .into_iter()
        .enumerate()
        .map(|(idx, id)| {
            let mut note = actions.get_note_by_id(id);
            Rc::make_mut(&mut note).rowid = idx as i64;
            note
        })
        .collect();
    let subjects = actions.get_subjects();
    (notes, subjects)
}

#[test]
fn proptest_undo_redo() {
    let mut actions = setup();
    for _ in 0..10 {
        actions.undo_queue.clear();
        actions.redo_queue.clear();
        let mut valid_notes = Vec::new();
        let mut valid_subjects = Vec::new();

        let start = store_state(&mut actions);

        let mut count = 0;
        while count < 10 {
            if random_action(&mut actions, &mut valid_notes, &mut valid_subjects).is_some() {
                count += 1;
            }
        }

        let after_init = store_state(&mut actions);

        let mut count = 0;
        while count < 10 {
            if random_action(&mut actions, &mut valid_notes, &mut valid_subjects).is_some() {
                count += 1;
            }
        }

        let after_actions = store_state(&mut actions);
        let second_actions = actions
            .undo_queue
            .iter()
            .skip(10)
            .cloned()
            .collect::<Vec<_>>();

        for _ in 0..10 {
            actions.undo();
        }

        let after_undo = store_state(&mut actions);
        let mut redo_actions = actions.redo_queue.clone().into_iter().collect::<Vec<_>>();
        redo_actions.reverse();

        for _ in 0..10 {
            actions.redo();
        }

        let after_redo = store_state(&mut actions);

        for _ in 0..20 {
            actions.undo();
        }

        let end = store_state(&mut actions);

        if after_init != after_undo {
            for (undo, redo) in second_actions.into_iter().zip(redo_actions) {
                println!("--");
                println!("    Undo: {:?}", undo);
                println!("    Redo: {:?}", redo);
            }
            assert_ne!(after_init, after_undo);
        }
        assert_eq!(after_actions, after_redo);
        assert_eq!(start, end);
    }
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
    actions.perform(AddSubject(None, "Subject".to_string()));
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
