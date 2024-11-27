// in app/state.rs

use crate::upload::{FileStatus, UploadedFile};
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
    pub status_receiver: Option<Receiver<FileStatus>>,
    pub uploaded_files_receiver: Option<Receiver<Vec<UploadedFile>>>,
}

impl UploadState {
    pub fn clear(&mut self) {
        self.progress = ActionProgress::NotStarted;
        self.current_file = None;
        self.file_statuses.clear();
        self.uploaded_files.clear();
        self.error_message = None;
        self.show_details = false;
        self.is_uploading = false;
        self.is_deleting = false;
        self.status_receiver = None;
        self.uploaded_files_receiver = None;
    }

    pub fn get_progress_percentage(&self) -> f32 {
        match &self.progress {
            ActionProgress::NotStarted => 0.0,
            ActionProgress::Uploading {
                total,
                current,
                successful,
                failed,
                skipped,
            } => {
                if *total == 0 {
                    0.0
                } else {
                    // Include both completed and currently processing files
                    (*current) as f32 / *total as f32
                }
            }
            ActionProgress::Deleting {
                total,
                current,
                successful,
                failed,
            } => {
                if *total == 0 {
                    0.0
                } else {
                    (*current) as f32 / *total as f32
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
            } => format!(
                "Progress: {}/{} files | ✅ Success: {} | ⏩ Skipped: {} | ❌ Failed: {}",
                current, total, successful, skipped, failed
            ),
            ActionProgress::Deleting {
                total,
                current,
                successful,
                failed,
            } => format!(
                "Deleting: {}/{} files | ✅ Success: {} | ❌ Failed: {}",
                current, total, successful, failed
            ),
            ActionProgress::Completed {
                total,
                successful,
                failed,
                skipped,
            } => format!(
                "Final Status: {}/{} files | ✅ Success: {} | ⏩ Skipped: {} | ❌ Failed: {}",
                total, total, successful, skipped, failed
            ),
        }
    }
}
