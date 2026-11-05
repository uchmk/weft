//! Image paste: the picture on the clipboard becomes `assets/<date>-<n>.webp`
//! next to the note, and `![](assets/…)` goes in at the caret. Ctrl+V cannot
//! carry it (the toolkit only hands over clipboard text), so it is the command
//! `edit.paste_image`. The reading, encoding and writing run on the file thread.

use std::path::Path;

use crate::App;

/// Lossy WebP quality (docs/design.md: around 80).
const QUALITY: f32 = 80.0;
/// Pictures wider than this are shrunk before they are stored (docs/design.md).
const MAX_WIDTH: u32 = 2000;
/// The folder next to the note.
const DIR: &str = "assets";

/// Encodes `img` and stores it under `note`'s `assets/` as `<stamp>-<n>.webp`
/// (never over a file). The same picture twice reuses the file it made the
/// first time. Returns the path to write in the link, with `/`.
pub(crate) fn store(note: &Path, stamp: &str, img: &ito_image::Rgba) -> Result<String, String> {
    let bytes = ito_image::encode_webp(&img.fit_width(MAX_WIDTH), QUALITY).map_err(|e| e.to_string())?;
    let dir = note.parent().unwrap_or(Path::new(".")).join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Same bytes as a file already there: use that one.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let same_kind = p.extension().is_some_and(|x| x == "webp");
            if same_kind && e.metadata().is_ok_and(|m| m.len() == bytes.len() as u64) && std::fs::read(&p).is_ok_and(|b| b == bytes) {
                return Ok(format!("{DIR}/{}", e.file_name().to_string_lossy()));
            }
        }
    }
    for n in 1u32.. {
        let name = format!("{stamp}-{n}.webp");
        let target = dir.join(&name);
        // create_new: a name that appeared meanwhile is not overwritten.
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&target) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(&bytes).map_err(|e| format!("{}: {e}", target.display()))?;
                return Ok(format!("{DIR}/{name}"));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", target.display())),
        }
    }
    unreachable!()
}

/// The worker's job: the clipboard's picture stored, `None` when there is none.
pub(crate) fn paste_job(note: &Path, stamp: &str) -> Result<Option<String>, String> {
    match ito_image::clipboard_image().map_err(|e| e.to_string())? {
        Some(img) => store(note, stamp, &img).map(Some),
        None => Ok(None),
    }
}

/// `YYYYMMDD-HHMM` in local time.
pub(crate) fn stamp() -> String {
    ito_common::now::now_local().format("%Y%m%d-%H%M").to_string()
}

impl App {
    /// `edit.paste_image`.
    pub(crate) fn paste_image(&mut self) {
        let Some(path) = self.doc.path() else {
            let title = self.table.tr("file.no_path").to_owned();
            self.outcome(ito_notices::Kind::Warn, title, None);
            return;
        };
        if self.doc.editor.is_none() {
            return;
        }
        self.send_file(crate::FileJob::PasteImage { path, stamp: stamp() });
    }

    /// The worker answered.
    pub(crate) fn image_done(&mut self, path: std::path::PathBuf, result: Result<Option<String>, String>) {
        match result {
            Ok(Some(rel)) => {
                let now = self.now_ms();
                if let Some(ed) = self.doc_mut(&path).and_then(|d| d.editor.as_mut()) {
                    ed.paste(&format!("![]({rel})"), now);
                }
                let note = self.table.trf("image.pasted", &[&rel]);
                self.announce(note.clone());
                self.note = Some(note);
            }
            Ok(None) => {
                let title = self.table.tr("image.none").to_owned();
                self.outcome(ito_notices::Kind::Info, title, None);
            }
            Err(e) => {
                let title = self.table.trf("image.failed", &[&e]);
                self.outcome(ito_notices::Kind::Err, title, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pic(shade: u8) -> ito_image::Rgba {
        let px: Vec<u8> = (0..16 * 16).flat_map(|i| [shade, (i % 256) as u8, 40, 255]).collect();
        ito_image::Rgba::new(16, 16, px).unwrap()
    }

    #[test]
    fn a_picture_is_stored_once_and_new_names_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("weft-image-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let note = dir.join("n.md");
        let a = store(&note, "20261105-1000", &pic(10)).unwrap();
        assert_eq!(a, "assets/20261105-1000-1.webp");
        assert!(dir.join(&a).is_file());
        // The same picture: the same file.
        assert_eq!(store(&note, "20261105-1001", &pic(10)).unwrap(), a);
        // Another picture in the same minute: the next number.
        assert_eq!(store(&note, "20261105-1000", &pic(200)).unwrap(), "assets/20261105-1000-2.webp");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wide_picture_is_stored_at_the_limit() {
        let dir = std::env::temp_dir().join(format!("weft-image-wide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wide = ito_image::Rgba::new(3000, 10, vec![120; 3000 * 10 * 4]).unwrap();
        let rel = store(&dir.join("n.md"), "20261105-1000", &wide).unwrap();
        let bytes = std::fs::read(dir.join(rel)).unwrap();
        // The WebP header holds the size; decoding is ito-image's job, so compare against the shrunk copy.
        let want = ito_image::encode_webp(&wide.fit_width(MAX_WIDTH), QUALITY).unwrap();
        assert_eq!(bytes, want);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
