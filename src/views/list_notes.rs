use crate::{
    data::{notes::Note, query::use_query, subjects::Subject},
    use_store, ShowInput,
};
use dioxus::prelude::*;
use std::{collections::BTreeMap, rc::Rc};

use super::select_subject::SelectSubject;
use dioxus::html::input_data::keyboard_types::{Key, Modifiers};

pub fn ListNotes(cx: Scope) -> Element {
    let query = use_query(cx).notes();
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();

    let mut groups = BTreeMap::new();
    for node in query.iter() {
        let date = node.created_at.naive_local().date();
        groups.entry(date).or_insert_with(Vec::new).push(node);
    }

    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.reverse();

    let mut groups = groups
        .into_iter()
        .map(|(date, nodes)| {
            (
                date,
                nodes
                    .into_iter()
                    .map(|note| {
                        rsx! { ViewNote { note: note.clone() } }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    let today = chrono::Local::now().naive_local().date();
    if groups.is_empty() || groups[0].0 != today {
        groups.insert(0, (today, vec![]));
    }

    let add_note = if show_input.read().0 {
        rsx! {
            NoteInput {
                on_create_note: |_| show_input.write().0 = false,
                on_cancel: |_| show_input.write().0 = false
            }
        }
    } else {
        rsx! {
            button {
                class: "add-note",
                onclick: |_| {
                    show_input.write().0 = true;
                },
                "Add note"
            }
        }
    };

    groups[0].1.insert(0, add_note);

    render! {
        div {
            class: "note-grid",
            groups.into_iter().map(|(date, nodes)| {
                rsx! {
                    div {
                        class: "date",
                        r#"{date.format("%Y-%m-%d")}"#
                    },
                    div {
                        class: "group",
                        nodes.into_iter()
                    }
                }
            })
        }
    }
}

#[inline_props]
fn ViewNote(cx: Scope, note: Note) -> Element {
    cx.render(rsx! {
        div {
            class: "note",
            "{note.text}"
        }
    })
}

#[derive(Props)]
struct NoteInputProps<'a> {
    on_create_note: EventHandler<'a, String>,
    on_cancel: EventHandler<'a, ()>,
}

fn NoteInput<'a>(cx: Scope<'a, NoteInputProps<'a>>) -> Element<'a> {
    let store = use_store(cx);
    let text = use_state(cx, String::new);
    let subjects = use_ref(cx, Vec::new);
    let show_subjects = use_state(cx, || false);
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
        Key::Character(c) if c == "@" => {
            show_subjects.set(true);
        }
        _ => {}
    };

    cx.render(rsx! {
        div {
            class: "note",
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
            if *show_subjects.get() {
                rsx! {
                    SelectSubject {
                        on_select: |subject: Subject| {
                            subjects.write().push(subject.id);
                            show_subjects.set(false);
                            textarea.get().as_ref().unwrap().set_focus(true);
                            // remove the @
                            text.set(text.get()[0 .. text.get().len() - 1].to_string());
                        },
                        on_cancel: |_| {
                            show_subjects.set(false);
                            textarea.get().as_ref().unwrap().set_focus(true);
                        }
                    }
                }
            }
        }
    })
}
