//! Word wrap: whether long lines fold at the pane's width. Kept between runs
//! in `wrap.toml`.

use serde::{Deserialize, Serialize};

use crate::App;

#[derive(Serialize, Deserialize, Default)]
pub struct Saved {
    pub wrap: bool,
}

pub fn state_path() -> Option<std::path::PathBuf> {
    ito_common::state_dir("weft").map(|d| d.join("wrap.toml"))
}

pub fn read() -> bool {
    state_path().and_then(|p| ito_common::kept::read::<Saved>(&p)).is_some_and(|s| s.wrap)
}

impl App {
    /// `view.toggle_wrap`.
    pub(crate) fn wrap_toggle(&mut self, ctx: &egui::Context) {
        self.wrap = !self.wrap;
        self.wrap_writer.save_toml(&Saved { wrap: self.wrap });
        self.doc.view.follow = true;
        ctx.request_repaint();
        let key = if self.wrap { "wrap.on" } else { "wrap.off" };
        let text = self.table.tr(key).to_owned();
        self.announce(text);
    }
}
