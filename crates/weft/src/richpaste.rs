//! Smart paste: Ctrl+V brings the clipboard's HTML as Markdown, or spreadsheet
//! text (tab-separated) as a table; anything else is pasted as the text it is.
//! The clipboard's HTML flavour is read on the file thread, never on the UI
//! thread, so the paste lands a moment after the key (docs/design.md: input is
//! never made to wait).

use std::path::PathBuf;

use crate::App;

/// What to paste instead of `text`: the HTML turned into Markdown, else the
/// tab-separated `text` turned into a table. `None` pastes `text` as it is.
pub(crate) fn convert(html: Option<&str>, text: &str) -> Option<String> {
    if let Some(html) = html {
        let md = ito_paste::html_to_markdown(html);
        let md = md.trim_end_matches(['\r', '\n']);
        // An HTML flavour that says no more than the text (a plain copy) is
        // pasted as the text, so its line breaks and spacing stay the user's.
        if !md.trim().is_empty() && md.trim() != text.trim() {
            return Some(md.to_owned());
        }
    }
    ito_paste::tsv_to_table(text)
}

/// The worker's job: the clipboard's HTML (none when it cannot be read) and the
/// conversion.
pub(crate) fn job(text: &str) -> Option<String> {
    let html = ito_paste::clipboard_html().ok().flatten();
    convert(html.as_deref(), text)
}

impl App {
    /// Ctrl+V with `text` from the toolkit. Returns false when there is no file
    /// thread job to make (the caller pastes the text itself).
    pub(crate) fn paste_rich(&mut self, text: &str) -> bool {
        let Some(path) = self.doc.path() else { return false };
        self.send_file(crate::FileJob::PasteRich { path, text: text.to_owned() });
        true
    }

    /// The worker answered.
    pub(crate) fn rich_done(&mut self, path: PathBuf, text: String, md: Option<String>) {
        let now = self.now_ms();
        let Some(d) = self.doc_mut(&path) else { return };
        if let Some(ed) = d.editor.as_mut() {
            ed.paste(md.as_deref().unwrap_or(&text), now);
            d.view.follow = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_markdown_and_a_plain_copy_stays_text() {
        assert_eq!(convert(Some("<p>a <b>b</b></p>"), "a b").as_deref(), Some("a **b**"));
        assert_eq!(convert(Some("<p>same</p>"), "same"), None);
        assert_eq!(convert(None, "just text"), None);
    }

    #[test]
    fn spreadsheet_text_becomes_a_table() {
        let t = convert(None, "a\tb\n1\t2").unwrap();
        assert!(t.contains("| a | b |") || t.contains("|a|b|"), "{t}");
    }
}
