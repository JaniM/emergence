use crate::views::{
    note_input::CreateNote,
    scroll_to::ScrollTo,
    use_view_state,
    view_note::{OnSubjectSelect, ViewNote},
    ViewState,
};
use dioxus::prelude::*;
use emergence::data::{
    layer::{use_layer, use_notes},
    notes::{Note, TaskState},
};
use std::collections::BTreeMap;

type NoteGroup<T> = (chrono::NaiveDate, String, Vec<T>);

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

pub fn ListNotes(cx: Scope) -> Element {
    let view_state = use_view_state(cx);

    let &ViewState {
        tasks_only,
        selected_subject,
        scroll_to_note,
        show_input,
        ..
    } = &*view_state.read();

    let subject_id_key = selected_subject.map_or_else(|| "none".to_string(), |id| id.0.to_string());
    let query = use_notes(cx);

    let mut groups = if !tasks_only {
        group_by_date(&*query.read())
    } else {
        let mut done = vec![];
        let mut undone = vec![];
        let query = query.read();
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
                                hide_subject: selected_subject,
                                subject_select: OnSubjectSelect::Switch
                            } },
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    if let Some(note_id) = scroll_to_note {
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
            let id = last_note.0 .0;
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

    let add_note = if show_input {
        rsx! {
            CreateNote {
                key: "input",
                subject: selected_subject,
                task: tasks_only,
                on_create_note: move |_| view_state.write().finish_note_input(true),
                on_cancel: move |_| view_state.write().finish_note_input(false),
            }
        }
    } else {
        rsx! {
            button {
                key: "add-note-button",
                class: "add-note",
                onclick: move |_| view_state.write().start_note_input(),
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
    let layer = use_layer(cx);

    let event_count = layer.read().event_count();
    let query_fut = use_future(cx, (search_text, &event_count), move |(search_text, _)| {
        let search_text = search_text.trim().to_string();
        let search = layer.read().search();
        async move { search.perform_search(search_text).await }
    });
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
                            subject_select: OnSubjectSelect::Switch,
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
