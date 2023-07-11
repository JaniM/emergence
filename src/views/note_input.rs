use std::rc::Rc;

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

#[derive(Props)]
pub struct NoteInputProps<'a> {
    #[props(!optional)]
    subject: Option<SubjectId>,
    on_create_note: EventHandler<'a, String>,
    on_cancel: EventHandler<'a, ()>,
}

pub fn NoteInput<'a>(cx: Scope<'a, NoteInputProps<'a>>) -> Element<'a> {
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum ShowSubjects {
        No,
        YesKeyboard,
        YesMouse,
    }

    let store = use_store(cx);
    let text = use_state(cx, String::new);
    let subjects = use_ref(cx, || cx.props.subject.into_iter().collect::<Vec<_>>());
    let show_subjects = use_state(cx, || ShowSubjects::No);
    let textarea = use_state(cx, || None::<Rc<MountedData>>);

    let rows = text.matches("\n").count() as i64 + 1;

    let onkeypress = |e: KeyboardEvent| match e.key() {
        Key::Enter if e.modifiers().contains(Modifiers::CONTROL) => {
            if !text.is_empty() {
                store
                    .read()
                    .add_note(text.get().clone(), subjects.read().clone())
                    .unwrap();
            }
            cx.props.on_create_note.call(text.get().clone());
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
            class: "note",
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
