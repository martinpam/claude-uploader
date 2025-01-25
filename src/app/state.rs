use crate::upload::{FileStatus, UploadedFile};
use crate::utils::claude_keep::ClaudeKeepConfig;
use derivative::Derivative;
use std::sync::mpsc::Receiver;

#[derive(Clone)]
pub enum ActionProgress {
    NotStarted,
    Uploading {
        total: usize,
        current: usize,
        successful: usize,
        failed: usize,
        skipped: usize,
    },
    Deleting {
        total: usize,
        current: usize,
        successful: usize,
        failed: usize,
    },
    Completed {
        total: usize,
        successful: usize,
        failed: usize,
        skipped: usize,
    },
}

impl Default for ActionProgress {
    fn default() -> Self {
        Self::NotStarted
    }
}

#[derive(Default)]
pub struct UploadState {
    pub progress: ActionProgress,
    pub current_file: Option<String>,
    pub file_statuses: Vec<FileStatus>,
    pub uploaded_files: Vec<UploadedFile>,
    pub error_message: Option<String>,
    pub show_details: bool,
    pub is_uploading: bool,
    pub is_deleting: bool,
    pub keep_config: Option<ClaudeKeepConfig>,
    pub selected_sections: Vec<String>,
    pub status_receiver: Option<Receiver<FileStatus>>,
    pub uploaded_files_receiver: Option<Receiver<Vec<UploadedFile>>>,
}

impl UploadState {
    pub fn clear(&mut self) {
        *self = UploadState::default();
    }

    pub fn clone_without_receivers(&self) -> Self {
        Self {
            progress: self.progress.clone(),
            current_file: self.current_file.clone(),
            file_statuses: self.file_statuses.clone(),
            uploaded_files: self.uploaded_files.clone(),
            error_message: self.error_message.clone(),
            show_details: self.show_details,
            is_uploading: self.is_uploading,
            is_deleting: self.is_deleting,
            keep_config: self.keep_config.clone(),
            selected_sections: self.selected_sections.clone(),
            status_receiver: None,
            uploaded_files_receiver: None,
        }
    }

    pub fn get_progress_percentage(&self) -> f32 {
        match &self.progress {
            ActionProgress::NotStarted => 0.0,
            ActionProgress::Uploading { total, current, .. } => {
                if *total == 0 {
                    0.0
                } else {
                    (*current as f32) / (*total as f32)
                }
            }
            ActionProgress::Deleting { total, current, .. } => {
                if *total == 0 {
                    0.0
                } else {
                    (*current as f32) / (*total as f32)
                }
            }
            ActionProgress::Completed { total, .. } => {
                if *total == 0 {
                    0.0
                } else {
                    1.0
                }
            }
        }
    }

    pub fn get_status_text(&self) -> String {
        match &self.progress {
            ActionProgress::NotStarted => String::new(),
            ActionProgress::Uploading {
                total,
                current,
                successful,
                failed,
                skipped,
            } => {
                format!(
                    "Progress: {}/{} files | ✅ Success: {} | ⏩ Skipped: {} | ❌ Failed: {}",
                    current, total, successful, skipped, failed
                )
            }
            ActionProgress::Deleting {
                total,
                current,
                successful,
                failed,
            } => {
                format!(
                    "Deleting: {}/{} files | ✅ Success: {} | ❌ Failed: {}",
                    current, total, successful, failed
                )
            }
            ActionProgress::Completed {
                total,
                successful,
                failed,
                skipped,
            } => {
                format!(
                    "Final Status: {}/{} files | ✅ Success: {} | ⏩ Skipped: {} | ❌ Failed: {}",
                    total, total, successful, skipped, failed
                )
            }
        }
    }
}
