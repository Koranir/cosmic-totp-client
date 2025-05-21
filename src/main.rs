use std::str::FromStr;

use tracing::{error, info};
use tracing_subscriber::filter::Directive;

#[cfg(debug_assertions)]
static DEFAULT_DIRECTIVE: &str = "cosmic_totp_client=debug";
#[cfg(not(debug_assertions))]
static DEFAULT_DIRECTIVE: &str = "cosmic_totp_client=warn";

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(Directive::from_str(DEFAULT_DIRECTIVE).unwrap())
                .from_env_lossy(),
        )
        .init();

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
