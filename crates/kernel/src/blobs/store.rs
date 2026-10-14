//! [`BlobStore`]: the object store plus its catalogue in Postgres.
//!
//! # Why a blob is never deleted under a reference
//!
//! Every object has a row in `kernel.blobs`, written *before* the object
//! (a put first upserts the row, which also sets `touched_at` to now, then
//! writes the object). The sweeper deletes a row and its object only while
//! holding that row's lock (`FOR UPDATE`), and only if, once locked, the row
//! has no reference and was not touched within the grace period:
//!
//! - **attach** locks the row `FOR KEY SHARE` before inserting the reference
//!   (and the reference's foreign key takes the same lock). If the sweeper
//!   holds the row, attach waits, then finds it gone and reports
//!   [`BlobError::NotStored`] without breaking the caller's transaction; if
//!   attach holds it, the sweeper skips it (`SKIP LOCKED`) and sees the new
//!   reference on its next pass.
//! - **put** of bytes the sweeper is deleting waits on the row lock, then
//!   inserts a fresh row and writes the object again. A put that touched the
//!   row first makes it ineligible for the whole grace period, which is ample
//!   time to finish writing the object and attach it.
//!
//! Crashes leave at worst a row without an object (swept after the grace
//! period) or a stale staging upload (swept by age), never an object the
//! sweeper cannot see.

use std::{fmt, sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::{StreamExt as _, TryStreamExt as _, stream::BoxStream};
use object_store::{
    ObjectStore, ObjectStoreExt as _, aws::AmazonS3Builder, buffered::BufWriter,
    local::LocalFileSystem, path::Path,
};
use secrecy::ExposeSecret as _;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

use super::{
    BlobBody, BlobError, BlobHash, BlobInfo, BlobOwner, BlobRef, ByteStream, Expected, SweepStats,
    check_owner, check_workspace, normalize_content_type,
    verify::{Hasher, Verified},
};
use crate::config::{BlobBackend, BlobsConfig, S3Config};

/// Top-level prefix of uploads whose hash is not known yet.
const STAGING: &str = "_staging";

/// Bytes buffered before a streamed upload switches to a multipart upload,
/// and the size of each part (S3 parts must be at least 5 MiB).
const PART_SIZE: usize = 8 * 1024 * 1024;

/// Parts uploaded at once by one streamed upload.
const PART_CONCURRENCY: usize = 2;

/// Blobs the sweeper deletes per transaction.
const SWEEP_BATCH: i64 = 100;

/// Where blobs are stored: an object store (local disk, S3-compatible, or in
/// memory for tests) and its catalogue in `kernel.blobs` /
/// `kernel.blob_refs`. Cheap to clone. See the [module docs](super).
#[derive(Clone, Default)]
pub struct BlobStore {
    /// `None` when this process has no blob storage configured
    /// ([`Default`]): every storage call fails with
    /// [`BlobError::NotConfigured`].
    inner: Option<Arc<Inner>>,
}

struct Inner {
    store: Arc<dyn ObjectStore>,
    db: PgPool,
    backend: &'static str,
    max_bytes: u64,
    grace: Duration,
}

impl fmt::Debug for BlobStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            Some(inner) => f
                .debug_struct("BlobStore")
                .field("backend", &inner.backend)
                .field("max_bytes", &inner.max_bytes)
                .field("grace", &inner.grace)
                .finish_non_exhaustive(),
            None => f.write_str("BlobStore(not configured)"),
        }
    }
}

impl BlobStore {
    /// The store `config` describes, cataloguing in `db`. Creates the `fs`
    /// directory if missing. The message names the offending setting.
    pub fn from_config(config: &BlobsConfig, db: PgPool) -> Result<Self, String> {
        config.check()?;
        let (store, backend): (Arc<dyn ObjectStore>, _) = match config.backend {
            BlobBackend::Fs => (Arc::new(local(config)?), "fs"),
            BlobBackend::S3 => (Arc::new(s3(&config.s3)?), "s3"),
        };
        Ok(Self::with_store(store, backend, config, db))
    }

    /// An in-memory store, for tests: nothing survives the process.
    #[cfg(any(test, feature = "test-util"))]
    pub fn memory(config: &BlobsConfig, db: PgPool) -> Self {
        Self::with_store(
            Arc::new(object_store::memory::InMemory::new()),
            "memory",
            config,
            db,
        )
    }

    fn with_store(
        store: Arc<dyn ObjectStore>,
        backend: &'static str,
        config: &BlobsConfig,
        db: PgPool,
    ) -> Self {
        Self {
            inner: Some(Arc::new(Inner {
                store,
                db,
                backend,
                max_bytes: config.max_object_bytes,
                grace: Duration::from_secs(config.sweep_grace_hours.saturating_mul(3_600)),
            })),
        }
    }

    /// The backend's name (`fs`, `s3`, `memory`), or `None` when not
    /// configured.
    pub fn backend(&self) -> Option<&'static str> {
        self.inner.as_ref().map(|inner| inner.backend)
    }

    fn inner(&self) -> Result<&Inner, BlobError> {
        self.inner.as_deref().ok_or(BlobError::NotConfigured)
    }

    /// Startup check that the store answers (bucket exists, credentials
    /// work, the directory is readable).
    pub async fn probe(&self) -> Result<(), BlobError> {
        let inner = self.inner()?;
        inner
            .store
            .list(Some(&Path::from(STAGING)))
            .take(1)
            .try_collect::<Vec<_>>()
            .await
            .map(drop)
            .or_else(not_found_ok)
    }

    /// Store `body` in `workspace` and return its [`BlobRef`]. Bytes already
    /// stored in the workspace are not stored twice. Attach the result to the
    /// record that holds it ([`attach`](Self::attach)) within the sweep grace
    /// period.
    pub async fn put(
        &self,
        workspace: &str,
        body: impl Into<BlobBody>,
        content_type: &str,
    ) -> Result<BlobRef, BlobError> {
        self.put_expecting(workspace, body, content_type, Expected::default())
            .await
    }

    /// [`put`](Self::put), rejecting a body whose length or SHA-256 differs
    /// from `expected` (nothing is stored then).
    pub async fn put_expecting(
        &self,
        workspace: &str,
        body: impl Into<BlobBody>,
        content_type: &str,
        expected: Expected,
    ) -> Result<BlobRef, BlobError> {
        check_workspace(workspace)?;
        let inner = self.inner()?;
        let (hash, size) = match body.into() {
            BlobBody::Bytes(bytes) => inner.put_bytes(workspace, bytes, expected).await?,
            BlobBody::Stream(stream) => inner.put_stream(workspace, stream, expected).await?,
        };
        tracing::debug!(workspace, %hash, size, backend = inner.backend, "blob stored");
        Ok(BlobRef {
            hash,
            size,
            content_type: normalize_content_type(content_type),
        })
    }

    /// The bytes of `blob` in `workspace`, streamed. The stream ends with
    /// [`BlobError::Corrupt`] if the stored bytes do not match the hash.
    pub async fn get(
        &self,
        workspace: &str,
        blob: &BlobRef,
    ) -> Result<ByteStream<BlobError>, BlobError> {
        check_workspace(workspace)?;
        let inner = self.inner()?;
        let object = inner
            .store
            .get(&key(workspace, &blob.hash))
            .await
            .map_err(not_found)?;
        Ok(Box::pin(Verified::new(
            object.into_stream(),
            blob.hash,
            blob.size,
        )))
    }

    /// The stored object's size and age, or `None` if `workspace` holds no
    /// such blob.
    pub async fn head(
        &self,
        workspace: &str,
        hash: &BlobHash,
    ) -> Result<Option<BlobInfo>, BlobError> {
        check_workspace(workspace)?;
        let inner = self.inner()?;
        match inner.store.head(&key(workspace, hash)).await {
            Ok(meta) => Ok(Some(BlobInfo {
                size: meta.size,
                last_modified: meta.last_modified,
            })),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Record, in the caller's transaction `tx`, that `owner` holds `blob`:
    /// the blob is kept while any owner holds it. Returns `false` if this
    /// owner already held it. Fails with [`BlobError::NotStored`] (leaving
    /// `tx` usable) if the blob is not stored in `workspace`.
    pub async fn attach(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace: &str,
        blob: &BlobRef,
        owner: &BlobOwner,
    ) -> Result<bool, BlobError> {
        check_workspace(workspace)?;
        check_owner(owner)?;
        let hash = blob.hash.to_string();
        // Hold the row against the sweeper until `tx` ends.
        let stored = sqlx::query_scalar!(
            r#"
            SELECT 1 AS "one!" FROM kernel.blobs
             WHERE workspace_id = $1 AND hash = $2
               FOR KEY SHARE
            "#,
            workspace,
            hash,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if stored.is_none() {
            return Err(BlobError::NotStored);
        }
        let inserted = sqlx::query!(
            r#"
            INSERT INTO kernel.blob_refs (workspace_id, hash, owner_type, owner_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING
            "#,
            workspace,
            hash,
            owner.kind(),
            owner.id(),
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        Ok(inserted > 0)
    }

    /// Record, in `tx`, that `owner` no longer holds the blob `hash`.
    /// Restarts the blob's grace period, so the detach can be undone by
    /// attaching again. Returns `false` if `owner` did not hold it.
    pub async fn detach(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace: &str,
        hash: &BlobHash,
        owner: &BlobOwner,
    ) -> Result<bool, BlobError> {
        check_workspace(workspace)?;
        check_owner(owner)?;
        let detached = sqlx::query_scalar!(
            r#"
            WITH gone AS (
                DELETE FROM kernel.blob_refs
                 WHERE workspace_id = $1 AND hash = $2 AND owner_type = $3 AND owner_id = $4
                RETURNING hash
            ), touched AS (
                UPDATE kernel.blobs b SET touched_at = now()
                  FROM gone WHERE b.workspace_id = $1 AND b.hash = gone.hash
            )
            SELECT count(*) AS "count!" FROM gone
            "#,
            workspace,
            hash.to_string(),
            owner.kind(),
            owner.id(),
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(detached > 0)
    }

    /// Detach, in `tx`, every blob `owner` holds (for example when the record
    /// is deleted). Returns how many it held.
    pub async fn detach_owner(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace: &str,
        owner: &BlobOwner,
    ) -> Result<u64, BlobError> {
        check_workspace(workspace)?;
        check_owner(owner)?;
        let detached = sqlx::query_scalar!(
            r#"
            WITH gone AS (
                DELETE FROM kernel.blob_refs
                 WHERE workspace_id = $1 AND owner_type = $2 AND owner_id = $3
                RETURNING hash
            ), touched AS (
                UPDATE kernel.blobs b SET touched_at = now()
                  FROM gone WHERE b.workspace_id = $1 AND b.hash = gone.hash
            )
            SELECT count(*) AS "count!" FROM gone
            "#,
            workspace,
            owner.kind(),
            owner.id(),
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(detached.unsigned_abs())
    }

    /// Detach, in `tx`, every blob held by the records `ids` of type `kind`
    /// (for example every message of a removed inbox), in one statement.
    /// Returns how many references were dropped.
    pub async fn detach_owners(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace: &str,
        kind: &str,
        ids: &[Uuid],
    ) -> Result<u64, BlobError> {
        check_workspace(workspace)?;
        check_owner(&BlobOwner::new(kind.to_owned(), Uuid::nil()))?;
        let detached = sqlx::query_scalar!(
            r#"
            WITH gone AS (
                DELETE FROM kernel.blob_refs
                 WHERE workspace_id = $1 AND owner_type = $2 AND owner_id = ANY($3)
                RETURNING hash
            ), touched AS (
                UPDATE kernel.blobs b SET touched_at = now()
                 WHERE b.workspace_id = $1 AND b.hash IN (SELECT hash FROM gone)
            )
            SELECT count(*) AS "count!" FROM gone
            "#,
            workspace,
            kind,
            ids,
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(detached.unsigned_abs())
    }

    /// Delete every blob of `workspace` — objects, catalogue rows and
    /// references — whoever holds them. For workspace deletion only (the
    /// `workspaces` module's purge job calls it once the grace period of a
    /// deleted workspace is over); idempotent, so
    /// a failed purge is simply run again. Returns how many objects it
    /// deleted.
    pub async fn purge_workspace(&self, workspace: &str) -> Result<u64, BlobError> {
        check_workspace(workspace)?;
        let inner = self.inner()?;
        // Objects first: if this fails half-way, the rows that remain make
        // the next run (or the sweeper) finish the job.
        let listed = inner
            .store
            .list(Some(&Path::from(workspace)))
            .map_ok(|meta| meta.location)
            .boxed();
        let deleted = inner.delete_all(listed).await?;
        let mut tx = inner.db.begin().await?;
        sqlx::query!(
            "DELETE FROM kernel.blob_refs WHERE workspace_id = $1",
            workspace
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM kernel.blobs WHERE workspace_id = $1",
            workspace
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(workspace, objects = deleted, "purged the workspace's blobs");
        Ok(deleted)
    }

    /// Delete blobs nothing holds whose grace period
    /// (`CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS`) has passed, and abandoned
    /// uploads older than it. Run hourly by [`SweepBlobs`](super::SweepBlobs).
    pub async fn sweep(&self) -> Result<SweepStats, BlobError> {
        let grace = self.inner()?.grace;
        self.sweep_older_than(grace).await
    }

    /// [`sweep`](Self::sweep) with an explicit grace period.
    pub(crate) async fn sweep_older_than(&self, grace: Duration) -> Result<SweepStats, BlobError> {
        let inner = self.inner()?;
        let mut stats = SweepStats::default();
        loop {
            let mut tx = inner.db.begin().await?;
            let claimed = claim_unreferenced(&mut tx, grace).await?;
            let deleted = inner.delete_claimed(&mut tx, &claimed, grace).await?;
            tx.commit().await?;
            stats.blobs += deleted;
            if claimed.len() < usize::try_from(SWEEP_BATCH).unwrap_or(usize::MAX) {
                break;
            }
        }
        let cutoff = chrono::Utc::now()
            - chrono::TimeDelta::from_std(grace).unwrap_or(chrono::TimeDelta::MAX);
        let stale = inner
            .store
            .list(Some(&Path::from(STAGING)))
            .try_filter(move |meta| std::future::ready(meta.last_modified < cutoff))
            .map_ok(|meta| meta.location)
            .boxed();
        stats.abandoned_uploads = inner.delete_all(stale).await?;
        Ok(stats)
    }
}

#[cfg(test)]
impl BlobStore {
    /// The object store itself, to inspect or damage objects in tests.
    pub(super) fn objects(&self) -> Arc<dyn ObjectStore> {
        self.inner.as_ref().map_or_else(
            || Arc::new(object_store::memory::InMemory::new()) as Arc<dyn ObjectStore>,
            |inner| Arc::clone(&inner.store),
        )
    }

    /// Step two of a sweep after [`claim_unreferenced`], for interleaving
    /// tests.
    pub(super) async fn finish_sweep(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        claimed: &[(String, String)],
        grace: Duration,
    ) -> Result<u64, BlobError> {
        self.inner()?.delete_claimed(tx, claimed, grace).await
    }
}

/// Step one of a sweep, inside `tx`: lock a batch of blobs that look
/// unreferenced and old enough, skipping any row an attach or put holds.
/// Once this returns, no reference to them can be added until `tx` ends.
pub(super) async fn claim_unreferenced(
    tx: &mut Transaction<'_, Postgres>,
    grace: Duration,
) -> Result<Vec<(String, String)>, BlobError> {
    let rows = sqlx::query!(
        r#"
        SELECT workspace_id, hash FROM kernel.blobs b
         WHERE touched_at < now() - make_interval(secs => $1)
           AND NOT EXISTS (SELECT 1 FROM kernel.blob_refs r
                            WHERE r.workspace_id = b.workspace_id AND r.hash = b.hash)
         ORDER BY touched_at
         LIMIT $2
           FOR UPDATE SKIP LOCKED
        "#,
        grace.as_secs_f64(),
        SWEEP_BATCH,
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().map(|r| (r.workspace_id, r.hash)).collect())
}

impl Inner {
    async fn put_bytes(
        &self,
        workspace: &str,
        bytes: Bytes,
        expected: Expected,
    ) -> Result<(BlobHash, u64), BlobError> {
        let size = bytes.len() as u64;
        if size > self.max_bytes {
            return Err(BlobError::TooLarge {
                limit: self.max_bytes,
            });
        }
        let hash = BlobHash::of(&bytes);
        check_expected(expected, hash, size)?;
        self.touch(workspace, &hash, size).await?;
        let key = key(workspace, &hash);
        if !self.exists(&key).await? {
            self.store.put(&key, bytes.into()).await?;
        }
        Ok((hash, size))
    }

    async fn put_stream(
        &self,
        workspace: &str,
        stream: ByteStream<std::io::Error>,
        expected: Expected,
    ) -> Result<(BlobHash, u64), BlobError> {
        let staging = Path::from(format!("{STAGING}/{}", Uuid::now_v7()));
        let promoted = async {
            let (hash, size) = self.upload(&staging, stream, expected).await?;
            self.touch(workspace, &hash, size).await?;
            let key = key(workspace, &hash);
            if self.exists(&key).await? {
                self.discard(&staging).await?;
            } else {
                self.store.rename(&staging, &key).await?;
            }
            Ok((hash, size))
        }
        .await;
        if promoted.is_err() {
            // Best effort; the sweeper removes stale uploads anyway.
            let _ = self.discard(&staging).await;
        }
        promoted
    }

    /// Stream `body` to `path`, hashing it and enforcing the size limit and
    /// `expected` on the way.
    async fn upload(
        &self,
        path: &Path,
        mut body: ByteStream<std::io::Error>,
        expected: Expected,
    ) -> Result<(BlobHash, u64), BlobError> {
        let mut writer = BufWriter::with_capacity(Arc::clone(&self.store), path.clone(), PART_SIZE)
            .with_max_concurrency(PART_CONCURRENCY);
        let mut hasher = Hasher::new();
        let mut size = 0_u64;
        let written = async {
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(BlobError::Read)?;
                size += chunk.len() as u64;
                if size > self.max_bytes {
                    return Err(BlobError::TooLarge {
                        limit: self.max_bytes,
                    });
                }
                if expected.size.is_some_and(|want| size > want) {
                    return Err(BlobError::Mismatch("size"));
                }
                hasher.update(&chunk);
                writer.put(chunk).await?;
            }
            // Completes the upload (one put, or the multipart's last part).
            writer.shutdown().await.map_err(|e| {
                BlobError::Store(object_store::Error::Generic {
                    store: "blobs",
                    source: Box::new(e),
                })
            })?;
            Ok(())
        }
        .await;
        if let Err(e) = written {
            let _ = writer.abort().await;
            return Err(e);
        }
        let hash = hasher.finish();
        check_expected(expected, hash, size)?;
        Ok((hash, size))
    }

    /// Upsert the catalogue row, restarting its grace period. Committed
    /// before the object is written (see the module docs).
    async fn touch(&self, workspace: &str, hash: &BlobHash, size: u64) -> Result<(), BlobError> {
        sqlx::query!(
            r#"
            INSERT INTO kernel.blobs (workspace_id, hash, size) VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, hash) DO UPDATE SET touched_at = now()
            "#,
            workspace,
            hash.to_string(),
            i64::try_from(size).unwrap_or(i64::MAX),
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Step two of a sweep, inside the claiming `tx`: delete the claimed
    /// rows that, seen afresh, still have no reference and are still old
    /// enough, then their objects. Returns how many it deleted.
    async fn delete_claimed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        claimed: &[(String, String)],
        grace: Duration,
    ) -> Result<u64, BlobError> {
        if claimed.is_empty() {
            return Ok(0);
        }
        let (workspaces, hashes): (Vec<String>, Vec<String>) = claimed.iter().cloned().unzip();
        let doomed = sqlx::query!(
            r#"
            DELETE FROM kernel.blobs b
             USING unnest($1::text[], $2::text[]) AS c(workspace_id, hash)
             WHERE b.workspace_id = c.workspace_id AND b.hash = c.hash
               AND b.touched_at < now() - make_interval(secs => $3)
               AND NOT EXISTS (SELECT 1 FROM kernel.blob_refs r
                                WHERE r.workspace_id = b.workspace_id AND r.hash = b.hash)
            RETURNING b.workspace_id, b.hash
            "#,
            &workspaces,
            &hashes,
            grace.as_secs_f64(),
        )
        .fetch_all(&mut **tx)
        .await?;
        let paths: Vec<_> = doomed
            .iter()
            .map(|row| Ok(key_of(&row.workspace_id, &row.hash)))
            .collect();
        self.delete_all(futures_util::stream::iter(paths).boxed())
            .await
    }

    /// Delete `paths`, treating already-missing objects as deleted. Returns
    /// how many objects it deleted.
    async fn delete_all(
        &self,
        paths: BoxStream<'static, object_store::Result<Path>>,
    ) -> Result<u64, BlobError> {
        let mut deleted = 0;
        let mut results = self.store.delete_stream(paths);
        while let Some(result) = results.next().await {
            match result {
                Ok(_) => deleted += 1,
                Err(object_store::Error::NotFound { .. }) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(deleted)
    }

    async fn exists(&self, path: &Path) -> Result<bool, BlobError> {
        match self.store.head(path).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    async fn discard(&self, path: &Path) -> Result<(), BlobError> {
        self.store.delete(path).await.or_else(not_found_ok)
    }
}

/// The object key of `hash` in `workspace`: `<workspace>/<aa>/<sha256>`.
fn key(workspace: &str, hash: &BlobHash) -> Path {
    key_of(workspace, &hash.to_string())
}

/// [`key`] from the hash's hex form, as the catalogue stores it.
fn key_of(workspace: &str, hex: &str) -> Path {
    let fan_out = hex.get(..2).unwrap_or_default();
    Path::from(format!("{workspace}/{fan_out}/{hex}"))
}

fn check_expected(expected: Expected, hash: BlobHash, size: u64) -> Result<(), BlobError> {
    if expected.size.is_some_and(|want| want != size) {
        return Err(BlobError::Mismatch("size"));
    }
    if expected.hash.is_some_and(|want| want != hash) {
        return Err(BlobError::Mismatch("SHA-256"));
    }
    Ok(())
}

fn not_found(err: object_store::Error) -> BlobError {
    match err {
        object_store::Error::NotFound { .. } => BlobError::NotFound,
        other => other.into(),
    }
}

fn not_found_ok(err: object_store::Error) -> Result<(), BlobError> {
    match err {
        object_store::Error::NotFound { .. } => Ok(()),
        other => Err(other.into()),
    }
}

/// The `fs` backend: atomic writes (temporary file, fsync, rename, fsync of
/// the directory).
fn local(config: &BlobsConfig) -> Result<LocalFileSystem, String> {
    let path = &config.path;
    std::fs::create_dir_all(path).map_err(|e| {
        format!(
            "CAMPBOOKS_BLOBS__PATH={}: cannot create the directory: {e}",
            path.display()
        )
    })?;
    LocalFileSystem::new_with_prefix(path)
        .map(|fs| fs.with_fsync(true))
        .map_err(|e| format!("CAMPBOOKS_BLOBS__PATH={}: {e}", path.display()))
}

/// The `s3` backend.
fn s3(config: &S3Config) -> Result<object_store::aws::AmazonS3, String> {
    let bucket = config.bucket.as_deref().unwrap_or_default().trim();
    let mut builder = AmazonS3Builder::new()
        .with_bucket_name(bucket)
        .with_region(config.region())
        .with_virtual_hosted_style_request(!config.path_style);
    if let Some(endpoint) = config.endpoint() {
        let endpoint = if config.path_style {
            endpoint.trim_end_matches('/').to_owned()
        } else {
            virtual_hosted(endpoint, bucket)?
        };
        builder = builder
            .with_allow_http(endpoint.starts_with("http://"))
            .with_endpoint(endpoint);
    }
    if let (Some(key), Some(secret)) = (&config.access_key_id, &config.secret_access_key) {
        builder = builder
            .with_access_key_id(key.trim())
            .with_secret_access_key(secret.expose_secret());
    }
    builder
        .build()
        .map_err(|e| format!("CAMPBOOKS_BLOBS__S3__*: {e}"))
}

/// `https://s3.example.com` → `https://<bucket>.s3.example.com`, as a
/// virtual-hosted-style store expects.
pub(super) fn virtual_hosted(endpoint: &str, bucket: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(endpoint)
        .map_err(|e| format!("CAMPBOOKS_BLOBS__S3__ENDPOINT={endpoint} is not a URL: {e}"))?;
    let host = url.host_str().unwrap_or_default().to_owned();
    url.set_host(Some(&format!("{bucket}.{host}")))
        .map_err(|e| format!("CAMPBOOKS_BLOBS__S3__BUCKET={bucket}: {e}"))?;
    Ok(url.as_str().trim_end_matches('/').to_owned())
}
