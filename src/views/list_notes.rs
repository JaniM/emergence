use crate::{
    data::{notes::Note, query::use_query},
    use_store, ShowInput,
};
use dioxus::prelude::*;
use std::collections::BTreeMap;

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

    if show_input.read().0 {
        groups[0].1.insert(
            0,
            rsx! {
                NoteInput {
                    on_create_note: |_| show_input.write().0 = false,
                    on_cancel: |_| show_input.write().0 = false
                }
            },
        );
    } else {
        groups[0].1.insert(
            0,
            rsx! {
                button {
                    class: "add-note",
                    onclick: |_| {
                        show_input.write().0 = true;
                    },
                    "Add note"
                }
            },
        );
    }

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

    let rows = text.matches("\n").count() as i64 + 1;

    cx.render(rsx! {
        textarea {
            class: "note",
            rows: rows,
            value: "{text}",
            onmounted: |e| { e.inner().set_focus(true); },
            oninput: |e| text.set(e.value.clone()),
            onkeypress: |e| {
                if e.key() == Key::Enter && e.modifiers().contains(Modifiers::CONTROL) {
                    if !text.is_empty() {
                        store.read().add_note(text.get().clone(), vec![]).unwrap();
                    }
                    cx.props.on_create_note.call(text.get().clone());
                    text.set(String::new());
                }

                if e.key() == Key::Escape {
                    cx.props.on_cancel.call(());
                }
            },
        }
    })
}
