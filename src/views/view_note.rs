use dioxus::{html::input_data::MouseButton, prelude::*};
use emergence::data::{
    notes::Note,
    query::{use_store, use_subject_query},
    subjects::{Subject, SubjectId},
};
use tracing::debug;

use crate::views::note_input::EditNote;

#[derive(Props)]
pub struct ViewNoteProps<'a> {
    note: Note,
    on_select_subject: EventHandler<'a, Subject>,
}

pub fn ViewNote<'a>(cx: Scope<'a, ViewNoteProps<'a>>) -> Element<'a> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Normal,
        Dropdown(f64, f64),
        Edit,
    }

    let store = use_store(cx);

    let state = use_state(cx, || State::Normal);

    let note = &cx.props.note;
    let time_text = note
        .created_at
        .naive_local()
        .format("%Y-%m-%d %H:%M")
        .to_string();

    let on_click = {
        let state = state.clone();
        move |e: MouseEvent| {
            debug!("Clicked: {:?}", e.trigger_button());
            // TODO: Change this to a right click
            if e.trigger_button() != Some(MouseButton::Primary) {
                return;
            }
            let coord = e.element_coordinates();
            match state.get() {
                State::Normal => state.set(State::Dropdown(coord.x, coord.y)),
                State::Dropdown(_, _) => state.set(State::Normal),
                State::Edit => {}
            }
        }
    };

    // TODO: Add a confirmation dialog
    let delete = {
        let note = note.clone();
        move |_| {
            store.read().delete_note(note.id).unwrap();
        }
    };

    let dropdown = if let State::Dropdown(x, y) = *state.get() {
        Some(rsx! {
            Dropdown {
                pos: (x, y),
                on_edit: |_| state.set(State::Edit),
                on_delete: delete,
                on_close: |_| state.set(State::Normal),
            }
        })
    } else {
        None
    };

    let text = if cx.props.note.text.is_empty() {
        "<<Empty Note>>"
    } else {
        &cx.props.note.text
    };

    let content = if *state.get() == State::Edit {
        rsx! {
            EditNote {
                note: note.clone(),
                on_done: |_| state.set(State::Normal),
            }
        }
    } else {
        rsx! {
            div {
                class: "note",
                SubjectCards {
                    sids: note.subjects.clone(),
                    on_click_subject: |subject: Subject| {
                        cx.props.on_select_subject.call(subject);
                    },
                },
                div {
                    class: "note-content",
                    title: "{time_text}",
                    onclick: on_click,
                    "{text}",
                },
                dropdown
            }
        }
    };

    cx.render(content)
}

#[derive(Props)]
struct DropdownProps<'a> {
    pos: (f64, f64),
    on_edit: EventHandler<'a, ()>,
    on_delete: EventHandler<'a, ()>,
    on_close: EventHandler<'a, ()>,
}

fn Dropdown<'a>(cx: Scope<'a, DropdownProps<'a>>) -> Element<'a> {
    cx.render(rsx! {
        div {
            style: "left: {cx.props.pos.0}px; top: {cx.props.pos.1}px;",
            class: "note-dropdown",
            tabindex: 0,
            onmounted: |e| {
                e.inner().set_focus(true);
            },
            onblur: |_| cx.props.on_close.call(()),
            div {
                class: "note-dropdown-item",
                onclick: |_| cx.props.on_edit.call(()),
                "Edit"
            },
            div {
                class: "note-dropdown-item",
                onclick: |_| cx.props.on_delete.call(()),
                "Delete"
            },
        }
    })
}

#[derive(Props)]
pub struct SubjectCardsProps<'a> {
    sids: Vec<SubjectId>,
    on_add_subject: Option<EventHandler<'a, ()>>,
    on_click_subject: Option<EventHandler<'a, Subject>>,
}

pub fn SubjectCards<'a>(cx: Scope<'a, SubjectCardsProps<'a>>) -> Element<'a> {
    let subjects = use_subject_query(cx).subjects();
    let mut cards = cx
        .props
        .sids
        .iter()
        .map(|sid| {
            let s = subjects.get(sid).unwrap().clone();
            let on_click_subject = &cx.props.on_click_subject;
            rsx! {
                div {
                    key: "{s.id.0}",
                    class: "subject-card",
                    onclick: move |_| {
                        if let Some(on_click_subject) = on_click_subject {
                            on_click_subject.call(s.clone());
                        }
                    },
                    "{s.name}"
                }
            }
        })
        .collect::<Vec<_>>();
    if let Some(on_add_subject) = &cx.props.on_add_subject {
        cards.push(rsx! {
            div {
                key: "add-subject",
                class: "subject-card",
                onclick: |_| on_add_subject.call(()),
                "+"
            }
        });
    }
    cx.render(rsx! {
        div {
            class: "note-subjects",
            cards.into_iter()
        }
    })
}
