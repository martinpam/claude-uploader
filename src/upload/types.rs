#[derive(Debug, Clone)]
pub enum UploadStatus {
    Processing,
    Success,
    Error(String),
    Skipped(String),
}

#[derive(Debug, Clone)]
pub struct FileStatus {
    pub name: String,
    pub status: UploadStatus,
}

#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub name: String,
    pub uuid: String,
}
