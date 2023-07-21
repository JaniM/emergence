use crate::{
    data::{query::use_note_query, subjects::Subject},
    views::{
        journal::SelectedSubject, note_input::CreateNote, scroll_to::ScrollTo,
        search_view::SearchOpen, view_note::ViewNote,
    },
    ShowInput,
};
use dioxus::prelude::*;
use emergence::data::{
    notes::{Note, NoteId, NoteSearch, TaskState},
    query::{use_store, use_store_event_query},
};
use std::collections::BTreeMap;

type NoteGroup<T> = (chrono::NaiveDate, String, Vec<T>);

pub struct ScrollToNote(pub Option<NoteId>);

fn group_by_date(query: &[Note]) -> Vec<NoteGroup<Note>> {
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

/// Reverse the order of the groups and the notes in each group.
/// This exists temporarily to test if chronological order is better.
fn reverse_groups<T>(groups: &mut [NoteGroup<T>]) {
    groups.reverse();
    for (_, _, notes) in groups.iter_mut() {
        notes.reverse();
    }
}

#[inline_props]
pub fn ListNotes(cx: Scope, tasks_only: bool) -> Element {
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();
    let scroll_to_note = use_shared_state::<ScrollToNote>(cx).unwrap();

    let subject_id = my_subject.read().0;
    let subject_id_key = subject_id.map_or_else(|| "none".to_string(), |id| id.0.to_string());
    let search = NoteSearch {
        subject_id,
        task_only: *tasks_only,
    };
    let query = use_note_query(cx, search).notes();

    let mut groups = if !*tasks_only {
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

    reverse_groups(&mut groups);

    let mut groups = groups
        .into_iter()
        .map(|(date, key, nodes)| {
            (
                date,
                key,
                nodes
                    .into_iter()
                    .map(|note| {
                        let id = note.id;
                        (
                            id,
                            rsx! { ViewNote {
                                key: "{note.id.0}",
                                note: note.clone(),
                                hide_subject: subject_id,
                                on_select_subject: move |subject: Subject| {
                                    my_subject.write().0 = Some(subject.id);
                                    scroll_to_note.write().0 = Some(id);
                                },
                            } },
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    if let Some(note_id) = scroll_to_note.read().0 {
        let key = format!("scroll-to-{subject_id_key}");
        for (_, _, group) in groups.iter_mut() {
            let idx = group.iter().position(|(id, _)| *id == note_id);
            if let Some(idx) = idx {
                let old = std::mem::replace(&mut group[idx].1, rsx! { div {} });
                let new = rsx! {
                    ScrollTo {
                        key: "{key}",
                        old
                    }
                };
                group[idx].1 = new;
                break;
            }
        }
    } else if let Some(last_group) = groups.last_mut() {
        if let Some(last_note) = last_group.2.pop() {
            let id = last_note.0.0;
            let key = format!("scroll-to-{subject_id_key}-{id}");
            let new_last = rsx! {
                ScrollTo {
                    key: "{key}",
                    last_note.1
                }
            };
            last_group.2.push((last_note.0, new_last));
        }
    }

    let add_note = if show_input.read().0 {
        rsx! {
            CreateNote {
                key: "input",
                subject: my_subject.read().0,
                task: *tasks_only,
                on_create_note: |_| {
                    show_input.write().0 = false;
                    scroll_to_note.write().0 = None;
                },
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

    render! {
        div {
            class: "note-grid-wrapper",
            div {
                class: "note-grid-scroll",
                div {
                    class: "place-at-end",
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
                                        nodes.into_iter().map(|(_, node)| node)
                                    }
                                }
                            }
                        })
                    }
                }
            }
            div {
                class: "group-wrapper",
                style: "margin-bottom: 10px;",
                div {
                    class: "date-wrapper",
                },
                div {
                    class: "group",
                    add_note
                }
            }
        }
    }
}

#[inline_props]
pub fn ListSearchResult(cx: Scope, search_text: String) -> Element {
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let search_open = use_shared_state::<SearchOpen>(cx).unwrap();
    let scroll_to_note = use_shared_state::<ScrollToNote>(cx).unwrap();
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
                            on_select_subject: move |subject: Subject| {
                                search_open.write().0 = false;
                                scroll_to_note.write().0 = Some(note.id);
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
                class: "note-grid-scroll",
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
}
