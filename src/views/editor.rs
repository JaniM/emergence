#![allow(dead_code)]
use dioxus::prelude::*;
use dioxus_signals::*;

enum Block {
    NormalText(String),
    ListItem(String),
}

struct EditorState {
    blocks: Vec<Vec<Block>>,
    current: (usize, usize),
}

type Key = (usize, usize);

impl EditorState {
    fn new() -> Self {
        let blocks = vec![vec![Block::NormalText(String::new())]];
        Self {
            blocks,
            current: (0, 0),
        }
    }

    fn current_block(&self) -> &Block {
        let (group, block) = self.current;
        &self.blocks[group][block]
    }

    fn get(&self, key: (usize, usize)) -> &Block {
        &self.blocks[key.0][key.1]
    }

    fn get_mut(&mut self, key: (usize, usize)) -> &mut Block {
        &mut self.blocks[key.0][key.1]
    }
}

pub fn Editor(cx: Scope) -> Element {
    let state = use_context_provider(cx, || Signal::new(EditorState::new()));

    let now = state.read();

    let blocks = now
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(group_idx, g)| {
            g.iter()
                .enumerate()
                .map(move |(block_idx, _)| (group_idx, block_idx))
        })
        .map(|block| rsx! { Block { id: block }});

    cx.render(rsx! {
        blocks
    })
}

#[inline_props]
fn Block(cx: Scope, id: Key) -> Element {
    todo!()
}

fn NormalText(cx: Scope) -> Element {
    todo!()
}

fn List(cx: Scope) -> Element {
    todo!()
}
