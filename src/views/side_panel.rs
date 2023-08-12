use dioxus::prelude::*;
use emergence::data::subjects::{Subject, SubjectId};
use sir::css;

use crate::views::{select_subject::SelectSubject, view_note::ViewNote};

use super::ViewState;

#[derive(Default, Clone)]
pub enum SidePanelState {
    #[default]
    Nothing,
    SubjectDetails(SubjectId),
    ListSimilar {
        text: String,
        previous: Box<SidePanelState>,
    },
}

impl SidePanelState {
    pub fn list_similar(&mut self, text: String) {
        let old = std::mem::replace(self, Self::Nothing);
        *self = match old {
            Self::ListSimilar { text: _, previous } => Self::ListSimilar {
                text,
                previous: previous.clone(),
            },
            _ => Self::ListSimilar {
                text,
                previous: Box::new(old),
            },
        }
    }

    pub fn back(&mut self) {
        let old = std::mem::replace(self, Self::Nothing);
        *self = match old {
            Self::ListSimilar { text: _, previous } => *previous,
            _ => old,
        }
    }
}

pub fn SidePanel(cx: Scope) -> Element {
    let view_state = use_shared_state::<ViewState>(cx).unwrap();
    let subjects = view_state.read().layer.get_subjects();

    let view_state_read = view_state.read();
    let ViewState {
        selected_subject, ..
    } = &*view_state_read;

    let subject_name = selected_subject
        .and_then(|id| subjects.get(&id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Journal".to_string());

    let content = match &view_state_read.side_panel {
        SidePanelState::Nothing => rsx! {
            div {
            }
        },
        SidePanelState::SubjectDetails(subject) => rsx! {
            SubjectDetails {
                subject_id: subject.clone(),
            }
        },
        SidePanelState::ListSimilar { text, .. } => rsx! {
            FindSimilar {
                text: text.clone(),
            }
        },
    };

    let style = css!(
        "
        background-color: #ddd;
        border-left: 1px solid #ccc;
        overflow: hidden;

        display: grid;
        grid-template-rows: auto 1fr;

        .side-panel-header {
            font-size: 1.2em;
            font-weight: bold;
            margin-bottom: 10px;
            text-align: center;
        }

    "
    );

    let header = css!(
        "
        padding: 10px;
        display: flex;
        flex-direction: column;
        border-bottom: 1px solid #bbb;

        .row {
            display: flex;
            flex-direction: row;
            
            .title {
                font-weight: bold;
                font-size: 1.4em;
                padding-left: 15px;
                flex-grow: 1;
            }
            .select-button {
                flex-grow: 0;
                padding: 5px;
                background-color: #bbb;
                font-weight: bold;
                cursor: pointer;
    
                &.selected {
                    background-color: rgb(180, 200, 230);
                }
            }
        }
    "
    );

    let show_subject_select = use_state(cx, || false);

    cx.render(rsx! {
        div {
            class: "{style}",
            div {
                class: "{header}",
                div {
                    class: "row",
                    div {
                        class: "title",
                        "{subject_name}"
                    }
                    if selected_subject.is_some() {
                        rsx! {
                            div {
                                class: "select-button",
                                onclick: move |_| {
                                    view_state.write().go_to_journal();
                                },
                                "Journal"
                            }
                        }
                    }
                    div {
                        class: "select-button",
                        onclick: move |_| {
                            show_subject_select.set(!*show_subject_select.get());
                        },
                        "▼"
                    }
                }
                if *show_subject_select.get() {
                    rsx! {
                        SelectSubject {
                            on_select: move |s: Subject| {
                                view_state.write().go_to_subject(s.id);
                                show_subject_select.set(false);
                            },
                            on_cancel: |_| show_subject_select.set(false),
                            ignore_subjects: vec![],
                        }
                    }
                }
            }
            content
        }
    })
}

#[inline_props]
fn SubjectDetails(cx: Scope, subject_id: SubjectId) -> Element {
    let view_state = use_shared_state::<ViewState>(cx).unwrap();

    let subjects = view_state.read().layer.get_subjects();
    let my_subject = subjects.get(&subject_id).unwrap().clone();

    let children = view_state
        .read()
        .layer
        .get_subject_children(*subject_id)
        .into_iter()
        .map(|subject| {
            rsx! {
                div {
                    key: "{subject.id.0}",
                    class: "subject-card",
                    onclick: move |_| {
                        view_state.write().go_to_subject(subject.id);
                    },
                    "{subject.name}"
                }
            }
        })
        .collect::<Vec<_>>();

    let show_parent_select = use_state(cx, || false);
    let set_parent = move |parent: Option<SubjectId>| {
        view_state
            .write()
            .layer
            .set_subject_parent(*subject_id, parent)
    };

    let style = css!(
        "
        padding: 10px;
        display: flex;
        flex-direction: column;
        gap: 10px;

        * .subject-card {
            font-size: 1.0em;
        }

        .parent-row {
            display: flex;
            flex-direction: row;
            align-items: center;
            gap: 10px;

            .clear {
                padding: 0 5px;
                cursor: pointer;

                &:hover {
                    color: red;
                    background-color: #bbb;
                }
            }
        }

        .children {
            display: flex;
            flex-direction: column;
            gap: 5px;
        }
    "
    );

    let parent = if let Some(parent_id) = my_subject.parent_id {
        let parent = subjects.get(&parent_id).unwrap().clone();
        rsx! {
            div {
                class: "parent-row",
                div {
                    "Parent:"
                }
                div {
                    class: "subject-card",
                    onclick: move |_| {
                        view_state.write().go_to_subject(parent.id);
                    },
                    div {
                        "{parent.name}"
                    }
                }
                div {
                    class: "clear",
                    onclick: move |_| {
                        set_parent(None);
                    },
                    div {
                        "✖"
                    }
                }
            }
        }
    } else {
        rsx! {
            div {
                class: "parent-row",
                div {
                    "Parent:"
                }
                div {
                    class: "subject-card",
                    onclick: move |_| {
                        show_parent_select.set(true);
                    },
                    div {
                        "<< Select Parent >>"
                    }
                }
            }
            if *show_parent_select.get() {
                rsx! {
                    SelectSubject {
                        on_select: move |subject: Subject| {
                            set_parent(Some(subject.id));
                            show_parent_select.set(false);
                        },
                        on_cancel: |_| {
                            show_parent_select.set(false);
                        },
                        ignore_subjects: vec![*subject_id],
                        show_above: false,
                    }
                }
            }
        }
    };

    cx.render(rsx! {
        div {
            class: "{style}",
            div {
                parent
            }
            div {
                class: "children",
                children.into_iter()
            }
        }
    })
}

#[inline_props]
fn FindSimilar(cx: Scope, text: String) -> Element {
    let view_state = use_shared_state::<ViewState>(cx).unwrap();

    let counter = view_state.read().layer.event_count();

    let similar = use_future(cx, (text, &counter), |(text, _)| {
        let search = view_state.read().layer.search();
        async move { search.find_similar(text).await }
    });

    let notes = similar.value()?;

    let style = css!(
        "
        display: flex;
        flex-direction: column;
        gap: 10px;
        overflow: hidden;
        padding: 10px 0px 0px 10px;

        .similar-notes {
            display: grid;
            gap: 10px;
            overflow-y: scroll;

            .wrapper {
                max-height: 100px;
                height: fit-content;
                padding-top: 5px;
                overflow: hidden;
            }
        }
    "
    );

    let elems = notes
        .iter()
        .map(|note| {
            rsx! {
                div {
                    class: "wrapper",
                    ViewNote {
                        key: "{note.id.0}",
                        note: note.clone(),
                        on_select_subject: |_| {},
                        hide_subject: None,
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    cx.render(rsx! {
        div {
            class: "{style}",
            div {
                class: "side-panel-header",
                "Similar Notes"
            }
            div {
                class: "similar-notes",
                elems.into_iter()
            }
        }
    })
}
