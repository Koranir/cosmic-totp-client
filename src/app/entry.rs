use std::path::PathBuf;

use cosmic::{iced::Length, widget};

use super::Message;

#[derive(Default)]
pub struct NewEntry {
    pub icon: Option<TotpIcon>,
    pub name: String,
    pub secret: String,
}
impl NewEntry {
    pub fn into_entry(self) -> Result<TotpEntry, (Self, String)> {
        let mut secret = match totp_rs::Secret::Encoded(self.secret.clone()).to_bytes() {
            Ok(s) => s,
            Err(err) => {
                return Err((self, format!("Failed to parse secret key: {err}")));
            }
        };
        if secret.len() == 10 {
            secret.extend_from_slice(&[0; 6]);
        }
        let mut decoded = CalcTotp::Uninit;
        if let Err(err) = decoded.update(&secret) {
            return Err((self, format!("Failed to calculate auth code: {err}")));
        };
        Ok(TotpEntry {
            icon: self
                .icon
                .unwrap_or_else(|| TotpIcon::default_for_name(&self.name)),
            name: self.name,
            secret,
            decoded,
            id: uuid::Uuid::new_v4(),
        })
    }

    pub fn view_dialog(&self) -> cosmic::Element<Message> {
        widget::column()
            .spacing(cosmic::theme::active().cosmic().space_s())
            .push(widget::text::title3("New Entry"))
            .push(
                cosmic::widget::row()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::heading("Icon").width(Length::Fixed(50.)))
                    .push(
                        cosmic::widget::text_input(
                            if let Some(TotpIcon::Image { path, .. }) = &self.icon {
                                path.to_string_lossy()
                            } else {
                                "-".into()
                            },
                            match &self.icon {
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
                        cosmic::widget::text_input("name", &self.name)
                            .on_input(Message::NewEntryName),
                    ),
            )
            .push(
                cosmic::widget::row()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::heading("Secret").width(Length::Fixed(50.)))
                    .push(
                        cosmic::widget::text_input("XXXXXXXXXXXXXXXX", &self.secret)
                            .on_input(Message::NewEntrySecret),
                    ),
            )
            .push(
                cosmic::widget::container(
                    widget::row()
                        .spacing(cosmic::theme::active().cosmic().space_xxs())
                        .push(
                            widget::button::destructive("Cancel").on_press(Message::CancelNewEntry),
                        )
                        .push(widget::button::suggested("Create").on_press(Message::SaveNewEntry)),
                )
                .align_right(Length::Fill),
            )
            .into()
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct TotpEntry {
    pub id: uuid::Uuid,
    pub icon: TotpIcon,
    pub name: String,
    pub secret: Vec<u8>,
    #[serde(skip)]
    pub decoded: CalcTotp,
}
impl TotpEntry {
    pub fn view_remove_page(&self) -> cosmic::Element<Message> {
        widget::container(
            widget::column()
                .push(widget::text::title1(format!("Delete '{}'?", self.name)))
                .push(
                    widget::row()
                        .push(widget::horizontal_space())
                        .push(
                            widget::button::suggested("Cancel")
                                .on_press(Message::CancelDeleteEntry),
                        )
                        .push(
                            widget::button::destructive("Delete")
                                .on_press(Message::DeleteEntry(self.id)),
                        )
                        .spacing(cosmic::theme::active().cosmic().space_xs()),
                ),
        )
        .class(cosmic::theme::Container::Dialog)
        .padding(cosmic::theme::active().cosmic().space_s())
        .into()
    }

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
    pub fn default_for_name(name: &str) -> Self {
        let fch = name
            .split_whitespace()
            .map(|s| s.chars().next().unwrap())
            .collect::<Vec<_>>();

        if fch.len() < 2 {
            let cch = name.trim().chars().collect::<Vec<_>>();
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
    }

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
