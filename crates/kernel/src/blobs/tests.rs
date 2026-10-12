//! Blob storage against a real Postgres (`#[sqlx::test]` gives each test a
//! fresh database with every migration applied), on the in-memory and the
//! local-disk backends.

use std::{io, path::PathBuf, time::Duration};

use bytes::Bytes;
use futures_util::TryStreamExt as _;
use object_store::{ObjectStoreExt as _, path::Path};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    store::{self, claim_unreferenced},
    *,
};
use crate::{
    config::{BlobBackend, BlobsConfig},
    jobs::Job as _,
};

const WS: &str = "0192f0c4-7c3a-7d4e-8a6b-111111111111";
const OTHER_WS: &str = "0192f0c4-7c3a-7d4e-8a6b-222222222222";
const DAY: Duration = Duration::from_hours(24);

fn memory(db: &PgPool) -> BlobStore {
    BlobStore::memory(&BlobsConfig::default(), db.clone())
}

fn owner(kind: &'static str) -> BlobOwner {
    BlobOwner::new(kind, Uuid::now_v7())
}

/// `bytes` as a stream of `chunk`-sized pieces.
fn chunked(bytes: &[u8], chunk: usize) -> BlobBody {
    let pieces: Vec<io::Result<Bytes>> = bytes
        .chunks(chunk.max(1))
        .map(|c| Ok(Bytes::copy_from_slice(c)))
        .collect();
    BlobBody::stream(futures_util::stream::iter(pieces))
}

async fn read(blobs: &BlobStore, workspace: &str, blob: &BlobRef) -> Result<Vec<u8>, BlobError> {
    let stream = blobs.get(workspace, blob).await?;
    let chunks: Vec<Bytes> = stream.try_collect().await?;
    Ok(chunks.concat())
}

/// Every object in the store, staging uploads included.
async fn objects(blobs: &BlobStore) -> Vec<String> {
    let mut all: Vec<String> = blobs
        .objects()
        .list(None)
        .map_ok(|meta| meta.location.to_string())
        .try_collect()
        .await
        .unwrap();
    all.sort();
    all
}

async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

async fn attach(
    blobs: &BlobStore,
    db: &PgPool,
    workspace: &str,
    blob: &BlobRef,
    owner: &BlobOwner,
) {
    let mut tx = db.begin().await.unwrap();
    assert!(blobs.attach(&mut tx, workspace, blob, owner).await.unwrap());
    tx.commit().await.unwrap();
}

/// Make every catalogued blob look `age` old.
async fn age_all(db: &PgPool, age: Duration) {
    sqlx::query("UPDATE kernel.blobs SET touched_at = now() - make_interval(secs => $1)")
        .bind(age.as_secs_f64())
        .execute(db)
        .await
        .unwrap();
}

/// A scratch directory for the `fs` backend, removed on drop.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("campbooks-blobs-{}", Uuid::now_v7())))
    }

    fn store(&self, db: &PgPool) -> BlobStore {
        let config = BlobsConfig {
            backend: BlobBackend::Fs,
            path: self.0.clone(),
            ..BlobsConfig::default()
        };
        BlobStore::from_config(&config, db.clone()).unwrap()
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// --- values ------------------------------------------------------------------

#[test]
fn hash_is_sha256_in_lower_case_hex() {
    let hash = BlobHash::of(b"abc");
    assert_eq!(
        hash.to_string(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(hash.to_string().parse::<BlobHash>(), Ok(hash));
    assert_eq!(
        hash.to_string().to_uppercase().parse::<BlobHash>(),
        Ok(hash)
    );
    for bad in ["", "abc", &"g".repeat(64), &format!("+{}", "a".repeat(63))] {
        assert_eq!(bad.parse::<BlobHash>(), Err(ParseBlobHashError), "{bad}");
    }
    let json = serde_json::to_value(BlobRef {
        hash,
        size: 3,
        content_type: "text/plain".into(),
    })
    .unwrap();
    assert_eq!(json["hash"], hash.to_string());
    assert_eq!(serde_json::from_value::<BlobRef>(json).unwrap().hash, hash);
}

#[test]
fn content_types_are_kept_but_not_trusted() {
    assert_eq!(normalize_content_type(" Message/RFC822 "), "message/rfc822");
    assert_eq!(
        normalize_content_type("text/plain; charset=UTF-8"),
        "text/plain; charset=utf-8"
    );
    for junk in ["", "pdf", "text/", "/x", "text/html\r\nX-Evil: 1", "a b/c"] {
        assert_eq!(
            normalize_content_type(junk),
            DEFAULT_CONTENT_TYPE,
            "{junk:?}"
        );
    }
}

#[test]
fn workspace_ids_and_owner_types_are_checked() {
    assert!(check_workspace(WS).is_ok());
    for bad in ["", "_staging", "a/b", "..", &"a".repeat(65), "ws 1"] {
        assert!(check_workspace(bad).is_err(), "{bad:?}");
    }
    assert!(check_owner(&owner("mail.message")).is_ok());
    for bad in ["mail", "Mail.message", "mail.message.part", ".x"] {
        assert!(check_owner(&owner(bad)).is_err(), "{bad:?}");
    }
}

#[tokio::test]
async fn s3_settings_build_a_store_without_touching_the_network() {
    assert_eq!(
        store::virtual_hosted("https://s3.example.com:9000/", "mail").unwrap(),
        "https://mail.s3.example.com:9000"
    );
    let db = crate::testing::unreachable_pool();
    let mut config = BlobsConfig {
        backend: BlobBackend::S3,
        ..BlobsConfig::default()
    };
    config.s3.bucket = Some("campbooks".into());
    config.s3.endpoint = Some("http://localhost:3900".into());
    config.s3.path_style = true;
    config.s3.access_key_id = Some("GK-example".into());
    config.s3.secret_access_key = Some("very-secret".into());
    let blobs = BlobStore::from_config(&config, db.clone()).unwrap();
    assert_eq!(blobs.backend(), Some("s3"));
    assert!(!format!("{blobs:?}{config:?}").contains("very-secret"));

    config.s3.path_style = false;
    config.s3.endpoint = Some("https://s3.example.com".into());
    assert!(BlobStore::from_config(&config, db.clone()).is_ok());

    config.s3.secret_access_key = None;
    let err = BlobStore::from_config(&config, db).unwrap_err();
    assert!(
        err.contains("CAMPBOOKS_BLOBS__S3__SECRET_ACCESS_KEY"),
        "{err}"
    );
}

#[tokio::test]
async fn a_blob_directory_that_cannot_be_created_names_the_setting() {
    let file = ScratchDir::new();
    std::fs::write(&file.0, b"not a directory").unwrap();
    let config = BlobsConfig {
        path: file.0.join("blobs"),
        ..BlobsConfig::default()
    };
    let err = BlobStore::from_config(&config, crate::testing::unreachable_pool()).unwrap_err();
    assert!(err.contains("CAMPBOOKS_BLOBS__PATH"), "{err}");
    let _ = std::fs::remove_file(&file.0);
}

// --- put / get ---------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn bytes_round_trip_in_memory(db: PgPool) {
    let blobs = memory(&db);
    let blob = blobs
        .put(WS, b"hello".as_slice(), "Text/Plain")
        .await
        .unwrap();
    assert_eq!(blob.hash, BlobHash::of(b"hello"));
    assert_eq!(blob.size, 5);
    assert_eq!(blob.content_type, "text/plain");
    assert_eq!(read(&blobs, WS, &blob).await.unwrap(), b"hello");

    let info = blobs.head(WS, &blob.hash).await.unwrap().unwrap();
    assert_eq!(info.size, 5);
    let hash = blob.hash.to_string();
    assert_eq!(
        objects(&blobs).await,
        [format!("{WS}/{}/{hash}", &hash[..2])],
        "content-addressed key"
    );
    // Another workspace cannot read it, even with the ref.
    assert!(matches!(
        read(&blobs, OTHER_WS, &blob).await,
        Err(BlobError::NotFound)
    ));
    assert_eq!(blobs.head(OTHER_WS, &blob.hash).await.unwrap(), None);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_large_stream_round_trips_through_a_multipart_upload(db: PgPool) {
    let blobs = memory(&db);
    // Bigger than one part, so the upload is multipart; odd chunk size.
    let body: Vec<u8> = (0..20 * 1024 * 1024 + 7_u32)
        .map(|i| (i % 251) as u8)
        .collect();
    let blob = blobs
        .put(WS, chunked(&body, 64 * 1024 + 3), "application/pdf")
        .await
        .unwrap();
    assert_eq!(blob.size, body.len() as u64);
    assert_eq!(blob.hash, BlobHash::of(&body));
    assert_eq!(read(&blobs, WS, &blob).await.unwrap(), body);
    assert_eq!(objects(&blobs).await.len(), 1, "no staging upload left");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_empty_body_is_a_blob(db: PgPool) {
    let blobs = memory(&db);
    let streamed = blobs.put(WS, chunked(b"", 1), "").await.unwrap();
    let in_memory = blobs.put(WS, Vec::new(), "").await.unwrap();
    assert_eq!(streamed, in_memory);
    assert_eq!(streamed.size, 0);
    assert_eq!(streamed.content_type, DEFAULT_CONTENT_TYPE);
    assert!(read(&blobs, WS, &streamed).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn round_trips_on_local_disk(db: PgPool) {
    let dir = ScratchDir::new();
    let blobs = dir.store(&db);
    assert_eq!(blobs.backend(), Some("fs"));
    blobs.probe().await.unwrap();

    let body: Vec<u8> = (0..9 * 1024 * 1024_u32).map(|i| (i % 13) as u8).collect();
    let streamed = blobs
        .put(WS, chunked(&body, 100_000), "message/rfc822")
        .await
        .unwrap();
    let small = blobs
        .put(WS, b"tiny".as_slice(), "text/plain")
        .await
        .unwrap();
    assert_eq!(read(&blobs, WS, &streamed).await.unwrap(), body);
    assert_eq!(read(&blobs, WS, &small).await.unwrap(), b"tiny");

    let hash = streamed.hash.to_string();
    let file = dir.0.join(WS).join(&hash[..2]).join(&hash);
    assert_eq!(std::fs::read(file).unwrap(), body, "plain file at its key");
    assert!(
        !dir.0.join("_staging").exists()
            || std::fs::read_dir(dir.0.join("_staging")).unwrap().count() == 0,
        "the staging upload was moved into place"
    );

    // Survives a new store over the same directory (a restart).
    let reopened = dir.store(&db);
    assert_eq!(read(&reopened, WS, &small).await.unwrap(), b"tiny");
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_same_bytes_are_stored_once_and_held_twice(db: PgPool) {
    let blobs = memory(&db);
    let first = blobs
        .put(WS, b"invoice.pdf bytes".as_slice(), "application/pdf")
        .await
        .unwrap();
    let second = blobs
        .put(WS, chunked(b"invoice.pdf bytes", 4), "application/pdf")
        .await
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(objects(&blobs).await.len(), 1, "one object");
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blobs").await, 1);

    let (a, b) = (owner("mail.message"), owner("mail.message"));
    attach(&blobs, &db, WS, &first, &a).await;
    attach(&blobs, &db, WS, &second, &b).await;
    let mut tx = db.begin().await.unwrap();
    assert!(
        !blobs.attach(&mut tx, WS, &first, &a).await.unwrap(),
        "attaching twice is a no-op"
    );
    tx.commit().await.unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blob_refs").await, 2);

    // The same bytes in another workspace are that workspace's own copy.
    blobs
        .put(OTHER_WS, b"invoice.pdf bytes".as_slice(), "application/pdf")
        .await
        .unwrap();
    assert_eq!(objects(&blobs).await.len(), 2);
}

// --- verification and limits ---------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn a_truncated_stream_fails_verification_and_stores_nothing(db: PgPool) {
    let blobs = memory(&db);
    let full = b"the whole attachment, every byte of it";
    let expected = Expected {
        hash: Some(BlobHash::of(full)),
        size: None,
    };
    let truncated = chunked(&full[..20], 7);
    assert!(matches!(
        blobs
            .put_expecting(WS, truncated, "text/plain", expected)
            .await,
        Err(BlobError::Mismatch("SHA-256"))
    ));
    let short = Expected {
        hash: None,
        size: Some(full.len() as u64),
    };
    assert!(matches!(
        blobs
            .put_expecting(WS, chunked(&full[..20], 7), "text/plain", short)
            .await,
        Err(BlobError::Mismatch("size"))
    ));
    assert!(matches!(
        blobs
            .put_expecting(WS, full[..20].to_vec(), "text/plain", expected)
            .await,
        Err(BlobError::Mismatch("SHA-256"))
    ));

    // A stream that breaks off with an error.
    let broken = BlobBody::stream(futures_util::stream::iter(vec![
        Ok(Bytes::from_static(b"first part")),
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "peer went away",
        )),
    ]));
    assert!(matches!(
        blobs.put(WS, broken, "text/plain").await,
        Err(BlobError::Read(_))
    ));

    assert!(
        objects(&blobs).await.is_empty(),
        "no object, no staging upload"
    );
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blobs").await, 0);

    // The real thing passes.
    let blob = blobs
        .put_expecting(
            WS,
            chunked(full, 7),
            "text/plain",
            Expected {
                hash: Some(BlobHash::of(full)),
                size: Some(full.len() as u64),
            },
        )
        .await
        .unwrap();
    assert_eq!(blob.hash, BlobHash::of(full));
}

#[sqlx::test(migrations = "../../migrations")]
async fn bodies_over_the_size_limit_are_refused(db: PgPool) {
    let config = BlobsConfig {
        max_object_bytes: 1_000,
        ..BlobsConfig::default()
    };
    let blobs = BlobStore::memory(&config, db.clone());
    let too_big = vec![7_u8; 1_001];
    assert!(matches!(
        blobs.put(WS, too_big.clone(), "x/y").await,
        Err(BlobError::TooLarge { limit: 1_000 })
    ));
    assert!(matches!(
        blobs.put(WS, chunked(&too_big, 100), "x/y").await,
        Err(BlobError::TooLarge { limit: 1_000 })
    ));
    assert!(objects(&blobs).await.is_empty());
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blobs").await, 0);
    // At the limit is fine.
    blobs.put(WS, vec![7_u8; 1_000], "x/y").await.unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_damaged_object_ends_its_stream_with_an_error(db: PgPool) {
    let blobs = memory(&db);
    let blob = blobs
        .put(WS, b"original".as_slice(), "text/plain")
        .await
        .unwrap();
    let hash = blob.hash.to_string();
    let key = Path::from(format!("{WS}/{}/{hash}", &hash[..2]));
    blobs
        .objects()
        .put(&key, Bytes::from_static(b"tampered").into())
        .await
        .unwrap();
    assert!(matches!(
        read(&blobs, WS, &blob).await,
        Err(BlobError::Corrupt(h)) if h == blob.hash
    ));
    blobs
        .objects()
        .put(&key, Bytes::from_static(b"original plus more").into())
        .await
        .unwrap();
    assert!(matches!(
        read(&blobs, WS, &blob).await,
        Err(BlobError::Corrupt(_))
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_unconfigured_store_refuses_storage_calls(db: PgPool) {
    let blobs = BlobStore::default();
    assert_eq!(blobs.backend(), None);
    assert!(matches!(
        blobs.put(WS, b"x".as_slice(), "text/plain").await,
        Err(BlobError::NotConfigured)
    ));
    assert!(matches!(blobs.sweep().await, Err(BlobError::NotConfigured)));
    let _ = db;
}

// --- references ----------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn attaching_an_unknown_blob_fails_without_breaking_the_transaction(db: PgPool) {
    let blobs = memory(&db);
    let ghost = BlobRef {
        hash: BlobHash::of(b"never stored"),
        size: 12,
        content_type: "text/plain".into(),
    };
    let mut tx = db.begin().await.unwrap();
    assert!(matches!(
        blobs
            .attach(&mut tx, WS, &ghost, &owner("mail.message"))
            .await,
        Err(BlobError::NotStored)
    ));
    let one: i32 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(one, 1, "the caller's transaction is still usable");
    tx.commit().await.unwrap();

    // Stored in another workspace only: still not this workspace's.
    let other = blobs
        .put(OTHER_WS, b"never stored".as_slice(), "text/plain")
        .await
        .unwrap();
    let mut tx = db.begin().await.unwrap();
    assert!(matches!(
        blobs
            .attach(&mut tx, WS, &other, &owner("mail.message"))
            .await,
        Err(BlobError::NotStored)
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn detaching_drops_the_reference_and_restarts_the_grace_period(db: PgPool) {
    let blobs = memory(&db);
    let blob = blobs
        .put(WS, b"attachment".as_slice(), "text/plain")
        .await
        .unwrap();
    let (a, b) = (owner("mail.message"), owner("files.file"));
    attach(&blobs, &db, WS, &blob, &a).await;
    attach(&blobs, &db, WS, &blob, &b).await;
    age_all(&db, 2 * DAY).await;

    let mut tx = db.begin().await.unwrap();
    assert!(blobs.detach(&mut tx, WS, &blob.hash, &a).await.unwrap());
    assert!(!blobs.detach(&mut tx, WS, &blob.hash, &a).await.unwrap());
    assert_eq!(blobs.detach_owner(&mut tx, WS, &b).await.unwrap(), 1);
    tx.commit().await.unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blob_refs").await, 0);

    assert_eq!(blobs.sweep().await.unwrap().blobs, 0, "detached just now");
    assert!(blobs.head(WS, &blob.hash).await.unwrap().is_some());
    age_all(&db, 2 * DAY).await;
    assert_eq!(blobs.sweep().await.unwrap().blobs, 1);
    assert!(blobs.head(WS, &blob.hash).await.unwrap().is_none());
}

// --- sweeping ------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn the_sweep_deletes_only_unreferenced_blobs_past_their_grace(db: PgPool) {
    let blobs = memory(&db);
    let held = blobs
        .put(WS, b"held".as_slice(), "text/plain")
        .await
        .unwrap();
    let orphan = blobs
        .put(WS, b"orphan".as_slice(), "text/plain")
        .await
        .unwrap();
    attach(&blobs, &db, WS, &held, &owner("mail.message")).await;
    age_all(&db, 2 * DAY).await;
    let fresh = blobs
        .put(WS, b"fresh".as_slice(), "text/plain")
        .await
        .unwrap();

    let stats = blobs.sweep().await.unwrap();
    assert_eq!(stats.blobs, 1);
    assert!(blobs.head(WS, &held.hash).await.unwrap().is_some());
    assert!(
        blobs.head(WS, &fresh.hash).await.unwrap().is_some(),
        "within grace"
    );
    assert!(blobs.head(WS, &orphan.hash).await.unwrap().is_none());
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blobs").await, 2);
    assert_eq!(blobs.sweep().await.unwrap(), SweepStats::default());
}

#[sqlx::test(migrations = "../../migrations")]
async fn putting_the_bytes_again_restarts_the_grace_period(db: PgPool) {
    let blobs = memory(&db);
    blobs
        .put(WS, b"again".as_slice(), "text/plain")
        .await
        .unwrap();
    age_all(&db, 2 * DAY).await;
    let blob = blobs
        .put(WS, b"again".as_slice(), "text/plain")
        .await
        .unwrap();
    assert_eq!(blobs.sweep().await.unwrap().blobs, 0);
    assert_eq!(read(&blobs, WS, &blob).await.unwrap(), b"again");
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_sweep_removes_abandoned_uploads(db: PgPool) {
    let blobs = memory(&db);
    blobs
        .objects()
        .put(
            &Path::from("_staging/crashed-upload"),
            Bytes::from_static(b"half").into(),
        )
        .await
        .unwrap();
    assert_eq!(
        blobs.sweep().await.unwrap().abandoned_uploads,
        0,
        "too recent"
    );
    let stats = blobs.sweep_older_than(Duration::ZERO).await.unwrap();
    assert_eq!(stats.abandoned_uploads, 1);
    assert!(objects(&blobs).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_attach_in_flight_keeps_its_blob_from_the_sweep(db: PgPool) {
    let blobs = memory(&db);
    let blob = blobs
        .put(WS, b"contested".as_slice(), "text/plain")
        .await
        .unwrap();
    age_all(&db, 2 * DAY).await;

    // The attach holds the row (uncommitted) while the sweep runs.
    let mut tx = db.begin().await.unwrap();
    blobs
        .attach(&mut tx, WS, &blob, &owner("mail.message"))
        .await
        .unwrap();
    assert_eq!(blobs.sweep().await.unwrap().blobs, 0, "skipped: locked");
    tx.commit().await.unwrap();

    assert_eq!(blobs.sweep().await.unwrap().blobs, 0, "now referenced");
    assert_eq!(read(&blobs, WS, &blob).await.unwrap(), b"contested");
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_attach_racing_a_claimed_sweep_waits_and_is_told_to_put_again(db: PgPool) {
    let blobs = memory(&db);
    let blob = blobs
        .put(WS, b"contested".as_slice(), "text/plain")
        .await
        .unwrap();
    age_all(&db, 2 * DAY).await;

    // The sweep claims the blob first…
    let mut sweep = db.begin().await.unwrap();
    let claimed = claim_unreferenced(&mut sweep, DAY).await.unwrap();
    assert_eq!(claimed.len(), 1);

    // …so the attach waits for it…
    let attaching = {
        let (blobs, db, blob) = (blobs.clone(), db.clone(), blob.clone());
        tokio::spawn(async move {
            let mut tx = db.begin().await.unwrap();
            let attached = blobs
                .attach(&mut tx, WS, &blob, &owner("mail.message"))
                .await;
            tx.commit().await.unwrap();
            attached
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!attaching.is_finished(), "attach waits on the sweep's lock");

    // …and, once the sweep has deleted it, learns it must put it again,
    // instead of holding a reference to nothing.
    assert_eq!(
        blobs.finish_sweep(&mut sweep, &claimed, DAY).await.unwrap(),
        1
    );
    sweep.commit().await.unwrap();
    assert!(matches!(
        attaching.await.unwrap(),
        Err(BlobError::NotStored)
    ));
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blob_refs").await, 0);

    let again = blobs
        .put(WS, b"contested".as_slice(), "text/plain")
        .await
        .unwrap();
    attach(&blobs, &db, WS, &again, &owner("mail.message")).await;
    assert_eq!(read(&blobs, WS, &again).await.unwrap(), b"contested");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_put_racing_a_claimed_sweep_stores_the_bytes_again(db: PgPool) {
    let blobs = memory(&db);
    blobs
        .put(WS, b"resurrected".as_slice(), "text/plain")
        .await
        .unwrap();
    age_all(&db, 2 * DAY).await;

    let mut sweep = db.begin().await.unwrap();
    let claimed = claim_unreferenced(&mut sweep, DAY).await.unwrap();
    let putting = {
        let blobs = blobs.clone();
        tokio::spawn(async move {
            blobs
                .put(WS, chunked(b"resurrected", 3), "text/plain")
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!putting.is_finished(), "put waits on the sweep's lock");
    assert_eq!(
        blobs.finish_sweep(&mut sweep, &claimed, DAY).await.unwrap(),
        1
    );
    sweep.commit().await.unwrap();

    let blob = putting.await.unwrap().unwrap();
    assert_eq!(read(&blobs, WS, &blob).await.unwrap(), b"resurrected");
    assert_eq!(count(&db, "SELECT count(*) FROM kernel.blobs").await, 1);
    attach(&blobs, &db, WS, &blob, &owner("mail.message")).await;
}

// --- workspace deletion ----------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn purging_a_workspace_deletes_its_blobs_only(db: PgPool) {
    for blobs in [memory(&db), ScratchDir::new().store(&db)] {
        sqlx::query("TRUNCATE kernel.blob_refs, kernel.blobs")
            .execute(&db)
            .await
            .unwrap();
        let mine = blobs
            .put(WS, b"mine".as_slice(), "text/plain")
            .await
            .unwrap();
        blobs
            .put(WS, chunked(b"also mine", 2), "text/plain")
            .await
            .unwrap();
        let theirs = blobs
            .put(OTHER_WS, b"mine".as_slice(), "text/plain")
            .await
            .unwrap();
        attach(&blobs, &db, WS, &mine, &owner("mail.message")).await;
        attach(&blobs, &db, OTHER_WS, &theirs, &owner("mail.message")).await;

        assert_eq!(blobs.purge_workspace(WS).await.unwrap(), 2);
        assert!(blobs.head(WS, &mine.hash).await.unwrap().is_none());
        assert_eq!(read(&blobs, OTHER_WS, &theirs).await.unwrap(), b"mine");
        let rows = "SELECT count(*) FROM kernel.blobs WHERE workspace_id = '0192f0c4-7c3a-7d4e-8a6b-111111111111'";
        assert_eq!(count(&db, rows).await, 0);
        assert_eq!(count(&db, "SELECT count(*) FROM kernel.blob_refs").await, 1);
        assert_eq!(blobs.purge_workspace(WS).await.unwrap(), 0, "idempotent");
    }
}

// --- the scheduled job -----------------------------------------------------------

#[test]
fn the_kernel_schedules_the_sweep() {
    let registry = crate::jobs::Registry::build(&[]).unwrap();
    assert!(registry.schedule_names().contains(&SweepBlobs::NAME));
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_sweep_job_runs_on_the_worker(db: PgPool) {
    let state =
        crate::testing::state(db.clone()).with_registry(crate::jobs::Registry::build(&[]).unwrap());
    let orphan = state
        .blobs
        .put(WS, b"orphan".as_slice(), "text/plain")
        .await
        .unwrap();
    age_all(&db, 2 * DAY).await;
    let worker = crate::jobs::Worker::new(state.clone());
    worker.upkeep().await;
    worker.run_until_idle().await;
    let done = "SELECT count(*) FROM kernel.jobs WHERE kind = 'kernel.sweep_blobs' AND state = 'succeeded'";
    assert_eq!(count(&db, done).await, 1);
    assert!(state.blobs.head(WS, &orphan.hash).await.unwrap().is_none());
}

// --- a real S3-compatible store ----------------------------------------------------

/// The `s3` backend end to end against a live S3-compatible store (`MinIO`,
/// Garage, …) with an existing, empty bucket. Not run in CI; run it with
/// `CAMPBOOKS_TEST_S3_ENDPOINT`, `…_BUCKET`, `…_ACCESS_KEY_ID` and
/// `…_SECRET_ACCESS_KEY` set and `cargo test -p campbooks-kernel -- --ignored s3`.
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "needs a live S3-compatible store (CAMPBOOKS_TEST_S3_*)"]
async fn s3_backend_against_a_live_store(db: PgPool) {
    let var = |name: &str| std::env::var(format!("CAMPBOOKS_TEST_S3_{name}")).ok();
    let mut config = BlobsConfig {
        backend: BlobBackend::S3,
        ..BlobsConfig::default()
    };
    config.s3.endpoint = var("ENDPOINT");
    config.s3.bucket = var("BUCKET");
    config.s3.access_key_id = var("ACCESS_KEY_ID");
    config.s3.secret_access_key = var("SECRET_ACCESS_KEY").map(Into::into);
    config.s3.path_style = true;
    let blobs = BlobStore::from_config(&config, db.clone()).unwrap();
    blobs.probe().await.unwrap();

    // Streamed past one part (multipart upload), and small (single put).
    let body: Vec<u8> = (0..17 * 1024 * 1024 + 5_u32)
        .map(|i| (i % 241) as u8)
        .collect();
    let large = blobs
        .put(WS, chunked(&body, 256 * 1024), "application/pdf")
        .await
        .unwrap();
    let small = blobs
        .put(WS, b"small".as_slice(), "text/plain")
        .await
        .unwrap();
    let again = blobs
        .put(WS, chunked(b"small", 2), "text/plain")
        .await
        .unwrap();
    assert_eq!(small, again);
    assert_eq!(read(&blobs, WS, &large).await.unwrap(), body);
    assert_eq!(read(&blobs, WS, &small).await.unwrap(), b"small");
    assert_eq!(
        objects(&blobs).await.len(),
        2,
        "deduplicated, no staging left"
    );

    // Truncated upload: refused, nothing left behind.
    let truncated = blobs
        .put_expecting(
            WS,
            chunked(&body[..9 * 1024 * 1024], 256 * 1024),
            "application/pdf",
            Expected {
                hash: Some(large.hash),
                size: None,
            },
        )
        .await;
    assert!(matches!(truncated, Err(BlobError::Mismatch("SHA-256"))));
    assert_eq!(objects(&blobs).await.len(), 2);

    // Sweep the unreferenced small blob, keep the attached large one.
    attach(&blobs, &db, WS, &large, &owner("mail.message")).await;
    age_all(&db, 2 * DAY).await;
    assert_eq!(blobs.sweep().await.unwrap().blobs, 1);
    assert!(blobs.head(WS, &small.hash).await.unwrap().is_none());

    assert_eq!(blobs.purge_workspace(WS).await.unwrap(), 1);
    assert!(objects(&blobs).await.is_empty());
}
