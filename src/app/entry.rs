use std::{path::PathBuf, sync::OnceLock, time::Duration};

use cosmic::{
    Apply,
    iced::{Alignment, Length, Subscription, font::Weight, futures::StreamExt, widget},
    iced_widget::stack,
    widget::{
        button, canvas, column, container, row,
        text::{self},
    },
};
use tokio::time::{Instant, interval_at};
use tracing::info;

#[derive(Debug, Clone, Copy, Hash)]
pub enum EntryR {
    NewEntry,
    Index(u32),
}

#[derive(Debug, Clone)]
pub enum EntryMessage {
    GetIconFile,
    SetIconFile(PathBuf),
    NameEdit(String),
    Algorithm(totp_rs::Algorithm),
    Digits(usize),
    Step(u64),
    Skew(u8),
    Secret(String),
    CancelledIconFile,
    Issuer(Option<String>),
    Stepped(cosmic::iced::time::Instant, u64),
    Animate(cosmic::iced::time::Instant),
    Noop,
    CopyOutput,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Entry {
    pub icon: TotpIcon,
    pub totp: totp_rs::TOTP,
    pub secret: String,
    #[serde(skip)]
    pub output: String,
    #[serde(skip)]
    pub percentage: f32,
    #[serde(skip, default = "std::time::Instant::now")]
    pub last_output: std::time::Instant,
    #[serde(skip, default = "std::time::Instant::now")]
    pub current_output: std::time::Instant,
}
impl Entry {
    pub fn new() -> Self {
        Self {
            icon: TotpIcon::Initials {
                initials: "-".into(),
            },
            totp: totp_rs::TOTP {
                algorithm: totp_rs::Algorithm::SHA1,
                digits: 6,
                skew: 1,
                step: 30,
                secret: Vec::new(),
                account_name: String::new(),
                issuer: None,
            },
            secret: String::new(),
            output: String::new(),
            percentage: 0.0,
            last_output: std::time::Instant::now(),
            current_output: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, message: EntryMessage) -> Result<cosmic::Task<EntryMessage>, String> {
        match message {
            EntryMessage::GetIconFile => {
                return Ok(cosmic::Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Entry Icon")
                        .pick_file(),
                    move |s| {
                        s.map_or(EntryMessage::CancelledIconFile, |s| {
                            EntryMessage::SetIconFile(s.path().to_path_buf())
                        })
                    },
                ));
            }
            EntryMessage::SetIconFile(path_buf) => {
                self.icon = TotpIcon::Image {
                    path: path_buf,
                    handle: OnceLock::new(),
                };
            }
            EntryMessage::NameEdit(s) => {
                self.totp.account_name = s;
                self.recalc_icon();
            }
            EntryMessage::Algorithm(algorithm) => self.totp.algorithm = algorithm,
            EntryMessage::Digits(s) => self.totp.digits = s,
            EntryMessage::Step(s) => self.totp.step = s,
            EntryMessage::Skew(s) => self.totp.skew = s,
            EntryMessage::Secret(s) => {
                self.secret = s;
                // FIXME: Add this validation to all the necessary steps, and display it properly.
                self.recalc_secret()?;
            }
            EntryMessage::CancelledIconFile => info!("User cancelled icon file set"),
            EntryMessage::Issuer(s) => {
                self.totp.issuer = s;
                self.recalc_icon();
            }
            EntryMessage::Stepped(instant, time) => {
                self.output = self.totp.generate(time);
                self.last_output = instant;
                self.percentage = 0.0;
                self.current_output = instant;
            }
            #[allow(clippy::cast_precision_loss)]
            EntryMessage::Animate(instant) => {
                self.current_output = instant;
                self.percentage = self
                    .current_output
                    .duration_since(self.last_output)
                    .as_secs_f32()
                    / self.totp.step as f32;
            }
            EntryMessage::Noop => {}
            EntryMessage::CopyOutput => {
                return Ok(cosmic::iced::clipboard::write(self.output.clone()));
            }
        }

        Ok(cosmic::Task::none())
    }

    pub fn recalc_icon(&mut self) {
        if matches!(self.icon, TotpIcon::Initials { .. }) {
            self.icon = TotpIcon::default_for_name(
                self.totp
                    .issuer
                    .as_deref()
                    .unwrap_or_else(|| &self.totp.account_name),
            );
        }
    }

    pub fn recalc_secret(&mut self) -> Result<(), String> {
        let mut secret = self.secret.clone();
        // Special case the microsoft authenticator 10-length secrets
        if secret.len() == 10 {
            secret.push_str("000000");
        }
        let raw = totp_rs::Secret::Encoded(secret)
            .to_bytes()
            .map_err(|e| format!("Invalid secret: {e}"))?;
        self.totp.secret = raw;

        Ok(())
    }

    pub fn view_settings(&self, new: bool) -> cosmic::Element<EntryMessage> {
        use cosmic::widget::{button, container, dropdown, settings, text, text_input};

        let icon_setting = button::custom(self.icon.view(20.0).map(|s| match s {}))
            .on_press(EntryMessage::GetIconFile);
        // settings::item_row(Vec::new())
        let home_row = settings::item_row(Vec::new())
            .push(icon_setting)
            .push(text_input("Name", &self.totp.account_name).on_input(EntryMessage::NameEdit));
        let issuer = settings::item(
            "Issuer",
            text_input("None", self.totp.issuer.as_deref().unwrap_or_default())
                .on_input(|s| EntryMessage::Issuer((!s.is_empty()).then_some(s))),
        );
        let secret = settings::item(
            "Secret",
            text_input("XXXXXXXX", &self.secret).on_input(EntryMessage::Secret),
        );
        let basic = settings::section().add(home_row).add(issuer).add(secret);
        let algorithm = settings::item::item(
            "Algorithm",
            dropdown(
                &["SHA1", "SHA256", "SHA512"],
                match self.totp.algorithm {
                    totp_rs::Algorithm::SHA1 => Some(0),
                    totp_rs::Algorithm::SHA256 => Some(1),
                    totp_rs::Algorithm::SHA512 => Some(2),
                },
                |s| {
                    EntryMessage::Algorithm(match s {
                        0 => totp_rs::Algorithm::SHA1,
                        1 => totp_rs::Algorithm::SHA256,
                        2 => totp_rs::Algorithm::SHA512,
                        _ => unreachable!(),
                    })
                },
            ),
        );

        let digits = settings::item(
            "Digits",
            cosmic::widget::spin_button(
                self.totp.digits.to_string(),
                self.totp.digits,
                1,
                0,
                16,
                EntryMessage::Digits,
            ),
        );
        let skew = settings::item(
            "Skew",
            cosmic::widget::spin_button(
                self.totp.skew.to_string(),
                self.totp.skew,
                1,
                0,
                16,
                EntryMessage::Skew,
            ),
        );
        let step = settings::item(
            "Step",
            cosmic::widget::spin_button(
                self.totp.step.to_string(),
                self.totp.step,
                1,
                0,
                3600,
                EntryMessage::Step,
            ),
        );
        let advanced = settings::section()
            .title("Advanced")
            .add(algorithm)
            .add(digits)
            .add(skew)
            .add(step);

        let col = settings::view_column(Vec::new())
            // .spacing(5)
            .push(if new {
                text::title1("New Entry")
            } else {
                text::title1("Edit Entry")
            })
            .push(basic)
            .push(advanced);

        container(col).into()
    }

    pub fn view<const SHOW_CODES: bool>(&self) -> cosmic::Element<EntryMessage> {
        let name = row()
            .push_maybe(self.totp.issuer.as_ref().map(|s| {
                container(text::text(s))
                    .padding([0.0, 5.0])
                    .style(|t| container::Style {
                        icon_color: None,
                        text_color: Some(t.current_container().component.on.into()),
                        background: Some(cosmic::iced::Background::Color(
                            t.cosmic().primary_container_color().into(),
                        )),
                        border: cosmic::iced::Border {
                            // color: t.cosmic().accent.base.into(),
                            color: t.cosmic().small_widget_divider().into(),
                            width: 1.0,
                            radius: [4.0; 4].into(),
                        },
                        shadow: cosmic::iced::Shadow {
                            color: cosmic::iced::Color::default(),
                            offset: cosmic::iced::Vector::ZERO,
                            blur_radius: 5.0,
                        },
                    })
            }))
            .push(text::text(&self.totp.account_name))
            .spacing(4);
        let code = if SHOW_CODES {
            Some(
                cosmic::widget::text(&self.output)
                    .class(cosmic::theme::Text::Accent)
                    .font(cosmic::font::mono().apply(|mut s| {
                        s.weight = Weight::Bold;
                        s
                    }))
                    .size(30),
            )
        } else {
            None
        };
        let content = column().push(name).push_maybe(code);
        let ttk = if SHOW_CODES {
            let ttk: cosmic::Element<'static, ()> = canvas(Ttk {
                percentage: 1.0 - self.percentage,
                thickness: 4.0,
            })
            .width(30.0)
            .height(30.0)
            .into();
            let ttk = stack([
                ttk.map(|()| unreachable!()),
                container(text::monotext(
                    (self
                        .totp
                        .step
                        .checked_sub(
                            self.current_output
                                .duration_since(self.last_output)
                                .as_secs(),
                        )
                        .unwrap_or_default())
                    .to_string(),
                ))
                .center(Length::Fill)
                .into(),
            ]);
            Some(ttk)
        } else {
            None
        };

        let content = row()
            .push(self.icon.view(20.0).map(|m| match m {}))
            .push(content)
            .push_maybe(ttk)
            .spacing(5)
            .align_y(Alignment::Center);

        if SHOW_CODES {
            button::custom(content)
                .width(Length::Shrink)
                .class(cosmic::theme::Button::ListItem)
                .padding(5)
                .on_press(EntryMessage::CopyOutput)
                .into()
        } else {
            content.into()
        }
    }

    pub fn subscription(&self, window_id: cosmic::iced::window::Id) -> Subscription<EntryMessage> {
        let curr_t = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        // let time_to = self.totp.next_step(curr_t.as_secs()) - curr_t.as_secs();
        let next_time = Duration::new(
            self.totp.step - curr_t.as_secs() % self.totp.step,
            u32::MAX - curr_t.subsec_nanos(),
        );
        let time_since = Duration::from_secs(self.totp.step)
            .checked_sub(next_time)
            .unwrap_or_default();
        let periodic = tokio_stream::once((
            tokio::time::Instant::now().checked_sub(time_since).unwrap(),
            curr_t.as_secs(),
        ))
        .chain(
            tokio_stream::wrappers::IntervalStream::new(interval_at(
                Instant::now() + next_time,
                Duration::from_secs(self.totp.step),
            ))
            .map(|i| {
                (
                    i,
                    std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                )
            }),
        )
        .map(|(i, t)| EntryMessage::Stepped(i.into(), t));
        Subscription::batch([
            Subscription::run_with_id(self.totp.step, periodic),
            cosmic::iced::window::frames()
                .with(window_id)
                .map(|(wi, (i, t))| {
                    if i == wi {
                        EntryMessage::Animate(t)
                    } else {
                        EntryMessage::Noop
                    }
                }),
        ])
    }
}

struct Ttk {
    percentage: f32,
    thickness: f32,
}
impl canvas::Program<(), cosmic::Theme> for Ttk {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        theme: &cosmic::Theme,
        bounds: cosmic::iced::Rectangle,
        _cursor: cosmic::iced_core::mouse::Cursor,
    ) -> Vec<canvas::Geometry<cosmic::Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let line = canvas::Path::new(|b| {
            b.ellipse(canvas::path::arc::Elliptical {
                center: frame.center(),
                radii: cosmic::iced::Vector::from(frame.size() - [self.thickness + 1.0; 2].into())
                    * 0.5,
                // radii: cosmic::iced::Vector::new(5.0, 5.0),
                rotation: cosmic::iced::Radians(-std::f32::consts::PI / 2.0),
                start_angle: cosmic::iced::Radians(0.0),
                end_angle: cosmic::iced::Radians(-self.percentage * 2.0 * std::f32::consts::PI),
            });
        });

        frame.stroke(
            &line,
            canvas::Stroke {
                style: canvas::Style::Solid(theme.cosmic().accent_color().into()),
                width: self.thickness,
                line_cap: canvas::LineCap::Round,
                line_join: canvas::LineJoin::Round,
                line_dash: canvas::LineDash::default(),
            },
        );

        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
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
            let initials = cch.get(0..2).map_or_else(
                || cch.first().copied().unwrap_or('-').to_string(),
                |s| s.iter().copied().collect(),
            );
            Self::Initials { initials }
        } else {
            Self::Initials {
                initials: fch[0..2].iter().collect(),
            }
        }
    }

    pub fn view(&self, radius: f32) -> cosmic::Element<std::convert::Infallible> {
        widget::container(match self {
            Self::Image { path, handle } => cosmic::Element::from(
                widget::image(handle.get_or_init(|| widget::image::Handle::from_path(path)))
                    .width(Length::Fixed(radius * 2.0))
                    .height(Length::Fixed(radius * 2.0))
                    .border_radius([radius; 4])
                    .content_fit(cosmic::iced::ContentFit::Cover),
            ),
            Self::Initials { initials } => cosmic::widget::text::title1(initials)
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
