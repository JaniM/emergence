use crate::{
    data::{
        query::use_note_query,
        subjects::{Subject, SubjectId},
    },
    views::{note_input::CreateNote, view_note::ViewNote},
    ShowInput,
};
use dioxus::prelude::*;
use std::collections::BTreeMap;

#[derive(Props)]
pub struct ListNotesProps<'a> {
    #[props(!optional)]
    pub subject: Option<SubjectId>,
    pub on_select_subject: EventHandler<'a, SubjectId>,
}

pub fn ListNotes<'a>(cx: Scope<'a, ListNotesProps<'a>>) -> Element<'a> {
    let query = use_note_query(cx, cx.props.subject).notes();
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
                        rsx! { ViewNote {
                            key: "{note.id.0}",
                            note: note.clone(),
                            on_select_subject: |subject: Subject| {
                                cx.props.on_select_subject.call(subject.id);
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
                subject: cx.props.subject,
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

    render! {
        div {
            class: "note-grid-wrapper",
            div {
                class: "note-grid",
                groups.into_iter().map(|(date, nodes)| {
                    rsx! {
                        div {
                            key: "{date.format(\"%Y-%m-%d\")}",
                            class: "group-wrapper",
                            div {
                                class: "date",
                                r#"{date.format("%Y-%m-%d")}"#
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
