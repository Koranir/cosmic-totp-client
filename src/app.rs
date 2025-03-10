use std::path::PathBuf;

use cosmic::{
    cosmic_config::{ConfigGet, ConfigSet},
    iced::Length,
    widget,
};

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

struct ErrorMsg {
    id: u32,
    msg: String,
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

pub enum PassphraseState {
    Inputting { input: String, hidden: bool },
    Recieved(age::secrecy::SecretString),
}

pub enum SecretState {
    NoSecretsFile,
    RequestingPassphrase { secret_data: Vec<u8> },
    LoadedSecrets { entries: Vec<TotpEntry> },
}

#[derive(Default)]
pub struct NewEntry {
    icon: Option<TotpIcon>,
    name: String,
    secret: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct TotpEntry {
    id: uuid::Uuid,
    icon: TotpIcon,
    name: String,
    secret: Vec<u8>,
    #[serde(skip)]
    decoded: CalcTotp,
}
impl TotpEntry {
    pub fn view_page(&self) -> cosmic::Element<Message> {
        let mut col = widget::column().spacing(cosmic::theme::active().cosmic().space_s());
        col = col.push(
            widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                .on_press(Message::CloseDetails),
        );
        col = col.push(
            widget::row()
                .spacing(cosmic::theme::active().cosmic().space_m())
                .push(self.icon.view(40.))
                .push(widget::text::title1(&self.name))
                .push(widget::horizontal_space())
                .push(widget::button::destructive("Delete").on_press(Message::MaybeDelete(self.id)))
                .align_y(cosmic::iced::alignment::Vertical::Center),
        );

        col = col.push(widget::divider::horizontal::heavy());

        let row = widget::container(
            widget::column()
                .spacing(cosmic::theme::active().cosmic().space_xs())
                .push(widget::text::heading("One-time passcode"))
                .push(
                    widget::row()
                        .align_y(cosmic::iced::alignment::Vertical::Center)
                        .spacing(cosmic::theme::active().cosmic().space_s())
                        .push(
                            widget::button::text(self.decoded.decoded_pretty())
                                .font_size(40)
                                .font_weight(cosmic::iced::font::Weight::Bold)
                                .on_press(Message::CopyCode(self.id)),
                        )
                        .push(widget::text::title3(
                            self.decoded.seconds_remaining().to_string(),
                        )),
                ),
        )
        .padding(cosmic::theme::active().cosmic().space_s())
        .width(Length::Fill)
        .class(cosmic::style::Container::Card);

        col = col.push(row);

        col.into()
    }

    pub fn view(&self) -> cosmic::Element<Message> {
        let mut row = widget::row()
            .spacing(cosmic::theme::active().cosmic().space_s())
            .align_y(cosmic::iced::alignment::Vertical::Center)
            .push(self.icon.view(20.));
        row = row.push(
            widget::column()
                .push(widget::text::title3(&self.name))
                .push(
                    widget::row()
                        .push(widget::text::title1(self.decoded.decoded_pretty()))
                        .push(widget::text::title3(
                            self.decoded.seconds_remaining().to_string(),
                        ))
                        .align_y(cosmic::iced::alignment::Vertical::Center)
                        .spacing(cosmic::theme::active().cosmic().space_xs()),
                ),
        );

        row = row.push(widget::horizontal_space());
        row = row.push(
            widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                .large()
                .on_press(Message::OpenDetails(self.id)),
        );

        widget::button::custom(row)
            .class(cosmic::style::Button::ListItem)
            .on_press(Message::CopyCode(self.id))
            .into()
    }
}

#[derive(Default)]
pub enum CalcTotp {
    #[default]
    Uninit,
    Calc {
        decoded: String,
        seconds_remaining: u64,
    },
}
impl CalcTotp {
    pub fn update(&mut self, secret: &[u8]) -> Result<(), totp_rs::TotpUrlError> {
        let totp = totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 1, 30, secret.to_vec())?;

        let unix_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let v = totp.generate(unix_time);

        *self = Self::Calc {
            decoded: v,
            seconds_remaining: 30 - unix_time % 30,
        };

        Ok(())
    }

    pub fn decoded_pretty(&self) -> String {
        match self {
            Self::Uninit => "...".into(),
            Self::Calc { decoded, .. } => {
                let mut decoded = decoded.clone();
                decoded.insert(3, ' ');
                decoded
            }
        }
    }

    pub fn decoded_raw(&self) -> Option<String> {
        match self {
            Self::Uninit => None,
            Self::Calc { decoded, .. } => Some(decoded.clone()),
        }
    }

    pub const fn seconds_remaining(&self) -> u64 {
        match self {
            Self::Uninit => 0,
            Self::Calc {
                seconds_remaining, ..
            } => *seconds_remaining,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum TotpIcon {
    Image {
        path: PathBuf,
        #[serde(skip)]
        handle: std::sync::OnceLock<widget::image::Handle>,
    },
    Initials {
        initials: String,
    },
}
impl TotpIcon {
    pub fn view(&self, radius: f32) -> cosmic::Element<Message> {
        widget::container(match self {
            Self::Image { path, handle } => cosmic::Element::from(
                widget::image(handle.get_or_init(|| widget::image::Handle::from_path(path)))
                    .width(Length::Fixed(radius * 2.0))
                    .height(Length::Fixed(radius * 2.0))
                    .border_radius([radius, radius, radius, radius])
                    .content_fit(cosmic::iced::ContentFit::Cover),
            ),
            Self::Initials { initials } => widget::text::title1(initials)
                .width(radius * 2.0)
                .height(radius * 2.0)
                .center()
                .size(radius)
                .into(),
        })
        .width(Length::Fixed(radius * 2.0))
        .height(Length::Fixed(radius * 2.0))
        .clip(true)
        .style(move |t: &cosmic::Theme| {
            cosmic::style::Container::secondary(t.cosmic()).border(cosmic::iced::Border {
                color: t.cosmic().secondary.divider.into(),
                width: 1.,
                radius: cosmic::iced::Radius::new(radius),
            })
        })
        .into()
    }
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
        let (secrets, errors) = match config.get("secrets") {
            Ok(sec) => (
                SecretState::RequestingPassphrase { secret_data: sec },
                Vec::new(),
            ),
            Err(e) => (SecretState::NoSecretsFile, vec![ErrorMsg::new(format!(
                "Failed to get secrets file: {e}"
            ))]),
        };
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
            let entry_dialog = widget::column()
                .spacing(cosmic::theme::active().cosmic().space_s())
                .push(widget::text::title3("New Entry"))
                .push(
                    cosmic::widget::row()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::heading("Icon").width(Length::Fixed(50.)))
                        .push(
                            cosmic::widget::text_input(
                                if let Some(TotpIcon::Image { path, .. }) = &entry.icon {
                                    path.to_string_lossy()
                                } else {
                                    "-".into()
                                },
                                match &entry.icon {
                                    Some(TotpIcon::Initials { initials }) => initials.as_str(),
                                    _ => "",
                                },
                            )
                            .on_input(Message::NewEntryIcon),
                        )
                        .push(
                            cosmic::widget::button::icon(
                                cosmic::widget::icon::from_name("folder-open-symbolic").handle(),
                            )
                            .on_press(Message::IconFileFind),
                        ),
                )
                .push(
                    cosmic::widget::row()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::heading("Name").width(Length::Fixed(50.)))
                        .push(
                            cosmic::widget::text_input("name", &entry.name)
                                .on_input(Message::NewEntryName),
                        ),
                )
                .push(
                    cosmic::widget::row()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(widget::text::heading("Secret").width(Length::Fixed(50.)))
                        .push(
                            cosmic::widget::text_input("XXXXXXXXXXXXXXXX", &entry.secret)
                                .on_input(Message::NewEntrySecret),
                        ),
                )
                .push(
                    cosmic::widget::container(
                        widget::row()
                            .spacing(cosmic::theme::active().cosmic().space_xxs())
                            .push(
                                widget::button::destructive("Cancel")
                                    .on_press(Message::CancelNewEntry),
                            )
                            .push(
                                widget::button::suggested("Create").on_press(Message::SaveNewEntry),
                            ),
                    )
                    .align_right(Length::Fill),
                );

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
                                widget::container(
                                    widget::button::suggested("New Entry").on_press_maybe(
                                        self.new_entry.is_none().then_some(Message::NewEntry),
                                    ),
                                )
                                .align_right(Length::Fill),
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
                    let mut secret = match totp_rs::Secret::Encoded(e.secret.clone()).to_bytes() {
                        Ok(s) => s,
                        Err(err) => {
                            self.eat_err(format!("Failed to parse secret key: {err}"));
                            self.new_entry = Some(e);
                            return cosmic::app::Task::none();
                        }
                    };
                    if secret.len() == 10 {
                        secret.extend_from_slice(&[0; 6]);
                    }
                    let mut decoded = CalcTotp::Uninit;
                    if let Err(err) = decoded.update(&secret) {
                        self.eat_err(format!("Failed to calculate auth code: {err}"));

                        self.new_entry = Some(e);
                        return cosmic::app::Task::none();
                    };
                    if let SecretState::LoadedSecrets { entries } = &mut self.secret_state {
                        entries.push(TotpEntry {
                            icon: e.icon.unwrap_or_else(|| {
                                let fch = e
                                    .name
                                    .split_whitespace()
                                    .map(|s| s.chars().next().unwrap())
                                    .collect::<Vec<_>>();

                                if fch.len() < 2 {
                                    let cch = e.name.trim().chars().collect::<Vec<_>>();
                                    let initials = cch
                                        .get(0..2)
                                        .map(|s| s.iter().copied().collect())
                                        .unwrap_or(cch.first().copied().unwrap_or('-').to_string());
                                    TotpIcon::Initials { initials }
                                } else {
                                    TotpIcon::Initials {
                                        initials: fch[0..2].iter().collect(),
                                    }
                                }
                            }),
                            name: e.name,
                            secret,
                            decoded,
                            id: uuid::Uuid::new_v4(),
                        });
                        if let Err(e) = self.try_save_secrets() {
                            self.eat_err(e);
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
            Message::Popup => {
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

        cosmic::app::Task::none()
    }

    fn dialog(&self) -> Option<cosmic::Element<Self::Message>> {
        self.potential_deletion.and_then(|id| {
            if let SecretState::LoadedSecrets { entries } = &self.secret_state {
                if let Some(e) = entries.iter().find(|e| e.id == id) {
                    return Some(
                        widget::container(
                            widget::column()
                                .push(widget::text::title1(format!("Delete '{}'?", e.name)))
                                .push(
                                    widget::row()
                                        .push(widget::horizontal_space())
                                        .push(
                                            widget::button::suggested("Cancel")
                                                .on_press(Message::CancelDeleteEntry),
                                        )
                                        .push(
                                            widget::button::destructive("Delete")
                                                .on_press(Message::DeleteEntry(id)),
                                        )
                                        .spacing(cosmic::theme::active().cosmic().space_xs()),
                                ),
                        )
                        .class(cosmic::theme::Container::Dialog)
                        .padding(cosmic::theme::active().cosmic().space_s())
                        .into(),
                    );
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
    pub fn try_save_secrets(&mut self) -> Result<(), String> {
        if let SecretState::LoadedSecrets { entries } = &self.secret_state {
            let s = serde_json::to_string(entries)
                .map_err(|e| format!("Could not serialize secrets: {e}"))?;

            let PassphraseState::Recieved(pass) = &self.passphrase else {
                return Err("Password has not been set, unable to encrypt secrets".into());
            };

            let s = match age::encrypt(&age::scrypt::Recipient::new(pass.clone()), s.as_bytes()) {
                Ok(s) => s,
                Err(e) => return Err(format!("Could not encrypt secrets: {e}")),
            };

            if let Err(e) = self.config.set("secrets", s) {
                return Err(format!("Could not save secrest file: {e}"));
            };
        }

        Ok(())
    }

    pub fn try_decode_secrets(&mut self) -> Result<(), String> {
        match (&self.secret_state, &self.passphrase) {
            (SecretState::NoSecretsFile, PassphraseState::Recieved(s)) => {
                _ = s;
                self.secret_state = SecretState::LoadedSecrets {
                    entries: Vec::new(),
                };
                Ok(())
            }
            (SecretState::RequestingPassphrase { secret_data }, PassphraseState::Recieved(s)) => {
                let secr = match age::decrypt(&age::scrypt::Identity::new(s.clone()), secret_data) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(format!(
                            "Could not decrypt secrets file - likey invalid passphrase: {e}"
                        ));
                    }
                };

                let secr = match serde_json::from_slice(&secr) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!("Could not deserialize from secrets file: {e}"));
                    }
                };

                self.secret_state = SecretState::LoadedSecrets { entries: secr };

                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn eat_err(&mut self, e: impl std::fmt::Display) {
        let msg = ErrorMsg::new(e.to_string());
        self.errors.push(msg);
    }
}
