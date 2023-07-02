use crate::data::{notes::Note, Store};
use dioxus::prelude::*;
use std::collections::BTreeMap;

#[inline_props]
pub fn ListNotes(cx: Scope, notes: Vec<Note>) -> Element {
    cx.render(rsx! {
        div {
            GroupByDate { notes: notes.clone() },
        }
    })
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

#[inline_props]
fn GroupByDate(cx: Scope, notes: Vec<Note>) -> Element {
    let mut groups = BTreeMap::new();
    for node in notes {
        let date = node.created_at.naive_local().date();
        groups.entry(date).or_insert_with(Vec::new).push(node);
    }
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.reverse();
    for (_, nodes) in &mut groups {
        nodes.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
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
                        nodes.into_iter().map(|note| {
                            rsx! { ViewNote { note: note.clone() } }
                        })
                    }
                }
            })
        }
    }
}
