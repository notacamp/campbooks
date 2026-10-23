//! Reading the documents events point at.

use campbooks_kernel::{
    JobError,
    blobs::{BlobRef, BlobStore},
};
use futures_util::StreamExt as _;
use serde::de::DeserializeOwned;

/// Most bytes of a document read.
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

/// The JSON document `blob` holds in `workspace`, if it is within `max`
/// bytes and parses as `T`. Failing says which, never what the document
/// held.
pub async fn read_json<T: DeserializeOwned>(
    blobs: &BlobStore,
    workspace: &str,
    blob: &BlobRef,
    max: usize,
) -> Result<T, JobError> {
    let mut stream = blobs.get(workspace, blob).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > max {
            return Err(JobError::fatal("the document is larger than the limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| JobError::fatal("the document does not parse"))
}
