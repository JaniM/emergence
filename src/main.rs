#![allow(non_snake_case)]

mod data;

use std::collections::BTreeMap;

// import the prelude to get access to the `rsx!` macro and the `Scope` and `Element` types
use data::{Note, Store};
use dioxus::{html::input_data::keyboard_types::{Key, Modifiers}, prelude::*};

fn main() {
    // launch the dioxus app in a webview
    dioxus_desktop::launch(App);
}

// define a component that renders a div with the text "Hello, world!"
fn App(cx: Scope) -> Element {
    use_shared_state_provider(cx, Store::new);
    let store = use_store(cx);

    cx.render(rsx! {
        div {
            NoteInput {},
            GroupByDate { nodes: store.read().get_notes() },
        }
    })
}

#[inline_props]
fn ViewNote(cx: Scope, node: data::Note) -> Element {
    let style = r"
        background: #eee;
    ";
    cx.render(rsx! {
        div {
            style: style,
            div { "{node.text}" }
        }
    })
}

#[inline_props]
fn GroupByDate(cx: Scope, nodes: Vec<Note>) -> Element {
    let style = r"
        display: grid;
        grid-template-columns: 100px 1fr;
        grid-gap: 5px;
    ";

    let group_style = r"
        display: grid;
        grid-template-columns: 1fr;
        grid-gap: 5px;
    ";

    let mut groups = BTreeMap::new();
    for node in nodes {
        let date = node.created_at.naive_local().date();
        groups.entry(date).or_insert_with(Vec::new).push(node);
    }
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.reverse();
    for (_, nodes) in &mut groups {
        nodes.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
    }
    render! {
        div {
            style: style,
            groups.into_iter().map(|(date, nodes)| {
                rsx! {
                    div {
                        r#"{date.format("%Y-%m-%d")}"#
                    },
                    div {
                        style: group_style,
                        nodes.into_iter().map(|node| {
                            rsx! { ViewNote { node: node.clone() } }
                        })
                    }
                }
            })
        }
    }
}

fn NoteInput(cx: Scope) -> Element {
    let text = use_state(cx, String::new);
    let store = use_store(cx);

    cx.render(rsx! {
        textarea {
            value: "{text}",
            oninput: |e| text.set(e.value.clone()),
            onkeypress: |e| {
                if e.key() == Key::Enter && e.modifiers().contains(Modifiers::CONTROL) {
                    store.write().add_node(text.to_string());
                    text.set(String::new());
                }
            },
        }
    })
}

fn use_store(cx: Scope) -> &UseSharedState<data::Store> {
    use_shared_state(cx).expect("Store context not set")
}
