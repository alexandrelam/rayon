use rayon_types::{CommandId, ImageAssetDefinition};
use std::fs;
use std::path::Path;

const IMAGE_ASSET_COMMAND_PREFIX: &str = "image-asset:";
const SUPPORTED_IMAGE_EXTENSIONS: &[&str] =
    &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub(super) fn load_images(config_dir: &Path) -> Result<Vec<ImageAssetDefinition>, String> {
    let image_root = config_dir.join("images");
    if !image_root.exists() {
        return Ok(Vec::new());
    }

    let mut images = Vec::new();
    collect_images(&image_root, &image_root, &mut images)?;
    images.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(images)
}

fn collect_images(
    image_root: &Path,
    current_dir: &Path,
    images: &mut Vec<ImageAssetDefinition>,
) -> Result<(), String> {
    let entries = fs::read_dir(current_dir).map_err(|error| {
        format!(
            "failed to read image assets directory {}: {error}",
            current_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_images(image_root, &path, images)?;
            continue;
        }

        if !is_supported_image(&path) {
            continue;
        }

        let relative_path = path
            .strip_prefix(image_root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let title = path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .ok_or_else(|| format!("invalid image asset filename: {}", path.display()))?
            .to_string();

        images.push(ImageAssetDefinition {
            id: CommandId::from(format!("{IMAGE_ASSET_COMMAND_PREFIX}{relative_path}")),
            title,
            relative_path,
            path: path.to_string_lossy().into_owned(),
        });
    }

    Ok(())
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str()))
}
