use crate::data::query::{use_store, use_subject_query};
use crate::data::subjects::{Subject, SubjectId};
use dioxus::html::input_data::keyboard_types::Key;
use dioxus::prelude::*;

#[derive(Props)]
pub struct Props<'a> {
    on_select: EventHandler<'a, Subject>,
    on_cancel: EventHandler<'a, ()>,
    ignore_subjects: Vec<SubjectId>,
}

pub fn SelectSubject<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let store = use_store(cx);
    let search = use_state(cx, String::new);

    let subjects = use_subject_query(cx).subjects();

    // TODO: Add semantic sorting
    let subjects = use_memo(
        cx,
        (&cx.props.ignore_subjects, &*subjects, search),
        |(ignore_subjects, subjects, search)| {
            let mut subjects = subjects
                .values()
                .filter(|s| !ignore_subjects.contains(&s.id))
                .filter(|s| s.name.to_lowercase().contains(&search.to_lowercase()))
                .cloned()
                .collect::<Vec<_>>();
            subjects.sort_unstable_by(|a, b| a.name.cmp(&b.name));
            std::rc::Rc::new(subjects)
        },
    );

    let onkeydown = {
        let subjects = subjects.clone();
        move |e: KeyboardEvent| {
            if e.key() == Key::Escape {
                cx.props.on_cancel.call(());
            }

            if e.key() == Key::Enter {
                let search = search.get();
                let subject = match subjects.first() {
                    Some(subject) if subject.name.to_lowercase() == search.to_lowercase() => {
                        subject.clone()
                    }
                    _ => store.write().add_subject(search.clone()).unwrap(),
                };
                cx.props.on_select.call(subject);
            }
        }
    };

    cx.render(rsx! {
        div {
            class: "select-subject-wrapper",
            div {
                class: "select-subject",
                textarea {
                    class: "subject-search",
                    value: "{search}",
                    rows: 1,
                    oninput: |e| search.set(e.value.clone()),
                    onkeydown: onkeydown,
                    onmounted: |e| { e.inner().set_focus(true); },
                },
                div {
                    class: "subjects",
                    subjects.iter().cloned().map(|subject| {
                        let subject2 = subject.clone();
                        rsx! {
                            div {
                                key: "{subject.id}",
                                tabindex: 0,
                                onclick: move |_| {
                                    cx.props.on_select.call(subject.clone());
                                },
                                onkeydown: move |e: KeyboardEvent| {
                                    if e.key() == Key::Enter || e.key() == Key::Character(" ".to_string()) {
                                        cx.props.on_select.call(subject2.clone());
                                    }
                                },
                                "{subject.name}"
                            }
                        }
                    })
                }
            }
        }
    })
}
