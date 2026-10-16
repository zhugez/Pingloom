//! Embedded portable assets by default; macOS release bundles store them only once.
#[cfg(feature = "embed-assets")]
use include_dir::Dir;
use std::{
    borrow::Cow,
    path::{Component, Path},
};
#[cfg(feature = "embed-assets")]
pub static ASSETS: &Dir<'_> = &include_dir::include_dir!("$CARGO_MANIFEST_DIR/assets");

pub(crate) fn get(path: impl AsRef<Path>) -> Option<Cow<'static, [u8]>> {
    let path = path.as_ref();
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    #[cfg(feature = "embed-assets")]
    {
        ASSETS.get_file(path).map(|f| Cow::Borrowed(f.contents()))
    }
    #[cfg(not(feature = "embed-assets"))]
    {
        std::fs::read(root()?.join(path)).ok().map(Cow::Owned)
    }
}
#[cfg(not(feature = "embed-assets"))]
fn root() -> Option<std::path::PathBuf> {
    Some(
        std::env::current_exe()
            .ok()?
            .parent()?
            .parent()?
            .join("Resources/assets"),
    )
}
pub(crate) fn validate() -> Result<(), String> {
    #[cfg(not(feature = "embed-assets"))]
    if get("emotes/facepalm.webp").is_none() {
        return Err(
            "Bundle Resources/assets is missing; reinstall the complete Pingloom.app".into(),
        );
    }
    Ok(())
}
#[cfg(test)]
pub(crate) fn files(directory: &str) -> Vec<std::path::PathBuf> {
    #[cfg(feature = "embed-assets")]
    {
        ASSETS
            .get_dir(directory)
            .map(|dir| dir.files().map(|f| f.path().to_owned()).collect())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "embed-assets"))]
    {
        root()
            .and_then(|r| std::fs::read_dir(r.join(directory)).ok())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|f| Path::new(directory).join(f.file_name()))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn resource_paths_are_bounded_and_bundle_assets_present() {
        assert!(super::get("../settings.json").is_none());
        assert!(super::get("/etc/passwd").is_none());
        super::validate().unwrap();
        assert!(super::get("emotes/facepalm.webp").is_some());
        assert!(super::get("not-present").is_none());
    }
}
