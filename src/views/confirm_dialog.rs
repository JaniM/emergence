use dioxus::prelude::*;

#[derive(Props)]
pub struct ConfirmDialogProps<'a> {
    pub on_confirm: EventHandler<'a, ()>,
    pub on_cancel: EventHandler<'a, ()>,
    pub message: &'a str,
    pub title: &'a str,
}

pub fn ConfirmDialog<'a>(cx: Scope<'a, ConfirmDialogProps<'a>>) -> Element<'a> {
    cx.render(rsx! {
        div {
            class: "confirm-dialog-container",
            div {
                class: "confirm-dialog",
                div {
                    class: "confirm-dialog-title",
                    "{cx.props.title}"
                }
                div {
                    class: "confirm-dialog-message",
                    "{cx.props.message}"
                },
                button {
                    class: "confirm-dialog-button",
                    onclick: |_| cx.props.on_confirm.call(()),
                    "Confirm"
                },
                button {
                    class: "confirm-dialog-button",
                    onclick: |_| cx.props.on_cancel.call(()),
                    "Cancel"
                }
            }
        }
    })
}
