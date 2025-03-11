use cosmic::{iced::Length, widget};

use super::App;

pub struct ErrorMsg {
    pub id: u32,
    pub msg: String,
}

impl ErrorMsg {
    pub fn new(msg: impl Into<String>) -> Self {
        static ID_GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let id = ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Self {
            id,
            msg: msg.into(),
        }
    }

    pub fn view(&self) -> cosmic::Element<u32> {
        widget::button::destructive(&self.msg)
            .on_press(self.id)
            .trailing_icon(widget::icon::from_name("close"))
            .width(Length::Fill)
            .into()
    }
}

impl App {
    pub fn eat_err(&mut self, e: impl std::fmt::Display) {
        let msg = ErrorMsg::new(e.to_string());
        self.errors.push(msg);
    }
}
