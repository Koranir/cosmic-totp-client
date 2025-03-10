use tracing::{error, info};

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt::init();

    info!(
        "Started COSMIC TOTP Client Applet v{}",
        env!("CARGO_PKG_VERSION")
    );

    if let Err(e) = cosmic_totp_client::run(cosmic_totp_client::AppConfig {}) {
        error!("Critical Application Error: {e}");
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
