use crate::upload::types::{FileStatus, UploadStatus, UploadedFile};
use ignore::Walk;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;

#[derive(Deserialize)]
struct UploadResponse {
    uuid: String,
    file_name: String,
}

#[derive(Clone)]
pub struct FileProcessor {
    folder_path: String,
    organization_id: String,
    project_id: String,
    headers: HeaderMap,
}

impl FileProcessor {
    pub fn new(
        folder_path: String,
        organization_id: String,
        project_id: String,
        headers: HeaderMap,
    ) -> Self {
        Self {
            folder_path,
            organization_id,
            project_id,
            headers,
        }
    }

    pub fn count_supported_files(&self) -> usize {
        let mut count = 0;
        for entry in Walk::new(&self.folder_path) {
            if let Ok(entry) = entry {
                if entry.path().is_file() && Self::is_supported_file(entry.path()) {
                    count += 1;
                }
            }
        }
        count
    }

    pub async fn process_files(&self, status_sender: &Sender<FileStatus>) -> Vec<UploadedFile> {
        let mut uploaded_files = Vec::new();

        for entry in Walk::new(&self.folder_path) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(file) = self.upload_file(path, status_sender).await {
                        if let Some(uploaded_file) = file {
                            uploaded_files.push(uploaded_file);
                        }
                    }
                }
            }
        }

        uploaded_files
    }

    async fn upload_file(
        &self,
        file_path: &Path,
        status_sender: &Sender<FileStatus>,
    ) -> Result<Option<UploadedFile>, String> {
        let file_name = file_path
            .file_name()
            .ok_or("Invalid filename")?
            .to_str()
            .ok_or("Invalid filename encoding")?
            .to_string();

        if !Self::is_supported_file(file_path) {
            let status = FileStatus {
                name: file_name,
                status: UploadStatus::Skipped("Unsupported file type".to_string()),
            };
            status_sender.send(status).unwrap_or_default();
            return Ok(None);
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
            self.organization_id, self.project_id
        );

        let response = client
            .post(&url)
            .headers(self.headers.clone())
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        let status = response.status();

        match status.as_u16() {
            200 | 201 => match response.json::<UploadResponse>().await {
                Ok(upload_response) => {
                    println!(
                        "Successfully uploaded file '{}' with ID: {}",
                        file_name, upload_response.uuid
                    );

                    let uploaded_file = UploadedFile {
                        name: file_name.clone(),
                        uuid: upload_response.uuid,
                    };

                    let status = FileStatus {
                        name: file_name,
                        status: UploadStatus::Success,
                    };
                    status_sender.send(status).unwrap_or_default();

                    Ok(Some(uploaded_file))
                }
                Err(e) => {
                    let error_msg =
                        format!("Failed to parse upload response for '{}': {}", file_name, e);
                    println!("Error: {}", error_msg);

                    let status = FileStatus {
                        name: file_name,
                        status: UploadStatus::Error(error_msg.clone()),
                    };
                    status_sender.send(status).unwrap_or_default();
                    Ok(None)
                }
            },
            status_code => {
                let error_msg = format!(
                    "Upload failed with status: {} for file '{}'",
                    status_code, file_name
                );
                println!("Error: {}", error_msg);

                let status = FileStatus {
                    name: file_name,
                    status: UploadStatus::Error(error_msg),
                };
                status_sender.send(status).unwrap_or_default();
                Ok(None)
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
            "html",
            "css",
            "js",
            "jsx",
            "ts",
            "tsx",
            "vue",
            "svelte",
            "py",
            "pyw",
            "pyx",
            "pyi",
            "rs",
            "md",
            "txt",
            "json",
            "yaml",
            "yml",
            "toml",
            "xml",
            "d.ts",
            "env",
            "gitignore",
            "prettierrc",
            "eslintrc",
            "eslintignore",
            "babelrc",
            "browserslistrc",
            "editorconfig",
            "npmrc",
            "log",
        ];

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return supported_extensions.contains(&ext.to_lowercase().as_str());
        }

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            return supported_extensions.contains(&name.to_lowercase().as_str());
        }

        false
    }
}
