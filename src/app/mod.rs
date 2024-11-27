mod state;
mod ui;

use crate::upload::{FileProcessor, FileStatus, UploadStatus, UploadedFile};
use crate::utils::curl_parser::CurlParser;
use eframe::{egui, App};
use reqwest::header::HeaderMap;
pub use state::{ActionProgress, UploadState};
use std::sync::mpsc as std_mpsc;

#[derive(Default)]
pub struct ClaudeUploader {
    curl_text: String,
    folder_path: Option<String>,
    state: UploadState,
    curl_parser: CurlParser,
}

impl ClaudeUploader {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        println!("Initializing Claude.ai File Uploader");
        Self {
            curl_text: String::new(),
            folder_path: None,
            state: UploadState::default(),
            curl_parser: CurlParser::new(),
        }
    }

    pub fn reset_upload_state(&mut self) {
        println!("Resetting application state");
        self.curl_text.clear();
        self.folder_path = None;
        self.state.clear();
        self.curl_parser = CurlParser::new();
    }
    pub fn delete_and_reupload(&mut self) {
        if self.state.uploaded_files.is_empty() {
            println!("No files to delete. Uploaded files list is empty.");
            self.state.error_message = Some("No files to delete".to_string());
            return;
        }

        println!("Starting delete and reupload process...");
        println!(
            "Files to delete: {:?}",
            self.state
                .uploaded_files
                .iter()
                .map(|f| (&f.name, &f.uuid))
                .collect::<Vec<_>>()
        );

        self.state.is_deleting = true;
        self.state.error_message = None;
        self.state.file_statuses.clear();

        let files_to_delete = self.state.uploaded_files.clone();
        let folder_path = self.folder_path.clone();

        if let Err(e) = self.curl_parser.parse(&self.curl_text) {
            let error_msg = format!("Error parsing curl command: {}", e);
            println!("Error: {}", error_msg);
            self.state.error_message = Some(error_msg);
            self.state.is_deleting = false;
            return;
        }

        let (sender, receiver) = std_mpsc::channel();
        self.state.status_receiver = Some(receiver);

        self.state.progress = ActionProgress::Deleting {
            total: files_to_delete.len(),
            current: 0,
            successful: 0,
            failed: 0,
        };

        let org_id = self.curl_parser.organization_id.clone().unwrap();
        let proj_id = self.curl_parser.project_id.clone().unwrap();
        let headers = self.curl_parser.headers.clone().unwrap();
        let state = &mut self.state;

        println!("Starting deletion of {} files", files_to_delete.len());

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // First delete all files
                for file in files_to_delete {
                    let status = Self::delete_file(&org_id, &proj_id, &file, &headers).await;
                    sender.send(status).unwrap_or_default();
                }

                println!("Deletion process completed, starting reupload...");

                // Then start the upload process if we have a folder path
                if let Some(folder_path) = folder_path {
                    let processor = FileProcessor::new(
                        folder_path.clone(),
                        org_id.clone(),
                        proj_id.clone(),
                        headers.clone(),
                    );

                    println!("Processing files in folder: {}", folder_path);
                    let (upload_sender, upload_receiver) = std_mpsc::channel();
                    let uploaded_files = processor.process_files(&upload_sender).await;
                    println!("Reupload completed. Uploaded files: {:?}", uploaded_files);

                    // Forward the upload statuses to the main sender
                    while let Ok(status) = upload_receiver.try_recv() {
                        sender.send(status).unwrap_or_default();
                    }
                }
            });
        });
    }

    async fn delete_file(
        org_id: &str,
        project_id: &str,
        file: &UploadedFile,
        headers: &HeaderMap,
    ) -> FileStatus {
        println!(
            "Attempting to delete file '{}' with ID: {}",
            file.name, file.uuid
        );

        let client = reqwest::Client::new();
        let url = format!(
            "https://claude.ai/api/organizations/{}/projects/{}/docs/{}",
            org_id, project_id, file.uuid
        );

        let response = client.delete(&url).headers(headers.clone()).send().await;

        match response {
            Ok(res) => {
                let status = res.status();
                if status.is_success() {
                    println!(
                        "Successfully deleted file '{}' with ID: {}",
                        file.name, file.uuid
                    );
                    FileStatus {
                        name: file.name.clone(),
                        status: UploadStatus::Success,
                    }
                } else {
                    let error_msg = format!("Failed to delete with status: {}", status);
                    println!(
                        "Error deleting file '{}' with ID {}: {}",
                        file.name, file.uuid, error_msg
                    );
                    FileStatus {
                        name: file.name.clone(),
                        status: UploadStatus::Error(error_msg),
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to send delete request: {}", e);
                println!(
                    "Error deleting file '{}' with ID {}: {}",
                    file.name, file.uuid, error_msg
                );
                FileStatus {
                    name: file.name.clone(),
                    status: UploadStatus::Error(error_msg),
                }
            }
        }
    }

    pub fn start_upload(&mut self) {
        println!("Starting upload process...");
        self.state.is_uploading = true;
        self.state.error_message = None;
        self.state.file_statuses.clear();
        self.state.uploaded_files.clear();

        if let Err(e) = self.curl_parser.parse(&self.curl_text) {
            let error_msg = format!("Error parsing curl command: {}", e);
            println!("Error: {}", error_msg);
            self.state.error_message = Some(error_msg);
            self.state.is_uploading = false;
            return;
        }

        if let Some(folder_path) = &self.folder_path {
            println!("Processing folder: {}", folder_path);

            let processor = FileProcessor::new(
                folder_path.clone(),
                self.curl_parser.organization_id.clone().unwrap(),
                self.curl_parser.project_id.clone().unwrap(),
                self.curl_parser.headers.clone().unwrap(),
            );

            let (status_sender, status_receiver) = std_mpsc::channel();
            let (files_sender, files_receiver) = std_mpsc::channel();
            self.state.status_receiver = Some(status_receiver);
            self.state.uploaded_files_receiver = Some(files_receiver);

            let total_files = processor.count_supported_files();
            println!("Found {} supported files to upload", total_files);

            self.state.progress = ActionProgress::Uploading {
                total: total_files,
                current: 0,
                successful: 0,
                failed: 0,
                skipped: 0,
            };

            let processor = processor;

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let uploaded_files = processor.process_files(&status_sender).await;
                    println!(
                        "Upload process completed. Uploaded files: {:?}",
                        uploaded_files
                    );

                    // Send the uploaded files back to the main thread
                    let _ = files_sender.send(uploaded_files);

                    let _ = status_sender.send(FileStatus {
                        name: String::from(""),
                        status: UploadStatus::Success,
                    });
                });
            });
        } else {
            println!("No folder selected for upload");
            self.state.error_message = Some("No folder selected".to_string());
            self.state.is_uploading = false;
        }
    }

    pub fn update_state(&mut self, ctx: &egui::Context) {
        ctx.request_repaint();
        // Add ctx parameter
        // Check for uploaded files updates
        if let Some(receiver) = &self.state.uploaded_files_receiver {
            if let Ok(files) = receiver.try_recv() {
                self.state.uploaded_files = files;
                self.state.uploaded_files_receiver = None;
                ctx.request_repaint();
            }
        }

        // Check for status updates
        if let Some(receiver) = &self.state.status_receiver {
            let mut had_updates = false;

            while let Ok(status) = receiver.try_recv() {
                had_updates = true;
                let mut should_complete = false;
                let mut completion_state = None;

                match &mut self.state.progress {
                    ActionProgress::Uploading {
                        current,
                        successful,
                        failed,
                        skipped,
                        total,
                    } => {
                        match &status.status {
                            UploadStatus::Processing => {
                                *current += 1;
                            }
                            UploadStatus::Success => *successful += 1,
                            UploadStatus::Error(_) => *failed += 1,
                            UploadStatus::Skipped(_) => *skipped += 1,
                        }

                        if (*successful + *failed + *skipped) >= *total {
                            should_complete = true;
                            completion_state = Some(ActionProgress::Completed {
                                total: *total,
                                successful: *successful,
                                failed: *failed,
                                skipped: *skipped,
                            });
                        }
                    }
                    ActionProgress::Deleting {
                        current,
                        successful,
                        failed,
                        total,
                    } => {
                        match &status.status {
                            UploadStatus::Processing => {
                                *current += 1;
                            }
                            UploadStatus::Success => *successful += 1,
                            UploadStatus::Error(_) => *failed += 1,
                            _ => {}
                        }

                        if (*successful + *failed) >= *total {
                            should_complete = true;
                            completion_state = Some(ActionProgress::Completed {
                                total: *total,
                                successful: *successful,
                                failed: *failed,
                                skipped: 0,
                            });
                        }
                    }
                    _ => {}
                }

                self.state.current_file = Some(status.name.clone());
                self.state.file_statuses.push(status);

                if should_complete {
                    if let Some(completion_state) = completion_state {
                        let has_failures = matches!(&completion_state, ActionProgress::Completed { failed, .. } if *failed > 0);
                        self.state.progress = completion_state;

                        if has_failures {
                            self.state.error_message = Some(
                                    "Operation completed with failures. Check details for more information."
                                        .to_string(),
                                );
                        }
                        self.state.is_uploading = false;
                        self.state.is_deleting = false;
                    }
                }
            }

            if had_updates {
                ctx.request_repaint();
            }
        }
    }
}

impl App for ClaudeUploader {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_state(ctx);
        self.render(ctx);
    }
}
