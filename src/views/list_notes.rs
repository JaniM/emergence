use crate::{
    data::{query::use_note_query, subjects::Subject},
    views::{
        journal::SelectedSubject, note_input::CreateNote, search_view::SearchOpen,
        view_note::ViewNote,
    },
    ShowInput,
};
use dioxus::prelude::*;
use emergence::data::{
    notes::{Note, NoteSearch, TaskState},
    query::{use_store, use_store_event_query},
};
use std::collections::BTreeMap;

fn group_by_date(query: &[Note]) -> Vec<(chrono::NaiveDate, String, Vec<Note>)> {
    let mut groups = BTreeMap::new();
    for node in query.iter() {
        let date = node.created_at.naive_local().date();
        groups
            .entry(date)
            .or_insert_with(Vec::new)
            .push(node.clone());
    }

    let mut groups = groups
        .into_iter()
        .map(|(date, notes)| (date, date.format("%Y-%m-%d").to_string(), notes))
        .collect::<Vec<_>>();
    groups.reverse();
    groups
}

#[inline_props]
pub fn ListNotes(cx: Scope, tasks_only: bool) -> Element {
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();

    let subject_id = my_subject.read().0;
    let search = NoteSearch {
        subject_id,
        task_only: *tasks_only,
    };
    let query = use_note_query(cx, search).notes();

    let groups = if !*tasks_only {
        group_by_date(&query)
    } else {
        let mut done = vec![];
        let mut undone = vec![];
        let mut query = query.iter().peekable();
        while let Some(first) = query.peek() {
            let mut group = vec![];
            let first_date = first.created_at.naive_local().date();
            let state = first.task_state;
            while let Some(node) = query.peek() {
                let date = node.created_at.naive_local().date();
                if date != first_date || node.task_state != state {
                    break;
                }
                group.push(query.next().unwrap().clone());
            }
            if state == TaskState::Done {
                let key = first_date.format("%Y-%m-%d-done").to_string();
                done.push((first_date, key, group));
            } else {
                let key = first_date.format("%Y-%m-%d-undone").to_string();
                undone.push((first_date, key, group));
            }
        }
        undone.extend(done.drain(..));
        undone
    };

    let mut groups = groups
        .into_iter()
        .map(|(date, key, nodes)| {
            (
                date,
                key,
                nodes
                    .into_iter()
                    .map(|note| {
                        rsx! { ViewNote {
                            key: "{note.id.0}",
                            note: note.clone(),
                            hide_subject: subject_id,
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
        groups.insert(0, (today, "today".to_string(), vec![]));
    }

    let add_note = if show_input.read().0 {
        rsx! {
            CreateNote {
                key: "input",
                subject: my_subject.read().0,
                task: *tasks_only,
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

    groups[0].2.insert(0, add_note);

    render! {
        div {
            class: "note-grid-wrapper",
            div {
                class: "note-grid",
                groups.into_iter().map(|(date, key, nodes)| {
                    let date_string = date.format("%Y-%m-%d");
                    rsx! {
                        div {
                            key: "{key}",
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

#[inline_props]
pub fn ListSearchResult(cx: Scope, search_text: String) -> Element {
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let search_open = use_shared_state::<SearchOpen>(cx).unwrap();
    let store = use_store(cx);

    let store_event = use_store_event_query(cx);

    let query_fut = use_future(
        cx,
        (search_text, &store_event.count()),
        move |(search_text, _)| {
            let search_text = search_text.trim().to_string();
            let search = store.read().search.clone();
            async move {
                if search_text.is_empty() {
                    return vec![];
                }
                search.perform_search(search_text).await
            }
        },
    );
    let query = match query_fut.value() {
        Some(query) => query,
        _ => return render! { div { "Loading..." } },
    };

    let groups = group_by_date(query);

    let groups = groups
        .into_iter()
        .map(|(date, key, nodes)| {
            (
                date,
                key,
                nodes
                    .into_iter()
                    .map(|note| {
                        rsx! { ViewNote {
                            key: "{note.id.0}",
                            note: note.clone(),
                            hide_subject: None,
                            on_select_subject: |subject: Subject| {
                                search_open.write().0 = false;
                                my_subject.write().0 = Some(subject.id);
                            },
                        } }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    render! {
        div {
            class: "note-grid-wrapper",
            div {
                class: "note-grid",
                groups.into_iter().map(|(date, key, nodes)| {
                    let date_string = date.format("%Y-%m-%d");
                    rsx! {
                        div {
                            key: "{key}",
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
