use std::path::{Path, PathBuf};

use tokio::fs;
use tokio::process::Command;

use crate::classes::errors::StageError;
use crate::classes::job_handler::JobLogger;
use crate::functions::find_binary::find_binary;
use crate::functions::walk_files::walk_files;

pub async fn sterilize(
    log: &JobLogger,
    download_dir: &Path,
    content_path: &PathBuf,
    extension_whitelist: &[String],
) -> Result<PathBuf, StageError> {

    // Walk content_path, collect whitelisted files and prune everything else
    log.log(&format!("Scanning {}", content_path.display()), false).await;

    let mut media_files: Vec<PathBuf> = Vec::new();
    let all_files = walk_files(content_path).await
        .map_err(|e| StageError::Fatal(format!("Failed to scan {}: {e}", content_path.display())))?;

    for path in all_files {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if extension_whitelist.iter().any(|w| w.eq_ignore_ascii_case(ext)) {
            log.log(&format!("Found: {}", path.display()), false).await;
            media_files.push(path);
        } else {
            log.log(&format!("Pruning: {}", path.display()), false).await;
            fs::remove_file(&path).await.map_err(|e| StageError::Fatal(format!("Failed to prune {}: {e}", path.display())))?;
        }
    }

    log.log(&format!("Found {} media file(s), running mkvmerge", media_files.len()), false).await;

    let mkvmerge = find_binary("mkvmerge")
        .ok_or_else(|| StageError::Fatal("mkvmerge not found in PATH".into()))?;

    let content_name = content_path
        .file_name()
        .ok_or_else(|| StageError::Fatal("Could not determine content folder name".into()))?
        .to_string_lossy();
    let output_dir = download_dir.join(format!("{content_name}_output"));
    fs::create_dir_all(&output_dir).await
        .map_err(|e| StageError::Fatal(format!("Failed to create output dir: {e}")))?;

    let total = media_files.len() as f64;
    for (i, file) in media_files.iter().enumerate() {
        let stem = file.file_stem().unwrap_or_default();
        let output_path = output_dir.join(stem).with_extension("mkv");

        log.log(&format!("Merging: {}", file.display()), false).await;

        let output = Command::new(&mkvmerge)
            .args(["-o", output_path.to_str().unwrap()])
            .args(["--no-subtitles", "--no-attachments", "--no-chapters", "--no-global-tags", "--no-track-tags"])
            .arg(file)
            .output()
            .await
            .map_err(|e| StageError::Fatal(format!("Failed to run mkvmerge: {e}")))?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if !line.trim().is_empty() {
                log.log(&format!("mkvmerge: {line}"), false).await;
            }
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StageError::Fatal(format!("mkvmerge failed on {}: {stderr}", file.display())));
        }

        log.progress((i + 1) as f64 / total).await;
    }

    Ok(output_dir)
}
