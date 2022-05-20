use crate::aws::AWS;
use crate::crypto::hash::ChunkedHash;
use crate::provider::{FileHash, FileSize, StorageId};
use anyhow::Result;
use aws_sdk_s3::model::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::types::ByteStream;
use bytes::BytesMut;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

const CHUNK_SIZE: usize = 100 * 1024 * 1024;

pub async fn s3_upload_file(
    aws: &AWS,
    path: &std::path::Path,
) -> Result<(StorageId, FileSize, FileHash)> {
    let storage_id = Uuid::new_v4().hyphenated().to_string();

    let mut file = File::open(path).await?;

    let start_resp = aws
        .s3_client()
        .create_multipart_upload()
        .bucket(aws.bucket().to_owned())
        .key(storage_id.to_owned())
        .send()
        .await?;

    let mut filesize = 0;
    let mut hash = ChunkedHash::keyed(&aws.file_hash_key());
    let mut parts = CompletedMultipartUpload::builder();

    // TODO abort upload if any piece fails
    for partnum in 1.. {
        let mut buffer = BytesMut::with_capacity(CHUNK_SIZE);

        // Tokio::io reads file in 16KB pieces; collate them before uploading.
        while buffer.len() < buffer.capacity() {
            if file.read_buf(&mut buffer).await? == 0 {
                break;
            }
        }

        if buffer.is_empty() {
            break;
        }

        filesize += buffer.len();

        let chunk = buffer.freeze();
        hash.update(chunk.to_owned());

        let upload_resp = aws
            .s3_client()
            .upload_part()
            .bucket(aws.bucket().to_owned())
            .key(storage_id.to_owned())
            .part_number(partnum)
            .set_upload_id(start_resp.upload_id.to_owned())
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

    aws.s3_client()
        .complete_multipart_upload()
        .bucket(aws.bucket().to_owned())
        .key(storage_id.to_owned())
        .set_upload_id(start_resp.upload_id.to_owned())
        .multipart_upload(parts.build())
        .send()
        .await?;

    Ok((
        StorageId { id: storage_id },
        FileSize {
            size: filesize as u64,
        },
        FileHash {
            hash: hex::encode(hash.finalize()),
        },
    ))
}
