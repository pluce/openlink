use openlink_models::OpenLinkEnvelope;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub source: String,
    pub content: String,
    pub timestamp: String,
    pub is_incoming: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Atc,
    Pilot,
}

#[derive(Debug)]
#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

pub trait AppController {
    fn update(&mut self, action: super::tui::Action);
    fn render(&mut self, f: &mut ratatui::Frame);
    fn should_quit(&self) -> bool;
}
