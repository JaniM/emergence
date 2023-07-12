use std::{ops::Deref, rc::Rc};

use crate::{
    data::{
        query::use_store,
        subjects::{Subject, SubjectId},
    },
    views::{select_subject::SelectSubject, view_note::SubjectCards},
};
use dioxus::{
    html::input_data::keyboard_types::{Key, Modifiers},
    prelude::*,
};
use emergence::data::notes::{Note, NoteData};

#[derive(Props)]
pub struct CreateNoteProps<'a> {
    #[props(!optional)]
    subject: Option<SubjectId>,
    on_create_note: EventHandler<'a, String>,
    on_cancel: EventHandler<'a, ()>,
}

pub fn CreateNote<'a>(cx: Scope<'a, CreateNoteProps<'a>>) -> Element<'a> {
    let store = use_store(cx);

    let on_create_note = |(text, subjects): (String, Vec<SubjectId>)| {
        if !text.is_empty() {
            store.read().add_note(text.clone(), subjects).unwrap();
        }
        cx.props.on_create_note.call(text);
    };

    cx.render(rsx! {
        NoteInput {
            on_create_note: on_create_note,
            on_cancel: |_| cx.props.on_cancel.call(()),
            initial_subjects: cx.props.subject.into_iter().collect(),
        }
    })
}

#[derive(Props)]
pub struct EditNoteProps<'a> {
    note: Note,
    on_done: EventHandler<'a, ()>,
}

pub fn EditNote<'a>(cx: Scope<'a, EditNoteProps<'a>>) -> Element<'a> {
    let store = use_store(cx);

    let on_done = move |_| {
        cx.props.on_done.call(());
    };

    let on_create_note = {
        let note = cx.props.note.clone();
        move |(text, subjects): (String, Vec<SubjectId>)| {
            let new_note = NoteData {
                text: text.clone(),
                subjects: subjects,
                ..note.deref().clone()
            }
            .to_note();
            store.write().update_note(new_note).unwrap();
            cx.props.on_done.call(());
        }
    };

    cx.render(rsx! {
        NoteInput {
            on_create_note: on_create_note,
            on_cancel: on_done,
            initial_text: cx.props.note.text.clone(),
            initial_subjects: cx.props.note.subjects.clone(),
        }
    })
}

#[derive(Props)]
struct NoteInputProps<'a> {
    on_create_note: EventHandler<'a, (String, Vec<SubjectId>)>,
    on_cancel: EventHandler<'a, ()>,
    initial_text: Option<String>,
    initial_subjects: Vec<SubjectId>,
}

fn NoteInput<'a>(cx: Scope<'a, NoteInputProps<'a>>) -> Element<'a> {
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum ShowSubjects {
        No,
        YesKeyboard,
        YesMouse,
    }

    // TODO: Combine these states.
    let text = use_state(cx, || cx.props.initial_text.clone().unwrap_or_default());
    let subjects = use_ref(cx, || cx.props.initial_subjects.clone());
    let show_subjects = use_state(cx, || ShowSubjects::No);
    let textarea = use_state(cx, || None::<Rc<MountedData>>);

    // TODO: Calculate rows based on horizontal overflow too.
    // I guess there should be a nice way to do it with javascript.
    let rows = text.matches("\n").count() as i64 + 1;

    let onkeypress = |e: KeyboardEvent| match e.key() {
        Key::Enter if e.modifiers().contains(Modifiers::CONTROL) => {
            cx.props
                .on_create_note
                .call((text.get().clone(), subjects.read().clone()));
            text.set(String::new());
        }
        Key::Escape => {
            cx.props.on_cancel.call(());
        }
        Key::Character(c) if c == "@" && *show_subjects.get() == ShowSubjects::No => {
            show_subjects.set(ShowSubjects::YesKeyboard);
        }
        _ => {}
    };

    let on_select_subject = |subject: Subject| {
        subjects.write().push(subject.id);
        show_subjects.set(ShowSubjects::No);
        textarea.get().as_ref().unwrap().set_focus(true);
        if *show_subjects.get() == ShowSubjects::YesKeyboard
            && text.get().chars().last() == Some('@')
        {
            // remove the @
            text.set(text.get()[0..text.get().len() - 1].to_string());
        }
    };

    cx.render(rsx! {
        div {
            class: "note note-row",
            SubjectCards {
                sids: subjects.read().clone(),
                on_add_subject: |_| show_subjects.set(ShowSubjects::YesMouse),
                on_click_subject: |subject: Subject| {
                    subjects.write().retain(|s| *s != subject.id);
                    textarea.get().as_ref().unwrap().set_focus(true);
                },
            },
            div {
                class: "note-content",
                textarea {
                    rows: rows,
                    value: "{text}",
                    onmounted: |e| {
                        textarea.set(Some(e.inner().clone()));
                        e.inner().set_focus(true);
                    },
                    oninput: |e| text.set(e.value.clone()),
                    onkeypress: onkeypress,
                }
            },
            if *show_subjects.get() != ShowSubjects::No {
                rsx! {
                    SelectSubject {
                        on_select: on_select_subject,
                        on_cancel: |_| {
                            show_subjects.set(ShowSubjects::No);
                            textarea.get().as_ref().unwrap().set_focus(true);
                        },
                        ignore_subjects: subjects.read().clone(),
                    }
                }
            }
        }
    })
}
