mod config;
mod error;
mod i18n;
mod logic;
mod window;

use crate::window::Window;

fn main() -> cosmic::iced::Result {
  let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
  i18n::init(&requested_languages);

  cosmic::applet::run::<Window>(())?;

  Ok(())
}
