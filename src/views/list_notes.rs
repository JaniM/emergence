use crate::{
    data::{query::use_note_query, subjects::Subject},
    views::{journal::SelectedSubject, note_input::CreateNote, view_note::ViewNote},
    ShowInput,
};
use dioxus::prelude::*;
use std::collections::BTreeMap;
use tracing::trace;

#[tracing::instrument(skip(cx))]
pub fn ListNotes(cx: Scope) -> Element {
    trace!("Begin VDOM creation");
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();
    let query = use_note_query(cx, my_subject.read().0).notes();

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
                        rsx! { ViewNote {
                            key: "{note.id.0}",
                            note: note.clone(),
                            on_select_subject: |subject: Subject| {
                                my_subject.write().0 = Some(subject.id);
                            },
                        } }
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
            CreateNote {
                key: "input",
                subject: my_subject.read().0,
                on_create_note: |_| show_input.write().0 = false,
                on_cancel: |_| show_input.write().0 = false
            }
        }
    } else {
        rsx! {
            button {
                key: "add-note-button",
                class: "add-note",
                onclick: |_| {
                    show_input.write().0 = true;
                },
                "Add note"
            }
        }
    };

    groups[0].1.insert(0, add_note);

    trace!("End VDOM creation");

    render! {
        div {
            class: "note-grid-wrapper",
            div {
                class: "note-grid",
                groups.into_iter().map(|(date, nodes)| {
                    let date_string = date.format("%Y-%m-%d");
                    rsx! {
                        div {
                            key: "{date_string}",
                            class: "group-wrapper",
                            div {
                                class: "date-wrapper",
                                div {
                                    class: "date",
                                    "{date_string}"
                                }
                            },
                            div {
                                class: "group",
                                nodes.into_iter()
                            }
                        }
                    }
                })
            }
        }
    }
}
