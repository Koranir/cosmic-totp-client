use std::path::PathBuf;

use cosmic::{cosmic_config::ConfigGet, iced::Length, widget};
use entry::{NewEntry, TotpIcon};
use errors::ErrorMsg;
use secrets::{PassphraseState, SecretState};

mod entry;
mod errors;
mod secrets;

fn get_secrets_data(config: &cosmic::cosmic_config::Config) -> (SecretState, Vec<ErrorMsg>) {
    match config.get("secrets") {
        Ok(sec) => (
            SecretState::RequestingPassphrase { secret_data: sec },
            Vec::new(),
        ),
        Err(e) => (
            SecretState::NoSecretsFile,
            vec![ErrorMsg::new(format!("Failed to get secrets file: {e}"))],
        ),
    }
}

pub struct App {
    core: cosmic::app::Core,
    passphrase: PassphraseState,
    secret_state: SecretState,
    errors: Vec<ErrorMsg>,
    toasts: cosmic::widget::Toasts<Message>,
    config: cosmic::cosmic_config::Config,
    new_entry: Option<NewEntry>,
    open_details: Option<uuid::Uuid>,
    potential_deletion: Option<uuid::Uuid>,
    popup: Option<cosmic::iced::window::Id>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePassphraseVisible,
    RemoveError(u32),
    PassphraseInput(String),
    PassphraseSubmitted,
    NewEntry,
    SaveNewEntry,
    NewEntryName(String),
    NewEntrySecret(String),
    NewEntryIcon(String),
    IconFileFind,
    IconFileFound(Option<rfd::FileHandle>),
    RecalcNeeded,
    OpenDetails(uuid::Uuid),
    CopyCode(uuid::Uuid),
    RemoveToast(widget::ToastId),
    CancelNewEntry,
    CloseDetails,
    MaybeDelete(uuid::Uuid),
    CancelDeleteEntry,
    DeleteEntry(uuid::Uuid),
    Popup,
    Logout,
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::single::Executor;

    type Flags = crate::AppConfig;

    type Message = Message;

    const APP_ID: &'static str = crate::APP_ID;

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::app::Core,
        _flags: Self::Flags,
    ) -> (Self, cosmic::app::Task<Self::Message>) {
        let config = cosmic::cosmic_config::Config::new(crate::APP_ID, crate::CONFIG_VER)
            .expect("there should be a config path available");
        let (secrets, errors) = get_secrets_data(&config);
        (
            Self {
                core,
                secret_state: secrets,
                passphrase: PassphraseState::Inputting {
                    input: String::new(),
                    hidden: true,
                },
                errors,
                config,
                new_entry: None,
                toasts: widget::Toasts::new(Message::RemoveToast),
                open_details: None,
                potential_deletion: None,
                popup: None,
            },
            cosmic::app::Task::none(),
        )
    }

    fn view(&self) -> cosmic::Element<Self::Message> {
        self.core
            .applet
            .icon_button("com.koranir.CosmicTotpClient-symbolic")
            .on_press(Message::Popup)
            .into()
    }

    fn view_window(&self, _id: cosmic::iced::window::Id) -> cosmic::Element<Self::Message> {
        let mut col = widget::column().padding(10).spacing(5);

        for e in &self.errors {
            col = col.push(e.view().map(Message::RemoveError));
        }

        if let Some(entry) = &self.new_entry {
            let entry_dialog = entry.view_dialog();

            col = col.push(
                widget::container(
                    widget::container(entry_dialog)
                        .class(cosmic::style::Container::Dialog)
                        .padding(cosmic::theme::active().cosmic().radius_m()[0]),
                )
                .padding(cosmic::theme::active().cosmic().space_s()),
            );
        } else {
            match &self.passphrase {
                PassphraseState::Inputting { input, hidden } => {
                    col = col.push(
                        widget::column()
                            .spacing(cosmic::theme::active().cosmic().space_xs())
                            .padding(cosmic::theme::active().cosmic().space_m())
                            .push(widget::text::heading("Enter Passphrase"))
                            .push(
                                widget::secure_input(
                                    "passphrase",
                                    input.clone(),
                                    Some(Message::TogglePassphraseVisible),
                                    *hidden,
                                )
                                .on_input(Message::PassphraseInput)
                                .on_submit(Message::PassphraseSubmitted),
                            ),
                    );
                }
                PassphraseState::Recieved(_secret_box) => {
                    if let SecretState::LoadedSecrets { entries } = &self.secret_state {
                        if let Some(id) = self.open_details {
                            if let Some(entry) = entries.iter().find(|e| e.id == id) {
                                col = col.push(entry.view_page());
                            }
                        } else {
                            col = col.push(
                                widget::row()
                                    .push(
                                        widget::button::destructive("Logout")
                                            .on_press(Message::Logout),
                                    )
                                    .push(widget::horizontal_space())
                                    .push(widget::button::suggested("New Entry").on_press_maybe(
                                        self.new_entry.is_none().then_some(Message::NewEntry),
                                    )),
                            );

                            if entries.is_empty() {
                                col = col.push(
                                    widget::text::title1("No Entries")
                                        .center()
                                        .width(Length::Fill)
                                        .height(Length::Fill),
                                );
                            } else {
                                let mut list = widget::list_column();

                                for entry in entries {
                                    list = list.add(entry.view());
                                }

                                col = col.push(list);
                            }
                        }
                    }
                }
            };
        }

        self.core
            .applet
            .popup_container(widget::container(widget::toaster(&self.toasts, col)))
            .into()
    }

    fn on_close_requested(&self, id: cosmic::iced::window::Id) -> Option<Self::Message> {
        if let Some(popup_id) = self.popup {
            if popup_id == id {
                return Some(Message::Popup);
            }
        }

        None
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::time::every(cosmic::iced::time::Duration::from_secs(1))
            .map(|_| Message::RecalcNeeded)
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::TogglePassphraseVisible => {
                if let PassphraseState::Inputting { hidden, .. } = &mut self.passphrase {
                    *hidden = !*hidden;
                }
            }
            Message::RemoveError(id) => self.errors.retain(|e| e.id != id),
            Message::PassphraseInput(s) => {
                if let PassphraseState::Inputting { input, .. } = &mut self.passphrase {
                    *input = s;
                }
            }
            Message::PassphraseSubmitted => {
                if let PassphraseState::Inputting { input, hidden } = &mut self.passphrase {
                    let hidden = *hidden;
                    let s = input.clone();
                    self.passphrase = PassphraseState::Recieved(s.as_str().into());
                    if let Err(e) = self.try_decode_secrets() {
                        self.eat_err(e);
                        self.passphrase = PassphraseState::Inputting { input: s, hidden }
                    }
                }
            }
            Message::NewEntry => self.new_entry = Some(NewEntry::default()),
            Message::CancelNewEntry => self.new_entry = None,
            Message::SaveNewEntry => {
                if let Some(e) = self.new_entry.take() {
                    match e.into_entry() {
                        Ok(e) => {
                            if let SecretState::LoadedSecrets { entries } = &mut self.secret_state {
                                entries.push(e);
                            }
                        }
                        Err((old_entry, error)) => {
                            self.new_entry = Some(old_entry);
                            self.eat_err(error);
                        }
                    }
                }
            }
            Message::NewEntryName(n) => {
                if let Some(e) = &mut self.new_entry {
                    e.name = n;
                }
            }
            Message::NewEntrySecret(s) => {
                if let Some(e) = &mut self.new_entry {
                    e.secret = s;
                }
            }
            Message::NewEntryIcon(s) => {
                if let Some(e) = &mut self.new_entry {
                    e.icon = Some(TotpIcon::Initials { initials: s });
                }
            }
            Message::IconFileFind => {
                return cosmic::app::Task::perform(
                    rfd::AsyncFileDialog::new()
                        .add_filter("images", &["png"])
                        .set_title("New Icon")
                        .pick_file(),
                    |f| cosmic::app::Message::App(Message::IconFileFound(f)),
                );
            }
            Message::IconFileFound(file_handle) => {
                if let Some(handle) = file_handle {
                    if let Some(e) = &mut self.new_entry {
                        e.icon = Some(TotpIcon::Image {
                            path: PathBuf::from(handle.path()),
                            handle: std::sync::OnceLock::new(),
                        });
                    }
                }
            }
            Message::RecalcNeeded => {
                if let SecretState::LoadedSecrets { entries } = &mut self.secret_state {
                    let mut errs = Vec::new();
                    for entry in entries {
                        if let Err(err) = entry.decoded.update(&entry.secret) {
                            errs.push(format!(
                                "Failed to calculate auth code for {}: {err}",
                                entry.name
                            ));
                        };
                    }
                    for err in errs {
                        self.eat_err(err);
                    }
                }
            }
            Message::OpenDetails(uuid) => {
                self.open_details = Some(uuid);
            }
            Message::CloseDetails => self.open_details = None,
            Message::CopyCode(uuid) => {
                if let SecretState::LoadedSecrets { entries } = &self.secret_state {
                    if let Some(e) = entries.iter().find(|e| e.id == uuid) {
                        if let Some(decoded) = e.decoded.decoded_raw() {
                            return cosmic::app::Task::batch([
                                cosmic::iced::clipboard::write(decoded.clone()),
                                self.toasts
                                    .push(widget::Toast::new(format!(
                                        "Copied '{decoded}' to clipboard"
                                    )))
                                    .map(cosmic::app::Message::App),
                            ]);
                        }
                    }
                }
            }
            Message::RemoveToast(toast_id) => self.toasts.remove(toast_id),
            Message::MaybeDelete(uuid) => self.potential_deletion = Some(uuid),
            Message::CancelDeleteEntry => self.potential_deletion = None,
            Message::DeleteEntry(uuid) => {
                self.potential_deletion = None;
                self.open_details = None;

                if let SecretState::LoadedSecrets { entries } = &mut self.secret_state {
                    entries.retain(|e| e.id != uuid);
                }

                if let Err(e) = self.try_save_secrets() {
                    self.eat_err(e);
                }
            }
            Message::Popup => return self.toggle_popup(),

            Message::Logout => {
                let (secrets, errors) = get_secrets_data(&self.config);
                for e in errors {
                    self.errors.push(e);
                }

                self.secret_state = secrets;
                self.passphrase = PassphraseState::Inputting {
                    input: String::new(),
                    hidden: true,
                };
            }
        }

        cosmic::app::Task::none()
    }

    fn dialog(&self) -> Option<cosmic::Element<Self::Message>> {
        self.potential_deletion.and_then(|id| {
            if let SecretState::LoadedSecrets { entries } = &self.secret_state {
                if let Some(e) = entries.iter().find(|e| e.id == id) {
                    return Some(e.view_remove_page());
                }
            }

            None
        })
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl App {
    pub fn toggle_popup(&mut self) -> cosmic::app::Task<Message> {
        match self.popup.take() {
            Some(id) => return cosmic::iced::platform_specific::shell::wayland::commands::popup::destroy_popup(id),
            None => {
                let id = cosmic::iced::window::Id::unique();
                let mut settings = self.core.applet.get_popup_settings(
                    self.core.main_window_id().unwrap(),
                    id,
                    None,
                    None,
                    None,
                );
                settings.positioner.size_limits = cosmic::iced::Limits::new(cosmic::iced::Size::new(200., 400.),
                    cosmic::iced::Size::new(600., 800.),
                );
                settings.positioner.size = Some((300, 600));
                self.popup = Some(id);
                return cosmic::iced::platform_specific::shell::wayland::commands::popup::get_popup(
                    settings,
                );
            },
        }
    }
}
