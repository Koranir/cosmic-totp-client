mod localize;

mod app;

pub const APP_ID: &str = "com.koranir.CosmicTotpClient";
pub const CONFIG_VER: u64 = 1;

pub struct AppConfig {}

pub fn run(args: AppConfig) -> cosmic::iced::Result {
    localize::init();

    cosmic::applet::run::<app::App>(args)
}
