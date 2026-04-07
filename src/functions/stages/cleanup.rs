use std::path::PathBuf;

use tokio::fs;

use crate::classes::errors::StageError;
use crate::classes::job_handler::JobLogger;

/// Delete the raw download folder and its sterilized output sibling.
pub async fn cleanup(log: &JobLogger, content_path: &PathBuf) -> Result<(), StageError> {
    // Derive the output dir from the content path name (mirrors sterilize stage naming)
    let content_name = content_path
        .file_name()
        .ok_or_else(|| StageError::Fatal("Could not determine content folder name".into()))?
        .to_string_lossy();
    let output_dir = content_path
        .parent()
        .ok_or_else(|| StageError::Fatal("Content path has no parent".into()))?
        .join(format!("{content_name}_output"));

    // Remove raw download folder
    log.log(&format!("Removing: {}", content_path.display()), false).await;
    fs::remove_dir_all(content_path)
        .await
        .map_err(|e| StageError::Fatal(format!("Failed to remove download folder: {e}")))?;

    // Remove sterilized output folder
    log.log(&format!("Removing: {}", output_dir.display()), false).await;
    fs::remove_dir_all(&output_dir)
        .await
        .map_err(|e| StageError::Fatal(format!("Failed to remove output folder: {e}")))?;

    Ok(())
}
