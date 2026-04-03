use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use git2::{ObjectType, Repository, Tree};
use tar::Builder as TarBuilder;
use tempfile::Builder as TempFileBuilder;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::error::AppError;
use crate::git::ArchiveResponse;

pub fn create_archive(
    local_path: &Path,
    repo_name: &str,
    ref_name: &str,
    format: &str,
) -> Result<ArchiveResponse, AppError> {
    let repo = Repository::open_bare(local_path)?;
    let commit = resolve_commit(&repo, ref_name)?;
    let tree = commit.tree()?;
    let root_dir = format!("{repo_name}-{}", ref_name.replace('/', "-"));

    let mut entries = Vec::new();
    collect_files(&repo, &tree, Path::new(""), &mut entries)?;

    match format {
        "tar.gz" => create_tar_gz(&repo, &entries, &root_dir, repo_name, ref_name),
        "zip" => create_zip(&repo, &entries, &root_dir, repo_name, ref_name),
        _ => Err(AppError::InvalidRequest(
            "format must be tar.gz or zip".to_string(),
        )),
    }
}

fn create_tar_gz(
    repo: &Repository,
    entries: &[(PathBuf, git2::Oid, i32)],
    root_dir: &str,
    repo_name: &str,
    ref_name: &str,
) -> Result<ArchiveResponse, AppError> {
    let tempfile = TempFileBuilder::new().suffix(".tar.gz").tempfile()?;
    let writer = tempfile.reopen()?;
    let encoder = GzEncoder::new(writer, Compression::default());
    let mut tar_builder = TarBuilder::new(encoder);

    for (entry_path, oid, mode) in entries {
        let blob = repo.find_blob(*oid)?;
        let bytes = blob.content();

        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode((*mode as u32) & 0o777);
        header.set_cksum();

        let archive_path = Path::new(root_dir).join(entry_path);
        tar_builder.append_data(&mut header, archive_path, bytes)?;
    }

    let encoder = tar_builder.into_inner()?;
    encoder.finish()?;

    let persisted_path = persist_tempfile(tempfile)?;
    Ok(ArchiveResponse {
        content_type: "application/gzip".to_string(),
        file_name: format!("{repo_name}-{ref_name}.tar.gz"),
        path: persisted_path,
    })
}

fn create_zip(
    repo: &Repository,
    entries: &[(PathBuf, git2::Oid, i32)],
    root_dir: &str,
    repo_name: &str,
    ref_name: &str,
) -> Result<ArchiveResponse, AppError> {
    let tempfile = TempFileBuilder::new().suffix(".zip").tempfile()?;
    let writer = tempfile.reopen()?;
    let mut zip_writer = ZipWriter::new(writer);

    for (entry_path, oid, mode) in entries {
        let blob = repo.find_blob(*oid)?;
        let bytes = blob.content();

        let archive_path = Path::new(root_dir)
            .join(entry_path)
            .to_string_lossy()
            .replace('\\', "/");
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions((*mode as u32) & 0o777);
        zip_writer
            .start_file(archive_path, options)
            .map_err(|err| AppError::IoError(err.to_string()))?;
        zip_writer.write_all(bytes)?;
    }

    zip_writer
        .finish()
        .map_err(|err| AppError::IoError(err.to_string()))?;

    let persisted_path = persist_tempfile(tempfile)?;
    Ok(ArchiveResponse {
        content_type: "application/zip".to_string(),
        file_name: format!("{repo_name}-{ref_name}.zip"),
        path: persisted_path,
    })
}

fn collect_files(
    repo: &Repository,
    tree: &Tree<'_>,
    prefix: &Path,
    output: &mut Vec<(PathBuf, git2::Oid, i32)>,
) -> Result<(), AppError> {
    for entry in tree {
        let name = entry.name().unwrap_or_default();
        let entry_path = prefix.join(name);

        if entry.kind() == Some(ObjectType::Tree) {
            let sub_tree = repo.find_tree(entry.id())?;
            collect_files(repo, &sub_tree, &entry_path, output)?;
        } else if entry.kind() == Some(ObjectType::Blob) {
            output.push((entry_path, entry.id(), entry.filemode()));
        }
    }
    Ok(())
}

fn resolve_commit<'a>(repo: &'a Repository, ref_name: &str) -> Result<git2::Commit<'a>, AppError> {
    let candidate_refs = [
        ref_name.to_string(),
        format!("refs/heads/{ref_name}"),
        format!("refs/tags/{ref_name}"),
        format!("refs/remotes/origin/{ref_name}"),
    ];

    for candidate in candidate_refs {
        if let Ok(object) = repo.revparse_single(&candidate) {
            if let Ok(commit) = object.peel_to_commit() {
                return Ok(commit);
            }
        }
    }

    Err(AppError::NotFound(format!("ref '{ref_name}' not found")))
}

fn persist_tempfile(tempfile: tempfile::NamedTempFile) -> Result<PathBuf, AppError> {
    let (_file, path) = tempfile
        .keep()
        .map_err(|err| AppError::IoError(err.error.to_string()))?;
    Ok(path)
}
