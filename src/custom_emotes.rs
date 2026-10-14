//! Local image library; IPC-supplied paths/filenames never become filesystem paths.
use image::ImageFormat;
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub(crate) const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
// ponytail: 16 images / 32 MiB library; add delete/rename controls when requested.
const MAX_FILES: usize = 16;
const MAX_LIBRARY_BYTES: u64 = 32 * 1024 * 1024;
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
fn read_file(directory: &Path, id: &str, extension: &str) -> Option<Vec<u8>> {
    if !pingloom_core::settings::is_custom_emote(id) || !["png", "gif", "webp"].contains(&extension)
    {
        return None;
    }
    let path = directory.join(format!("{id}.{extension}"));
    if !fs::symlink_metadata(&path).ok()?.file_type().is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (!bytes.is_empty() && bytes.len() <= MAX_INPUT_BYTES).then_some(bytes)
}
fn stored(directory: &Path, id: &str) -> Option<(Vec<u8>, &'static str)> {
    ["png", "gif", "webp"]
        .into_iter()
        .find_map(|ext| read_file(directory, id, ext).map(|bytes| (bytes, ext)))
}
pub(crate) fn media(id: &str) -> Option<(Vec<u8>, &'static str)> {
    stored(directory().ok()?, id)
}
fn thumbnail(bytes: &[u8], extension: &str) -> Option<Vec<u8>> {
    if extension == "png" {
        return Some(bytes.to_vec());
    }
    let frames = crate::animation::fit_custom(crate::animation::decode_media(bytes, true).ok()?);
    let mut output = Cursor::new(Vec::new());
    frames
        .first()?
        .image
        .write_to(&mut output, ImageFormat::Png)
        .ok()?;
    Some(output.into_inner())
}
pub(crate) fn asset(path: &str) -> Option<Vec<u8>> {
    let id = path.strip_prefix("emotes/")?.strip_suffix(".png")?;
    let (bytes, ext) = media(id)?;
    thumbnail(&bytes, ext)
}
pub(crate) fn animated(id: &str) -> bool {
    if !pingloom_core::settings::is_custom_emote(id) {
        return false;
    }
    let Ok(dir) = directory() else { return false };
    ["gif", "webp"].into_iter().any(|ext| {
        fs::symlink_metadata(dir.join(format!("{id}.{ext}"))).is_ok_and(|m| m.file_type().is_file())
    })
}
pub(crate) fn exists(id: &str) -> bool {
    media(id).is_some()
}
fn ids(directory: &Path) -> Vec<String> {
    let mut ids: Vec<_> = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_file())
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| ["png", "gif", "webp"].contains(&x))
        })
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
        .filter_map(|id| {
            let (bytes, ext) = stored(dir, &id)?;
            Some((id, thumbnail(&bytes, ext)?))
        })
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
        return Err("Choose a PNG, JPG, GIF or WebP up to 8 MiB".into());
    }
    let format = crate::animation::format(bytes)?;
    // Fully validate every frame before any write, not just the thumbnail.
    let frames = crate::animation::decode_media(bytes, false)?;
    let (output, extension) = if matches!(format, ImageFormat::Gif | ImageFormat::WebP) {
        // Preserve original compression, GIF disposal, alpha, timing and WebP quality.
        (
            bytes.to_vec(),
            if format == ImageFormat::Gif {
                "gif"
            } else {
                "webp"
            },
        )
    } else {
        let frames = crate::animation::fit_custom(frames);
        let mut output = Cursor::new(Vec::new());
        frames[0]
            .image
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        (output.into_inner(), "png")
    };
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let mut library_bytes = 0u64;
    for id in ids(directory) {
        for ext in ["png", "gif", "webp"] {
            if let Ok(metadata) = fs::symlink_metadata(directory.join(format!("{id}.{ext}"))) {
                library_bytes = library_bytes.saturating_add(metadata.len());
            }
        }
        if stored(directory, &id)
            .is_some_and(|(existing, ext)| ext == extension && existing == output)
        {
            return Ok(id);
        }
    }
    if ids(directory).len() >= MAX_FILES {
        return Err("Custom emote library is full (16 images)".into());
    }
    if library_bytes.saturating_add(output.len() as u64) > MAX_LIBRARY_BYTES {
        return Err("Custom emote library exceeds 32 MiB".into());
    }
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    file.write_all(&output).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos() as u64;
    let id = format!("custom_{stamp:016x}");
    file.persist_noclobber(directory.join(format!("{id}.{extension}")))
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
        let restored = image::load_from_memory(&read_file(dir.path(), &id, "png").unwrap())
            .unwrap()
            .into_rgba8();
        assert_eq!(restored.dimensions(), (512, 256));
        assert_eq!(restored.get_pixel(200, 100).0, [255, 0, 0, 128]);
        assert!(read_file(dir.path(), "../../settings", "png").is_none());
        assert!(import_into(dir.path(), b"not an image").is_err());
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
        let narrow_dir = tempfile::tempdir().unwrap();
        let narrow = image::RgbaImage::new(1, 2048);
        let mut bytes = Cursor::new(Vec::new());
        narrow.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let id = import_into(narrow_dir.path(), bytes.get_ref()).unwrap();
        assert_eq!(
            image::load_from_memory(&read_file(narrow_dir.path(), &id, "png").unwrap())
                .unwrap()
                .height(),
            512
        );
    }
    #[test]
    fn gif_webp_and_jpg_keep_valid_media_with_png_thumbnails() {
        use image::{codecs::gif::GifEncoder, Delay, Frame, RgbaImage};
        let dir = tempfile::tempdir().unwrap();
        let mut gif = Vec::new();
        GifEncoder::new(&mut gif)
            .encode_frames([
                Frame::from_parts(
                    RgbaImage::new(2, 2),
                    0,
                    0,
                    Delay::from_numer_denom_ms(50, 1),
                ),
                Frame::from_parts(
                    RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
                    0,
                    0,
                    Delay::from_numer_denom_ms(100, 1),
                ),
            ])
            .unwrap();
        let id = import_into(dir.path(), &gif).unwrap();
        assert_eq!(import_into(dir.path(), &gif).unwrap(), id);
        let (restored, ext) = stored(dir.path(), &id).unwrap();
        assert_eq!(restored, gif);
        assert_eq!(ext, "gif");
        assert_eq!(
            crate::animation::decode_media(&restored, false)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            image::guess_format(&thumbnail(&restored, ext).unwrap()).unwrap(),
            ImageFormat::Png
        );
        let webp = crate::assets::ASSETS
            .get_file("emotes/facepalm.webp")
            .unwrap()
            .contents();
        let id = import_into(dir.path(), webp).unwrap();
        assert_eq!(stored(dir.path(), &id).unwrap().0, webp);
        let mut jpg = Cursor::new(Vec::new());
        image::RgbImage::new(3, 2)
            .write_to(&mut jpg, ImageFormat::Jpeg)
            .unwrap();
        let id = import_into(dir.path(), jpg.get_ref()).unwrap();
        assert_eq!(stored(dir.path(), &id).unwrap().1, "png");
        let overflow = tempfile::tempdir().unwrap();
        for n in 0..4 {
            let file =
                fs::File::create(overflow.path().join(format!("custom_{n:016x}.gif"))).unwrap();
            file.set_len(8 * 1024 * 1024).unwrap();
        }
        assert!(import_into(overflow.path(), &gif).is_err());
    }
}
