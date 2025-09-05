use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TorrentMetadata {
    pub name: String,
    pub size: u64,
    pub created_at: Option<i64>,
    pub files: Vec<TorrentMetadataFile>,
}

#[derive(Debug, Serialize)]
pub struct TorrentMetadataFile {
    pub path: Vec<String>,
    pub size: u64,
}
