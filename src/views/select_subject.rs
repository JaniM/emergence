use crate::data::subjects::Subject;
use crate::use_store;
use dioxus::html::input_data::keyboard_types::Key;
use dioxus::prelude::*;

#[derive(Props)]
pub struct Props<'a> {
    on_select: EventHandler<'a, Subject>,
    on_cancel: EventHandler<'a, ()>,
}

pub fn SelectSubject<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let store = use_store(cx);
    let search = use_state(cx, String::new);
    let subjects = store.read().find_subjects(search.get().as_str()).unwrap();
    let subjects = std::rc::Rc::new(subjects);

    let onkeydown = {
        let subjects = subjects.clone();
        move |e: KeyboardEvent| {
            if e.key() == Key::Escape {
                cx.props.on_cancel.call(());
            }

            if e.key() == Key::Enter {
                let subject = if let Some(subject) = subjects.first() {
                    subject.clone()
                } else {
                    store.write().add_subject(search.get().clone()).unwrap()
                };
                cx.props.on_select.call(subject);
            }
        }
    };

    cx.render(rsx! {
        div {
            class: "select-subject",
            textarea {
                class: "search",
                value: "{search}",
                rows: 1,
                oninput: |e| search.set(e.value.clone()),
                onkeydown: onkeydown,
                onmounted: |e| { e.inner().set_focus(true); },
            },
            div {
                class: "subjects",
                subjects.iter().cloned().map(|subject| {
                    rsx! {
                        div {
                            onclick: move |_| {
                                cx.props.on_select.call(subject.clone());
                            },
                            "{subject.name}"
                        }
                    }
                })
            }
        }
    })
}
