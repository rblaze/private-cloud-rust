use crate::aws::s3::{s3_download_file, s3_upload_file};
use crate::crypto::hash::HashKey;
use crate::crypto::master_key::MasterKey;
use crate::provider::*;
use anyhow::Result;
use async_trait::async_trait;
use aws_config::RetryConfig;
use aws_smithy_async::rt::sleep::TokioSleep;
use aws_types::app_name::AppName;
use aws_types::credentials::SharedCredentialsProvider;
use aws_types::region::Region;
use aws_types::{Credentials, SdkConfig};
use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
struct AwsConfig {
    s3_bucket: String,
    aws_region: String,
    aws_access_key_id: String,
    aws_secret_access_key: String,
    master_key: String,
}

impl std::fmt::Debug for AwsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsConfig")
            .field("s3_bucket", &self.s3_bucket)
            .field("aws_region", &self.aws_region)
            .field("aws_access_key_id", &self.aws_access_key_id)
            .field("aws_secret_access_key", &"*****")
            .field("master_key", &"*****")
            .finish()
    }
}

#[instrument]
pub fn create_aws_config() -> Result<CloudProviderConfig> {
    // TODO build config in smart way
    let config = AwsConfig {
        s3_bucket: "privatecloud-manual-test".to_owned(),
        aws_region: "us-east-1".to_owned(),
        aws_access_key_id: std::env::var("KEYID")?,
        aws_secret_access_key: std::env::var("SECRETKEY")?,
        master_key: std::env::var("MASTER_KEY")?,
    };

    let mut writer = BytesMut::with_capacity(1024).writer();
    serde_pickle::to_writer(&mut writer, &config, serde_pickle::SerOptions::new())?;

    Ok(CloudProviderConfig {
        data: writer.into_inner().freeze(),
    })
}

#[derive(Debug)]
pub struct AWS {
    bucket: String,
    s3_client: aws_sdk_s3::Client,
    master_key: MasterKey,
    file_hash_key: HashKey,
}

impl AWS {
    pub(crate) fn bucket(&self) -> &String {
        &self.bucket
    }

    pub(crate) fn s3_client(&self) -> &aws_sdk_s3::Client {
        &self.s3_client
    }

    pub(crate) fn file_hash_key(&self) -> &HashKey {
        &self.file_hash_key
    }
}

#[async_trait]
impl CloudProvider for AWS {
    async fn load_from_config(config: CloudProviderConfig) -> Result<Self> {
        aws_load_from_config(config).await
    }

    async fn upload_file(&self, path: &std::path::Path) -> Result<(StorageId, FileSize, FileHash)> {
        s3_upload_file(self, path).await
    }

    async fn download_file(
        &self,
        storage_id: StorageId,
        expected_hash: &FileHash,
        expected_size: &FileSize,
        path: &std::path::Path,
    ) -> Result<()> {
        s3_download_file(self, storage_id, expected_hash, expected_size, path).await
    }
}

#[instrument]
async fn aws_load_from_config(config: CloudProviderConfig) -> Result<AWS> {
    crate::crypto::init();

    let aws_config: AwsConfig =
        serde_pickle::from_reader(config.data.reader(), serde_pickle::DeOptions::new())?;

    let creds = Credentials::new(
        aws_config.aws_access_key_id,
        aws_config.aws_secret_access_key,
        None,
        None,
        "private_cloud",
    );

    let sdk_config = SdkConfig::builder()
        .app_name(AppName::new("PrivateCloud")?)
        .credentials_provider(SharedCredentialsProvider::new(creds))
        .region(Region::new(aws_config.aws_region))
        .retry_config(RetryConfig::new())
        .build();
    let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
        .sleep_impl(std::sync::Arc::new(TokioSleep::new()))
        .build();
    let s3_client = aws_sdk_s3::Client::from_conf(s3_config);

    let master_key = MasterKey::from(&aws_config.master_key)?;
    let file_hash_key = HashKey::new(&master_key, 1, "filehash")?;

    Ok(AWS {
        bucket: aws_config.s3_bucket,
        s3_client,
        master_key,
        file_hash_key,
    })
}
