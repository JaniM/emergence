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

use emergence::data::{notes::NoteId, subjects::SubjectId};

use self::side_panel::SidePanelState;

pub struct ViewState {
    pub show_input: bool,
    pub show_search: bool,
    pub search_text: String,
    pub tasks_only: bool,
    pub scroll_to_note: Option<NoteId>,
    pub selected_subject: Option<SubjectId>,
    pub side_panel: SidePanelState,
}

impl ViewState {
    pub fn new() -> Self {
        Self {
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
    }

    pub fn go_to_note(&mut self, note: NoteId, subject: SubjectId) {
        self.go_to_subject(subject);
        self.scroll_to_note = Some(note);
    }

    pub fn show_search(&mut self) {
        self.show_search = true;
        self.tasks_only = false;
    }

    pub fn show_tasks_only(&mut self) {
        self.tasks_only = true;
        self.show_search = false;
    }

    pub fn show_notes_only(&mut self) {
        self.tasks_only = false;
        self.show_search = false;
    }

    pub fn go_to_journal(&mut self) {
        self.selected_subject = None;
        self.scroll_to_note = None;
        self.side_panel = SidePanelState::Nothing;
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
