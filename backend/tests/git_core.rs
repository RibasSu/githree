mod common;

use std::fs;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::read::GzDecoder;
use githree::error::AppError;
use githree::git;
use tar::Archive as TarArchive;
use tempfile::tempdir;
use zip::ZipArchive;

#[test]
fn clone_fetch_refs_tree_blob_and_commit_operations_work() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let config = common::test_config(temp.path());
    let local_path = temp.path().join("repos").join("sample");
    fs::create_dir_all(local_path.parent().expect("parent path")).expect("create dirs");

    git::clone::clone_repo(&fixture.remote_url(), &local_path, &config).expect("clone repo");
    let repo = git::clone::open_bare_repo(&local_path).expect("open bare repo");
    let default_branch = git::clone::default_branch(&repo).expect("resolve default branch");
    assert_eq!(default_branch, "main");
    let repo_size = git::clone::repo_size_kb(&local_path).expect("repo size");
    assert!(repo_size < u64::MAX);

    let refs = git::refs::list_refs(&local_path).expect("list refs");
    assert!(refs.branches.iter().any(|branch| branch == "main"));
    // list_refs merges local and remote-tracking branch names.
    assert!(refs.tags.iter().any(|tag| tag == "v1.0.0"));

    let root = git::browse::list_tree(&local_path, "main", "").expect("list root tree");
    assert!(!root.is_empty());
    assert!(root.iter().any(|entry| entry.entry_type == "tree"));
    assert!(root.iter().any(|entry| entry.name == "README.md"));

    let docs = git::browse::list_tree(&local_path, "main", "docs").expect("list docs tree");
    assert!(docs.iter().any(|entry| entry.name == "renamed-guide.md"));

    let language_stats = git::browse::language_stats(&local_path, "main").expect("language stats");
    assert!(!language_stats.is_empty());
    assert!(
        language_stats
            .iter()
            .any(|entry| entry.language == "rust" && entry.bytes > 0)
    );
    assert!(
        language_stats
            .iter()
            .all(|entry| entry.percentage >= 0.0 && entry.percentage <= 100.0)
    );

    let missing_tree = git::browse::list_tree(&local_path, "main", "missing/path")
        .expect_err("missing tree should error");
    match missing_tree {
        AppError::NotFound(_) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }

    let readme = git::browse::read_blob(&local_path, "main", "README.md").expect("read readme");
    assert_eq!(readme.encoding, "utf8");
    assert!(!readme.is_binary);
    assert!(!readme.is_truncated);
    assert_eq!(readme.language, "markdown");
    assert!(readme.content.contains("Sample Repo"));

    let binary = git::browse::read_blob(&local_path, "main", "binary.bin").expect("read binary");
    assert!(binary.is_binary);
    assert!(!binary.is_truncated);
    assert_eq!(binary.encoding, "base64");
    let decoded = STANDARD
        .decode(binary.content)
        .expect("decode base64 binary");
    assert!(!decoded.is_empty());

    let blob_on_tree = git::browse::read_blob(&local_path, "main", "docs")
        .expect_err("directory should not be readable as blob");
    match blob_on_tree {
        AppError::InvalidRequest(_) => {}
        other => panic!("expected InvalidRequest, got {other:?}"),
    }

    let raw = git::browse::read_raw(&local_path, "main", "README.md").expect("read raw");
    assert_eq!(raw.file_name, "README.md");
    assert!(!raw.content.is_empty());

    match git::browse::read_raw(&local_path, "main", "docs") {
        Err(AppError::InvalidRequest(_)) => {}
        Err(other) => panic!("expected InvalidRequest, got {other:?}"),
        Ok(_) => panic!("directory raw read should fail"),
    }

    let readme_doc = git::browse::read_readme(&local_path, "main").expect("read README");
    assert_eq!(readme_doc.filename, "README.md");

    let history =
        git::browse::commit_history(&local_path, "main", None, 0, 3).expect("commit history");
    assert_eq!(history.len(), 3);
    assert!(history.iter().all(|commit| !commit.hash.is_empty()));

    let skipped_history = git::browse::commit_history(&local_path, "main", None, 1, 2)
        .expect("commit history with skip");
    assert_eq!(skipped_history.len(), 2);
    assert_ne!(skipped_history[0].hash, history[0].hash);

    let filtered =
        git::browse::commit_history(&local_path, "main", Some("docs/renamed-guide.md"), 0, 10)
            .expect("filtered history");
    assert!(!filtered.is_empty());

    let detail =
        git::browse::commit_detail(&local_path, "main~1").expect("commit detail by revparse expr");
    assert_eq!(detail.commit.hash, history[1].hash);
    assert!(!detail.diffs.is_empty());

    let initial_hash = fixture.resolve_ref("main~3");
    let initial_detail =
        git::browse::commit_detail(&local_path, &initial_hash).expect("initial commit detail");
    assert!(initial_detail.parents.is_empty());

    let missing_ref = git::browse::commit_history(&local_path, "not-a-ref", None, 0, 1)
        .expect_err("missing ref should error");
    match missing_ref {
        AppError::NotFound(_) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }

    let missing_commit = git::browse::commit_detail(&local_path, "deadbeef")
        .expect_err("missing commit should error");
    match missing_commit {
        AppError::GitError(_) | AppError::NotFound(_) => {}
        other => panic!("expected GitError or NotFound, got {other:?}"),
    }

    let new_hash = fixture.add_remote_commit(
        "after-fetch.txt",
        b"new content after local clone\n",
        "feat: commit after clone",
    );
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config).expect("fetch repo");

    let branch_tree = git::browse::list_tree(&local_path, "main", "").expect("list main tree");
    assert!(
        branch_tree
            .iter()
            .any(|entry| entry.name == "after-fetch.txt")
    );

    let remote_tree = git::browse::list_tree(&local_path, "refs/remotes/origin/main", "")
        .expect("list remote-tracking main tree");
    assert!(
        remote_tree
            .iter()
            .any(|entry| entry.name == "after-fetch.txt")
    );

    common::run_git(
        &["checkout", "-b", "post-clone-branch"],
        Some(&fixture.work_dir),
    );
    std::fs::write(fixture.work_dir.join("post-branch.txt"), "branch commit\n")
        .expect("write branch file");
    common::run_git(&["add", "post-branch.txt"], Some(&fixture.work_dir));
    common::run_git(
        &["commit", "-m", "feat: post clone branch"],
        Some(&fixture.work_dir),
    );
    common::run_git(
        &["push", "-u", "origin", "post-clone-branch"],
        Some(&fixture.work_dir),
    );
    common::run_git(&["checkout", "main"], Some(&fixture.work_dir));

    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config).expect("fetch branches");
    let refs_after_fetch = git::refs::list_refs(&local_path).expect("list refs after fetch");
    assert!(
        refs_after_fetch
            .branches
            .iter()
            .any(|branch| branch == "post-clone-branch")
    );

    let fetched_detail = git::browse::commit_detail(&local_path, &new_hash)
        .expect("commit detail for fetched commit");
    assert_eq!(fetched_detail.commit.hash, new_hash);

    let huge_content = "line-for-large-diff-and-blob-limits\n".repeat(40_000);
    let huge_hash = fixture.add_remote_commit(
        "huge/very-large.txt",
        huge_content.as_bytes(),
        "feat: add very large text fixture",
    );
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config)
        .expect("fetch large commit");

    let huge_blob = git::browse::read_blob(
        &local_path,
        "refs/remotes/origin/main",
        "huge/very-large.txt",
    )
    .expect("read huge blob");
    assert!(!huge_blob.is_binary);
    assert!(huge_blob.is_truncated);
    assert!(huge_blob.content.is_empty());
    assert!(huge_blob.truncated_reason.is_some());

    let huge_detail =
        git::browse::commit_detail(&local_path, &huge_hash).expect("commit detail for huge commit");
    assert!(huge_detail.is_truncated);
    assert!(huge_detail.truncated_reason.is_some());
    assert!(huge_detail.displayed_file_count > 0);
    assert!(huge_detail.displayed_line_count > 0);
    assert!(huge_detail.displayed_line_count < huge_content.lines().count());

    let huge_binary = vec![0_u8; 6 * 1024 * 1024];
    fixture.add_remote_commit(
        "binary/very-large.bin",
        &huge_binary,
        "feat: add very large binary fixture",
    );
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config)
        .expect("fetch very large binary");
    let huge_binary_blob = git::browse::read_blob(
        &local_path,
        "refs/remotes/origin/main",
        "binary/very-large.bin",
    )
    .expect("read very large binary blob");
    assert!(huge_binary_blob.is_binary);
    assert!(huge_binary_blob.is_truncated);
    assert!(huge_binary_blob.truncated_reason.is_some());

    common::run_git(&["checkout", "main"], Some(&fixture.work_dir));
    for index in 0..320 {
        let file_path = fixture
            .work_dir
            .join("many-files")
            .join(format!("file-{index:03}.txt"));
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("create many-files dir");
        }
        fs::write(&file_path, format!("line for {index}\n")).expect("write many-files fixture");
    }
    common::run_git(&["add", "."], Some(&fixture.work_dir));
    common::run_git(
        &["commit", "-m", "feat: add many files for diff truncation"],
        Some(&fixture.work_dir),
    );
    common::run_git(&["push", "origin", "main"], Some(&fixture.work_dir));
    let many_files_hash = common::git_stdout(&["rev-parse", "HEAD"], Some(&fixture.work_dir))
        .trim()
        .to_string();
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config)
        .expect("fetch many files commit");
    let many_files_detail = git::browse::commit_detail(&local_path, &many_files_hash)
        .expect("commit detail for many files commit");
    assert!(many_files_detail.is_truncated);
    assert!(many_files_detail.displayed_file_count <= 300);
    assert!(many_files_detail.truncated_reason.is_some());

    common::run_git(&["checkout", "main"], Some(&fixture.work_dir));
    let head_hash = common::git_stdout(&["rev-parse", "HEAD"], Some(&fixture.work_dir))
        .trim()
        .to_string();
    common::run_git(
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &head_hash,
            "vendor-submodule",
        ],
        Some(&fixture.work_dir),
    );
    common::run_git(
        &["commit", "-m", "chore: add gitlink tree entry"],
        Some(&fixture.work_dir),
    );
    common::run_git(&["push", "origin", "main"], Some(&fixture.work_dir));
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config)
        .expect("fetch gitlink tree entry");
    let root_after_gitlink = git::browse::list_tree(&local_path, "refs/remotes/origin/main", "")
        .expect("list root tree after gitlink");
    assert!(
        !root_after_gitlink
            .iter()
            .any(|entry| entry.name == "vendor-submodule")
    );

    fixture.add_remote_commit("README.md", b"", "chore: empty readme");
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config)
        .expect("fetch empty readme");
    let empty_readme = git::browse::read_blob(&local_path, "refs/remotes/origin/main", "README.md")
        .expect("read empty readme");
    assert!(!empty_readme.is_binary);
    assert_eq!(empty_readme.content, "");

    common::run_git(&["checkout", "main"], Some(&fixture.work_dir));
    common::run_git(&["rm", "README.md"], Some(&fixture.work_dir));
    common::run_git(
        &["commit", "-m", "chore: remove readme for coverage"],
        Some(&fixture.work_dir),
    );
    common::run_git(&["push", "origin", "main"], Some(&fixture.work_dir));
    git::clone::fetch_repo(&local_path, &fixture.remote_url(), &config)
        .expect("fetch readme removal");
    let missing_readme = git::browse::read_readme(&local_path, "refs/remotes/origin/main")
        .expect_err("missing readme should fail");
    match missing_readme {
        AppError::NotFound(_) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn archive_generation_supports_tar_gz_and_zip() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let config = common::test_config(temp.path());
    let local_path = temp.path().join("repos").join("archive-repo");
    fs::create_dir_all(local_path.parent().expect("parent path")).expect("create dirs");
    git::clone::clone_repo(&fixture.remote_url(), &local_path, &config).expect("clone repo");

    let tar_response = git::archive::create_archive(&local_path, "archive-repo", "main", "tar.gz")
        .expect("create tar.gz archive");
    assert_eq!(tar_response.content_type, "application/gzip");
    assert!(tar_response.file_name.ends_with(".tar.gz"));
    assert!(tar_response.path.exists());

    let tar_file = fs::File::open(&tar_response.path).expect("open tar.gz");
    let decoder = GzDecoder::new(tar_file);
    let mut archive = TarArchive::new(decoder);
    let mut tar_entries = vec![];
    for entry in archive.entries().expect("tar entries") {
        let entry = entry.expect("valid tar entry");
        let path = entry
            .path()
            .expect("tar entry path")
            .to_string_lossy()
            .to_string();
        tar_entries.push(path);
    }
    assert!(tar_entries.iter().any(|path| path.ends_with("README.md")));

    let zip_response = git::archive::create_archive(&local_path, "archive-repo", "main", "zip")
        .expect("create zip archive");
    assert_eq!(zip_response.content_type, "application/zip");
    assert!(zip_response.file_name.ends_with(".zip"));
    assert!(zip_response.path.exists());

    let zip_file = fs::File::open(&zip_response.path).expect("open zip archive");
    let mut zip = ZipArchive::new(zip_file).expect("read zip archive");
    let mut found_readme = false;
    for idx in 0..zip.len() {
        let file = zip.by_index(idx).expect("zip file entry");
        if file.name().ends_with("README.md") {
            found_readme = true;
            break;
        }
    }
    assert!(found_readme);

    let commit_hash = common::git_stdout(&["rev-parse", "HEAD"], Some(&fixture.work_dir));
    let hash_archive =
        git::archive::create_archive(&local_path, "archive-repo", commit_hash.trim(), "zip")
            .expect("create archive using explicit commit hash");
    assert!(hash_archive.path.exists());

    let bad_format = git::archive::create_archive(&local_path, "archive-repo", "main", "7z")
        .expect_err("unsupported archive format should fail");
    match bad_format {
        AppError::InvalidRequest(_) => {}
        other => panic!("expected InvalidRequest, got {other:?}"),
    }

    let bad_ref = git::archive::create_archive(&local_path, "archive-repo", "missing", "zip")
        .expect_err("missing ref should fail");
    match bad_ref {
        AppError::NotFound(_) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn refs_fallback_to_main_when_origin_head_is_absent() {
    let temp = tempdir().expect("tempdir");
    let bare_path = temp.path().join("empty.git");
    common::run_git(
        &["init", "--bare", bare_path.to_str().expect("utf-8 path")],
        None,
    );

    let refs = git::refs::list_refs(&bare_path).expect("list refs on empty bare repo");
    assert!(refs.branches.is_empty());
    assert!(refs.tags.is_empty());
    assert_eq!(refs.default_branch, "main");
}

#[test]
fn refs_use_first_branch_when_origin_head_missing_but_branches_exist() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let bare_path = temp.path().join("bare-no-origin-head.git");
    common::run_git(
        &[
            "clone",
            "--bare",
            common::path_to_str(&fixture.work_dir),
            bare_path.to_str().expect("utf-8 path"),
        ],
        None,
    );

    let refs = git::refs::list_refs(&bare_path).expect("list refs");
    assert!(!refs.branches.is_empty());
    assert_eq!(refs.default_branch, refs.branches[0]);
}
