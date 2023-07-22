use dioxus::prelude::*;
use emergence::data::{
    query::{use_store, use_store_event_query, use_subject_query},
    subjects::{Subject, SubjectId},
};
use sir::css;

use crate::views::{select_subject::SelectSubject, view_note::ViewNote};

use super::journal::SelectedSubject;

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
    let state = use_shared_state::<SidePanelState>(cx).unwrap();

    let state = state.read();
    let content = match &*state {
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

    let style = css!("
        padding: 10px;
        padding-right: 0;
        background-color: #ddd;
        border-left: 1px solid #ccc;
        overflow: hidden;

        .side-panel-header {
            font-size: 1.2em;
            font-weight: bold;
            margin-bottom: 10px;
            text-align: center;
        }
    ");

    cx.render(rsx! {
        div {
            class: "{style}",
            content
        }
    })
}

#[inline_props]
fn SubjectDetails(cx: Scope, subject_id: SubjectId) -> Element {
    let selected_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let side_panel = use_shared_state::<SidePanelState>(cx).unwrap();
    let subjects = use_subject_query(cx).subjects();
    let my_subject = subjects.get(&subject_id).unwrap().clone();

    let store = use_store(cx);
    let children = store
        .read()
        .get_subject_children(*subject_id)
        .unwrap()
        .into_iter()
        .map(|subject| {
            rsx! {
                div {
                    key: "{subject.id.0}",
                    class: "subject-card",
                    onclick: move |_| {
                        selected_subject.write().0 = Some(subject.id);
                        *side_panel.write() = SidePanelState::SubjectDetails(subject.id);
                    },
                    "{subject.name}"
                }
            }
        })
        .collect::<Vec<_>>();

    let show_parent_select = use_state(cx, || false);
    let set_parent = move |parent: Option<SubjectId>| {
        store
            .read()
            .set_subject_parent(*subject_id, parent)
            .unwrap();
    };

    let style = css!(
        "
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
                    key: "{parent.id.0}",
                    class: "subject-card",
                    onclick: move |_| {
                        selected_subject.write().0 = Some(parent.id);
                        *side_panel.write() = SidePanelState::SubjectDetails(parent.id);
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
                    key: "{parent.id.0}",
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
                class: "side-panel-header",
                "Subject Details"
            }
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
    let search = use_store(cx).read().search.clone();

    let counter = use_store_event_query(cx).count();

    let similar = use_future(cx, (text, &counter), |(text, _)| async move {
        search.find_similar(text.clone()).await
    });

    let notes = similar.value()?;

    let style = css!(
        "
        display: flex;
        flex-direction: column;
        gap: 10px;

        .similar-notes {
            display: flex;
            flex-direction: column;
            gap: 10px;

            .wrapper {
                max-height: 100px;
                height: fit-content;
                padding-top: 5px;
                overflow: hidden;
            }
        }
    ");

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
