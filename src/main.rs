use eframe::{egui, App, CreationContext};
use ignore::Walk;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use tokio::runtime::Runtime;

#[derive(Debug, Clone)]
enum UploadStatus {
    Success,
    Error(String),
    Skipped(String),
}

#[derive(Debug, Clone)]
struct FileStatus {
    name: String,
    status: UploadStatus,
}
#[derive(Clone, Default)]
struct UploadState {
    total_files: usize,
    processed_files: usize,
    successful_uploads: usize,
    failed_uploads: usize,
    skipped_count: usize,
    current_file: Option<String>,
    file_statuses: Vec<FileStatus>,
    error_message: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct UploadPayload {
    file_name: String,
    content: String,
}

#[derive(Default)]
struct MyApp {
    curl_text: String,
    folder_path: Option<String>,
    status_text: String,
    is_uploading: bool,
    headers: Option<HeaderMap>,
    organization_id: Option<String>,
    project_id: Option<String>,
    show_details: bool,
    upload_state: UploadState,
    status_receiver: Option<Receiver<FileStatus>>,
}

impl MyApp {
    fn show_header(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Claude.ai File Uploader 📁");
            ui.add_space(5.0);
            ui.label("Upload your files to Claude.ai projects easily");
            ui.add_space(20.0);
        });
    }

    fn show_curl_input(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Paste the curl request from Claude.ai");
                ui.add_space(4.0);

                ui.label("ℹ").on_hover_text_at_pointer(
                    "To get the curl command:\n\
                    1. Open Developer Tools (F12)\n\
                    2. Go to Network tab\n\
                    3. Upload a single file manually on Claude.ai\n\
                    4. Find the upload request (first 'docs' rq)\n\
                    5. Right-click and Copy as cURL",
                );
            });

            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), 100.0],
                        egui::TextEdit::multiline(&mut self.curl_text)
                            .hint_text("curl 'https://claude.ai/api/organizations/<org-id>/projects/<project-id>/docs' \\\n  -H 'accept: */*' \\\n  -H 'accept-language: en-US' \\\n  ..."),
                    );
                });
        });
    }

    fn show_folder_selection(&mut self, ui: &mut egui::Ui) {
        ui.label("Note: Files listed in .gitignore will be automatically skipped");
        ui.add_space(5.0);

        ui.group(|ui| {
            ui.horizontal(|ui| {
                if ui.button("📁 Select Folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.folder_path = Some(path.display().to_string());
                        self.status_text = format!("Selected folder: {}", path.display());
                    }
                }
                if let Some(folder) = &self.folder_path {
                    ui.label(format!("Selected: {}", folder));
                }
            });
        });
    }

    fn show_progress(&self, ui: &mut egui::Ui) {
        if self.is_uploading || self.upload_state.processed_files > 0 {
            ui.add_space(10.0);
            ui.group(|ui| {
                if let Some(current_file) = &self.upload_state.current_file {
                    if self.is_uploading {
                        ui.label(format!("📤 Uploading: {}", current_file));
                    } else if self.upload_state.failed_uploads == 0
                        && self.upload_state.skipped_count == 0
                    {
                        ui.colored_label(
                            egui::Color32::from_rgb(0, 180, 0),
                            "✅ Upload completed successfully!",
                        );
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(0, 180, 0),
                            format!(
                                "✅ Upload completed with {} skipped files",
                                self.upload_state.skipped_count
                            ),
                        );
                    }
                }

                let progress = if self.upload_state.total_files > 0 {
                    self.upload_state.processed_files as f32 / self.upload_state.total_files as f32
                } else {
                    0.0
                };

                let progress_bar = egui::ProgressBar::new(progress)
                    .show_percentage()
                    .animate(true);
                ui.add(progress_bar);

                ui.label(format!(
                    "Progress: {}/{} files | ✅ Success: {} | ⏩ Skipped: {} | ❌ Failed: {}",
                    self.upload_state.processed_files,
                    self.upload_state.total_files,
                    self.upload_state.successful_uploads,
                    self.upload_state.skipped_count,
                    self.upload_state.failed_uploads
                ));
            });
        }
    }

    fn show_footer(&self, ui: &mut egui::Ui) {
        let footer_width = 200.0;
        let indent = (ui.available_width() - footer_width) / 2.0;
        let available_space = ui.available_height();
        ui.allocate_space(egui::vec2(ui.available_width(), available_space - 24.0));

        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.scope(|ui| {
                ui.set_width(footer_width);
                ui.horizontal_centered(|ui| {
                    ui.label("Made with ");
                    ui.label("♥");
                    ui.label(" by ");
                    if ui.link("@OnePromptMagic").clicked() {
                        if let Err(e) = open::that("https://x.com/OnePromptMagic") {
                            eprintln!("Failed to open link: {}", e);
                        }
                    }
                });
            });
        });

        ui.add_space(5.0);
        if self.upload_state.error_message.is_some() {
            ui.vertical_centered(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 50, 50),
                    self.upload_state.error_message.as_ref().unwrap(),
                );
            });
        }
    }

    fn show_details(&mut self, ui: &mut egui::Ui) {
        if !self.upload_state.file_statuses.is_empty() {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(if self.show_details {
                        "Hide Details"
                    } else {
                        "Show Details"
                    })
                    .clicked()
                {
                    self.show_details = !self.show_details;
                }
            });
        }
    }

    fn show_details_with_max_height(&mut self, ui: &mut egui::Ui, max_height: f32) {
        if !self.upload_state.file_statuses.is_empty() {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(if self.show_details {
                        "Hide Details"
                    } else {
                        "Show Details"
                    })
                    .clicked()
                {
                    self.show_details = !self.show_details;
                }
            });

            if self.show_details {
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .max_height(max_height.min(300.0))
                    .show(ui, |ui| {
                        egui::Frame::none()
                            .fill(ui.style().visuals.extreme_bg_color)
                            .show(ui, |ui| {
                                ui.add_space(8.0);
                                for status in &self.upload_state.file_statuses {
                                    match &status.status {
                                        UploadStatus::Success => {
                                            ui.horizontal(|ui| {
                                                ui.label("✅");
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(0, 180, 0),
                                                    &status.name,
                                                );
                                            });
                                        }
                                        UploadStatus::Error(err) => {
                                            ui.horizontal(|ui| {
                                                ui.label("❌");
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(220, 50, 50),
                                                    &format!("{} - {}", status.name, err),
                                                );
                                            });
                                        }
                                        UploadStatus::Skipped(reason) => {
                                            ui.horizontal(|ui| {
                                                ui.label("⏩");
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(150, 150, 150),
                                                    &format!("{} - {}", status.name, reason),
                                                );
                                            });
                                        }
                                    }
                                    ui.add_space(4.0);
                                }
                                ui.add_space(8.0);
                            });
                    });
            }
        }
    }

    fn is_supported_file(path: &Path) -> bool {
        let ignored_files = [
            "package-lock.json",
            ".DS_Store",
            "node_modules",
            ".env",
            ".nuxt",
            ".nitro",
            ".cache",
            "dist",
        ];

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if ignored_files.contains(&file_name) {
                return false;
            }
        }

        let supported_extensions = [
            // Web
            "html",
            "css",
            "js",
            "jsx",
            "ts",
            "tsx",
            "vue",
            "svelte",
            // Python
            "py",
            "pyw",
            "pyx",
            "pyi",
            // Rust
            "rs",
            // Documentation/Config
            "md",
            "txt",
            "json",
            "yaml",
            "yml",
            "toml",
            "xml",
            // TypeScript definitions
            "d.ts",
            // Config files
            "env",
            "gitignore",
            "prettierrc",
            "eslintrc",
            "eslintignore",
            // Common web config files
            "babelrc",
            "browserslistrc",
            "editorconfig",
            "npmrc",
            // Logging
            "log",
        ];

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return supported_extensions.contains(&ext.to_lowercase().as_str());
        }

        // Handle files without extensions (like .env, .gitignore)
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            return supported_extensions.contains(&name.to_lowercase().as_str());
        }

        false
    }

    fn extract_ids_from_curl(&mut self) -> Result<(), String> {
        let curl_text = self.curl_text.clone();

        // Extract organization ID
        let org_id = curl_text
            .find("/organizations/")
            .and_then(|start_idx| {
                let start = start_idx + "/organizations/".len();
                let remaining = &curl_text[start..];
                remaining
                    .find('/')
                    .map(|end_idx| remaining[..end_idx].to_string())
            })
            .ok_or("Could not find organization ID in curl command".to_string())?;

        // Extract project ID
        let proj_id = curl_text
            .find("/projects/")
            .and_then(|start_idx| {
                let start = start_idx + "/projects/".len();
                let remaining = &curl_text[start..];
                remaining
                    .find('/')
                    .map(|end_idx| remaining[..end_idx].to_string())
            })
            .ok_or("Could not find project ID in curl command".to_string())?;

        // Extract headers
        let mut headers = HeaderMap::new();

        // Process each line to extract headers
        for line in curl_text.lines() {
            if !line.starts_with("  -H '") {
                continue;
            }

            let content = line
                .trim_start_matches("  -H '")
                .trim_end_matches('\'')
                .to_string();

            let parts: Vec<&str> = content.split(": ").collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0].to_lowercase();
            let value = parts[1];

            match HeaderName::from_str(&key) {
                Ok(header_name) => {
                    if let Ok(header_value) = HeaderValue::from_str(value) {
                        headers.insert(header_name, header_value);
                    }
                }
                Err(_) => continue,
            }
        }

        // Add essential headers
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );

        headers.insert(
            HeaderName::from_static("origin"),
            HeaderValue::from_static("https://claude.ai"),
        );

        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_str(&format!("https://claude.ai/project/{}", proj_id)).unwrap(),
        );

        self.organization_id = Some(org_id);
        self.project_id = Some(proj_id);
        self.headers = Some(headers);
        Ok(())
    }

    async fn process_folder(
        &self,
        folder_path: &Path,
        status_sender: &Sender<FileStatus>,
    ) -> (usize, usize, Vec<(PathBuf, String)>) {
        let mut successful_uploads = 0;
        let mut failed_uploads = 0;
        let mut errors = Vec::new();

        for result in Walk::new(folder_path) {
            match result {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_file() {
                        match self.upload_file(path, status_sender).await {
                            Ok(_) => {
                                successful_uploads += 1;
                            }
                            Err(e) => {
                                if e != "Unsupported file type" {
                                    errors.push((path.to_path_buf(), e));
                                    failed_uploads += 1;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("⚠️ Error walking directory: {}", e);
                }
            }
        }

        (successful_uploads, failed_uploads, errors)
    }

    async fn upload_file(
        &self,
        file_path: &Path,
        status_sender: &Sender<FileStatus>,
    ) -> Result<(), String> {
        let file_name = file_path
            .file_name()
            .ok_or("Invalid filename")?
            .to_str()
            .ok_or("Invalid filename encoding")?
            .to_string();

        // Skip unsupported files
        if !Self::is_supported_file(file_path) {
            let status = FileStatus {
                name: file_name,
                status: UploadStatus::Skipped("Unsupported file type".to_string()),
            };
            status_sender.send(status).unwrap_or_default();
            return Err("Unsupported file type".to_string());
        }

        let content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                let status = FileStatus {
                    name: file_name.clone(),
                    status: UploadStatus::Error(format!("Failed to read file: {}", e)),
                };
                status_sender.send(status).unwrap_or_default();
                return Err(format!("Failed to read file: {}", e));
            }
        };

        let payload = json!({
            "file_name": file_name.clone(),
            "content": content
        });

        let client = reqwest::Client::new();
        let url = format!(
            "https://claude.ai/api/organizations/{}/projects/{}/docs",
            self.organization_id.as_ref().unwrap(),
            self.project_id.as_ref().unwrap()
        );

        let response = client
            .post(&url)
            .headers(self.headers.clone().unwrap())
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        let status = response.status();

        let status_result = match status.as_u16() {
                404 => Err("API endpoint not found. Please check your authentication (cookies/session may have expired).".to_string()),
                401 => Err("Unauthorized. Please ensure you're logged in to Claude.ai and copy a fresh curl command.".to_string()),
                403 => Err("Forbidden. Please ensure you're logged in to Claude.ai and copy a fresh curl command.".to_string()),
                _ if !status.is_success() => Err(format!("Upload failed with status: {}", status)),
                _ => Ok(())
            };

        // Send status update
        let file_status = FileStatus {
            name: file_name,
            status: match &status_result {
                Ok(_) => UploadStatus::Success,
                Err(e) => UploadStatus::Error(e.clone()),
            },
        };
        status_sender.send(file_status).unwrap_or_default();

        status_result
    }
}
impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for status updates from the upload thread
        if let Some(receiver) = &self.status_receiver {
            while let Ok(status) = receiver.try_recv() {
                self.upload_state.processed_files += 1;
                match &status.status {
                    UploadStatus::Success => self.upload_state.successful_uploads += 1,
                    UploadStatus::Error(_) => self.upload_state.failed_uploads += 1,
                    UploadStatus::Skipped(_) => self.upload_state.skipped_count += 1,
                }
                self.upload_state.current_file = Some(status.name.clone());
                self.upload_state.file_statuses.push(status);

                // Check if we're done uploading
                if self.upload_state.processed_files >= self.upload_state.total_files {
                    self.is_uploading = false;
                    if self.upload_state.failed_uploads > 0 {
                        self.upload_state.error_message = Some(format!(
                            "Upload completed with {} failures. Check details for more information.",
                            self.upload_state.failed_uploads
                        ));
                    }

                    // Keep showing the final progress
                    self.upload_state.current_file = Some("Upload Complete".to_string());
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Create a containing vertical layout
            let content_height = ui.available_height() - 40.0; // Reserve space for footer

            egui::ScrollArea::vertical()
                .max_height(content_height)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        self.show_header(ui);
                        ui.add_space(10.0);
                        self.show_curl_input(ui);
                        ui.add_space(10.0);
                        self.show_folder_selection(ui);
                        ui.add_space(10.0);

                        // Upload button and progress
                        let can_upload = !self.curl_text.is_empty()
                            && self.folder_path.is_some()
                            && !self.is_uploading;

                        ui.vertical_centered(|ui| {
                            let upload_button = if self.is_uploading {
                                egui::Button::new("⏳ Uploading...")
                            } else if self.upload_state.processed_files > 0 {
                                // Show restart button after completion
                                if ui.button("🔄 Start New Upload").clicked() {
                                    self.folder_path = None;
                                    self.upload_state = UploadState::default();
                                    self.status_receiver = None;
                                    self.show_details = false;
                                    // Keep the curl_text as requested
                                }
                                return;
                            } else {
                                egui::Button::new("📤 Upload Files")
                            }.min_size([160.0, 36.0].into()); // Bigger button

                            if !self.is_uploading && self.upload_state.processed_files == 0 {
                                if ui.add_enabled(can_upload, upload_button).clicked() {
                                    // Reset state for new upload
                                    self.is_uploading = true;
                                    self.upload_state = UploadState::default();
                                    self.status_text = "Processing...".to_string();

                                    // Extract IDs and headers from curl
                                    if let Err(e) = self.extract_ids_from_curl() {
                                        self.status_text = format!("Error: {}", e);
                                        self.is_uploading = false;
                                        return;
                                    }

                                    let folder_path = PathBuf::from(self.folder_path.clone().unwrap());
                                    let app_clone = self.clone();

                                    // Create channel for status updates
                                    let (sender, receiver) = channel();
                                    self.status_receiver = Some(receiver);

                                    // Count total files before starting
                                    let mut total_files = 0;
                                    for entry in Walk::new(&folder_path) {
                                        if let Ok(entry) = entry {
                                            if entry.path().is_file() && Self::is_supported_file(entry.path()) {
                                                total_files += 1;
                                            }
                                        }
                                    }
                                    self.upload_state.total_files = total_files;

                                    // Create a new thread for file processing
                                    std::thread::spawn(move || {
                                        let rt = Runtime::new().unwrap();
                                        rt.block_on(async {
                                            let (_successful_uploads, failed_uploads, _errors) =
                                                app_clone.process_folder(&folder_path, &sender).await;

                                            if failed_uploads > 0 {
                                                println!("\n🔍 Troubleshooting Tips:");
                                                println!("1. Make sure your curl command is up to date (copy it right after uploading a file manually)");
                                                println!("2. Check if you're logged in to Claude.ai in your browser");
                                                println!("3. Verify the file types are supported");
                                                println!("4. Ensure files are not too large (max 10MB)");
                                                println!("5. Try uploading a single file manually first to verify access");

                                                if failed_uploads == total_files {
                                                    println!("\n⚠️ All uploads failed. Most likely causes:");
                                                    println!("- Session expired (copy a fresh curl command)");
                                                    println!("- Not logged in to Claude.ai");
                                                    println!("- Network issues");
                                                }
                                            }
                                        });
                                    });
                                }
                            }
                        });

                        // Show progress if uploading or completed
                        if self.is_uploading || self.upload_state.processed_files > 0 {
                            self.show_progress(ui);
                        }

                        // Show details section with constrained height
                        if self.show_details {
                            let remaining_height = content_height - ui.min_rect().height();
                            if remaining_height > 0.0 {
                                self.show_details_with_max_height(ui, remaining_height);
                            }
                        } else {
                            self.show_details(ui);
                        }

                        ui.add_space(20.0); // Add some space before footer
                    });
                });

            // Footer will always be at the bottom
            self.show_footer(ui);
        });
    }
}

impl Clone for MyApp {
    fn clone(&self) -> Self {
        MyApp {
            curl_text: self.curl_text.clone(),
            folder_path: self.folder_path.clone(),
            status_text: self.status_text.clone(),
            is_uploading: self.is_uploading,
            headers: self.headers.clone(),
            organization_id: self.organization_id.clone(),
            project_id: self.project_id.clone(),
            show_details: self.show_details,
            upload_state: self.upload_state.clone(),
            status_receiver: None,
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([600.0, 600.0])
            .with_min_inner_size([400.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Claude.ai File Uploader",
        options,
        Box::new(|_cc: &CreationContext| Box::new(MyApp::default())),
    );
}
