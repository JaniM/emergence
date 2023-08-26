use std::collections::BTreeMap;
use std::rc::Rc;

use crate::data::subjects::{Subject, SubjectId};
use dioxus::html::input_data::keyboard_types::Key;
use dioxus::prelude::*;
use emergence::data::layer::{use_layer, use_subjects};
use sir::css;

const FOLDER_ICON: &str = "▼";

#[derive(Props)]
pub struct Props<'a> {
    on_select: EventHandler<'a, Subject>,
    on_cancel: EventHandler<'a, ()>,
    ignore_subjects: Vec<SubjectId>,
    #[props(default = false)]
    show_above: bool,
}

pub fn SelectSubject<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let layer = use_layer(cx);
    let all_subjects = use_subjects(cx);
    let search = use_state(cx, String::new);

    // TODO: Add semantic sorting
    let (filtered_subjects, subject_tree) = use_memo(
        cx,
        (&cx.props.ignore_subjects, &*all_subjects.read(), search),
        |(ignore_subjects, all_subjects, search)| {
            let mut subjects = all_subjects
                .values()
                .filter(|s| !ignore_subjects.contains(&s.id))
                .filter(|s| s.name.to_lowercase().contains(&search.to_lowercase()))
                .cloned()
                .collect::<Vec<_>>();
            subjects.sort_unstable_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            let subject_tree = construct_subject_tree(
                &all_subjects,
                &subjects.iter().map(|s| s.id).collect::<Vec<_>>(),
            );
            (std::rc::Rc::new(subjects), subject_tree)
        },
    );

    let onkeydown = {
        let subjects = filtered_subjects.clone();
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
                    _ => layer.create_subject(search.clone()),
                };
                cx.props.on_select.call(subject);
            }
        }
    };

    let above_style = if cx.props.show_above {
        "bottom: 0; top: auto;"
    } else {
        ""
    };

    let textarea = rsx! {
        textarea {
            class: "subject-search",
            value: "{search}",
            rows: 1,
            tabindex: 100,
            oninput: |e| search.set(e.value.clone()),
            onkeydown: onkeydown,
            onmounted: |e| { e.inner().set_focus(true); },
        },
    };

    let tree_view_style = css!(
        "
        display: flex;
        flex-direction: column;
        overflow-y: scroll;
        max-height: 400px;
        gap: 5px;
        "
    );

    let all_subjects = all_subjects.read();
    let tree_view = rsx! {
        div {
            class: "{tree_view_style}",
            subject_tree.roots.iter().cloned().map(|root| {
                let all_subjects = all_subjects.clone();
                let tree = subject_tree.children.clone();
                rsx! {
                    SubjectTreeView {
                        key: "{root.0}",
                        on_select: |s| cx.props.on_select.call(s),
                        subjects: all_subjects,
                        tree: tree,
                        node: root,
                    }
                }
            })
        }
    };

    let wrapper = css!(
        "
        position: relative;
        display: flex;
        flex-direction: row;
        justify-content: flex-end;

        .select-subject {
            position: absolute;
            z-index: 10;
            min-width: 200px;
            overflow: hidden;
            border: 1px solid #ccc;
            background-color: #ccd;
            display: grid;
            grid-template-columns: 1fr;
            grid-gap: 5px;

            .subject-search {
                width: auto;
                resize: none;
            }
        }
        "
    );

    cx.render(rsx! {
        div {
            class: "{wrapper}",
            div {
                class: "select-subject",
                style: above_style,
                if cx.props.show_above {
                    rsx! { tree_view, textarea }
                } else {
                    rsx! { textarea, tree_view }
                }
            }
        }
    })
}

#[derive(Props)]
struct SubjectTreeProps<'a> {
    on_select: EventHandler<'a, Subject>,
    subjects: Rc<BTreeMap<SubjectId, Subject>>,
    tree: Rc<BTreeMap<SubjectId, Vec<SubjectId>>>,
    node: SubjectId,
}

fn SubjectTreeView<'a>(cx: Scope<'a, SubjectTreeProps<'a>>) -> Element {
    let node = cx.props.node;
    let subjects = cx.props.subjects.clone();
    let tree = cx.props.tree.clone();

    let my_subject = subjects.get(&node).unwrap().clone();
    let on_select_me = {
        to_owned![my_subject];
        move |_| {
            cx.props.on_select.call(my_subject.clone());
        }
    };

    let on_keydown = {
        to_owned![my_subject];
        move |e: KeyboardEvent| {
            if e.key() == Key::Enter || e.key() == Key::Character(" ".to_string()) {
                cx.props.on_select.call(my_subject.clone());
            }
        }
    };

    let container = css!(
        "
        display: flex;
        flex-direction: row;
        width: 100%;

        .fold-button {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 15px;
            cursor: pointer;
        }

        .name {
            flex-grow: 1;
        }

        &:hover, &:focus {
            background-color: #eee;
            cursor: pointer;
        }
    "
    );

    static STATIC_VEC: Vec<SubjectId> = Vec::new();
    let children = tree.get(&node).unwrap_or(&STATIC_VEC);

    let card = rsx! {
        div {
            key: "{my_subject.id}",
            class: "{container}",
            tabindex: 101,
            onkeydown: on_keydown,
            div {
                class: "fold-button",
                if children.is_empty() {
                    ""
                } else {
                    FOLDER_ICON
                }
            }
            div {
                class: "name",
                onclick: on_select_me,
                "{my_subject.name}"
            }
        }
    };

    let children_container = css!(
        "
        display: flex;
        flex-direction: column;
        /* padding + margin + border = 15px */
        padding-left: 7px;
        margin-left: 7px;
        border-left: 1px solid #888;
    "
    );

    let subjects = subjects.clone();
    let tree = tree.clone();
    render! {
        div {
            card,
            if !children.is_empty() {
                rsx! {
                    div {
                        class: "{children_container}",
                        children.iter().cloned().map(|child| {
                            let subjects = subjects.clone();
                            let tree = tree.clone();
                            rsx! {
                                SubjectTreeView {
                                    key: "{child.0}",
                                    on_select: |s| cx.props.on_select.call(s),
                                    subjects: subjects,
                                    tree: tree,
                                    node: child,
                                }
                            }
                        })
                    }
                }
            }
        }
    }
}

struct SubjectTree {
    roots: Vec<SubjectId>,
    children: Rc<BTreeMap<SubjectId, Vec<SubjectId>>>,
}

/// Construct a tree of subjects, starting from the subjects filtered by the search query.
/// Ibcludes all parents of the filtered subjects, but not their siblings.
/// Includes all children of the filtered subjects, too.
fn construct_subject_tree(
    all_subjects: &BTreeMap<SubjectId, Subject>,
    filtered: &[SubjectId],
) -> SubjectTree {
    use std::collections::VecDeque;

    let mut queue = filtered.iter().cloned().collect::<VecDeque<_>>();
    let mut tree = BTreeMap::new();
    let mut roots = Vec::new();

    while let Some(subject_id) = queue.pop_front() {
        let subject = all_subjects.get(&subject_id).unwrap();
        tree.entry(subject_id).or_insert_with(Vec::new);
        if let Some(parent_id) = subject.parent_id {
            let entry = tree.entry(parent_id).or_insert_with(Vec::new);
            if entry.is_empty() && !filtered.contains(&parent_id) {
                queue.push_back(parent_id);
            }
            entry.push(subject_id);
        } else {
            roots.push(subject_id);
        }
    }

    // Fill missing children for matched subjects
    // TODO: Produce children list in database.
    let mut queue = tree
        .keys()
        .cloned()
        .filter(|k| filtered.contains(k))
        .collect::<VecDeque<_>>();

    while let Some(subject_id) = queue.pop_front() {
        let children_in_tree = tree.entry(subject_id).or_insert_with(Vec::new);
        let all_children = all_subjects
            .values()
            .filter(|s| s.parent_id == Some(subject_id))
            .map(|s| s.id);
        for child in all_children {
            if !children_in_tree.contains(&child) {
                children_in_tree.push(child);
            }
            queue.push_back(child);
        }
    }

    SubjectTree {
        roots,
        children: tree.into(),
    }
}
