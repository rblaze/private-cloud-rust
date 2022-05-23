use crate::aws::AWS;
use crate::crypto::hash::ChunkedHash;
use crate::provider::{FileHash, FileSize, StorageId};
use anyhow::{anyhow, Result};
use aws_sdk_s3::model::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::types::ByteStream;
use bytes::BytesMut;
use tokio::fs::{remove_file, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::StreamExt;
use tracing::{error, instrument, trace};
use uuid::Uuid;

const CHUNK_SIZE: usize = 100 * 1024 * 1024;

#[instrument]
pub async fn s3_upload_file(
    aws: &AWS,
    path: &std::path::Path,
) -> Result<(StorageId, FileSize, FileHash)> {
    let storage_id = Uuid::new_v4().hyphenated().to_string();

    trace!(%storage_id, "uploading file");

    let mut file = File::open(path).await?;
    let start_resp = aws
        .s3_client()
        .create_multipart_upload()
        .bucket(aws.bucket().to_owned())
        .key(storage_id.to_owned())
        .send()
        .await?;
    trace!(upload_id = ?start_resp.upload_id, "upload started");

    match send_parts(aws, &mut file, &storage_id, &start_resp.upload_id).await {
        Ok((parts, size, hash)) => {
            aws.s3_client()
                .complete_multipart_upload()
                .bucket(aws.bucket().to_owned())
                .key(storage_id.to_owned())
                .set_upload_id(start_resp.upload_id)
                .multipart_upload(parts)
                .send()
                .await?;

            Ok((StorageId { id: storage_id }, size, hash))
        }
        Err(e) => {
            trace!(error = %e, "upload failed");

            if let Err(error) = aws
                .s3_client()
                .abort_multipart_upload()
                .bucket(aws.bucket().to_owned())
                .key(storage_id.to_owned())
                .set_upload_id(start_resp.upload_id)
                .send()
                .await
            {
                error!(%error, "error aborting upload");
            }

            Err(e)
        }
    }
}

async fn send_parts(
    aws: &AWS,
    file: &mut File,
    storage_id: &String,
    upload_id: &Option<String>,
) -> Result<(CompletedMultipartUpload, FileSize, FileHash)> {
    let mut filesize = 0;
    let mut hash = ChunkedHash::keyed(&aws.file_hash_key());
    let mut parts = CompletedMultipartUpload::builder();

    for partnum in 1.. {
        let mut buffer = BytesMut::with_capacity(CHUNK_SIZE);

        // Tokio::io reads file in 16KB pieces; collate them before uploading.
        while buffer.len() < buffer.capacity() {
            if file.read_buf(&mut buffer).await? == 0 {
                break;
            }
        }

        if buffer.is_empty() {
            trace!("eof reached");
            break;
        }

        let chunk = buffer.freeze();

        trace!(
            part = partnum,
            part_offset = filesize,
            part_len = chunk.len(),
            "uploading chunk"
        );
        filesize += chunk.len();
        hash.update(chunk.to_owned());

        let upload_resp = aws
            .s3_client()
            .upload_part()
            .bucket(aws.bucket().to_owned())
            .key(storage_id.to_owned())
            .part_number(partnum)
            .set_upload_id(upload_id.to_owned())
            .body(ByteStream::from(chunk))
            .send()
            .await?;

        parts = parts.parts(
            CompletedPart::builder()
                .set_e_tag(upload_resp.e_tag)
                .part_number(partnum)
                .build(),
        );
    }

    Ok((
        parts.build(),
        FileSize {
            size: filesize as u64,
        },
        FileHash {
            hash: hex::encode(hash.finalize()),
        },
    ))
}

#[instrument]
pub async fn s3_download_file(
    aws: &AWS,
    storage_id: StorageId,
    expected_hash: &FileHash,
    expected_size: &FileSize,
    path: &std::path::Path,
) -> Result<()> {
    let result = s3_download_file_impl(aws, storage_id, expected_hash, expected_size, path).await;

    // Cleanup failed downloads
    if let Err(e) = &result {
        trace!(error= ?e, "download failed");

        if let Err(error) = remove_file(path).await {
            error!(?error, "error deleting partial download");
        }
    }

    result
}

async fn s3_download_file_impl(
    aws: &AWS,
    storage_id: StorageId,
    expected_hash: &FileHash,
    expected_size: &FileSize,
    path: &std::path::Path,
) -> Result<()> {
    trace!("downloading file");
    let mut resp = aws
        .s3_client()
        .get_object()
        .bucket(aws.bucket().to_owned())
        .key(storage_id.id)
        .send()
        .await?;

    trace!(content_length = resp.content_length, "download started");

    if resp.content_length() < 0 || resp.content_length() as u64 != expected_size.size {
        return Err(anyhow!(
            "File size mismatch: expected {}, got {}",
            expected_size.size,
            resp.content_length(),
        ));
    }

    let mut hash = ChunkedHash::keyed(&aws.file_hash_key());
    let mut file = File::create(path).await?;

    while let Some(mut bytes) = resp.body.try_next().await? {
        trace!(size = bytes.len(), "received body chunk");
        hash.update(bytes.clone());
        file.write_all_buf(&mut bytes).await?;
    }

    trace!("eof reached");
    file.flush().await?;

    let actual_hash = hex::encode(hash.finalize());

    if actual_hash != expected_hash.hash {
        return Err(anyhow!(
            "File hash mismatch: expected {}, got {}",
            expected_hash.hash,
            actual_hash,
        ));
    }

    Ok(())
}
