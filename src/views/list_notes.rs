use crate::data::notes::Note;
use dioxus::prelude::*;
use std::collections::BTreeMap;

use dioxus::html::input_data::keyboard_types::{Key, Modifiers};

#[derive(Props)]
pub struct ListNotesProps<'a> {
    notes: Vec<Note>,
    create_note: bool,
    on_create_note: EventHandler<'a, String>,
}

pub fn ListNotes<'a>(cx: Scope<'a, ListNotesProps<'a>>) -> Element<'a> {
    let mut groups = BTreeMap::new();
    for node in cx.props.notes.clone() {
        let date = node.created_at.naive_local().date();
        groups.entry(date).or_insert_with(Vec::new).push(node);
    }
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.reverse();
    for (_, nodes) in &mut groups {
        nodes.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
    }
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

    if cx.props.create_note {
        groups[0].1.insert(
            0,
            rsx! {
                NoteInput {
                    on_create_note: move |e| cx.props.on_create_note.call(e),
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
}

fn NoteInput<'a>(cx: Scope<'a, NoteInputProps<'a>>) -> Element<'a> {
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
                    cx.props.on_create_note.call(text.get().clone());
                    text.set(String::new());
                }
            },
        }
    })
}
