use cosmic::{
    app::Task,
    cosmic_config::{ConfigGet, ConfigSet},
    iced::Subscription,
};
use tracing::{error, info, warn};

mod entry;
mod errors;
mod secrets;

pub struct App {
    core: cosmic::app::Core,
    config: cosmic::cosmic_config::Config,
    popup: Option<cosmic::iced::window::Id>,

    secret: secrets::State,
    new_entry: Option<entry::Entry>,
    entry_error: Option<String>,

    user: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    RetrievedKey(Result<secrets::State, String>),
    UsernameInput(String),
    UsernameSubmit(String),
    Logout,
    Save,
    SetKey(Result<(), String>),
    NewEntry,
    Entry(entry::EntryR, entry::EntryMessage),
    EntryClearError,
    NewEntryCancel,
    NewEntryAccept,
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
        let user = config.get::<Option<String>>("last-user").ok().flatten();
        (
            Self {
                core,
                config,
                popup: None,
                secret: secrets::State::PendingUser,
                user,
                new_entry: None,
                entry_error: None,
            },
            cosmic::app::Task::none(),
        )
    }

    fn view(&self) -> cosmic::Element<Self::Message> {
        self.core
            .applet
            .icon_button("com.koranir.CosmicTotpClient-symbolic")
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: cosmic::iced::window::Id) -> cosmic::Element<Self::Message> {
        use cosmic::widget::{button, column, horizontal_space, icon, row, text_input, warning};

        let mut content = column().padding(10).spacing(5);
        if matches!(&self.secret, secrets::State::PendingUser) {
            content = content.push(
                text_input("username", self.user.as_deref().unwrap_or(""))
                    .password()
                    .on_input(Message::UsernameInput)
                    .on_submit(Message::UsernameSubmit),
            );
        } else if let Some(entry) = &self.new_entry {
            content = content
                .push(
                    entry
                        .view_settings(true)
                        .map(|m| Message::Entry(entry::EntryR::NewEntry, m)),
                )
                .push(
                    row()
                        .push(button::destructive("Cancel").on_press(Message::NewEntryCancel))
                        .push(horizontal_space())
                        .push(button::suggested("Create").on_press(Message::NewEntryAccept)),
                )
                .push_maybe(
                    self.entry_error
                        .as_deref()
                        .map(|s| warning(s).on_close(Message::EntryClearError)),
                );
        } else {
            let logout =
                button::icon(icon::from_name("system-log-out-symbolic")).on_press(Message::Logout);
            let new_entry =
                button::icon(icon::from_name("list-add-symbolic")).on_press(Message::NewEntry);
            let system_bar = row().push(logout).push(horizontal_space()).push(new_entry);
            content = content.push(system_bar);
            let mut column = cosmic::widget::list_column();
            for (idx, entry) in self.secret.as_array().iter().enumerate() {
                column = column.add(entry.view().map(move |m| {
                    Message::Entry(entry::EntryR::Index(idx.try_into().unwrap()), m)
                }));
            }
            content = content.push(column);
        }

        self.core.applet.popup_container(content).into()
    }

    fn on_close_requested(&self, id: cosmic::iced::window::Id) -> Option<Self::Message> {
        if let Some(popup_id) = self.popup
            && popup_id == id
        {
            return Some(Message::TogglePopup);
        }

        None
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        self.popup.map_or_else(Subscription::none, |p| {
            Subscription::batch(
                self.secret
                    .as_array()
                    .iter()
                    .enumerate()
                    .map(|(idx, entry)| {
                        entry
                            .subscription(p)
                            .with(entry::EntryR::Index(idx.try_into().unwrap()))
                            .map(move |(r, m)| Message::Entry(r, m))
                    }),
            )
        })
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "as the most important function in the application, it is expected to be complex"
    )]
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => return self.toggle_popup(),
            Message::RetrievedKey(state) => match state {
                Ok(state) => {
                    self.secret = state;
                }
                Err(e) => {
                    error!("Failed to retrieve secret key: {e}");
                }
            },
            Message::SetKey(r) => {
                if let Err(e) = r {
                    error!("Failed to set secret key: {e}");
                }
            }
            Message::UsernameInput(s) => self.user = Some(s),
            Message::UsernameSubmit(s) => {
                self.user = Some(s);
                let task = self.update(Message::Save);
                return Task::batch([task, self.get_secret_key()]);
            }
            Message::Logout => {
                self.secret = secrets::State::PendingUser;
                self.user = None;
                return self.update(Message::Save);
            }
            Message::Save => {
                info!("Saving last used user '{:?}'", self.user);
                if let Err(e) = self.config.set("last-user", self.user.clone()) {
                    error!("Couldn't save last user: {e}");
                }
                return self.set_secret_key();
            }
            Message::NewEntry => {
                if self.new_entry.is_none() {
                    self.new_entry = Some(entry::Entry::new());
                }
            }
            Message::Entry(entry_r, message) => {
                let entry = match entry_r {
                    entry::EntryR::NewEntry => self.new_entry.as_mut(),
                    entry::EntryR::Index(idx) => self.secret.as_mut_array().get_mut(idx as usize),
                };
                if let Some(entry_mut) = entry {
                    match entry_mut.update(message) {
                        Ok(m) => {
                            self.entry_error = None;
                            return m.map(move |m| cosmic::Action::App(Message::Entry(entry_r, m)));
                        }
                        Err(e) => {
                            warn!("{e}");
                            self.entry_error = Some(e);
                        }
                    }
                } else {
                    error!("Wanted to pass message to entry {entry_r:?}, but it did not exist");
                }
            }
            Message::EntryClearError => self.entry_error = None,
            Message::NewEntryCancel => self.new_entry = None,
            Message::NewEntryAccept => {
                if let Some(entry) = self.new_entry.take() {
                    match self.secret.try_push(entry) {
                        Ok(()) => {
                            return self.update(Message::Save);
                        }
                        Err(e) => {
                            self.new_entry = Some(e);
                            error!("Failed to insert entry, not loaded yet?");
                        }
                    }
                }
            }
        }
        cosmic::app::Task::none()
    }

    // fn dialog(&self) -> Option<cosmic::Element<Self::Message>> {}

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl App {
    pub fn toggle_popup(&mut self) -> cosmic::app::Task<Message> {
        info!("Toggling popup window");

        if let Some(id) = self.popup.take() {
            info!("Popup exists, removing");
            return cosmic::iced::platform_specific::shell::wayland::commands::popup::destroy_popup(
                id,
            );
        }

        info!("Popup doesn't exist, creating");
        let id = cosmic::iced::window::Id::unique();
        let mut settings = self.core.applet.get_popup_settings(
            self.core.main_window_id().unwrap(),
            id,
            None,
            None,
            None,
        );
        settings.positioner.size_limits = cosmic::iced::Limits::new(
            cosmic::iced::Size::new(200., 400.),
            cosmic::iced::Size::new(600., 800.),
        );
        settings.positioner.size = Some((300, 600));
        self.popup = Some(id);

        let popup_task =
            cosmic::iced::platform_specific::shell::wayland::commands::popup::get_popup(settings);
        let secret_task = match &self.secret {
            secrets::State::PendingUser => self.get_secret_key(),
            secrets::State::Secrets(_) => Task::none(),
        };

        Task::batch([popup_task, secret_task])
    }

    pub fn get_secret_key(&self) -> Task<Message> {
        self.user.clone().map_or_else(Task::none, |user| {
            Task::perform(secrets::get_secret_key(user), |s| {
                cosmic::Action::App(Message::RetrievedKey(s))
            })
        })
    }
    pub fn set_secret_key(&self) -> Task<Message> {
        self.user
            .clone()
            .map_or_else(Task::none, |user| match &self.secret {
                secrets::State::PendingUser => Task::none(),
                secrets::State::Secrets(hash_map) => {
                    Task::perform(secrets::set_secret_key(user, hash_map.clone()), |s| {
                        cosmic::Action::App(Message::SetKey(s))
                    })
                }
            })
    }
}
