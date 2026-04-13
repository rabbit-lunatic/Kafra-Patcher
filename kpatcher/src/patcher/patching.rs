use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use gruf::grf::{GrfArchive, GrfArchiveBuilder};
use gruf::thor::{ThorArchive, ThorFileEntry};

/// Indicates the method that should be used when patching GRF files.
pub enum GrfPatchingMethod {
    OutOfPlace,
    InPlace,
}

/// Patches a GRF file with a THOR archive/patch.
pub fn apply_patch_to_grf<R: Read + Seek>(
    patching_method: GrfPatchingMethod,
    create_if_needed: bool,
    grf_file_path: impl AsRef<Path>,
    thor_archive: &mut ThorArchive<R>,
) -> Result<()> {
    if !grf_file_path.as_ref().exists() && create_if_needed {
        // Create a new GRF file if needed
        let new_grf = fs::File::create(&grf_file_path)?;
        GrfArchiveBuilder::create(new_grf, 2, 0)?;
    }
    match patching_method {
        GrfPatchingMethod::InPlace => apply_patch_to_grf_ip(grf_file_path, thor_archive),
        GrfPatchingMethod::OutOfPlace => apply_patch_to_grf_oop(grf_file_path, thor_archive),
    }
}

/// Patches a GRF file with another GRF archive/patch.
pub fn apply_grf_to_grf(
    patching_method: GrfPatchingMethod,
    create_if_needed: bool,
    target_grf_path: impl AsRef<Path>,
    source_grf: &mut GrfArchive,
) -> Result<()> {
    if !target_grf_path.as_ref().exists() && create_if_needed {
        let new_grf = fs::File::create(&target_grf_path)?;
        GrfArchiveBuilder::create(new_grf, 2, 0)?;
    }
    match patching_method {
        GrfPatchingMethod::InPlace => apply_grf_to_grf_ip(target_grf_path, source_grf),
        GrfPatchingMethod::OutOfPlace => apply_grf_to_grf_oop(target_grf_path, source_grf),
    }
}

fn apply_grf_to_grf_ip(
    target_grf_path: impl AsRef<Path>,
    source_grf: &mut GrfArchive,
) -> Result<()> {
    let mut builder = GrfArchiveBuilder::open(target_grf_path)?;
    let mut entries: Vec<gruf::grf::GrfFileEntry> = source_grf.take_entries().collect();
    // Sort by offset for optimal sequential read performance
    entries.sort_unstable_by(|a, b| a.offset.cmp(&b.offset));
    for entry in entries {
        builder.import_entry_from_grf(source_grf, entry)?;
    }
    Ok(())
}

fn apply_grf_to_grf_oop(
    target_grf_path: impl AsRef<Path>,
    source_grf: &mut GrfArchive,
) -> Result<()> {
    // Rename file to back it up
    let mut backup_file_path = target_grf_path.as_ref().to_path_buf();
    backup_file_path.set_extension("grf.bak");
    fs::rename(target_grf_path.as_ref(), &backup_file_path)?;

    // Add files from the original archive
    let mut target_archive = GrfArchive::open(&backup_file_path)?;
    // Preserve original GRF version
    let original_version_major = target_archive.version_major();
    let original_version_minor = target_archive.version_minor();

    // Process GRF entries directly, skipping those overwritten by the patch
    let mut target_paths: Vec<(u64, String)> = target_archive
        .get_entries()
        .filter_map(|entry| {
            if source_grf.get_file_entry(&entry.relative_path).is_some() {
                None
            } else {
                Some((entry.offset, entry.relative_path.clone()))
            }
        })
        .collect();
    // Sort by offset for optimal sequential read performance
    target_paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    // Process patch entries directly
    let mut source_paths: Vec<(u64, String)> = source_grf
        .get_entries()
        .map(|entry| (entry.offset, entry.relative_path.clone()))
        .collect();
    // Sort by offset for optimal sequential read performance
    source_paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    // Build the patched GRF; restore backup on failure
    let build_result = (|| -> Result<()> {
        let grf_file = fs::File::create(target_grf_path.as_ref())?;
        // Use original GRF version to preserve encryption
        let mut builder =
            GrfArchiveBuilder::create(grf_file, original_version_major, original_version_minor)?;

        for (_, path) in target_paths {
            builder.import_raw_entry_from_grf(&mut target_archive, path)?;
        }
        for (_, path) in source_paths {
            builder.import_raw_entry_from_grf(source_grf, path)?;
        }
        Ok(())
    })();

    if let Err(e) = build_result {
        // Restore backup on failure
        log::error!("Patching failed, restoring backup: {}", e);
        let _ = fs::remove_file(target_grf_path.as_ref());
        fs::rename(&backup_file_path, target_grf_path.as_ref())
            .with_context(|| "Failed to restore GRF backup after patching error")?;
        return Err(e);
    }

    // Remove backup file on success
    Ok(fs::remove_file(backup_file_path)?)
}

/// Patches a GRF in an in-place manner.
///
/// This is faster but produces output of bigger size and can corrupt file in
/// case of error.
fn apply_patch_to_grf_ip<R: Read + Seek>(
    grf_file_path: impl AsRef<Path>,
    thor_archive: &mut ThorArchive<R>,
) -> Result<()> {
    let mut builder = GrfArchiveBuilder::open(grf_file_path)?;
    let mut thor_entries: Vec<ThorFileEntry> = thor_archive
        .get_entries()
        .filter(|e| !e.is_internal())
        .cloned()
        .collect();
    thor_entries.sort_unstable_by(|a, b| a.offset.cmp(&b.offset));
    for entry in thor_entries {
        if entry.is_removed {
            let _ = builder.remove_file(&entry.relative_path);
        } else {
            builder.import_raw_entry_from_thor(thor_archive, entry.relative_path)?;
        }
    }
    Ok(())
}

/// Patches a GRF in an out-of-place manner.
///
/// This is safer and produces output of smaller size but slower.
fn apply_patch_to_grf_oop<R: Read + Seek>(
    grf_file_path: impl AsRef<Path>,
    thor_archive: &mut ThorArchive<R>,
) -> Result<()> {
    // Rename file to back it up
    let mut backup_file_path = grf_file_path.as_ref().to_path_buf();
    backup_file_path.set_extension("grf.bak");
    fs::rename(grf_file_path.as_ref(), &backup_file_path)?;

    // Add files from the original archive while discarding files removed in the patch
    let mut grf_archive = GrfArchive::open(&backup_file_path)?;

    // Preserve original GRF version
    let original_version_major = grf_archive.version_major();
    let original_version_minor = grf_archive.version_minor();

    // Process GRF entries directly, skipping those removed or overwritten by the patch
    let mut grf_paths: Vec<(u64, String)> = grf_archive
        .get_entries()
        .filter_map(|entry| {
            if let Some(e) = thor_archive.get_file_entry(&entry.relative_path) {
                // If the patch has an internal file with the same name, we should keep the GRF one,
                // because the patch won't overwrite it.
                if !e.is_removed && e.is_internal() {
                    return Some((entry.offset, entry.relative_path.clone()));
                }
                // Skip if removed or overwritten by the patch
                return None;
            }
            Some((entry.offset, entry.relative_path.clone()))
        })
        .collect();
    // Sort by offset for optimal sequential read performance
    grf_paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    // Process patch entries directly
    let mut thor_paths: Vec<(u64, String)> = thor_archive
        .get_entries()
        .filter_map(|entry| {
            if entry.is_removed || entry.is_internal() {
                None
            } else {
                Some((entry.offset, entry.relative_path.clone()))
            }
        })
        .collect();
    // Sort by offset for optimal sequential read performance
    thor_paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    // Build the patched GRF; restore backup on failure
    let build_result = (|| -> Result<()> {
        let grf_file = fs::File::create(grf_file_path.as_ref())?;
        // Use original GRF version to preserve encryption
        let mut builder =
            GrfArchiveBuilder::create(grf_file, original_version_major, original_version_minor)?;

        for (_, path) in grf_paths {
            builder.import_raw_entry_from_grf(&mut grf_archive, path)?;
        }
        for (_, path) in thor_paths {
            builder.import_raw_entry_from_thor(thor_archive, path)?;
        }
        Ok(())
    })();

    if let Err(e) = build_result {
        // Restore backup on failure
        log::error!("Patching failed, restoring backup: {}", e);
        let _ = fs::remove_file(grf_file_path.as_ref());
        fs::rename(&backup_file_path, grf_file_path.as_ref())
            .with_context(|| "Failed to restore GRF backup after patching error")?;
        return Err(e);
    }

    // Remove backup file once the patched GRF has been built
    Ok(fs::remove_file(backup_file_path)?)
}

/// Patches files located in the game client's directory with a THOR
/// archive/patch.
pub fn apply_patch_to_disk<R: Read + Seek>(
    root_directory: impl AsRef<Path>,
    thor_archive: &mut ThorArchive<R>,
) -> Result<()> {
    let root_directory = root_directory.as_ref();
    let backup_dir = root_directory.join(".patch_backup");
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }
    fs::create_dir_all(&backup_dir)?;

    let mut backed_up_files = Vec::new();
    let mut created_files = Vec::new();
    let mut seen_files = HashSet::new();

    let mut file_entries: Vec<ThorFileEntry> = thor_archive
        .get_entries()
        .filter(|e| !e.is_internal())
        .cloned()
        .collect();
    file_entries.sort_unstable_by(|a, b| a.offset.cmp(&b.offset));

    let apply_result = (|| -> Result<()> {
        for entry in file_entries {
            let mut dest_path = join_windows_relative_path(root_directory, &entry.relative_path);
            if let Ok(current_exe) = env::current_exe() {
                if dest_path == current_exe {
                    dest_path = dest_path.with_extension("exe.new");
                }
            }

            if !seen_files.contains(&dest_path) {
                if dest_path.exists() {
                    // Backup existing file
                    let relative_path = dest_path
                        .strip_prefix(root_directory)
                        .with_context(|| "Failed to strip root directory prefix")?;
                    let backup_path = backup_dir.join(relative_path);
                    if let Some(parent) = backup_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(&dest_path, &backup_path)
                        .with_context(|| format!("Failed to backup file {:?}", dest_path))?;
                    backed_up_files.push((dest_path.clone(), backup_path));
                } else if !entry.is_removed {
                    created_files.push(dest_path.clone());
                }
                seen_files.insert(dest_path.clone());
            } else if entry.is_removed {
                // If it was already seen, it's either in created_files or backed_up_files.
                // If we are now removing it, we should just delete it from disk if it was just created/updated.
                // Actually, if it's in created_files, it will be deleted on rollback.
                // If it's in backed_up_files, it will be restored on rollback.
                // For now, we just need to make sure it's removed from its current location.
                let _ = fs::remove_file(&dest_path);
            }

            if !entry.is_removed {
                // Create parent directory if needed
                if let Some(parent_dir) = dest_path.parent() {
                    fs::create_dir_all(parent_dir)?
                }
                // Extract file
                thor_archive
                    .extract_file(&entry.relative_path, &dest_path)
                    .with_context(|| format!("Failed to extract file {:?}", dest_path))?;
            }
        }
        Ok(())
    })();

    if let Err(e) = apply_result {
        // Restore backup on failure
        log::error!("Patching failed, restoring backup: {}", e);
        for path in created_files {
            let _ = fs::remove_file(&path);
        }
        for (dest, backup) in backed_up_files {
            // On Windows, rename fails if the destination already exists.
            if dest.exists() {
                let res = if dest.is_dir() {
                    fs::remove_dir_all(&dest)
                } else {
                    fs::remove_file(&dest)
                };
                if let Err(err) = res {
                    log::warn!("Failed to remove {:?} during rollback: {}", dest, err);
                }
            }
            if let Err(err) = fs::rename(&backup, &dest) {
                log::warn!(
                    "Failed to restore {:?} from backup during rollback: {}",
                    dest,
                    err
                );
            }
        }
        let _ = fs::remove_dir_all(&backup_dir);
        return Err(e);
    }

    // Success, remove backup
    let _ = fs::remove_dir_all(&backup_dir);
    Ok(())
}

/// Utility function used to join path-like segments the same way it's done in
/// the GRF file format (Windows style).
fn join_windows_relative_path(path: &Path, windows_relative_path: &str) -> PathBuf {
    let mut result = PathBuf::from(path);
    for component in windows_relative_path.split('\\') {
        result.push(component);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use walkdir::WalkDir;

    #[test]
    fn test_apply_patch_backup_rollback() {
        let thor_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/thor");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let root = temp_dir.path();

        let thor_archive_path = thor_dir_path.join("small.thor");
        let thor_data = fs::read(&thor_archive_path).expect("Failed to read thor archive file");
        let mut thor_archive = ThorArchive::new(std::io::Cursor::new(thor_data))
            .expect("Failed to parse thor archive");

        // 1. Prepare an existing file that should be preserved/restored.
        let existing_file_relative = "data\\wav\\se_subterranean_rustyengine.wav";
        let existing_file_path = join_windows_relative_path(root, existing_file_relative);
        fs::create_dir_all(existing_file_path.parent().unwrap())
            .expect("Failed to create parent dir for existing file");
        let original_content = b"original content";
        fs::write(&existing_file_path, original_content).expect("Failed to write existing file");

        // 2. Prepare a failure: create a file where a directory is expected.
        // "data\\texture" is a parent for many files in small.thor.
        // It's a sibling of "data\\wav", so creating it as a file won't affect existing_file_path.
        let conflict_path = root.join("data").join("texture");
        fs::create_dir_all(conflict_path.parent().unwrap()).unwrap();
        fs::write(&conflict_path, "conflict").expect("Failed to create conflict file");

        // Attempt patching
        let result = apply_patch_to_disk(root, &mut thor_archive);

        // Verify failure
        assert!(result.is_err(), "Patching should have failed");

        // Verify restoration/preservation
        assert!(
            existing_file_path.exists(),
            "Existing file should still exist"
        );
        assert_eq!(
            fs::read(&existing_file_path).unwrap(),
            original_content,
            "Content should be restored"
        );

        // Verify conflict file still exists
        assert!(conflict_path.exists());
        assert!(conflict_path.is_file());

        // Verify backup dir is removed
        assert!(!root.join(".patch_backup").exists());
    }

    #[test]
    fn test_apply_patch_to_disk() {
        let thor_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/thor");
        let temp_dir = tempdir().unwrap();
        {
            let count_files = |dir_path| {
                WalkDir::new(dir_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter_map(|entry| entry.metadata().ok())
                    .filter(|metadata| metadata.is_file())
                    .count()
            };
            let expected_file_path = temp_dir
                .path()
                .join("data/wav/se_subterranean_rustyengine.wav");
            let thor_archive_path = thor_dir_path.join("small.thor");
            let mut thor_archive = ThorArchive::open(&thor_archive_path).unwrap();
            let nb_of_added_files = thor_archive.file_count() - 1;

            // Before patching
            assert!(!expected_file_path.exists());
            assert_eq!(0, count_files(temp_dir.path()));

            apply_patch_to_disk(temp_dir.path(), &mut thor_archive).unwrap();

            // After patching
            assert!(expected_file_path.exists());
            assert_eq!(nb_of_added_files, count_files(temp_dir.path()));
            // Check content
            let expected_content = thor_archive
                .read_file_content(r"data\wav\se_subterranean_rustyengine.wav")
                .unwrap();
            let actual_content = fs::read(&expected_file_path).unwrap();
            assert_eq!(expected_content, actual_content);
        }
    }

    #[test]
    fn test_apply_patch_to_grf_ip_empty() {
        let grf_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/grf");
        let thor_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/thor");
        let temp_dir = tempdir().unwrap();
        let thor_archive_path = thor_dir_path.join("small.thor");
        let grf_archive_path = temp_dir.path().join("empty.grf");
        {
            fs::copy(grf_dir_path.join("200-empty.grf"), &grf_archive_path).unwrap();

            // Before patching
            let grf_archive = GrfArchive::open(&grf_archive_path).unwrap();
            assert_eq!(0, grf_archive.file_count());
            let grf_version_major = grf_archive.version_major();
            let grf_version_minor = grf_archive.version_minor();

            let mut thor_archive = ThorArchive::open(&thor_archive_path).unwrap();
            let nb_of_added_files = thor_archive.file_count() - 1;
            apply_patch_to_grf(
                GrfPatchingMethod::InPlace,
                false,
                &grf_archive_path,
                &mut thor_archive,
            )
            .unwrap();

            // After patching
            let grf_archive = GrfArchive::open(&grf_archive_path).unwrap();
            assert_eq!(nb_of_added_files, grf_archive.file_count());
            assert_eq!(grf_archive.version_major(), grf_version_major);
            assert_eq!(grf_archive.version_minor(), grf_version_minor);
        }
        assert!(patch_maintained_integrity(&thor_archive_path, &grf_archive_path).unwrap());
    }

    #[test]
    fn test_apply_patch_to_grf_ip_empty_create() {
        let thor_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/thor");
        let temp_dir = tempdir().unwrap();
        let grf_archive_path = temp_dir.path().join("empty.grf");
        let thor_archive_path = thor_dir_path.join("small.thor");
        {
            let mut thor_archive = ThorArchive::open(&thor_archive_path).unwrap();
            let nb_of_added_files = thor_archive.file_count() - 1;
            apply_patch_to_grf(
                GrfPatchingMethod::InPlace,
                true,
                &grf_archive_path,
                &mut thor_archive,
            )
            .unwrap();

            // After patching
            let grf_archive = GrfArchive::open(&grf_archive_path).unwrap();
            assert_eq!(nb_of_added_files, grf_archive.file_count());
            assert_eq!(grf_archive.version_major(), 2);
            assert_eq!(grf_archive.version_minor(), 0);
        }
        assert!(patch_maintained_integrity(&thor_archive_path, &grf_archive_path).unwrap());
    }

    #[test]
    fn test_apply_patch_to_grf_oop_empty() {
        let grf_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/grf");
        let thor_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/thor");
        let temp_dir = tempdir().unwrap();
        let thor_archive_path = thor_dir_path.join("small.thor");
        let grf_archive_path = temp_dir.path().join("empty.grf");
        {
            fs::copy(grf_dir_path.join("200-empty.grf"), &grf_archive_path).unwrap();

            // Before patching
            let grf_archive = GrfArchive::open(&grf_archive_path).unwrap();
            assert_eq!(0, grf_archive.file_count());
            let grf_version_major = grf_archive.version_major();
            let grf_version_minor = grf_archive.version_minor();

            let mut thor_archive = ThorArchive::open(&thor_archive_path).unwrap();
            let nb_of_added_files = thor_archive.file_count() - 1;
            apply_patch_to_grf(
                GrfPatchingMethod::OutOfPlace,
                false,
                &grf_archive_path,
                &mut thor_archive,
            )
            .unwrap();

            // After patching
            let grf_archive = GrfArchive::open(&grf_archive_path).unwrap();
            assert_eq!(nb_of_added_files, grf_archive.file_count());
            assert_eq!(grf_archive.version_major(), grf_version_major);
            assert_eq!(grf_archive.version_minor(), grf_version_minor);
        }
        assert!(patch_maintained_integrity(&thor_archive_path, &grf_archive_path).unwrap());
    }

    #[test]
    fn test_apply_patch_to_grf_oop_empty_create() {
        let thor_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/tests/thor");
        let temp_dir = tempdir().unwrap();
        let thor_archive_path = thor_dir_path.join("small.thor");
        let grf_archive_path = temp_dir.path().join("empty.grf");
        {
            let mut thor_archive = ThorArchive::open(&thor_archive_path).unwrap();
            let nb_of_added_files = thor_archive.file_count() - 1;
            apply_patch_to_grf(
                GrfPatchingMethod::OutOfPlace,
                true,
                &grf_archive_path,
                &mut thor_archive,
            )
            .unwrap();

            // After patching
            let grf_archive = GrfArchive::open(&grf_archive_path).unwrap();
            assert_eq!(nb_of_added_files, grf_archive.file_count());
            assert_eq!(grf_archive.version_major(), 2);
            assert_eq!(grf_archive.version_minor(), 0);
        }
        assert!(patch_maintained_integrity(&thor_archive_path, &grf_archive_path).unwrap());
    }

    fn patch_maintained_integrity(
        thor_file_path: &PathBuf,
        grf_file_path: &PathBuf,
    ) -> Result<bool> {
        let mut thor_archive = ThorArchive::open(&thor_file_path)?;
        let mut grf_archive = GrfArchive::open(&grf_file_path)?;
        let thor_entries: Vec<ThorFileEntry> = thor_archive.get_entries().cloned().collect();
        for file_entry in thor_entries {
            if file_entry.is_internal() || file_entry.is_removed {
                continue;
            }
            let expected_content = thor_archive.read_file_content(&file_entry.relative_path)?;
            let file_content = grf_archive.read_file_content(&file_entry.relative_path)?;
            if expected_content != file_content {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
