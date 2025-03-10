// Stolen from the cosmic-applets repo.

use std::sync::LazyLock;

use cosmic::Also;
use i18n_embed::{
    LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use tracing::error;

#[derive(rust_embed::RustEmbed)]
#[folder = "i18n/"]
struct Localize;

pub static LL: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    fluent_language_loader!().also(|l| {
        l.load_fallback_language(&Localize)
            .expect("should have loaded the fallback language");
    })
});

pub fn init() {
    let ll = i18n_embed::DefaultLocalizer::new(&*LL, &Localize);

    let langs = i18n_embed::DesktopLanguageRequester::requested_languages();

    if let Err(e) = ll.select(&langs) {
        error!("Could not load requested language: {e}");
    }
}

#[macro_export]
macro_rules! ll {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::localize::LL, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::localize::LL, $message_id, $($args), *)
    }};
}
