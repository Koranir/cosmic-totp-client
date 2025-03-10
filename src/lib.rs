mod localize;

mod app;

pub const APP_ID: &'static str = "com.koranir.CosmicTotpClient";
pub const CONFIG_VER: u64 = 1;

pub struct AppConfig {}

pub fn run(args: AppConfig) -> cosmic::iced::Result {
    localize::init();

    cosmic::app::run::<app::App>(cosmic::app::Settings::default(), args)
}
