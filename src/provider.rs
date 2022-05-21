use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StorageId {
    pub id: String,
}

#[derive(Copy, Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FileSize {
    pub size: u64,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FileHash {
    pub hash: String,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CloudProviderConfig {
    pub data: Bytes,
}

#[async_trait]
pub trait CloudProvider {
    // Initialize from serialized config.
    async fn load_from_config(config: CloudProviderConfig) -> Result<Self>
    where
        Self: Sized;

    // Send file to cloud, return its ID and metadata.
    async fn upload_file(&self, path: &std::path::Path) -> Result<(StorageId, FileSize, FileHash)>;

    // Load file from cloud and save locally, check hash, return download size.
    async fn download_file(
        &self,
        storage_id: StorageId,
        expected_hash: &FileHash,
        expected_size: &FileSize,
        path: &std::path::Path,
    ) -> Result<()>;
}
