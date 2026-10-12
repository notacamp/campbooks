//! Hashing bytes as they pass, and checking them on the way out.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use aws_lc_rs::digest;
use bytes::Bytes;
use futures_util::Stream;

use super::{BlobError, BlobHash, ByteStream};

/// An incremental SHA-256.
pub(super) struct Hasher(digest::Context);

impl Hasher {
    pub(super) fn new() -> Self {
        Self(digest::Context::new(&digest::SHA256))
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub(super) fn finish(self) -> BlobHash {
        let mut out = [0_u8; 32];
        out.copy_from_slice(self.0.finish().as_ref());
        BlobHash(out)
    }
}

/// A stored object's bytes, hashed as they are read: if the object turns out
/// longer, shorter or different from its address, the stream ends with
/// [`BlobError::Corrupt`] instead of ending cleanly.
pub(super) struct Verified {
    inner: ByteStream<object_store::Error>,
    hasher: Option<Hasher>,
    expected: BlobHash,
    size: u64,
    read: u64,
}

impl Verified {
    pub(super) fn new(
        inner: ByteStream<object_store::Error>,
        expected: BlobHash,
        size: u64,
    ) -> Self {
        Self {
            inner,
            hasher: Some(Hasher::new()),
            expected,
            size,
            read: 0,
        }
    }
}

impl Stream for Verified {
    type Item = Result<Bytes, BlobError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Finished: the verdict was given.
        if self.hasher.is_none() {
            return Poll::Ready(None);
        }
        match self.inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Err(e))) => {
                self.hasher = None;
                Poll::Ready(Some(Err(match e {
                    object_store::Error::NotFound { .. } => BlobError::NotFound,
                    other => BlobError::Store(other),
                })))
            }
            Poll::Ready(Some(Ok(chunk))) => {
                self.read += chunk.len() as u64;
                if self.read > self.size {
                    self.hasher = None;
                    return Poll::Ready(Some(Err(BlobError::Corrupt(self.expected))));
                }
                if let Some(hasher) = self.hasher.as_mut() {
                    hasher.update(&chunk);
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(None) => {
                let intact = self
                    .hasher
                    .take()
                    .is_some_and(|hasher| hasher.finish() == self.expected)
                    && self.read == self.size;
                if intact {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Err(BlobError::Corrupt(self.expected))))
                }
            }
        }
    }
}
