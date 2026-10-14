//! Local PNG library; no IPC-supplied paths or filenames become filesystem paths.
use image::{ImageFormat, ImageReader, Limits};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub(crate) const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
// ponytail: 16 static PNGs; add library deletion/animated imports when requested.
const MAX_FILES: usize = 16;
const MAX_STORED_BYTES: usize = 2 * 1024 * 1024;
static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static IMPORT: Mutex<()> = Mutex::new(());

pub(crate) fn initialize(settings: &Path) {
    if let Some(parent) = settings.parent() {
        let _ = DIRECTORY.set(parent.join("custom-emotes"));
    }
}
fn directory() -> Result<&'static Path, String> {
    DIRECTORY
        .get()
        .map(PathBuf::as_path)
        .ok_or_else(|| "Emote library is unavailable".into())
}
fn read_png(directory: &Path, id: &str) -> Option<Vec<u8>> {
    if !pingloom_core::settings::is_custom_emote(id) {
        return None;
    }
    let path = directory.join(format!("{id}.png"));
    if fs::symlink_metadata(&path).ok()?.file_type().is_symlink() {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_STORED_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_STORED_BYTES).then_some(bytes)
}
pub(crate) fn asset(path: &str) -> Option<Vec<u8>> {
    let id = path.strip_prefix("emotes/")?.strip_suffix(".png")?;
    read_png(directory().ok()?, id)
}
pub(crate) fn exists(id: &str) -> bool {
    directory().ok().and_then(|d| read_png(d, id)).is_some()
}
fn ids(directory: &Path) -> Vec<String> {
    let mut ids: Vec<_> = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.path().file_stem()?.to_str().map(str::to_owned))
        .filter(|id| pingloom_core::settings::is_custom_emote(id))
        .collect();
    ids.sort();
    ids.dedup();
    ids.truncate(MAX_FILES);
    ids
}
pub(crate) fn catalog() -> Vec<(String, Vec<u8>)> {
    let Ok(dir) = directory() else {
        return Vec::new();
    };
    ids(dir)
        .into_iter()
        .filter_map(|id| read_png(dir, &id).map(|bytes| (id, bytes)))
        .collect()
}
pub(crate) fn import(bytes: &[u8]) -> Result<String, String> {
    let _guard = IMPORT
        .try_lock()
        .map_err(|_| "Another emote import is in progress")?;
    import_into(directory()?, bytes)
}
fn import_into(directory: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err("Choose a PNG up to 8 MiB".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Invalid PNG or image exceeds 2048×2048")?
        .into_rgba8();
    if image.width() == 0 || image.height() == 0 {
        return Err("Empty image".into());
    }
    let scale = (512.0 / image.width().max(image.height()) as f32).min(1.0);
    let image = crate::raster::resize_art_to(
        &image,
        (image.width() as f32 * scale).round().max(1.0) as u32,
        (image.height() as f32 * scale).round().max(1.0) as u32,
    );
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let output = output.into_inner();
    if output.len() > MAX_STORED_BYTES {
        return Err("PNG exceeds the library size limit".into());
    }
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    // Deduplicate normalized artwork; never overwrite an existing library item.
    for id in ids(directory) {
        if read_png(directory, &id).as_deref() == Some(output.as_slice()) {
            return Ok(id);
        }
    }
    if ids(directory).len() >= MAX_FILES {
        return Err("Custom emote library is full (16 images)".into());
    }
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    file.write_all(&output).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos() as u64;
    let id = format!("custom_{stamp:016x}");
    file.persist_noclobber(directory.join(format!("{id}.png")))
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_png_is_bounded_persistent_deduplicated_and_path_safe() {
        let dir = tempfile::tempdir().unwrap();
        let source = image::RgbaImage::from_pixel(1024, 512, image::Rgba([255, 0, 0, 128]));
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let id = import_into(dir.path(), bytes.get_ref()).unwrap();
        assert!(pingloom_core::settings::is_custom_emote(&id));
        assert_eq!(import_into(dir.path(), bytes.get_ref()).unwrap(), id);
        let restored = image::load_from_memory(&read_png(dir.path(), &id).unwrap())
            .unwrap()
            .into_rgba8();
        assert_eq!(restored.dimensions(), (512, 256));
        assert_eq!(restored.get_pixel(200, 100).0, [255, 0, 0, 128]);
        assert!(read_png(dir.path(), "../../settings").is_none());
        assert!(import_into(dir.path(), b"not a PNG").is_err());
        assert!(import_into(dir.path(), &vec![0; MAX_INPUT_BYTES + 1]).is_err());
        let huge = image::RgbaImage::new(2049, 1);
        let mut bytes = Cursor::new(Vec::new());
        huge.write_to(&mut bytes, ImageFormat::Png).unwrap();
        assert!(import_into(dir.path(), bytes.get_ref()).is_err());
        for n in 1..MAX_FILES {
            fs::write(dir.path().join(format!("custom_{n:016x}.png")), b"occupied").unwrap();
        }
        let other = image::RgbaImage::new(1, 1);
        let mut bytes = Cursor::new(Vec::new());
        other.write_to(&mut bytes, ImageFormat::Png).unwrap();
        assert!(import_into(dir.path(), bytes.get_ref()).is_err());
        assert_eq!(ids(dir.path()).len(), MAX_FILES);
        let narrow_dir = tempfile::tempdir().unwrap();
        let narrow = image::RgbaImage::new(1, 2048);
        let mut bytes = Cursor::new(Vec::new());
        narrow.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let id = import_into(narrow_dir.path(), bytes.get_ref()).unwrap();
        assert_eq!(
            image::load_from_memory(&read_png(narrow_dir.path(), &id).unwrap())
                .unwrap()
                .height(),
            512
        );
    }
}
