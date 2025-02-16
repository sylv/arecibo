use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TorrentMetadata {
    pub name: String,
    pub size: u64,
    /// The files in the torrent. If None, the torrent is a single file.
    pub files: Option<Vec<TorrentMetadataFile>>,
}

#[derive(Debug, Serialize)]
pub struct TorrentMetadataFile {
    pub path: Vec<String>,
    pub size: u64,
}
