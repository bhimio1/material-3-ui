//! Thumbnails decoded on a background thread and kept as textures, keyed
//! by path and modification time so a changed file is decoded again.

use egui::{ColorImage, TextureHandle};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::SystemTime;

type Done = (PathBuf, SystemTime, Option<ColorImage>);

/// The thumbnail cache. `load` runs on one worker thread, in request order;
/// it returns `None` for files it can't show (the tile then keeps its
/// fallback icon).
pub struct Thumbnails {
    ctx: egui::Context,
    textures: HashMap<PathBuf, (SystemTime, Option<TextureHandle>)>,
    requested: HashSet<PathBuf>,
    tx: mpsc::Sender<(PathBuf, SystemTime)>,
    rx: mpsc::Receiver<Done>,
}

impl Thumbnails {
    pub fn new(ctx: &egui::Context, load: impl Fn(&Path) -> Option<ColorImage> + Send + 'static) -> Self {
        let (tx, jobs) = mpsc::channel::<(PathBuf, SystemTime)>();
        let (done, rx) = mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            for (path, mtime) in jobs {
                let img = load(&path);
                if done.send((path, mtime, img)).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        Self { ctx: ctx.clone(), textures: HashMap::new(), requested: HashSet::new(), tx, rx }
    }

    /// The thumbnail of `path` as of `modified`: ready, or `None` while it
    /// loads (it's requested on the first call) or when it can't be shown.
    pub fn get(&mut self, path: &Path, modified: SystemTime) -> Option<TextureHandle> {
        self.receive();
        match self.textures.get(path) {
            Some((mtime, tex)) if *mtime == modified => tex.clone(),
            _ => {
                if self.requested.insert(path.to_path_buf()) {
                    let _ = self.tx.send((path.to_path_buf(), modified));
                }
                None
            }
        }
    }

    /// Whether `path` is still being decoded.
    pub fn loading(&self, path: &Path) -> bool {
        self.requested.contains(path)
    }

    /// Drop one thumbnail (after the file was deleted or replaced).
    pub fn forget(&mut self, path: &Path) {
        self.textures.remove(path);
    }

    pub fn clear(&mut self) {
        self.textures.clear();
    }

    fn receive(&mut self) {
        for (path, mtime, img) in self.rx.try_iter() {
            let tex = img.map(|i| self.ctx.load_texture(path.to_string_lossy(), i, egui::TextureOptions::LINEAR));
            self.requested.remove(&path);
            self.textures.insert(path, (mtime, tex));
        }
    }
}

/// Decode an image file and shrink it to fit `max_side` (keeping its
/// aspect ratio), for [`Thumbnails::new`]. PNG, JPEG and WebP.
#[cfg(feature = "image")]
pub fn load_image(path: &Path, max_side: u32) -> Option<ColorImage> {
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    Some(color_image(&img.thumbnail(max_side, max_side)))
}

/// Like [`load_image`], from bytes in memory (say, a video frame from
/// `ffmpegthumbnailer -o -`).
#[cfg(feature = "image")]
pub fn load_image_bytes(bytes: &[u8], max_side: u32) -> Option<ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    Some(color_image(&img.thumbnail(max_side, max_side)))
}

#[cfg(feature = "image")]
fn color_image(img: &image::DynamicImage) -> ColorImage {
    let rgba = img.to_rgba8();
    ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait_for(thumbs: &mut Thumbnails, path: &Path, mtime: SystemTime) -> Option<TextureHandle> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let tex = thumbs.get(path, mtime);
            if !thumbs.loading(path) {
                return tex;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("thumbnail never arrived");
    }

    #[test]
    fn decodes_off_thread_and_reloads_changed_files() {
        let ctx = egui::Context::default();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = calls.clone();
        let mut thumbs = Thumbnails::new(&ctx, move |p| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            (p.extension()? == "png").then(|| ColorImage::filled([4, 2], egui::Color32::RED))
        });
        let t0 = SystemTime::UNIX_EPOCH;
        let png = Path::new("/x/a.png");
        assert!(thumbs.get(png, t0).is_none(), "not loaded yet");
        let tex = wait_for(&mut thumbs, png, t0).expect("decoded");
        assert_eq!(tex.size(), [4, 2]);
        // Cached: no second decode.
        assert!(thumbs.get(png, t0).is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // A newer mtime decodes again.
        let t1 = t0 + Duration::from_secs(1);
        assert!(wait_for(&mut thumbs, png, t1).is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        // Files the loader can't show stay `None` without retrying.
        let txt = Path::new("/x/b.txt");
        assert!(wait_for(&mut thumbs, txt, t0).is_none());
        assert!(thumbs.get(txt, t0).is_none());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[cfg(feature = "image")]
    #[test]
    fn image_files_are_shrunk_to_fit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.png");
        image::RgbaImage::from_pixel(400, 100, image::Rgba([1, 2, 3, 255])).save(&path).unwrap();
        let img = load_image(&path, 200).unwrap();
        assert_eq!(img.size, [200, 50]);
        assert!(load_image(&dir.path().join("missing.png"), 200).is_none());
        assert!(load_image_bytes(b"not an image", 200).is_none());
    }
}
