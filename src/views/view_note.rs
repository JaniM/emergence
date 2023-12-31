use dioxus::{html::input_data::MouseButton, prelude::*};
use emergence::data::{
    layer::{use_layer, use_subjects},
    notes::{Note, NoteBuilder, TaskState},
    subjects::{Subject, SubjectId},
};

use crate::views::{
    confirm_dialog::ConfirmDialog, markdown::Markdown, note_input::EditNote, use_view_state,
};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum OnSubjectSelect {
    Switch,
    Ignore,
}

#[derive(Props, PartialEq)]
pub struct ViewNoteProps {
    note: Note,
    subject_select: OnSubjectSelect,
    #[props(!optional)]
    hide_subject: Option<SubjectId>,
}

pub fn ViewNote(cx: Scope<'_, ViewNoteProps>) -> Element<'_> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Normal,
        Dropdown(f64, f64),
        Edit,
        ConfirmDelete,
    }

    let layer = use_layer(cx);
    let view_state = use_view_state(cx);

    let state = use_state(cx, || State::Normal);

    let note = &cx.props.note;
    let time_text = note
        .created_at
        .naive_local()
        .format("%Y-%m-%d %H:%M")
        .to_string();

    // TODO: This probably should use oncontextmenu
    // See https://developer.mozilla.org/en-US/docs/Web/API/Element/contextmenu_event
    let on_mousedown = {
        let state = state.clone();
        move |e: MouseEvent| {
            if e.trigger_button() != Some(MouseButton::Secondary) {
                return;
            }
            let coord = e.page_coordinates();
            match state.get() {
                State::Normal => state.set(State::Dropdown(coord.x, coord.y)),
                State::Dropdown(_, _) => state.set(State::Normal),
                _ => {}
            }
        }
    };

    let actually_delete = {
        let note_id = note.id;
        move |_| {
            layer.delete_note(note_id);
        }
    };

    let make_task = {
        let note = note.clone();
        move |_| {
            let new_state = match note.task_state {
                TaskState::NotATask => TaskState::Todo,
                TaskState::Todo => TaskState::NotATask,
                TaskState::Done => TaskState::NotATask,
            };
            layer.edit_note(note.id, NoteBuilder::new().task_state(new_state));
            state.set(State::Normal);
        }
    };

    let on_dropdown_action = move |action: DropdownAction| match action {
        DropdownAction::Edit => state.set(State::Edit),
        DropdownAction::Delete => state.set(State::ConfirmDelete),
        DropdownAction::MakeTask => make_task(()),
        DropdownAction::Bump => {
            layer.edit_note(note.id, NoteBuilder::new().created_at(chrono::Local::now()));
            state.set(State::Normal);
        }
    };

    let dropdown = if let State::Dropdown(x, y) = *state.get() {
        Some(rsx! {
            Dropdown {
                pos: (x, y),
                note: note.clone(),
                on_action: on_dropdown_action,
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

    let task_button = match cx.props.note.task_state {
        TaskState::NotATask => rsx! { div { class: "task-button-place" } },
        TaskState::Todo => {
            let onclick = move |_| {
                layer.edit_note(
                    cx.props.note.id,
                    NoteBuilder::new()
                        .task_state(TaskState::Done)
                        .done_at(Some(chrono::Local::now())),
                );
            };
            rsx! {
                div {
                    class: "task-button todo",
                    onclick: onclick,
                    title: "TODO"
                }
            }
        }
        TaskState::Done => {
            let onclick = move |_| {
                layer.edit_note(
                    cx.props.note.id,
                    NoteBuilder::new().task_state(TaskState::Todo).done_at(None),
                );
            };
            rsx! {
                div {
                    class: "task-button done",
                    onclick: onclick,
                    title: "DONE"
                }
            }
        }
    };

    // Overlay for done notes
    let overlay = if cx.props.note.task_state == TaskState::Done {
        Some(rsx! {
            div {
                class: "note-overlay",
                style: "background-color: rgba(255, 255, 255, 0.5);"
            }
        })
    } else {
        None
    };
    let overlay = if let State::Dropdown { .. } | State::ConfirmDelete = *state.get() {
        Some(rsx! {
            overlay,
            div {
                class: "note-overlay",
                style: "background-color: rgba(200, 200, 255, 0.3);"
            }
        })
    } else {
        overlay
    };

    let confirm_delete = if let State::ConfirmDelete = *state.get() {
        Some(rsx! {
            ConfirmDialog {
                title: "Delete Note",
                message: "Are you sure you want to delete this note?",
                on_confirm: actually_delete,
                on_cancel: |_| state.set(State::Normal),
            }
        })
    } else {
        None
    };

    let subjects = note
        .subjects
        .iter()
        .copied()
        .filter(|s| Some(*s) != cx.props.hide_subject)
        .collect();

    let on_click_subject = move |subject: Subject| match cx.props.subject_select {
        OnSubjectSelect::Switch => {
            view_state.write().go_to_note(note.id, subject.id);
        }
        OnSubjectSelect::Ignore => {}
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
                class: "note-row",
                SubjectCards {
                    sids: subjects,
                    on_click_subject: on_click_subject,
                },
                task_button,
                div {
                    class: "note",
                    onmousedown: on_mousedown,
                    overlay,
                    div {
                        class: "note-content",
                        title: "{time_text}",
                        Markdown {
                            source: text.to_owned(),
                        }
                    },
                },
                dropdown,
                confirm_delete,
            }
        }
    };

    cx.render(content)
}

enum DropdownAction {
    Edit,
    Delete,
    MakeTask,
    Bump,
}

#[derive(Props)]
struct DropdownProps<'a> {
    pos: (f64, f64),
    note: Note,
    on_action: EventHandler<'a, DropdownAction>,
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
                onclick: |_| cx.props.on_action.call(DropdownAction::MakeTask),
                if cx.props.note.task_state == TaskState::NotATask {
                    "Make Task"
                } else {
                    "Make Note"
                }
            },
            div {
                class: "note-dropdown-item",
                onclick: |_| cx.props.on_action.call(DropdownAction::Bump),
                "Bump to Today"
            },
            div {
                class: "note-dropdown-item",
                onclick: |_| cx.props.on_action.call(DropdownAction::Edit),
                "Edit"
            },
            div {
                class: "note-dropdown-item",
                onclick: |_| cx.props.on_action.call(DropdownAction::Delete),
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
    let subjects = use_subjects(cx).read().clone();

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
