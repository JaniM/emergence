pub mod command_palette;
pub mod confirm_dialog;
pub mod journal;
pub mod list_notes;
pub mod markdown;
pub mod note_input;
pub mod scroll_to;
pub mod search_view;
pub mod select_subject;
pub mod side_panel;
pub mod view_note;

use dioxus::prelude::{use_context, ScopeState};
use dioxus_signals::Signal;
use emergence::data::{
    layer::Layer,
    notes::{NoteId, NoteSearch},
    subjects::SubjectId,
};

use self::side_panel::SidePanelState;

pub struct ViewState {
    pub layer: Signal<Layer>,
    pub show_input: bool,
    pub show_search: bool,
    pub search_text: String,
    pub tasks_only: bool,
    pub scroll_to_note: Option<NoteId>,
    pub selected_subject: Option<SubjectId>,
    pub side_panel: SidePanelState,
    pub command_palette: bool,
}

impl ViewState {
    pub fn new(layer: Signal<Layer>) -> Self {
        Self {
            layer,
            show_input: false,
            show_search: false,
            search_text: String::new(),
            tasks_only: false,
            scroll_to_note: None,
            selected_subject: None,
            side_panel: SidePanelState::Nothing,
        }
    }

    pub fn go_to_subject(&mut self, subject: SubjectId) {
        self.selected_subject = Some(subject);
        self.scroll_to_note = None;
        self.side_panel = SidePanelState::SubjectDetails(subject);
        self.show_search = false;
        self.update_notes();
    }

    pub fn go_to_note(&mut self, note: NoteId, subject: SubjectId) {
        self.go_to_subject(subject);
        self.scroll_to_note = Some(note);
    }

    fn update_notes(&self) {
        self.layer.write().set_search(NoteSearch {
            subject_id: self.selected_subject,
            task_only: self.tasks_only,
        })
    }

    pub fn show_search(&mut self) {
        self.show_search = true;
        self.tasks_only = false;
    }

    pub fn show_tasks_only(&mut self) {
        self.tasks_only = true;
        self.show_search = false;
        self.update_notes();
    }

    pub fn show_notes_only(&mut self) {
        self.tasks_only = false;
        self.show_search = false;
        self.update_notes();
    }

    pub fn go_to_journal(&mut self) {
        self.selected_subject = None;
        self.scroll_to_note = None;
        self.side_panel = SidePanelState::Nothing;
        self.update_notes();
    }

    pub fn start_note_input(&mut self) {
        self.show_input = true;
    }

    pub fn finish_note_input(&mut self, created_new: bool) {
        self.show_input = false;
        if created_new {
            self.scroll_to_note = None;
        }
    }

    pub fn set_search_text(&mut self, text: String) {
        self.search_text = text;
    }
}

pub fn use_view_state(cx: &ScopeState) -> Signal<ViewState> {
    *use_context(cx).expect("Layer should be provided")
}
