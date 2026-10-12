use include_dir::Dir;

pub static ASSETS: &Dir<'_> = &include_dir::include_dir!("$CARGO_MANIFEST_DIR/assets");
