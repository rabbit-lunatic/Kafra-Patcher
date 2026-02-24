use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, Result};
use gruf::thor::ThorArchiveBuilder;
use walkdir::WalkDir;

use crate::patch_definition::PatchDefinition;

pub fn generate_patch_from_definition<P1, P2>(
    patch_definition: PatchDefinition,
    patch_data_directory: P1,
    output_path: P2,
) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let output_file = File::create(output_path)?;
    let mut archive_builder = ThorArchiveBuilder::new(
        output_file,
        patch_definition.use_grf_merging,
        patch_definition.target_grf_name,
        patch_definition.include_checksums,
    )?;
    for entry in patch_definition.entries {
        let win32_relative_path = win32_path(&entry.relative_path);
        let target_win32_relative_path = entry.in_grf_path.unwrap_or(win32_relative_path.clone());

        if entry.is_removed {
            log::trace!("'{}' will be REMOVED", &win32_relative_path);
            archive_builder.append_file_removal(win32_relative_path);
            continue;
        }

        let native_path = patch_data_directory
            .as_ref()
            .join(posix_path(entry.relative_path));
        if native_path.is_file() {
            // Path points to a single file
            log::trace!("'{}' will be UPDATED", &target_win32_relative_path);
            let file = File::open(native_path)?;
            archive_builder.append_file_update(target_win32_relative_path, file)?;
        } else if native_path.is_dir() {
            // Path points to a directory
            append_directory_update(
                &mut archive_builder,
                patch_data_directory.as_ref(),
                native_path,
            )?;
        } else {
            return Err(anyhow!(
                "Path '{}' is invalid or does not exist",
                native_path.to_string_lossy()
            ));
        }
    }
    Ok(())
}

fn append_directory_update<P1, P2>(
    archive_builder: &mut ThorArchiveBuilder<File>,
    patch_data_directory: P1,
    directory_path: P2,
) -> Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let walker = WalkDir::new(directory_path).follow_links(false).into_iter();
    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel_path = entry.path().strip_prefix(patch_data_directory.as_ref())?;
            let rel_path_str = rel_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid file path encountered"))?;
            let win32_relative_path = win32_path(rel_path_str);
            log::trace!("'{}' will be UPDATED", &win32_relative_path);
            let file = File::open(entry.path())?;
            archive_builder.append_file_update(win32_relative_path, file)?;
        }
    }
    Ok(())
}

// Utility functions to make sure paths are serialized/accessed correctly between posix and Windows platforms
fn posix_path<S: AsRef<str>>(path: S) -> String {
    path.as_ref().replace("\\", "/")
}

fn win32_path<S: AsRef<str>>(path: S) -> String {
    path.as_ref().replace("/", "\\")
}
