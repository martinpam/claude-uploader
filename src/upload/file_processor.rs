use crate::upload::types::{FileStatus, UploadStatus, UploadedFile};
use crate::utils::claude_keep::ClaudeKeepConfig;
use ignore::Walk;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;

#[derive(Deserialize, Debug)]
struct UploadResponse {
    uuid: String,
    file_name: String,
}

#[derive(Deserialize, Debug)]
struct ErrorResponse {
    detail: Option<String>,
    message: Option<String>,
}

#[derive(Clone)]
pub struct FileProcessor {
    folder_path: String,
    organization_id: String,
    project_id: String,
    headers: HeaderMap,
    keep_config: Option<ClaudeKeepConfig>,
    selected_sections: Vec<String>,
}

impl FileProcessor {
    pub fn new(
        folder_path: String,
        organization_id: String,
        project_id: String,
        headers: HeaderMap,
        keep_config: Option<ClaudeKeepConfig>,
        selected_sections: Vec<String>,
    ) -> Self {
        Self {
            folder_path,
            organization_id,
            project_id,
            headers,
            keep_config,
            selected_sections,
        }
    }

    pub fn count_supported_files(&self) -> usize {
        let mut count = 0;
        for entry in Walk::new(&self.folder_path) {
            if let Ok(entry) = entry {
                if entry.path().is_file() && self.is_supported_file(entry.path()) {
                    count += 1;
                }
            }
        }
        count
    }

    pub async fn process_files(&self, status_sender: &Sender<FileStatus>) -> Vec<UploadedFile> {
        let mut uploaded_files = Vec::new();
        let mut files_to_process = Vec::new();

        for entry in Walk::new(&self.folder_path) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && self.is_supported_file(path) {
                    files_to_process.push(path.to_path_buf());
                }
            }
        }

        // First, verify we can connect by testing with a small file
        if !files_to_process.is_empty() {
            let test_path = &files_to_process[0];
            let file_name = test_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
                
            status_sender
                .send(FileStatus {
                    name: format!("Testing connection with {}", file_name),
                    status: UploadStatus::Processing,
                })
                .unwrap_or_default();
                
            let result = self.test_authentication(test_path).await;
            if let Err(error) = result {
                status_sender
                    .send(FileStatus {
                        name: "Authentication test".to_string(),
                        status: UploadStatus::Error(error),
                    })
                    .unwrap_or_default();
                    
                return uploaded_files;
            }
        }

        for file_path in files_to_process {
            let file_name = file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            status_sender
                .send(FileStatus {
                    name: file_name.clone(),
                    status: UploadStatus::Processing,
                })
                .unwrap_or_default();

            if let Ok(file) = self.upload_file(&file_path, status_sender).await {
                if let Some(uploaded_file) = file {
                    uploaded_files.push(uploaded_file);
                }
            }
        }

        uploaded_files
    }
    
    async fn test_authentication(&self, file_path: &Path) -> Result<(), String> {
        let file_name = file_path
            .file_name()
            .ok_or("Invalid filename")?
            .to_str()
            .ok_or("Invalid filename encoding")?
            .to_string();
            
        // Read a small portion of the file to test
        let content = match fs::read_to_string(file_path) {
            Ok(content) => {
                // Only use the first 100 chars for testing
                content.chars().take(100).collect::<String>() + "..."
            }
            Err(e) => {
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
        
        // Print headers for debugging
        println!("Testing authentication with headers:");
        for (key, value) in self.headers.iter() {
            if key == "cookie" || key == "authorization" {
                println!("  {}: [REDACTED]", key);
            } else {
                println!("  {}: {}", key, value.to_str().unwrap_or("[binary]"));
            }
        }
        
        println!("Request URL: {}", url);

        let response = client
            .post(&url)
            .headers(self.headers.clone())
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;
            
        let status = response.status();
        println!("Authentication test response status: {}", status);
        
        if status.is_success() {
            // Clean up the test upload if successful
            if let Ok(response_data) = response.json::<UploadResponse>().await {
                // Try to delete the test file
                self.delete_test_file(&response_data.uuid).await;
            }
            Ok(())
        } else {
            // Try to extract error message
            let error_body = response.text().await.unwrap_or_default();
            println!("Error response body: {}", error_body);
            
            let error_message = if error_body.contains("403") {
                "Authentication failed (403 Forbidden). Your session may have expired. Please update your curl command from Claude.ai.".to_string()
            } else if error_body.contains("401") {
                "Authentication failed (401 Unauthorized). Your session tokens are invalid. Please update your curl command from Claude.ai.".to_string()
            } else {
                format!("Upload failed with status: {}. Response: {}", status, error_body)
            };
            
            Err(error_message)
        }
    }
    
    async fn delete_test_file(&self, uuid: &str) {
        let client = reqwest::Client::new();
        let url = format!(
            "https://claude.ai/api/organizations/{}/projects/{}/docs/{}",
            self.organization_id, self.project_id, uuid
        );

        let _ = client.delete(&url).headers(self.headers.clone()).send().await;
        println!("Cleaned up test file with UUID: {}", uuid);
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

        if !self.is_supported_file(file_path) {
            let status = FileStatus {
                name: file_name,
                status: UploadStatus::Skipped(
                    "Not included in selected sections or unsupported type".to_string(),
                ),
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

        match response.status().as_u16() {
            200 | 201 => match response.json::<UploadResponse>().await {
                Ok(upload_response) => {
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
                    let error_msg = format!("Failed to parse upload response: {}", e);
                    let status = FileStatus {
                        name: file_name,
                        status: UploadStatus::Error(error_msg.clone()),
                    };
                    status_sender.send(status).unwrap_or_default();
                    Ok(None)
                }
            },
            403 => {
                let error_msg = "Access forbidden (403). Your session may have expired. Please update your curl command.".to_string();
                let status = FileStatus {
                    name: file_name,
                    status: UploadStatus::Error(error_msg),
                };
                status_sender.send(status).unwrap_or_default();
                Ok(None)
            },
            401 => {
                let error_msg = "Unauthorized (401). Your authentication tokens are invalid. Please update your curl command.".to_string();
                let status = FileStatus {
                    name: file_name,
                    status: UploadStatus::Error(error_msg),
                };
                status_sender.send(status).unwrap_or_default();
                Ok(None)
            },
            status_code => {
                let error_body = response.text().await.unwrap_or_default();
                let error_msg = format!("Upload failed with status: {}. Response: {}", status_code, error_body);
                let status = FileStatus {
                    name: file_name,
                    status: UploadStatus::Error(error_msg),
                };
                status_sender.send(status).unwrap_or_default();
                Ok(None)
            }
        }
    }

    fn is_supported_file(&self, path: &Path) -> bool {
        let ignored_paths = [
            "node_modules",
            ".nuxt",
            ".output",
            ".data",
            ".nitro",
            ".cache",
            "dist",
            "logs",
            ".wallet-db",
            ".fleet",
            ".idea",
        ];

        // Check if file is in an ignored directory
        if let Ok(canonical_path) = path.canonicalize() {
            let path_str = canonical_path.to_string_lossy();
            if ignored_paths
                .iter()
                .any(|ignored| path_str.contains(ignored))
            {
                return false;
            }
        }

        let ignored_files = [
            "package-lock.json",
            ".DS_Store",
            ".env",
            ".env.local",
            ".env.development",
            ".env.production",
        ];

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if ignored_files.contains(&file_name) {
                return false;
            }
        }

        // Check against .claudekeep configuration
        if let Some(config) = &self.keep_config {
            if !config.should_include_file(path, &self.selected_sections) {
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
            "gitignore",
            "prettierrc",
            "eslintrc",
            "eslintignore",
            "babelrc",
            "browserslistrc",
            "editorconfig",
            "npmrc",
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