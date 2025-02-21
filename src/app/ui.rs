use super::ActionProgress;
use super::ClaudeUploader;
use crate::upload::FileProcessor;
use crate::upload::UploadStatus;
use crate::utils::claude_keep::ClaudeKeepConfig;
use eframe::egui::{self, Align, Color32, RichText};
use reqwest::header::HeaderMap;
use rfd::FileDialog;
use std::path::Path;

impl ClaudeUploader {
    pub fn render(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let total_height = ui.available_height();
            let footer_height = 40.0;
            let footer_margin = 15.0;
            let content_height = total_height - footer_height - footer_margin;

            egui::ScrollArea::vertical()
                .max_height(content_height)
                .show(ui, |ui| {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("Claude.ai File Uploader");
                        ui.add_space(3.0);
                        ui.add_space(5.0);
                        ui.label(RichText::new("Upload your files to Claude.ai projects easily")
                            .color(ui.visuals().text_color().gamma_multiply(0.7)));
                    });

                    ui.add_space(20.0);

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

                        ui.add_space(8.0);

                        egui::Frame::none()
                            .inner_margin(0.0)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(150.0)
                                    .show(ui, |ui| {
                                        let text_edit = egui::TextEdit::multiline(&mut self.curl_text)
                                            .desired_width(ui.available_width())
                                            .font(egui::TextStyle::Monospace)
                                            .hint_text("curl 'https://claude.ai/api/organizations/<org-id>/projects/<project-id>/docs' ...");

                                        ui.add_sized(
                                            [ui.available_width(), 150.0],
                                            text_edit
                                        );
                                    });
                            });
                    });

                    ui.add_space(20.0);

                    ui.label("Note: Files listed in .gitignore will be automatically skipped");
                    ui.add_space(10.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("📁 Select Folder").clicked() {
                                if let Some(path) = FileDialog::new().pick_folder() {
                                    self.folder_path = Some(path.display().to_string());

                                    // Load .claudekeep configuration
                                    let path = Path::new(&path);
                                    self.state.keep_config = ClaudeKeepConfig::from_file(path);
                                    self.state.selected_sections.clear();
                                }
                            }
                            if let Some(folder) = &self.folder_path {
                                ui.label(format!("Selected: {}", folder));
                            }
                        });
                    });

                    // Section selector with file preview
                    if let Some(config) = &self.state.keep_config {
                        ui.add_space(10.0);
                        ui.group(|ui| {
                            ui.label(RichText::new("Select sections to upload:").strong());
                            ui.add_space(5.0);

                            let processor = FileProcessor::new(
                                self.folder_path.clone().unwrap_or_default(),
                                String::new(),
                                String::new(),
                                HeaderMap::new(),
                                Some(config.clone()),
                                self.state.selected_sections.clone(),
                            );
                            let file_count = processor.count_supported_files();

                            for section in &config.sections {
                                let mut selected = self.state.selected_sections.contains(section);
                                if ui.checkbox(&mut selected, section).changed() {
                                    if selected {
                                        self.state.selected_sections.push(section.clone());
                                    } else {
                                        self.state.selected_sections.retain(|s| s != section);
                                    }
                                }
                            }

                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("Files to be uploaded: {}", file_count))
                                .color(Color32::from_rgb(100, 150, 255)));
                        });
                    }

                    ui.add_space(20.0);

                    ui.vertical_centered(|ui| {
                        if !matches!(self.state.progress, ActionProgress::Completed { .. }) {
                            let can_upload = !self.curl_text.is_empty()
                                && self.folder_path.is_some()
                                && !self.state.is_uploading
                                && !self.state.is_deleting;

                            ui.add_enabled_ui(can_upload, |ui| {
                                let button = egui::Button::new("📤 Upload Files")
                                    .min_size(egui::vec2(200.0, 40.0));
                                if ui.add(button).clicked() {
                                    self.start_upload();
                                }
                            });
                        } else {
                            let can_delete = !self.state.is_uploading && !self.state.is_deleting;
                            let can_upload = !self.curl_text.is_empty() && self.folder_path.is_some();

                            ui.add_enabled_ui(can_delete && can_upload, |ui| {
                                if ui.button("🔄 Delete & Reupload").clicked() {
                                    self.delete_and_reupload();
                                }
                            });

                            ui.add_space(5.0);
                            if ui.button("🗑 Clear All").clicked() {
                                self.reset_upload_state();
                            }
                        }
                    });

                    ui.add_space(20.0);

                    if !matches!(self.state.progress, ActionProgress::NotStarted) {
                        ui.group(|ui| {
                            if let Some(current_file) = &self.state.current_file {
                                let status_text = match &self.state.progress {
                                    ActionProgress::Completed { failed, .. } => {
                                        if *failed > 0 {
                                            "Upload Failed"
                                        } else {
                                            "Upload Complete"
                                        }
                                    }
                                    _ => {
                                        if self.state.is_deleting {
                                            "🗑 Deleting"
                                        } else {
                                            "📤 Uploading"
                                        }
                                    }
                                };
                                ui.label(format!("{}: {}", status_text, current_file));
                            }

                            let progress = self.state.get_progress_percentage();
                            let progress_bar = egui::ProgressBar::new(progress)
                                .show_percentage()
                                .animate(false)
                                .fill(Color32::from_rgb(161, 89, 225));
                            ui.add(progress_bar);

                            ui.label(self.state.get_status_text());
                        });
                    }

                    if !self.state.file_statuses.is_empty() {
                        ui.add_space(10.0);
                        self.render_details(ui);
                    }

                    ui.add_space(20.0);
                });

            ui.with_layout(egui::Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(footer_margin);
                self.render_footer(ui);
            });
        });
    }

    fn show_auth_help_dialog(&self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        
        ui.group(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(220, 50, 50),
                "⚠️ Authentication Error (403 Forbidden)",
            );
            
            ui.add_space(5.0);
            
            ui.label("Claude.ai's authentication has changed or your session has expired.");
            ui.label("Here's how to fix it:");
            
            ui.add_space(5.0);
            
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("1. Open Claude.ai in your browser (");
                if ui
                    .add(egui::Label::new("https://claude.ai").sense(egui::Sense::click()))
                    .clicked()
                {
                    let _ = open::that("https://claude.ai");
                }
                ui.label(")");
            });
            
            ui.label("2. Open Developer Tools (F12 or right-click → Inspect)");
            ui.label("3. Go to the Network tab");
            ui.label("4. Upload any file manually in Claude.ai");
            ui.label("5. Find the upload request (search for 'docs' in the filter)");
            ui.label("6. Right-click the request and select 'Copy as cURL'");
            ui.label("7. Paste the new curl command in the text box above");
            
            ui.add_space(5.0);
            
            ui.colored_label(
                egui::Color32::from_rgb(100, 150, 255),
                "Note: Claude.ai sessions expire regularly. You may need to repeat this process when that happens.",
            );
        });
    }


    fn render_details(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(if self.state.show_details {
                "Hide Details"
            } else {
                "Show Details"
            })
            .clicked()
        {
            self.state.show_details = !self.state.show_details;
        }

        if self.state.show_details {
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Frame::none()
                        .fill(ui.style().visuals.extreme_bg_color)
                        .show(ui, |ui| {
                            ui.add_space(8.0);
                            for status in &self.state.file_statuses {
                                match &status.status {
                                    UploadStatus::Processing => {
                                        ui.horizontal(|ui| {
                                            ui.label("⏳");
                                            ui.colored_label(
                                                Color32::from_rgb(150, 150, 150),
                                                &format!("{} - Processing...", status.name),
                                            );
                                        });
                                    }
                                    UploadStatus::Success => {
                                        ui.horizontal(|ui| {
                                            ui.label("✅");
                                            ui.colored_label(
                                                Color32::from_rgb(0, 180, 0),
                                                &status.name,
                                            );
                                        });
                                    }
                                    UploadStatus::Error(err) => {
                                        ui.horizontal(|ui| {
                                            ui.label("❌");
                                            ui.colored_label(
                                                Color32::from_rgb(220, 50, 50),
                                                &format!("{} - {}", status.name, err),
                                            );
                                        });
                                    }
                                    UploadStatus::Skipped(reason) => {
                                        ui.horizontal(|ui| {
                                            ui.label("⏩");
                                            ui.colored_label(
                                                Color32::from_rgb(150, 150, 150),
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

    fn render_footer(&self, ui: &mut egui::Ui) {
        let footer_width = 200.0;
        let indent = (ui.available_width() - footer_width) / 2.0;

        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.scope(|ui| {
                ui.set_width(footer_width);
                ui.horizontal_centered(|ui| {
                    ui.label("Made with");
                    ui.colored_label(Color32::from_rgb(161, 89, 225), "♥");
                    ui.label("by");
                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new("@OnePromptMagic")
                                    .color(Color32::from_rgb(161, 89, 225)),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .clicked()
                    {
                        let _ = open::that("https://x.com/OnePromptMagic");
                    }
                });
            });
        });

        if let Some(error) = &self.state.error_message {
            ui.add_space(5.0);
            ui.vertical_centered(|ui| {
                ui.colored_label(Color32::from_rgb(220, 50, 50), error);
            });
        }

        if let Some(error) = &self.state.error_message {
            if error.contains("403") || error.contains("401") || error.contains("Authentication failed") {
                self.show_auth_help_dialog(ui);
            }
        }
    }
}
