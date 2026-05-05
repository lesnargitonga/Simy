use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    config::{Credentials, Region},
    presigning::{PresignedRequest, PresigningConfig},
    Client as S3Client,
};
use base64ct::{Base64, Encoding};
use chrono::{DateTime, Duration, Utc};
use comm_core::{
    build_blob_padding_plan, CoreError, DeviceRecord, IdentityPublicKeys, PreKeyBundle,
};
use ed25519_dalek::{Signature, VerifyingKey};
use rand_core::{OsRng, RngCore};
use redis::aio::MultiplexedConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, FromRow, PgPool};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    net::SocketAddr,
    sync::Arc,
    time::Duration as StdDuration,
};
use thiserror::Error;
use tokio::{net::TcpListener, time};
use tracing::{error, info, warn};
use uuid::Uuid;
use x25519_dalek::PublicKey as X25519PublicKey;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
static LIVE_TEST_PAGE: &str = include_str!("../static/index.html");
const LIVE_TEST_PAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/static/index.html");

#[derive(Clone)]
struct AppState {
    db: PgPool,
    redis: redis::Client,
    s3: S3Client,
    config: Arc<RelayConfig>,
}

#[derive(Clone, Debug)]
struct RelayConfig {
    bind_addr: SocketAddr,
    admin_token: String,
    postgres_dsn: String,
    redis_url: String,
    media_object_store_endpoint: String,
    media_object_store_bucket: String,
    media_object_store_region: String,
    media_object_store_access_key_id: String,
    media_object_store_secret_access_key: String,
    min_ttl_seconds: i64,
    default_ttl_seconds: i64,
    max_ttl_seconds: i64,
    max_ciphertext_bytes: usize,
    replay_ttl_margin_seconds: i64,
    media_upload_intent_ttl_seconds: i64,
    media_access_grant_ttl_seconds: i64,
    media_default_chunk_size_bytes: u32,
    media_max_original_size_bytes: u64,
    submit_rate_limit_per_minute: u32,
    retrieve_rate_limit_per_minute: u32,
    cleanup_interval_seconds: u64,
}

impl RelayConfig {
    fn from_env() -> Result<Self, ApiError> {
        let bind_addr = env::var("RELAY_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .map_err(|_| ApiError::Config("invalid RELAY_BIND_ADDR".to_string()))?;

        let postgres_dsn = env::var("POSTGRES_DSN")
            .map_err(|_| ApiError::Config("POSTGRES_DSN is required".to_string()))?;
        let admin_token = env::var("RELAY_ADMIN_TOKEN")
            .map_err(|_| ApiError::Config("RELAY_ADMIN_TOKEN is required".to_string()))?;
        let redis_url = env::var("REDIS_URL")
            .map_err(|_| ApiError::Config("REDIS_URL is required".to_string()))?;
        let media_object_store_endpoint = env::var("MEDIA_OBJECT_STORE_ENDPOINT")
            .map_err(|_| ApiError::Config("MEDIA_OBJECT_STORE_ENDPOINT is required".to_string()))?;
        let media_object_store_bucket = env::var("MEDIA_OBJECT_STORE_BUCKET")
            .map_err(|_| ApiError::Config("MEDIA_OBJECT_STORE_BUCKET is required".to_string()))?;
        let media_object_store_region = env::var("MEDIA_OBJECT_STORE_REGION")
            .unwrap_or_else(|_| "us-east-1".to_string());
        let media_object_store_access_key_id = env::var("MEDIA_OBJECT_STORE_ACCESS_KEY_ID")
            .or_else(|_| env::var("MINIO_ROOT_USER"))
            .map_err(|_| ApiError::Config("MEDIA_OBJECT_STORE_ACCESS_KEY_ID is required".to_string()))?;
        let media_object_store_secret_access_key = env::var("MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY")
            .or_else(|_| env::var("MINIO_ROOT_PASSWORD"))
            .map_err(|_| ApiError::Config("MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY is required".to_string()))?;

        let min_ttl_seconds = parse_i64_env("RELAY_MIN_TTL_SECONDS", 300)?;
        let default_ttl_seconds = parse_i64_env("RELAY_DEFAULT_TTL_SECONDS", 60 * 60 * 24 * 7)?;
        let max_ttl_seconds = parse_i64_env("RELAY_MAX_TTL_SECONDS", 60 * 60 * 24 * 30)?;
        let max_ciphertext_bytes = parse_usize_env("RELAY_MAX_CIPHERTEXT_BYTES", 262_144)?;
        let replay_ttl_margin_seconds = parse_i64_env("RELAY_REPLAY_TTL_MARGIN_SECONDS", 300)?;
        let media_upload_intent_ttl_seconds = parse_i64_env("MEDIA_UPLOAD_INTENT_TTL_SECONDS", 900)?;
        let media_access_grant_ttl_seconds = parse_i64_env("MEDIA_ACCESS_GRANT_TTL_SECONDS", 900)?;
        let media_default_chunk_size_bytes =
            parse_u32_env("MEDIA_DEFAULT_CHUNK_SIZE_BYTES", 262_144)?;
        let media_max_original_size_bytes =
            parse_u64_env("MEDIA_MAX_ORIGINAL_SIZE_BYTES", 52_428_800)?;
        let submit_rate_limit_per_minute =
            parse_u32_env("RELAY_SUBMIT_RATE_LIMIT_PER_MINUTE", 120)?;
        let retrieve_rate_limit_per_minute =
            parse_u32_env("RELAY_RETRIEVE_RATE_LIMIT_PER_MINUTE", 240)?;
        let cleanup_interval_seconds = parse_u64_env("RELAY_CLEANUP_INTERVAL_SECONDS", 60)?;

        if min_ttl_seconds <= 0
            || default_ttl_seconds < min_ttl_seconds
            || max_ttl_seconds < default_ttl_seconds
        {
            return Err(ApiError::Config(
                "relay TTL configuration is inconsistent".to_string(),
            ));
        }

        if max_ciphertext_bytes == 0 {
            return Err(ApiError::Config(
                "RELAY_MAX_CIPHERTEXT_BYTES must be greater than zero".to_string(),
            ));
        }

        if media_upload_intent_ttl_seconds <= 0
            || media_access_grant_ttl_seconds <= 0
            || media_default_chunk_size_bytes == 0
            || media_max_original_size_bytes == 0
        {
            return Err(ApiError::Config(
                "media upload configuration is inconsistent".to_string(),
            ));
        }

        Ok(Self {
            bind_addr,
            admin_token,
            postgres_dsn,
            redis_url,
            media_object_store_endpoint,
            media_object_store_bucket,
            media_object_store_region,
            media_object_store_access_key_id,
            media_object_store_secret_access_key,
            min_ttl_seconds,
            default_ttl_seconds,
            max_ttl_seconds,
            max_ciphertext_bytes,
            replay_ttl_margin_seconds,
            media_upload_intent_ttl_seconds,
            media_access_grant_ttl_seconds,
            media_default_chunk_size_bytes,
            media_max_original_size_bytes,
            submit_rate_limit_per_minute,
            retrieve_rate_limit_per_minute,
            cleanup_interval_seconds,
        })
    }
}

fn parse_i64_env(key: &str, default: i64) -> Result<i64, ApiError> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| ApiError::Config(format!("invalid {key}")))
        })
        .transpose()?
        .map_or(Ok(default), Ok)
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, ApiError> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| ApiError::Config(format!("invalid {key}")))
        })
        .transpose()?
        .map_or(Ok(default), Ok)
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, ApiError> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| ApiError::Config(format!("invalid {key}")))
        })
        .transpose()?
        .map_or(Ok(default), Ok)
}

fn parse_usize_env(key: &str, default: usize) -> Result<usize, ApiError> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| ApiError::Config(format!("invalid {key}")))
        })
        .transpose()?
        .map_or(Ok(default), Ok)
}

#[derive(Debug, Deserialize)]
struct CreateMailboxRequest {
    mailbox_id: String,
    mailbox_token_b64: String,
    codename: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateMailboxResponse {
    mailbox_id: String,
    codename: String,
    role: String,
    status: String,
    created_at: DateTime<Utc>,
    activated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct ExistingMailboxRow {
    access_token_hash: Vec<u8>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct AdminBootstrapAccountRequest {
    mailbox_id: String,
    mailbox_token_b64: Option<String>,
    codename: Option<String>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct AccountProfileResponse {
    mailbox_id: String,
    codename: String,
    role: String,
    status: String,
    owner_mailbox_id: Option<String>,
    created_at: DateTime<Utc>,
    activated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct UpsertContactRequest {
    contact_mailbox_id: String,
    codename: Option<String>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct ContactSummaryResponse {
    mailbox_id: String,
    contact_mailbox_id: String,
    codename: String,
    contact_role: String,
    contact_status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListContactsResponse {
    mailbox_id: String,
    contact_count: usize,
    contacts: Vec<ContactSummaryResponse>,
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct DeleteContactResponse {
    mailbox_id: String,
    contact_mailbox_id: String,
    deleted: bool,
}

#[derive(Debug, Deserialize)]
struct CreateFeedPostRequest {
    post_id: Option<Uuid>,
    audience: Option<String>,
    reply_policy: Option<String>,
    author_ciphertext_b64: String,
    deliveries: Vec<CreateFeedDeliveryRequest>,
    ttl_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CreateFeedDeliveryRequest {
    recipient_mailbox_id: String,
    ciphertext_b64: String,
}

#[derive(Debug, Serialize)]
struct CreateFeedPostResponse {
    post_id: Uuid,
    mailbox_id: String,
    audience: String,
    reply_policy: String,
    recipient_count: usize,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ListFeedPostsQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CreateFeedReplyRequest {
    reply_id: Option<Uuid>,
    recipient_mailbox_id: Option<String>,
    author_ciphertext_b64: String,
    recipient_ciphertext_b64: String,
    ttl_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateFeedReplyResponse {
    reply_id: Uuid,
    post_id: Uuid,
    mailbox_id: String,
    recipient_mailbox_id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct FeedReplyVisibleResponse {
    reply_id: Uuid,
    post_id: Uuid,
    author_mailbox_id: String,
    author_codename: String,
    recipient_mailbox_id: String,
    ciphertext_b64: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    visibility: String,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct FeedReplyTargetResponse {
    mailbox_id: String,
    codename: String,
}

#[derive(Debug, FromRow, Clone)]
struct FeedPostVisibleRow {
    post_id: Uuid,
    author_mailbox_id: String,
    author_codename: String,
    audience: String,
    reply_policy: String,
    ciphertext_b64: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    visibility: String,
}

#[derive(Debug, Serialize, Clone)]
struct FeedPostVisibleResponse {
    post_id: Uuid,
    author_mailbox_id: String,
    author_codename: String,
    audience: String,
    reply_policy: String,
    ciphertext_b64: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    visibility: String,
    can_reply: bool,
    reply_targets: Vec<FeedReplyTargetResponse>,
    replies: Vec<FeedReplyVisibleResponse>,
}

#[derive(Debug, Serialize)]
struct ListFeedPostsResponse {
    mailbox_id: String,
    post_count: usize,
    posts: Vec<FeedPostVisibleResponse>,
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct PublishPreKeyBundleRequest {
    identity_signing_key_b64: String,
    identity_exchange_key_b64: String,
    signed_prekey_b64: String,
    signed_prekey_signature_b64: String,
    signed_prekey_expires_at: Option<DateTime<Utc>>,
    one_time_prekeys_b64: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PublishPreKeyBundleResponse {
    mailbox_id: String,
    signed_prekey_expires_at: DateTime<Utc>,
    one_time_prekey_count: usize,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct FetchPreKeyBundleResponse {
    mailbox_id: String,
    identity_signing_key_b64: String,
    identity_exchange_key_b64: String,
    signed_prekey_b64: String,
    signed_prekey_signature_b64: String,
    signed_prekey_expires_at: DateTime<Utc>,
    one_time_prekey_b64: Option<String>,
    fetched_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PreKeyInventoryStatusResponse {
    mailbox_id: String,
    signed_prekey_expires_at: DateTime<Utc>,
    available_one_time_prekeys: i64,
    checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PublishDeviceRecordResponse {
    mailbox_id: String,
    device_id: String,
    revoked_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListDeviceRecordsResponse {
    mailbox_id: String,
    devices: Vec<DeviceRecord>,
}

#[derive(Debug, Deserialize)]
struct CreateMediaUploadIntentRequest {
    media_type: String,
    original_size_bytes: u64,
    chunk_size_bytes: Option<u32>,
    ttl_seconds: Option<i64>,
    content_sha256_b64: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateMediaUploadIntentResponse {
    intent_id: Uuid,
    mailbox_id: String,
    bucket: String,
    object_key: String,
    media_type: String,
    original_size_bytes: u64,
    padded_size_bytes: u64,
    chunk_size_bytes: u32,
    region: String,
    upload_endpoint: String,
    presigned_upload: PresignedRequestResponse,
    content_sha256_b64: Option<String>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PresignedRequestResponse {
    method: String,
    url: String,
    headers: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct CreateMediaManifestRequest {
    intent_id: Uuid,
    content_sha256_b64: Option<String>,
    upload_completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct StoredMediaManifest {
    object_id: Uuid,
    intent_id: Option<Uuid>,
    bucket: String,
    object_key: String,
    media_type: String,
    original_size_bytes: i64,
    padded_size_bytes: i64,
    chunk_size_bytes: i32,
    content_sha256_b64: Option<String>,
    created_at: DateTime<Utc>,
    upload_completed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct CreateMediaManifestResponse {
    object_id: Uuid,
    mailbox_id: String,
    bucket: String,
    object_key: String,
    media_type: String,
    original_size_bytes: i64,
    padded_size_bytes: i64,
    chunk_size_bytes: i32,
    content_sha256_b64: Option<String>,
    upload_completed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListMediaManifestsResponse {
    mailbox_id: String,
    media: Vec<StoredMediaManifest>,
}

#[derive(Debug, Deserialize)]
struct CreateMediaAccessGrantRequest {
    object_key: String,
    operation: String,
    ttl_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateMediaAccessGrantResponse {
    grant_id: Uuid,
    grant_token: String,
    operation: String,
    bucket: String,
    object_key: String,
    media_type: String,
    upload_endpoint: String,
    resolve_path: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ResolveMediaAccessGrantResponse {
    grant_id: Uuid,
    operation: String,
    bucket: String,
    object_key: String,
    media_type: String,
    original_size_bytes: i64,
    padded_size_bytes: i64,
    chunk_size_bytes: i32,
    content_sha256_b64: Option<String>,
    object_store_endpoint: String,
    presigned_request: PresignedRequestResponse,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ListMediaQuery {
    limit: Option<usize>,
}

#[derive(Debug, FromRow)]
struct StoredUploadIntent {
    intent_id: Uuid,
    bucket: String,
    object_key: String,
    media_type: String,
    original_size_bytes: i64,
    padded_size_bytes: i64,
    chunk_size_bytes: i32,
    content_sha256_b64: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StoredPreKeyBundleRow {
    mailbox_id: String,
    identity_signing_key: Vec<u8>,
    identity_exchange_key: Vec<u8>,
    signed_prekey: Vec<u8>,
    signed_prekey_signature: Vec<u8>,
    signed_prekey_expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StoredPreKeyInventoryStatus {
    signed_prekey_expires_at: DateTime<Utc>,
    available_one_time_prekeys: i64,
}

#[derive(Debug, FromRow)]
struct StoredDeviceRecordRow {
    device_id: String,
    device_label: String,
    device_signing_key: Vec<u8>,
    device_exchange_key: Vec<u8>,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    signature: Vec<u8>,
}

#[derive(Debug)]
struct ParsedPreKeyBundleUpload {
    identity_signing_key: [u8; 32],
    identity_exchange_key: [u8; 32],
    signed_prekey: [u8; 32],
    signed_prekey_signature: [u8; 64],
    one_time_prekeys: Vec<[u8; 32]>,
    signed_prekey_expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ResolvedGrantRow {
    grant_id: Uuid,
    operation: String,
    expires_at: DateTime<Utc>,
    bucket: String,
    object_key: String,
    media_type: String,
    original_size_bytes: i64,
    padded_size_bytes: i64,
    chunk_size_bytes: i32,
    content_sha256_b64: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubmitMessageRequest {
    ciphertext_b64: String,
    sender_device_hint: Option<String>,
    ttl_seconds: Option<i64>,
    message_id: Option<Uuid>,
    replay_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct StoredEnvelope {
    message_id: Uuid,
    sender_device_hint: Option<String>,
    ciphertext_b64: String,
    received_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SubmitMessageResponse {
    message_id: Uuid,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ListMessagesQuery {
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ListMessagesResponse {
    mailbox_id: String,
    messages: Vec<StoredEnvelope>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    server_time: DateTime<Utc>,
    postgres: &'static str,
    redis: &'static str,
}

#[derive(Debug, Serialize)]
struct DeleteResponse {
    mailbox_id: String,
    message_id: Uuid,
    deleted: bool,
}

#[derive(Debug, Serialize, FromRow)]
struct AdminMailboxSummary {
    mailbox_id: String,
    codename: String,
    role: String,
    status: String,
    owner_mailbox_id: Option<String>,
    created_at: DateTime<Utc>,
    message_count: i64,
    media_count: i64,
    device_count: i64,
    has_prekey_bundle: bool,
}

#[derive(Debug, Serialize)]
struct AdminOverviewResponse {
    mailbox_count: usize,
    mailboxes: Vec<AdminMailboxSummary>,
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminProvisionMailboxResponse {
    mailbox_id: String,
    mailbox_token_b64: String,
    suggested_codename: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct AdminSetManagedUserStatusRequest {
    status: String,
}

#[derive(Debug, Serialize)]
struct AdminManagedUserAccessResponse {
    mailbox_id: String,
    mailbox_token_b64: String,
    codename: String,
    status: String,
    owner_mailbox_id: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminDeleteManagedUserResponse {
    mailbox_id: String,
    deleted: bool,
}

#[derive(Debug, Error)]
enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("too many requests: {0}")]
    TooManyRequests(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("dependency error: {0}")]
    Dependency(String),
    #[error("internal error")]
    Internal,
}

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        ApiError::BadRequest(value.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Config(_) | ApiError::Dependency(_) | ApiError::Internal => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let body = Json(serde_json::json!({ "error": self.to_string() }));
        (status, body).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "relay=info,sqlx=warn".into()),
        )
        .init();

    let config = Arc::new(RelayConfig::from_env()?);
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.postgres_dsn)
        .await
        .map_err(|error| ApiError::Dependency(format!("postgres connection failed: {error}")))?;
    MIGRATOR
        .run(&db)
        .await
        .map_err(|error| ApiError::Dependency(format!("database migration failed: {error}")))?;

    let redis = redis::Client::open(config.redis_url.clone())
        .map_err(|error| ApiError::Config(format!("invalid REDIS_URL: {error}")))?;
    ping_redis(&redis).await?;
    let s3 = build_s3_client(&config).await;

    let state = AppState {
        db: db.clone(),
        redis: redis.clone(),
        s3,
        config: config.clone(),
    };

    tokio::spawn(cleanup_expired_messages(state.clone()));

    let app = Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/healthz", get(healthz))
        .route("/v1/admin/bootstrap-account", post(admin_bootstrap_account))
        .route("/v1/admin/overview", get(admin_overview))
        .route("/v1/admin/provision-mailbox", post(admin_provision_mailbox))
        .route(
            "/v1/admin/users/{mailbox_id}/status",
            post(admin_set_managed_user_status),
        )
        .route(
            "/v1/admin/users/{mailbox_id}/reset-access",
            post(admin_reset_managed_user_access),
        )
        .route(
            "/v1/admin/users/{mailbox_id}",
            delete(admin_delete_managed_user),
        )
        .route("/v1/mailboxes", post(create_mailbox))
        .route("/v1/mailboxes/{mailbox_id}/account", get(get_account_profile))
        .route(
            "/v1/mailboxes/{mailbox_id}/contacts",
            post(upsert_contact).get(list_contacts),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/contacts/{contact_mailbox_id}",
            delete(delete_contact),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/feed/posts",
            post(create_feed_post).get(list_feed_posts),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/feed/posts/{post_id}/replies",
            post(create_feed_reply),
        )
        .route("/v1/prekeys/{mailbox_id}", get(fetch_prekey_bundle))
        .route("/v1/devices/{mailbox_id}", get(list_device_records))
        .route(
            "/v1/media/access-grants/{grant_token}",
            get(resolve_media_access_grant),
        )
        .route(
            "/v1/media/access-grants/{grant_token}/content",
            get(fetch_media_access_grant_content),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/prekeys",
            post(publish_prekey_bundle),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/prekeys/status",
            get(get_prekey_inventory_status),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/devices",
            post(publish_device_record),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/media/upload-intents",
            post(create_media_upload_intent),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/media/manifests",
            post(create_media_manifest).get(list_media_manifests),
        )
        .route(
            "/v1/mailboxes/{mailbox_id}/media/access-grants",
            post(create_media_access_grant),
        )
        .route("/v1/mailboxes/{mailbox_id}/messages", post(submit_message).get(list_messages))
        .route(
            "/v1/mailboxes/{mailbox_id}/messages/{message_id}",
            delete(delete_message),
        )
        .with_state(state);

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|_| ApiError::Internal)?;

    info!(addr = %config.bind_addr, "relay listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| ApiError::Internal)
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        warn!("failed to register Ctrl+C handler");
    }
}

async fn index() -> Html<String> {
    match tokio::fs::read_to_string(LIVE_TEST_PAGE_PATH).await {
        Ok(page) => Html(page),
        Err(error) => {
            warn!(path = LIVE_TEST_PAGE_PATH, ?error, "failed to read live test page from disk; serving embedded fallback");
            Html(LIVE_TEST_PAGE.to_string())
        }
    }
}

async fn favicon() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn cleanup_expired_messages(state: AppState) {
    let mut ticker = time::interval(StdDuration::from_secs(state.config.cleanup_interval_seconds));

    loop {
        ticker.tick().await;

        if let Err(error) = sqlx::query(
            "DELETE FROM relay_messages WHERE expires_at <= NOW() OR deleted_at IS NOT NULL",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup expired messages");
        }

        if let Err(error) = sqlx::query(
            "DELETE FROM relay_feed_posts WHERE expires_at <= NOW()",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup expired feed posts");
        }

        if let Err(error) = sqlx::query(
            "DELETE FROM relay_feed_post_replies WHERE expires_at <= NOW()",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup expired feed replies");
        }

        if let Err(error) = sqlx::query(
            "DELETE FROM media_upload_intents WHERE expires_at <= NOW()",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup expired media upload intents");
        }

        if let Err(error) = sqlx::query(
            "DELETE FROM media_access_grants WHERE expires_at <= NOW()",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup expired media access grants");
        }

        if let Err(error) = sqlx::query(
            "DELETE FROM relay_one_time_prekeys WHERE consumed_at IS NOT NULL OR NOT EXISTS (SELECT 1 FROM relay_prekey_bundles bundles WHERE bundles.mailbox_id = relay_one_time_prekeys.mailbox_id AND bundles.signed_prekey_expires_at > NOW())",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup consumed one-time prekeys");
        }

        if let Err(error) = sqlx::query(
            "DELETE FROM relay_prekey_bundles WHERE signed_prekey_expires_at <= NOW()",
        )
        .execute(&state.db)
        .await
        {
            error!(%error, "failed to cleanup expired prekey bundles");
        }
    }
}

async fn healthz(State(state): State<AppState>) -> Result<(StatusCode, Json<HealthResponse>), ApiError> {
    let postgres_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();
    let redis_ok = ping_redis(&state.redis).await.is_ok();

    let status = if postgres_ok && redis_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    Ok((
        status,
        Json(HealthResponse {
            status: if postgres_ok && redis_ok { "ok" } else { "degraded" },
            server_time: Utc::now(),
            postgres: if postgres_ok { "ok" } else { "error" },
            redis: if redis_ok { "ok" } else { "error" },
        }),
    ))
}

async fn admin_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminOverviewResponse>, ApiError> {
    let admin_mailbox_id = authenticate_admin(&state.db, &state.config, &headers).await?;

    let mailboxes = sqlx::query_as::<_, AdminMailboxSummary>(
        "SELECT m.mailbox_id, a.codename, a.role, a.status, a.owner_mailbox_id, m.created_at, COUNT(DISTINCT msg.message_id)::BIGINT AS message_count, COUNT(DISTINCT media.object_id)::BIGINT AS media_count, COUNT(DISTINCT dev.device_id)::BIGINT AS device_count, EXISTS(SELECT 1 FROM relay_prekey_bundles bundles WHERE bundles.mailbox_id = m.mailbox_id AND bundles.signed_prekey_expires_at > NOW()) AS has_prekey_bundle FROM relay_accounts a JOIN relay_mailboxes m ON m.mailbox_id = a.mailbox_id LEFT JOIN relay_messages msg ON msg.mailbox_id = m.mailbox_id AND msg.deleted_at IS NULL AND msg.expires_at > NOW() LEFT JOIN media_objects media ON media.mailbox_id = m.mailbox_id LEFT JOIN relay_device_records dev ON dev.mailbox_id = m.mailbox_id AND dev.revoked_at IS NULL WHERE a.mailbox_id = $1 OR a.owner_mailbox_id = $1 GROUP BY m.mailbox_id, a.codename, a.role, a.status, a.owner_mailbox_id, m.created_at ORDER BY m.created_at DESC"
    )
    .bind(&admin_mailbox_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("admin overview query failed: {error}")))?;

    Ok(Json(AdminOverviewResponse {
        mailbox_count: mailboxes.len(),
        mailboxes,
        generated_at: Utc::now(),
    }))
}

async fn admin_provision_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<AdminProvisionMailboxResponse>), ApiError> {
    let admin_mailbox_id = authenticate_admin(&state.db, &state.config, &headers).await?;

    let mailbox_id = Uuid::new_v4().to_string();
    let mailbox_token = random_token_bytes(32);
    let mailbox_token_b64 = Base64::encode_string(&mailbox_token);
    let access_token_hash = hash_mailbox_token(&mailbox_id, &mailbox_token);
    let created_at = Utc::now();
    let suggested_codename = suggested_codename(&mailbox_id);

    sqlx::query(
        "INSERT INTO relay_mailboxes (mailbox_id, access_token_hash, created_at) VALUES ($1, $2, $3)",
    )
    .bind(&mailbox_id)
    .bind(access_token_hash)
    .bind(created_at)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("admin mailbox provisioning failed: {error}")))?;

    sqlx::query(
        "INSERT INTO relay_accounts (mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at) VALUES ($1, $2, 'user', 'provisioned', $3, $4, NULL)"
    )
    .bind(&mailbox_id)
    .bind(&suggested_codename)
    .bind(&admin_mailbox_id)
    .bind(created_at)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("admin account provisioning failed: {error}")))?;

    Ok((
        StatusCode::CREATED,
        Json(AdminProvisionMailboxResponse {
            mailbox_id,
            mailbox_token_b64,
            suggested_codename,
            created_at,
        }),
    ))
}

async fn create_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateMailboxRequest>,
) -> Result<(StatusCode, Json<CreateMailboxResponse>), ApiError> {
    validate_mailbox_id(&request.mailbox_id)?;
    let token = decode_mailbox_token(&request.mailbox_token_b64)?;
    let access_token_hash = hash_mailbox_token(&request.mailbox_id, &token);
    let created_at = Utc::now();
    let activated_at = Some(Utc::now());
    let codename = normalize_codename(request.codename.as_deref(), &suggested_codename(&request.mailbox_id))?;

    let existing = sqlx::query_as::<_, ExistingMailboxRow>(
        "SELECT access_token_hash, created_at FROM relay_mailboxes WHERE mailbox_id = $1",
    )
    .bind(&request.mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("mailbox lookup failed: {error}")))?;

    if let Some(existing_mailbox) = existing {
        if existing_mailbox.access_token_hash == access_token_hash {
            let profile = upsert_account_profile(
                &state.db,
                &request.mailbox_id,
                &codename,
                existing_mailbox.created_at,
                activated_at,
            )
            .await?;

            return Ok((
                StatusCode::OK,
                Json(CreateMailboxResponse {
                    mailbox_id: request.mailbox_id,
                    codename: profile.codename,
                    role: profile.role,
                    status: profile.status,
                    created_at: profile.created_at,
                    activated_at: profile.activated_at,
                }),
            ));
        }

        return Err(ApiError::Unauthorized(
            "mailbox_id is already provisioned and the token does not match".to_string(),
        ));
    }

    authorize_mailbox_creation(&state.db, &state.config, &headers).await?;

    let result = sqlx::query(
        "INSERT INTO relay_mailboxes (mailbox_id, access_token_hash, created_at) VALUES ($1, $2, $3)",
    )
    .bind(&request.mailbox_id)
    .bind(access_token_hash)
    .bind(created_at)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let profile = upsert_account_profile(
                &state.db,
                &request.mailbox_id,
                &codename,
                created_at,
                activated_at,
            )
            .await?;

            Ok((
                StatusCode::CREATED,
                Json(CreateMailboxResponse {
                    mailbox_id: request.mailbox_id,
                    codename: profile.codename,
                    role: profile.role,
                    status: profile.status,
                    created_at: profile.created_at,
                    activated_at: profile.activated_at,
                }),
            ))
        }
        Err(error) if is_unique_violation(&error) => Err(ApiError::Conflict(
            "mailbox_id already exists".to_string(),
        )),
        Err(error) => Err(ApiError::Dependency(format!(
            "mailbox creation failed: {error}"
        ))),
    }
}

async fn admin_bootstrap_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdminBootstrapAccountRequest>,
) -> Result<(StatusCode, Json<AccountProfileResponse>), ApiError> {
    validate_mailbox_id(&request.mailbox_id)?;

    let existing_mailbox = sqlx::query_as::<_, ExistingMailboxRow>(
        "SELECT access_token_hash, created_at FROM relay_mailboxes WHERE mailbox_id = $1",
    )
    .bind(&request.mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("mailbox lookup failed: {error}")))?;

    let created_at = if let Some(existing_mailbox) = existing_mailbox {
        authenticate_admin_bootstrap(&state.db, &state.config, &headers, &request.mailbox_id).await?;
        existing_mailbox.created_at
    } else {
        ensure_admin_token(&state.config, &headers)?;
        let mailbox_token_b64 = request
            .mailbox_token_b64
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("mailbox_token_b64 is required when bootstrapping a new admin mailbox".to_string()))?;
        let mailbox_token = decode_mailbox_token(mailbox_token_b64)?;
        let access_token_hash = hash_mailbox_token(&request.mailbox_id, &mailbox_token);
        let created_at = Utc::now();

        sqlx::query(
            "INSERT INTO relay_mailboxes (mailbox_id, access_token_hash, created_at) VALUES ($1, $2, $3)",
        )
        .bind(&request.mailbox_id)
        .bind(access_token_hash)
        .bind(created_at)
        .execute(&state.db)
        .await
        .map_err(|error| ApiError::Dependency(format!("admin mailbox bootstrap failed: {error}")))?;

        created_at
    };

    if let Some(existing_profile) = lookup_account_profile(&state.db, &request.mailbox_id).await? {
        if existing_profile.role == "user" && existing_profile.owner_mailbox_id.is_some() {
            return Err(ApiError::Conflict(
                "managed user mailboxes cannot be promoted into admin accounts".to_string(),
            ));
        }
    }

    let codename = normalize_codename(request.codename.as_deref(), &suggested_codename(&request.mailbox_id))?;
    let activated_at = Some(Utc::now());

    let profile = sqlx::query_as::<_, AccountProfileResponse>(
        "INSERT INTO relay_accounts (mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at) VALUES ($1, $2, 'admin', 'active', NULL, $3, $4) ON CONFLICT (mailbox_id) DO UPDATE SET codename = EXCLUDED.codename, role = 'admin', status = 'active', owner_mailbox_id = NULL, activated_at = EXCLUDED.activated_at RETURNING mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at"
    )
    .bind(&request.mailbox_id)
    .bind(&codename)
    .bind(created_at)
    .bind(activated_at)
    .fetch_one(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("admin bootstrap failed: {error}")))?;

    Ok((StatusCode::OK, Json(profile)))
}

async fn get_account_profile(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AccountProfileResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    let profile = ensure_account_profile(&state.db, &mailbox_id).await?;
    Ok(Json(profile))
}

async fn upsert_contact(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<UpsertContactRequest>,
) -> Result<(StatusCode, Json<ContactSummaryResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    validate_mailbox_id(&request.contact_mailbox_id)?;

    if mailbox_id == request.contact_mailbox_id {
        return Err(ApiError::BadRequest(
            "cannot add the current mailbox as its own contact".to_string(),
        ));
    }

    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    ensure_mailbox_exists(&state.db, &request.contact_mailbox_id).await?;

    let contact_profile = ensure_account_profile(&state.db, &request.contact_mailbox_id).await?;
    let codename = normalize_codename(
        request.codename.as_deref(),
        &contact_profile.codename,
    )?;
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO relay_contacts (owner_mailbox_id, contact_mailbox_id, codename, created_at, updated_at) VALUES ($1, $2, $3, $4, $4) ON CONFLICT (owner_mailbox_id, contact_mailbox_id) DO UPDATE SET codename = EXCLUDED.codename, updated_at = EXCLUDED.updated_at",
    )
    .bind(&mailbox_id)
    .bind(&request.contact_mailbox_id)
    .bind(&codename)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("contact upsert failed: {error}")))?;

    let contact = fetch_contact_summary(&state.db, &mailbox_id, &request.contact_mailbox_id).await?;
    Ok((StatusCode::OK, Json(contact)))
}

async fn list_contacts(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ListContactsResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;

    let contacts = sqlx::query_as::<_, ContactSummaryResponse>(
        "SELECT c.owner_mailbox_id AS mailbox_id, c.contact_mailbox_id, c.codename, a.role AS contact_role, a.status AS contact_status, c.created_at, c.updated_at FROM relay_contacts c JOIN relay_accounts a ON a.mailbox_id = c.contact_mailbox_id WHERE c.owner_mailbox_id = $1 ORDER BY c.updated_at DESC, c.created_at DESC",
    )
    .bind(&mailbox_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("contact listing failed: {error}")))?;

    Ok(Json(ListContactsResponse {
        mailbox_id,
        contact_count: contacts.len(),
        contacts,
        generated_at: Utc::now(),
    }))
}

async fn delete_contact(
    State(state): State<AppState>,
    Path((mailbox_id, contact_mailbox_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<DeleteContactResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    validate_mailbox_id(&contact_mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;

    let deleted = sqlx::query(
        "DELETE FROM relay_contacts WHERE owner_mailbox_id = $1 AND contact_mailbox_id = $2",
    )
    .bind(&mailbox_id)
    .bind(&contact_mailbox_id)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("contact deletion failed: {error}")))?
    .rows_affected()
        > 0;

    if !deleted {
        return Err(ApiError::NotFound(
            "contact not found for this mailbox".to_string(),
        ));
    }

    Ok(Json(DeleteContactResponse {
        mailbox_id,
        contact_mailbox_id,
        deleted: true,
    }))
}

async fn create_feed_post(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateFeedPostRequest>,
) -> Result<(StatusCode, Json<CreateFeedPostResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;

    let audience = match request.audience.as_deref().unwrap_or("contacts") {
        "contacts" => "contacts",
        _ => return Err(ApiError::BadRequest("audience must be contacts".to_string())),
    };
    let reply_policy = match request.reply_policy.as_deref().unwrap_or("contacts_only") {
        "contacts_only" => "contacts_only",
        "no_replies" => "no_replies",
        _ => {
            return Err(ApiError::BadRequest(
                "reply_policy must be no_replies or contacts_only".to_string(),
            ))
        }
    };

    if request.deliveries.is_empty() {
        return Err(ApiError::BadRequest(
            "deliveries must include at least one recipient".to_string(),
        ));
    }

    let ttl_seconds = request
        .ttl_seconds
        .unwrap_or(state.config.default_ttl_seconds);
    if ttl_seconds < state.config.min_ttl_seconds || ttl_seconds > state.config.max_ttl_seconds {
        return Err(ApiError::BadRequest(format!(
            "ttl_seconds must be between {} and {}",
            state.config.min_ttl_seconds, state.config.max_ttl_seconds
        )));
    }

    validate_ciphertext(&request.author_ciphertext_b64, state.config.max_ciphertext_bytes)?;

    let mut seen_recipients = BTreeSet::new();
    let mut recipient_ids = Vec::with_capacity(request.deliveries.len());
    for delivery in &request.deliveries {
        validate_mailbox_id(&delivery.recipient_mailbox_id)?;
        validate_ciphertext(&delivery.ciphertext_b64, state.config.max_ciphertext_bytes)?;
        if delivery.recipient_mailbox_id == mailbox_id {
            return Err(ApiError::BadRequest(
                "author mailbox must not appear in deliveries".to_string(),
            ));
        }
        if !seen_recipients.insert(delivery.recipient_mailbox_id.clone()) {
            return Err(ApiError::BadRequest(
                "deliveries contain a duplicate recipient".to_string(),
            ));
        }
        recipient_ids.push(delivery.recipient_mailbox_id.clone());
    }

    let allowed_recipients = sqlx::query_scalar::<_, String>(
        "SELECT c.contact_mailbox_id FROM relay_contacts c JOIN relay_accounts a ON a.mailbox_id = c.contact_mailbox_id WHERE c.owner_mailbox_id = $1 AND c.contact_mailbox_id = ANY($2) AND a.status = 'active'",
    )
    .bind(&mailbox_id)
    .bind(&recipient_ids)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("feed recipient lookup failed: {error}")))?;

    let allowed_set: BTreeSet<String> = allowed_recipients.into_iter().collect();
    if allowed_set.len() != seen_recipients.len() || !seen_recipients.is_subset(&allowed_set) {
        return Err(ApiError::BadRequest(
            "all feed recipients must be active relay-backed contacts for this mailbox".to_string(),
        ));
    }

    let author_profile = ensure_account_profile(&state.db, &mailbox_id).await?;
    let post_id = request.post_id.unwrap_or_else(Uuid::new_v4);
    let created_at = Utc::now();
    let expires_at = created_at + Duration::seconds(ttl_seconds);

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|error| ApiError::Dependency(format!("feed transaction start failed: {error}")))?;

    sqlx::query(
        "INSERT INTO relay_feed_posts (post_id, author_mailbox_id, author_codename, audience, reply_policy, author_ciphertext_b64, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(post_id)
    .bind(&mailbox_id)
    .bind(&author_profile.codename)
    .bind(audience)
    .bind(reply_policy)
    .bind(&request.author_ciphertext_b64)
    .bind(created_at)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::Conflict("post_id already exists".to_string())
        } else {
            ApiError::Dependency(format!("feed post creation failed: {error}"))
        }
    })?;

    for delivery in &request.deliveries {
        sqlx::query(
            "INSERT INTO relay_feed_post_deliveries (delivery_id, post_id, recipient_mailbox_id, ciphertext_b64, created_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(post_id)
        .bind(&delivery.recipient_mailbox_id)
        .bind(&delivery.ciphertext_b64)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| ApiError::Dependency(format!("feed delivery creation failed: {error}")))?;
    }

    tx.commit()
        .await
        .map_err(|error| ApiError::Dependency(format!("feed transaction commit failed: {error}")))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateFeedPostResponse {
            post_id,
            mailbox_id,
            audience: audience.to_string(),
            reply_policy: reply_policy.to_string(),
            recipient_count: request.deliveries.len(),
            created_at,
            expires_at,
        }),
    ))
}

async fn list_feed_posts(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    Query(query): Query<ListFeedPostsQuery>,
    headers: HeaderMap,
) -> Result<Json<ListFeedPostsResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    let limit = query.limit.unwrap_or(50).min(200) as i64;

    let posts = sqlx::query_as::<_, FeedPostVisibleRow>(
        "SELECT * FROM (SELECT p.post_id, p.author_mailbox_id, p.author_codename, p.audience, p.reply_policy, p.author_ciphertext_b64 AS ciphertext_b64, p.created_at, p.expires_at, 'authored' AS visibility FROM relay_feed_posts p WHERE p.author_mailbox_id = $1 AND p.expires_at > NOW() UNION ALL SELECT p.post_id, p.author_mailbox_id, p.author_codename, p.audience, p.reply_policy, d.ciphertext_b64, p.created_at, p.expires_at, 'received' AS visibility FROM relay_feed_posts p JOIN relay_feed_post_deliveries d ON d.post_id = p.post_id WHERE d.recipient_mailbox_id = $1 AND p.expires_at > NOW()) visible_posts ORDER BY created_at DESC LIMIT $2",
    )
    .bind(&mailbox_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("feed listing failed: {error}")))?;

    let post_ids: Vec<Uuid> = posts.iter().map(|post| post.post_id).collect();
    let reply_rows = if post_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, FeedReplyVisibleResponse>(
            "SELECT r.reply_id, r.post_id, r.author_mailbox_id, r.author_codename, r.recipient_mailbox_id, CASE WHEN r.author_mailbox_id = $1 THEN r.author_ciphertext_b64 ELSE r.recipient_ciphertext_b64 END AS ciphertext_b64, r.created_at, r.expires_at, CASE WHEN r.author_mailbox_id = $1 THEN 'authored' ELSE 'received' END AS visibility FROM relay_feed_post_replies r WHERE r.post_id = ANY($2) AND r.expires_at > NOW() AND ($1 = r.author_mailbox_id OR $1 = r.recipient_mailbox_id) ORDER BY r.created_at ASC",
        )
        .bind(&mailbox_id)
        .bind(&post_ids)
        .fetch_all(&state.db)
        .await
        .map_err(|error| ApiError::Dependency(format!("feed reply listing failed: {error}")))?
    };

    let mut replies_by_post: BTreeMap<Uuid, Vec<FeedReplyVisibleResponse>> = BTreeMap::new();
    for reply in reply_rows {
        replies_by_post.entry(reply.post_id).or_default().push(reply);
    }

    let mut visible_posts = Vec::with_capacity(posts.len());
    for post in posts {
        let reply_targets = resolve_feed_reply_targets(&state.db, &mailbox_id, &post).await?;
        visible_posts.push(FeedPostVisibleResponse {
            post_id: post.post_id,
            author_mailbox_id: post.author_mailbox_id,
            author_codename: post.author_codename,
            audience: post.audience,
            reply_policy: post.reply_policy.clone(),
            ciphertext_b64: post.ciphertext_b64,
            created_at: post.created_at,
            expires_at: post.expires_at,
            visibility: post.visibility,
            can_reply: post.reply_policy != "no_replies",
            reply_targets,
            replies: replies_by_post.remove(&post.post_id).unwrap_or_default(),
        });
    }

    Ok(Json(ListFeedPostsResponse {
        mailbox_id,
        post_count: visible_posts.len(),
        posts: visible_posts,
        generated_at: Utc::now(),
    }))
}

#[derive(Debug, FromRow)]
struct FeedPostAccessRow {
    post_id: Uuid,
    author_mailbox_id: String,
    reply_policy: String,
    expires_at: DateTime<Utc>,
    recipient_visible: bool,
}

async fn create_feed_reply(
    State(state): State<AppState>,
    Path((mailbox_id, post_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<CreateFeedReplyRequest>,
) -> Result<(StatusCode, Json<CreateFeedReplyResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;

    let post = sqlx::query_as::<_, FeedPostAccessRow>(
        "SELECT p.post_id, p.author_mailbox_id, p.reply_policy, p.expires_at, EXISTS(SELECT 1 FROM relay_feed_post_deliveries d WHERE d.post_id = p.post_id AND d.recipient_mailbox_id = $2) AS recipient_visible FROM relay_feed_posts p WHERE p.post_id = $1 AND p.expires_at > NOW()",
    )
    .bind(post_id)
    .bind(&mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("feed post lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("feed post not found".to_string()))?;

    let viewer_is_author = post.author_mailbox_id == mailbox_id;
    if !viewer_is_author && !post.recipient_visible {
        return Err(ApiError::NotFound("feed post not found".to_string()));
    }

    if post.reply_policy == "no_replies" {
        return Err(ApiError::BadRequest(
            "replies are disabled for this feed post".to_string(),
        ));
    }

    validate_ciphertext(&request.author_ciphertext_b64, state.config.max_ciphertext_bytes)?;
    validate_ciphertext(&request.recipient_ciphertext_b64, state.config.max_ciphertext_bytes)?;

    let recipient_mailbox_id = if viewer_is_author {
        let requested = request
            .recipient_mailbox_id
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("recipient_mailbox_id is required for author replies".to_string()))?;
        validate_mailbox_id(requested)?;

        let is_valid_recipient = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM relay_feed_post_deliveries d JOIN relay_accounts a ON a.mailbox_id = d.recipient_mailbox_id WHERE d.post_id = $1 AND d.recipient_mailbox_id = $2 AND a.status = 'active')",
        )
        .bind(post_id)
        .bind(requested)
        .fetch_one(&state.db)
        .await
        .map_err(|error| ApiError::Dependency(format!("reply recipient lookup failed: {error}")))?;

        if !is_valid_recipient {
            return Err(ApiError::BadRequest(
                "author replies must target an active recipient from the original post audience".to_string(),
            ));
        }

        requested.to_string()
    } else {
        if let Some(requested) = request.recipient_mailbox_id.as_deref() {
            validate_mailbox_id(requested)?;
            if requested != post.author_mailbox_id {
                return Err(ApiError::BadRequest(
                    "recipient replies can only target the original post author".to_string(),
                ));
            }
        }

        post.author_mailbox_id.clone()
    };

    if recipient_mailbox_id == mailbox_id {
        return Err(ApiError::BadRequest(
            "feed replies must target another mailbox".to_string(),
        ));
    }

    let ttl_seconds = request
        .ttl_seconds
        .unwrap_or(state.config.default_ttl_seconds);
    if ttl_seconds < state.config.min_ttl_seconds || ttl_seconds > state.config.max_ttl_seconds {
        return Err(ApiError::BadRequest(format!(
            "ttl_seconds must be between {} and {}",
            state.config.min_ttl_seconds, state.config.max_ttl_seconds
        )));
    }

    let created_at = Utc::now();
    let requested_expiry = created_at + Duration::seconds(ttl_seconds);
    let expires_at = if requested_expiry < post.expires_at {
        requested_expiry
    } else {
        post.expires_at
    };
    let reply_id = request.reply_id.unwrap_or_else(Uuid::new_v4);
    let author_profile = ensure_account_profile(&state.db, &mailbox_id).await?;

    sqlx::query(
        "INSERT INTO relay_feed_post_replies (reply_id, post_id, author_mailbox_id, author_codename, recipient_mailbox_id, author_ciphertext_b64, recipient_ciphertext_b64, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(reply_id)
    .bind(post_id)
    .bind(&mailbox_id)
    .bind(&author_profile.codename)
    .bind(&recipient_mailbox_id)
    .bind(&request.author_ciphertext_b64)
    .bind(&request.recipient_ciphertext_b64)
    .bind(created_at)
    .bind(expires_at)
    .execute(&state.db)
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::Conflict("reply_id already exists".to_string())
        } else {
            ApiError::Dependency(format!("feed reply creation failed: {error}"))
        }
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateFeedReplyResponse {
            reply_id,
            post_id: post.post_id,
            mailbox_id,
            recipient_mailbox_id,
            created_at,
            expires_at,
        }),
    ))
}

async fn admin_set_managed_user_status(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AdminSetManagedUserStatusRequest>,
) -> Result<Json<AccountProfileResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let admin_mailbox_id = authenticate_admin(&state.db, &state.config, &headers).await?;
    let normalized_status = match request.status.as_str() {
        "active" => "active",
        "disabled" => "disabled",
        "provisioned" => "provisioned",
        _ => {
            return Err(ApiError::BadRequest(
                "status must be one of active, disabled, or provisioned".to_string(),
            ))
        }
    };

    let activated_at = if normalized_status == "active" {
        Some(Utc::now())
    } else {
        None
    };

    let profile = sqlx::query_as::<_, AccountProfileResponse>(
        "UPDATE relay_accounts SET status = $1, activated_at = $2 WHERE mailbox_id = $3 AND owner_mailbox_id = $4 AND role = 'user' RETURNING mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at"
    )
    .bind(normalized_status)
    .bind(activated_at)
    .bind(&mailbox_id)
    .bind(&admin_mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("managed user status update failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("managed user not found for this admin".to_string()))?;

    Ok(Json(profile))
}

async fn admin_reset_managed_user_access(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AdminManagedUserAccessResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let admin_mailbox_id = authenticate_admin(&state.db, &state.config, &headers).await?;

    let account = sqlx::query_as::<_, AccountProfileResponse>(
        "SELECT mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at FROM relay_accounts WHERE mailbox_id = $1 AND owner_mailbox_id = $2 AND role = 'user'"
    )
    .bind(&mailbox_id)
    .bind(&admin_mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("managed user lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("managed user not found for this admin".to_string()))?;

    let mailbox_token = random_token_bytes(32);
    let mailbox_token_b64 = Base64::encode_string(&mailbox_token);
    let access_token_hash = hash_mailbox_token(&mailbox_id, &mailbox_token);

    sqlx::query("UPDATE relay_mailboxes SET access_token_hash = $1 WHERE mailbox_id = $2")
        .bind(access_token_hash)
        .bind(&mailbox_id)
        .execute(&state.db)
        .await
        .map_err(|error| ApiError::Dependency(format!("managed user mailbox reset failed: {error}")))?;

    let updated = sqlx::query_as::<_, AccountProfileResponse>(
        "UPDATE relay_accounts SET status = 'provisioned', activated_at = NULL WHERE mailbox_id = $1 RETURNING mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at"
    )
    .bind(&mailbox_id)
    .fetch_one(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("managed user account reset failed: {error}")))?;

    Ok(Json(AdminManagedUserAccessResponse {
        mailbox_id,
        mailbox_token_b64,
        codename: updated.codename,
        status: updated.status,
        owner_mailbox_id: account.owner_mailbox_id.unwrap_or(admin_mailbox_id),
        created_at: updated.created_at,
    }))
}

async fn admin_delete_managed_user(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AdminDeleteManagedUserResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let admin_mailbox_id = authenticate_admin(&state.db, &state.config, &headers).await?;

    let deleted = sqlx::query(
        "DELETE FROM relay_mailboxes WHERE mailbox_id = $1 AND EXISTS (SELECT 1 FROM relay_accounts WHERE mailbox_id = $1 AND owner_mailbox_id = $2 AND role = 'user')"
    )
    .bind(&mailbox_id)
    .bind(&admin_mailbox_id)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("managed user deletion failed: {error}")))?
    .rows_affected()
        > 0;

    if !deleted {
        return Err(ApiError::NotFound(
            "managed user not found for this admin".to_string(),
        ));
    }

    Ok(Json(AdminDeleteManagedUserResponse {
        mailbox_id,
        deleted: true,
    }))
}

async fn publish_prekey_bundle(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PublishPreKeyBundleRequest>,
) -> Result<(StatusCode, Json<PublishPreKeyBundleResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    enforce_rate_limit(
        &state.redis,
        &format!("prekey-publish:{mailbox_id}"),
        state.config.submit_rate_limit_per_minute,
        60,
    )
    .await?;

    let parsed = parse_prekey_bundle_upload(&request)?;
    let updated_at = Utc::now();
    let signed_prekey_created_at = updated_at;
    let mut transaction = state
        .db
        .begin()
        .await
        .map_err(|error| ApiError::Dependency(format!("prekey transaction start failed: {error}")))?;

    sqlx::query(
        "INSERT INTO relay_prekey_bundles (mailbox_id, identity_signing_key, identity_exchange_key, signed_prekey, signed_prekey_signature, signed_prekey_created_at, signed_prekey_expires_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (mailbox_id) DO UPDATE SET identity_signing_key = EXCLUDED.identity_signing_key, identity_exchange_key = EXCLUDED.identity_exchange_key, signed_prekey = EXCLUDED.signed_prekey, signed_prekey_signature = EXCLUDED.signed_prekey_signature, signed_prekey_created_at = EXCLUDED.signed_prekey_created_at, signed_prekey_expires_at = EXCLUDED.signed_prekey_expires_at, updated_at = EXCLUDED.updated_at",
    )
    .bind(&mailbox_id)
    .bind(parsed.identity_signing_key.to_vec())
    .bind(parsed.identity_exchange_key.to_vec())
    .bind(parsed.signed_prekey.to_vec())
    .bind(parsed.signed_prekey_signature.to_vec())
    .bind(signed_prekey_created_at)
    .bind(parsed.signed_prekey_expires_at)
    .bind(updated_at)
    .execute(&mut *transaction)
    .await
    .map_err(|error| ApiError::Dependency(format!("prekey bundle upsert failed: {error}")))?;

    sqlx::query("DELETE FROM relay_one_time_prekeys WHERE mailbox_id = $1 AND consumed_at IS NULL")
        .bind(&mailbox_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApiError::Dependency(format!("prekey cleanup failed: {error}")))?;

    for public_key in &parsed.one_time_prekeys {
        sqlx::query(
            "INSERT INTO relay_one_time_prekeys (prekey_id, mailbox_id, public_key, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(&mailbox_id)
        .bind(public_key.to_vec())
        .bind(updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApiError::Dependency(format!("one-time prekey insert failed: {error}")))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| ApiError::Dependency(format!("prekey transaction commit failed: {error}")))?;

    Ok((
        StatusCode::CREATED,
        Json(PublishPreKeyBundleResponse {
            mailbox_id,
            signed_prekey_expires_at: parsed.signed_prekey_expires_at,
            one_time_prekey_count: parsed.one_time_prekeys.len(),
            updated_at,
        }),
    ))
}

async fn fetch_prekey_bundle(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
) -> Result<Json<FetchPreKeyBundleResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    ensure_mailbox_exists(&state.db, &mailbox_id).await?;
    enforce_rate_limit(
        &state.redis,
        &format!("prekey-fetch:{mailbox_id}"),
        state.config.retrieve_rate_limit_per_minute,
        60,
    )
    .await?;

    let mut transaction = state
        .db
        .begin()
        .await
        .map_err(|error| ApiError::Dependency(format!("prekey fetch transaction start failed: {error}")))?;

    let bundle = sqlx::query_as::<_, StoredPreKeyBundleRow>(
        "SELECT mailbox_id, identity_signing_key, identity_exchange_key, signed_prekey, signed_prekey_signature, signed_prekey_expires_at FROM relay_prekey_bundles WHERE mailbox_id = $1 AND signed_prekey_expires_at > NOW()",
    )
    .bind(&mailbox_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| ApiError::Dependency(format!("prekey bundle lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("active prekey bundle not found".to_string()))?;

    let one_time_prekey = sqlx::query_scalar::<_, Vec<u8>>(
        "WITH candidate AS (SELECT prekey_id FROM relay_one_time_prekeys WHERE mailbox_id = $1 AND consumed_at IS NULL ORDER BY created_at ASC LIMIT 1 FOR UPDATE SKIP LOCKED) UPDATE relay_one_time_prekeys AS prekeys SET consumed_at = NOW() FROM candidate WHERE prekeys.prekey_id = candidate.prekey_id RETURNING prekeys.public_key",
    )
    .bind(&mailbox_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| ApiError::Dependency(format!("one-time prekey fetch failed: {error}")))?;

    transaction
        .commit()
        .await
        .map_err(|error| ApiError::Dependency(format!("prekey fetch commit failed: {error}")))?;

    Ok(Json(FetchPreKeyBundleResponse {
        mailbox_id: bundle.mailbox_id,
        identity_signing_key_b64: Base64::encode_string(&bundle.identity_signing_key),
        identity_exchange_key_b64: Base64::encode_string(&bundle.identity_exchange_key),
        signed_prekey_b64: Base64::encode_string(&bundle.signed_prekey),
        signed_prekey_signature_b64: Base64::encode_string(&bundle.signed_prekey_signature),
        signed_prekey_expires_at: bundle.signed_prekey_expires_at,
        one_time_prekey_b64: one_time_prekey.map(|value| Base64::encode_string(&value)),
        fetched_at: Utc::now(),
    }))
}

async fn get_prekey_inventory_status(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<PreKeyInventoryStatusResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;

    let status = sqlx::query_as::<_, StoredPreKeyInventoryStatus>(
        "SELECT bundles.signed_prekey_expires_at, COUNT(prekeys.public_key)::BIGINT AS available_one_time_prekeys FROM relay_prekey_bundles bundles LEFT JOIN relay_one_time_prekeys prekeys ON prekeys.mailbox_id = bundles.mailbox_id AND prekeys.consumed_at IS NULL WHERE bundles.mailbox_id = $1 AND bundles.signed_prekey_expires_at > NOW() GROUP BY bundles.signed_prekey_expires_at",
    )
    .bind(&mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("prekey status lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("active prekey bundle not found".to_string()))?;

    Ok(Json(PreKeyInventoryStatusResponse {
        mailbox_id,
        signed_prekey_expires_at: status.signed_prekey_expires_at,
        available_one_time_prekeys: status.available_one_time_prekeys,
        checked_at: Utc::now(),
    }))
}

async fn publish_device_record(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<DeviceRecord>,
) -> Result<(StatusCode, Json<PublishDeviceRecordResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;

    let identity_signing_key = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT identity_signing_key FROM relay_prekey_bundles WHERE mailbox_id = $1 AND signed_prekey_expires_at > NOW()",
    )
    .bind(&mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("device identity lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("active prekey bundle not found for mailbox".to_string()))?;

    let identity_signing_key: [u8; 32] = identity_signing_key
        .try_into()
        .map_err(|_| ApiError::Internal)?;
    let identity_signing_key = VerifyingKey::from_bytes(&identity_signing_key)
        .map_err(|_| ApiError::Internal)?;
    request.verify(&identity_signing_key)?;
    let updated_at = Utc::now();

    sqlx::query(
        "INSERT INTO relay_device_records (mailbox_id, device_id, device_label, device_signing_key, device_exchange_key, created_at, revoked_at, signature, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (mailbox_id, device_id) DO UPDATE SET device_label = EXCLUDED.device_label, device_signing_key = EXCLUDED.device_signing_key, device_exchange_key = EXCLUDED.device_exchange_key, created_at = EXCLUDED.created_at, revoked_at = EXCLUDED.revoked_at, signature = EXCLUDED.signature, updated_at = EXCLUDED.updated_at",
    )
    .bind(&mailbox_id)
    .bind(&request.device_id)
    .bind(&request.device_label)
    .bind(Base64::decode_vec(&request.device_signing_key_b64).map_err(|_| ApiError::BadRequest("device_signing_key_b64 must be valid base64".to_string()))?)
    .bind(Base64::decode_vec(&request.device_exchange_key_b64).map_err(|_| ApiError::BadRequest("device_exchange_key_b64 must be valid base64".to_string()))?)
    .bind(request.created_at)
    .bind(request.revoked_at)
    .bind(Base64::decode_vec(&request.signature_b64).map_err(|_| ApiError::BadRequest("signature_b64 must be valid base64".to_string()))?)
    .bind(updated_at)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("device record upsert failed: {error}")))?;

    Ok((
        StatusCode::CREATED,
        Json(PublishDeviceRecordResponse {
            mailbox_id,
            device_id: request.device_id,
            revoked_at: request.revoked_at,
            updated_at,
        }),
    ))
}

async fn list_device_records(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
) -> Result<Json<ListDeviceRecordsResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    ensure_mailbox_exists(&state.db, &mailbox_id).await?;

    let devices = sqlx::query_as::<_, StoredDeviceRecordRow>(
        "SELECT device_id, device_label, device_signing_key, device_exchange_key, created_at, revoked_at, signature FROM relay_device_records WHERE mailbox_id = $1 ORDER BY created_at DESC",
    )
    .bind(&mailbox_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("device record listing failed: {error}")))?
    .into_iter()
    .map(|row| DeviceRecord {
        version: "simy-device-record-v1".to_string(),
        device_id: row.device_id,
        device_label: row.device_label,
        device_signing_key_b64: Base64::encode_string(&row.device_signing_key),
        device_exchange_key_b64: Base64::encode_string(&row.device_exchange_key),
        created_at: row.created_at,
        revoked_at: row.revoked_at,
        signature_b64: Base64::encode_string(&row.signature),
    })
    .collect();

    Ok(Json(ListDeviceRecordsResponse { mailbox_id, devices }))
}

async fn create_media_upload_intent(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateMediaUploadIntentRequest>,
) -> Result<(StatusCode, Json<CreateMediaUploadIntentResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    enforce_rate_limit(
        &state.redis,
        &format!("media-intent:{mailbox_id}"),
        state.config.submit_rate_limit_per_minute,
        60,
    )
    .await?;

    validate_media_type(&request.media_type)?;
    if request.original_size_bytes == 0 || request.original_size_bytes > state.config.media_max_original_size_bytes {
        return Err(ApiError::BadRequest(format!(
            "original_size_bytes must be between 1 and {}",
            state.config.media_max_original_size_bytes
        )));
    }

    let chunk_size_bytes = request
        .chunk_size_bytes
        .unwrap_or(state.config.media_default_chunk_size_bytes);
    let ttl_seconds = request
        .ttl_seconds
        .unwrap_or(state.config.media_upload_intent_ttl_seconds);
    if ttl_seconds <= 0 || ttl_seconds > state.config.max_ttl_seconds {
        return Err(ApiError::BadRequest(format!(
            "ttl_seconds must be between 1 and {}",
            state.config.max_ttl_seconds
        )));
    }

    if let Some(content_sha256_b64) = request.content_sha256_b64.as_deref() {
        validate_sha256_b64(content_sha256_b64)?;
    }

    let padding_plan = build_blob_padding_plan(request.original_size_bytes, chunk_size_bytes)?;
    let intent_id = Uuid::new_v4();
    let created_at = Utc::now();
    let expires_at = created_at + Duration::seconds(ttl_seconds);
    let object_key = format!(
        "mailboxes/{}/{}/{}/{}",
        mailbox_id,
        created_at.format("%Y/%m"),
        request.media_type.replace('/', "-"),
        intent_id
    );
    let presigned_upload = presign_upload_request(
        &state.s3,
        &state.config,
        &object_key,
        &request.media_type,
        ttl_seconds,
    )
    .await?;

    sqlx::query(
        "INSERT INTO media_upload_intents (intent_id, mailbox_id, bucket, object_key, media_type, original_size_bytes, padded_size_bytes, chunk_size_bytes, content_sha256_b64, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(intent_id)
    .bind(&mailbox_id)
    .bind(&state.config.media_object_store_bucket)
    .bind(&object_key)
    .bind(&request.media_type)
    .bind(request.original_size_bytes as i64)
    .bind(padding_plan.padded_size_bytes as i64)
    .bind(i32::try_from(chunk_size_bytes).map_err(|_| ApiError::BadRequest("chunk_size_bytes is too large".to_string()))?)
    .bind(&request.content_sha256_b64)
    .bind(created_at)
    .bind(expires_at)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("media upload intent creation failed: {error}")))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateMediaUploadIntentResponse {
            intent_id,
            mailbox_id,
            bucket: state.config.media_object_store_bucket.clone(),
            object_key,
            media_type: request.media_type,
            original_size_bytes: request.original_size_bytes,
            padded_size_bytes: padding_plan.padded_size_bytes,
            chunk_size_bytes,
            region: state.config.media_object_store_region.clone(),
            upload_endpoint: state.config.media_object_store_endpoint.clone(),
            presigned_upload,
            content_sha256_b64: request.content_sha256_b64,
            expires_at,
        }),
    ))
}

async fn create_media_manifest(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateMediaManifestRequest>,
) -> Result<(StatusCode, Json<CreateMediaManifestResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    if let Some(content_sha256_b64) = request.content_sha256_b64.as_deref() {
        validate_sha256_b64(content_sha256_b64)?;
    }

    let intent = sqlx::query_as::<_, StoredUploadIntent>(
        "SELECT intent_id, bucket, object_key, media_type, original_size_bytes, padded_size_bytes, chunk_size_bytes, content_sha256_b64, created_at FROM media_upload_intents WHERE intent_id = $1 AND mailbox_id = $2 AND expires_at > NOW()",
    )
    .bind(request.intent_id)
    .bind(&mailbox_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("media upload intent lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("active media upload intent not found".to_string()))?;

    let object_id = Uuid::new_v4();
    let upload_completed_at = request.upload_completed_at.unwrap_or_else(Utc::now);
    let content_sha256_b64 = request
        .content_sha256_b64
        .or(intent.content_sha256_b64.clone());

    let result = sqlx::query(
        "INSERT INTO media_objects (object_id, mailbox_id, intent_id, bucket, object_key, media_type, original_size_bytes, padded_size_bytes, chunk_size_bytes, content_sha256_b64, created_at, upload_completed_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(object_id)
    .bind(&mailbox_id)
    .bind(intent.intent_id)
    .bind(&intent.bucket)
    .bind(&intent.object_key)
    .bind(&intent.media_type)
    .bind(intent.original_size_bytes)
    .bind(intent.padded_size_bytes)
    .bind(intent.chunk_size_bytes)
    .bind(&content_sha256_b64)
    .bind(intent.created_at)
    .bind(upload_completed_at)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Ok((
            StatusCode::CREATED,
            Json(CreateMediaManifestResponse {
                object_id,
                mailbox_id,
                bucket: intent.bucket,
                object_key: intent.object_key,
                media_type: intent.media_type,
                original_size_bytes: intent.original_size_bytes,
                padded_size_bytes: intent.padded_size_bytes,
                chunk_size_bytes: intent.chunk_size_bytes,
                content_sha256_b64,
                upload_completed_at,
            }),
        )),
        Err(error) if is_unique_violation(&error) => {
            Err(ApiError::Conflict("media object already registered for this intent or object key".to_string()))
        }
        Err(error) => Err(ApiError::Dependency(format!(
            "media manifest creation failed: {error}"
        ))),
    }
}

async fn list_media_manifests(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    Query(query): Query<ListMediaQuery>,
    headers: HeaderMap,
) -> Result<Json<ListMediaManifestsResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let media = sqlx::query_as::<_, StoredMediaManifest>(
        "SELECT object_id, intent_id, bucket, object_key, media_type, original_size_bytes, padded_size_bytes, chunk_size_bytes, content_sha256_b64, created_at, upload_completed_at FROM media_objects WHERE mailbox_id = $1 ORDER BY upload_completed_at DESC LIMIT $2",
    )
    .bind(&mailbox_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("media manifest listing failed: {error}")))?;

    Ok(Json(ListMediaManifestsResponse { mailbox_id, media }))
}

async fn create_media_access_grant(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateMediaAccessGrantRequest>,
) -> Result<(StatusCode, Json<CreateMediaAccessGrantResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, token).await?;
    validate_grant_operation(&request.operation)?;
    let ttl_seconds = request
        .ttl_seconds
        .unwrap_or(state.config.media_access_grant_ttl_seconds);
    if ttl_seconds <= 0 || ttl_seconds > state.config.max_ttl_seconds {
        return Err(ApiError::BadRequest(format!(
            "ttl_seconds must be between 1 and {}",
            state.config.max_ttl_seconds
        )));
    }

    let media = sqlx::query_as::<_, StoredMediaManifest>(
        "SELECT object_id, intent_id, bucket, object_key, media_type, original_size_bytes, padded_size_bytes, chunk_size_bytes, content_sha256_b64, created_at, upload_completed_at FROM media_objects WHERE mailbox_id = $1 AND object_key = $2",
    )
    .bind(&mailbox_id)
    .bind(&request.object_key)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("media object lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("media object not found".to_string()))?;

    let grant_id = Uuid::new_v4();
    let created_at = Utc::now();
    let expires_at = created_at + Duration::seconds(ttl_seconds);
    let grant_token = random_hex_token(32);
    let resolve_path = format!("/v1/media/access-grants/{}", grant_token);
    let grant_token_hash = sha256_bytes(grant_token.as_bytes());

    sqlx::query(
        "INSERT INTO media_access_grants (grant_id, object_id, mailbox_id, grant_token_hash, operation, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(grant_id)
    .bind(media.object_id)
    .bind(&mailbox_id)
    .bind(grant_token_hash)
    .bind(&request.operation)
    .bind(created_at)
    .bind(expires_at)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("media access grant creation failed: {error}")))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateMediaAccessGrantResponse {
            grant_id,
            grant_token,
            operation: request.operation,
            bucket: media.bucket,
            object_key: media.object_key,
            media_type: media.media_type,
            upload_endpoint: state.config.media_object_store_endpoint.clone(),
            resolve_path,
            expires_at,
        }),
    ))
}

async fn resolve_media_access_grant(
    State(state): State<AppState>,
    Path(grant_token): Path<String>,
) -> Result<Json<ResolveMediaAccessGrantResponse>, ApiError> {
    validate_grant_token(&grant_token)?;
    let row = lookup_media_access_grant(&state.db, &grant_token).await?;
    mark_media_access_grant_redeemed(&state.db, row.grant_id).await?;
    let presigned_request = presign_media_access_request(
        &state.s3,
        &row.bucket,
        &row.object_key,
        &row.media_type,
        &row.operation,
        row.expires_at,
    )
    .await?;

    Ok(Json(ResolveMediaAccessGrantResponse {
        grant_id: row.grant_id,
        operation: row.operation,
        bucket: row.bucket,
        object_key: row.object_key,
        media_type: row.media_type,
        original_size_bytes: row.original_size_bytes,
        padded_size_bytes: row.padded_size_bytes,
        chunk_size_bytes: row.chunk_size_bytes,
        content_sha256_b64: row.content_sha256_b64,
        object_store_endpoint: state.config.media_object_store_endpoint.clone(),
        presigned_request,
        expires_at: row.expires_at,
    }))
}

async fn fetch_media_access_grant_content(
    State(state): State<AppState>,
    Path(grant_token): Path<String>,
) -> Result<Response, ApiError> {
    validate_grant_token(&grant_token)?;
    let row = lookup_media_access_grant(&state.db, &grant_token).await?;
    if row.operation != "download" {
        return Err(ApiError::BadRequest(
            "media access grant does not allow downloads".to_string(),
        ));
    }
    mark_media_access_grant_redeemed(&state.db, row.grant_id).await?;

    let object = state
        .s3
        .get_object()
        .bucket(&row.bucket)
        .key(&row.object_key)
        .send()
        .await
        .map_err(|error| ApiError::Dependency(format!("media object fetch failed: {error}")))?;

    let bytes = object
        .body
        .collect()
        .await
        .map_err(|error| ApiError::Dependency(format!("media object read failed: {error}")))?
        .into_bytes();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(bytes))
        .map_err(|_| ApiError::Internal)
}

async fn lookup_media_access_grant(
    db: &PgPool,
    grant_token: &str,
) -> Result<ResolvedGrantRow, ApiError> {
    let grant_token_hash = sha256_bytes(grant_token.as_bytes());
    sqlx::query_as::<_, ResolvedGrantRow>(
        "SELECT g.grant_id, g.operation, g.expires_at, o.bucket, o.object_key, o.media_type, o.original_size_bytes, o.padded_size_bytes, o.chunk_size_bytes, o.content_sha256_b64 FROM media_access_grants g INNER JOIN media_objects o ON o.object_id = g.object_id WHERE g.grant_token_hash = $1 AND g.expires_at > NOW()",
    )
    .bind(grant_token_hash)
    .fetch_optional(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("media access grant lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("media access grant not found or expired".to_string()))
}

async fn mark_media_access_grant_redeemed(db: &PgPool, grant_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE media_access_grants SET redeemed_at = NOW() WHERE grant_id = $1")
        .bind(grant_id)
        .execute(db)
        .await
        .map_err(|error| ApiError::Dependency(format!("media access grant redemption failed: {error}")))?;
    Ok(())
}

async fn submit_message(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    Json(request): Json<SubmitMessageRequest>,
) -> Result<(StatusCode, Json<SubmitMessageResponse>), ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    ensure_mailbox_exists(&state.db, &mailbox_id).await?;
    let ciphertext = validate_ciphertext(&request.ciphertext_b64, state.config.max_ciphertext_bytes)?;
    validate_sender_device_hint(request.sender_device_hint.as_deref())?;
    validate_replay_token(&request.replay_token)?;
    enforce_rate_limit(
        &state.redis,
        &format!("submit:{mailbox_id}"),
        state.config.submit_rate_limit_per_minute,
        60,
    )
    .await?;

    let ttl_seconds = request.ttl_seconds.unwrap_or(state.config.default_ttl_seconds);
    if ttl_seconds < state.config.min_ttl_seconds || ttl_seconds > state.config.max_ttl_seconds {
        return Err(ApiError::BadRequest(format!(
            "ttl_seconds must be between {} and {}",
            state.config.min_ttl_seconds, state.config.max_ttl_seconds
        )));
    }

    let replay_reserved = reserve_replay_token(
        &state.redis,
        &mailbox_id,
        &request.replay_token,
        ttl_seconds + state.config.replay_ttl_margin_seconds,
    )
    .await?;
    if !replay_reserved {
        return Err(ApiError::Conflict("duplicate replay_token".to_string()));
    }

    let received_at = Utc::now();
    let message_id = request.message_id.unwrap_or_else(Uuid::new_v4);
    let expires_at = received_at + Duration::seconds(ttl_seconds);

    let result = sqlx::query(
        "INSERT INTO relay_messages (message_id, mailbox_id, sender_device_hint, ciphertext, received_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(message_id)
    .bind(&mailbox_id)
    .bind(request.sender_device_hint)
    .bind(ciphertext)
    .bind(received_at)
    .bind(expires_at)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Ok((
            StatusCode::ACCEPTED,
            Json(SubmitMessageResponse {
                message_id,
                expires_at,
            }),
        )),
        Err(error) if is_unique_violation(&error) => {
            Err(ApiError::Conflict("message_id already exists".to_string()))
        }
        Err(error) => Err(ApiError::Dependency(format!(
            "message persistence failed: {error}"
        ))),
    }
}

async fn list_messages(
    State(state): State<AppState>,
    Path(mailbox_id): Path<String>,
    Query(query): Query<ListMessagesQuery>,
    headers: HeaderMap,
) -> Result<Json<ListMessagesResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, &token).await?;
    enforce_rate_limit(
        &state.redis,
        &format!("retrieve:{mailbox_id}"),
        state.config.retrieve_rate_limit_per_minute,
        60,
    )
    .await?;

    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let messages = sqlx::query_as::<_, StoredEnvelope>(
        "SELECT message_id, sender_device_hint, encode(ciphertext, 'base64') AS ciphertext_b64, received_at, expires_at FROM relay_messages WHERE mailbox_id = $1 AND deleted_at IS NULL AND expires_at > NOW() ORDER BY received_at ASC LIMIT $2",
    )
    .bind(&mailbox_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("message retrieval failed: {error}")))?;

    Ok(Json(ListMessagesResponse { mailbox_id, messages }))
}

async fn delete_message(
    State(state): State<AppState>,
    Path((mailbox_id, message_id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<DeleteResponse>, ApiError> {
    validate_mailbox_id(&mailbox_id)?;
    let token = read_mailbox_token_header(&headers)?;
    authenticate_mailbox(&state.db, &mailbox_id, &token).await?;
    enforce_rate_limit(
        &state.redis,
        &format!("retrieve:{mailbox_id}"),
        state.config.retrieve_rate_limit_per_minute,
        60,
    )
    .await?;

    let rows_affected = sqlx::query(
        "UPDATE relay_messages SET deleted_at = NOW() WHERE mailbox_id = $1 AND message_id = $2 AND deleted_at IS NULL",
    )
    .bind(&mailbox_id)
    .bind(message_id)
    .execute(&state.db)
    .await
    .map_err(|error| ApiError::Dependency(format!("message deletion failed: {error}")))?
    .rows_affected();

    Ok(Json(DeleteResponse {
        mailbox_id,
        message_id,
        deleted: rows_affected > 0,
    }))
}

async fn ensure_mailbox_exists(db: &PgPool, mailbox_id: &str) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM relay_mailboxes WHERE mailbox_id = $1)",
    )
    .bind(mailbox_id)
    .fetch_one(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("mailbox lookup failed: {error}")))?;

    if exists {
        Ok(())
    } else {
        Err(ApiError::NotFound("mailbox_id not found".to_string()))
    }
}

async fn authenticate_mailbox(db: &PgPool, mailbox_id: &str, token_b64: &str) -> Result<(), ApiError> {
    let token = decode_mailbox_token(token_b64)?;
    let expected_hash = hash_mailbox_token(mailbox_id, &token);

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM relay_mailboxes m LEFT JOIN relay_accounts a ON a.mailbox_id = m.mailbox_id WHERE m.mailbox_id = $1 AND m.access_token_hash = $2 AND COALESCE(a.status, 'active') <> 'disabled')",
    )
    .bind(mailbox_id)
    .bind(expected_hash)
    .fetch_one(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("mailbox authentication failed: {error}")))?;

    if exists {
        Ok(())
    } else {
        Err(ApiError::Unauthorized("invalid mailbox token".to_string()))
    }
}

async fn authorize_mailbox_creation(
    db: &PgPool,
    config: &RelayConfig,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    if headers.get("x-admin-token").is_some() {
        ensure_admin_token(config, headers)?;
        return Ok(());
    }

    let origin_mailbox_id = read_optional_header(headers, "x-origin-mailbox-id")?;
    let origin_mailbox_token = read_optional_header(headers, "x-origin-mailbox-token")?;

    match (origin_mailbox_id, origin_mailbox_token) {
        (Some(mailbox_id), Some(token)) => {
            validate_mailbox_id(mailbox_id)?;
            authenticate_mailbox(db, mailbox_id, token).await
        }
        (None, None) => Err(ApiError::Unauthorized(
            "new mailbox creation requires protected admin access or an authenticated origin mailbox".to_string(),
        )),
        _ => Err(ApiError::Unauthorized(
            "origin mailbox headers must include both x-origin-mailbox-id and x-origin-mailbox-token".to_string(),
        )),
    }
}

fn ensure_admin_token(config: &RelayConfig, headers: &HeaderMap) -> Result<(), ApiError> {
    let token = headers
        .get("x-admin-token")
        .ok_or_else(|| ApiError::Unauthorized("missing x-admin-token header".to_string()))?
        .to_str()
        .map_err(|_| ApiError::Unauthorized("invalid x-admin-token header".to_string()))?;

    if token == config.admin_token {
        Ok(())
    } else {
        Err(ApiError::Unauthorized("invalid admin token".to_string()))
    }
}

async fn authenticate_admin(
    db: &PgPool,
    config: &RelayConfig,
    headers: &HeaderMap,
) -> Result<String, ApiError> {
    ensure_admin_token(config, headers)?;

    let admin_mailbox_id = read_admin_mailbox_id_header(headers)?;
    let mailbox_token = read_mailbox_token_header(headers)?;
    authenticate_mailbox(db, &admin_mailbox_id, mailbox_token).await?;

    let is_admin = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM relay_accounts WHERE mailbox_id = $1 AND role = 'admin' AND status = 'active')",
    )
    .bind(&admin_mailbox_id)
    .fetch_one(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("admin role lookup failed: {error}")))?;

    if is_admin {
        Ok(admin_mailbox_id)
    } else {
        Err(ApiError::Unauthorized("mailbox is not an active admin account".to_string()))
    }
}

async fn authenticate_admin_bootstrap(
    db: &PgPool,
    config: &RelayConfig,
    headers: &HeaderMap,
    mailbox_id: &str,
) -> Result<(), ApiError> {
    ensure_admin_token(config, headers)?;

    let mailbox_token = read_mailbox_token_header(headers)?;
    authenticate_mailbox(db, mailbox_id, mailbox_token).await
}

fn read_optional_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ApiError> {
    match headers.get(name) {
        Some(value) => value
            .to_str()
            .map(Some)
            .map_err(|_| ApiError::Unauthorized(format!("invalid {name} header"))),
        None => Ok(None),
    }
}

fn read_admin_mailbox_id_header(headers: &HeaderMap) -> Result<String, ApiError> {
    let mailbox_id = headers
        .get("x-admin-mailbox-id")
        .ok_or_else(|| ApiError::Unauthorized("missing x-admin-mailbox-id header".to_string()))?
        .to_str()
        .map_err(|_| ApiError::Unauthorized("invalid x-admin-mailbox-id header".to_string()))?;
    validate_mailbox_id(mailbox_id)?;
    Ok(mailbox_id.to_string())
}

fn read_mailbox_token_header(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get("x-mailbox-token")
        .ok_or_else(|| ApiError::Unauthorized("missing x-mailbox-token header".to_string()))?
        .to_str()
        .map_err(|_| ApiError::Unauthorized("invalid x-mailbox-token header".to_string()))
}

fn validate_mailbox_id(value: &str) -> Result<(), ApiError> {
    if value.len() < 16 || value.len() > 128 {
        return Err(ApiError::BadRequest("mailbox_id length is invalid".to_string()));
    }

    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ApiError::BadRequest(
            "mailbox_id contains unsupported characters".to_string(),
        ));
    }

    Ok(())
}

fn normalize_codename(value: Option<&str>, fallback: &str) -> Result<String, ApiError> {
    let normalized = value.unwrap_or("").trim();
    let codename = if normalized.is_empty() { fallback } else { normalized };

    if codename.len() < 2 || codename.len() > 64 {
        return Err(ApiError::BadRequest("codename length is invalid".to_string()));
    }

    Ok(codename.to_string())
}

async fn mailbox_created_at(db: &PgPool, mailbox_id: &str) -> Result<DateTime<Utc>, ApiError> {
    sqlx::query_scalar::<_, DateTime<Utc>>("SELECT created_at FROM relay_mailboxes WHERE mailbox_id = $1")
        .bind(mailbox_id)
        .fetch_optional(db)
        .await
        .map_err(|error| ApiError::Dependency(format!("mailbox lookup failed: {error}")))?
        .ok_or_else(|| ApiError::NotFound("mailbox_id not found".to_string()))
}

async fn upsert_account_profile(
    db: &PgPool,
    mailbox_id: &str,
    codename: &str,
    created_at: DateTime<Utc>,
    activated_at: Option<DateTime<Utc>>,
) -> Result<AccountProfileResponse, ApiError> {
    sqlx::query_as::<_, AccountProfileResponse>(
        "INSERT INTO relay_accounts (mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at) VALUES ($1, $2, 'user', 'active', NULL, $3, $4) ON CONFLICT (mailbox_id) DO UPDATE SET codename = EXCLUDED.codename, status = 'active', activated_at = EXCLUDED.activated_at RETURNING mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at"
    )
    .bind(mailbox_id)
    .bind(codename)
    .bind(created_at)
    .bind(activated_at)
    .fetch_one(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("account upsert failed: {error}")))
}

async fn ensure_account_profile(
    db: &PgPool,
    mailbox_id: &str,
) -> Result<AccountProfileResponse, ApiError> {
    if let Some(profile) = lookup_account_profile(db, mailbox_id).await? {
        return Ok(profile);
    }

    let created_at = mailbox_created_at(db, mailbox_id).await?;
    let fallback_codename = suggested_codename(mailbox_id);
    upsert_account_profile(db, mailbox_id, &fallback_codename, created_at, Some(created_at)).await
}

async fn lookup_account_profile(
    db: &PgPool,
    mailbox_id: &str,
) -> Result<Option<AccountProfileResponse>, ApiError> {
    sqlx::query_as::<_, AccountProfileResponse>(
        "SELECT mailbox_id, codename, role, status, owner_mailbox_id, created_at, activated_at FROM relay_accounts WHERE mailbox_id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("account lookup failed: {error}")))
}

async fn fetch_contact_summary(
    db: &PgPool,
    mailbox_id: &str,
    contact_mailbox_id: &str,
) -> Result<ContactSummaryResponse, ApiError> {
    sqlx::query_as::<_, ContactSummaryResponse>(
        "SELECT c.owner_mailbox_id AS mailbox_id, c.contact_mailbox_id, c.codename, a.role AS contact_role, a.status AS contact_status, c.created_at, c.updated_at FROM relay_contacts c JOIN relay_accounts a ON a.mailbox_id = c.contact_mailbox_id WHERE c.owner_mailbox_id = $1 AND c.contact_mailbox_id = $2",
    )
    .bind(mailbox_id)
    .bind(contact_mailbox_id)
    .fetch_optional(db)
    .await
    .map_err(|error| ApiError::Dependency(format!("contact lookup failed: {error}")))?
    .ok_or_else(|| ApiError::NotFound("contact not found for this mailbox".to_string()))
}

async fn resolve_feed_reply_targets(
    db: &PgPool,
    viewer_mailbox_id: &str,
    post: &FeedPostVisibleRow,
) -> Result<Vec<FeedReplyTargetResponse>, ApiError> {
    if post.reply_policy == "no_replies" {
        return Ok(Vec::new());
    }

    if post.author_mailbox_id == viewer_mailbox_id {
        return sqlx::query_as::<_, FeedReplyTargetResponse>(
            "SELECT d.recipient_mailbox_id AS mailbox_id, a.codename FROM relay_feed_post_deliveries d JOIN relay_accounts a ON a.mailbox_id = d.recipient_mailbox_id WHERE d.post_id = $1 AND a.status = 'active' ORDER BY a.codename ASC",
        )
        .bind(post.post_id)
        .fetch_all(db)
        .await
        .map_err(|error| ApiError::Dependency(format!("feed reply target lookup failed: {error}")));
    }

    Ok(vec![FeedReplyTargetResponse {
        mailbox_id: post.author_mailbox_id.clone(),
        codename: post.author_codename.clone(),
    }])
}

fn decode_mailbox_token(value: &str) -> Result<Vec<u8>, ApiError> {
    let decoded = Base64::decode_vec(value)
        .map_err(|_| ApiError::BadRequest("mailbox token must be valid base64".to_string()))?;

    if decoded.len() < 32 {
        return Err(ApiError::BadRequest(
            "mailbox token must be at least 32 bytes".to_string(),
        ));
    }

    Ok(decoded)
}

fn validate_ciphertext(value: &str, max_ciphertext_bytes: usize) -> Result<Vec<u8>, ApiError> {
    let decoded = Base64::decode_vec(value)
        .map_err(|_| ApiError::BadRequest("ciphertext_b64 must be valid base64".to_string()))?;

    if decoded.is_empty() {
        return Err(ApiError::BadRequest(
            "ciphertext_b64 must not be empty".to_string(),
        ));
    }

    if decoded.len() > max_ciphertext_bytes {
        return Err(ApiError::BadRequest(format!(
            "ciphertext exceeds maximum size of {max_ciphertext_bytes} bytes"
        )));
    }

    Ok(decoded)
}

fn validate_sender_device_hint(value: Option<&str>) -> Result<(), ApiError> {
    if let Some(value) = value {
        if value.len() > 128 {
            return Err(ApiError::BadRequest(
                "sender_device_hint exceeds 128 characters".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_replay_token(value: &str) -> Result<(), ApiError> {
    if value.len() < 16 || value.len() > 128 {
        return Err(ApiError::BadRequest(
            "replay_token length is invalid".to_string(),
        ));
    }

    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ApiError::BadRequest(
            "replay_token contains unsupported characters".to_string(),
        ));
    }

    Ok(())
}

fn validate_media_type(value: &str) -> Result<(), ApiError> {
    if value.len() < 3 || value.len() > 128 || !value.contains('/') || value.contains(' ') {
        return Err(ApiError::BadRequest(
            "media_type must look like a MIME type".to_string(),
        ));
    }

    Ok(())
}

fn validate_sha256_b64(value: &str) -> Result<(), ApiError> {
    let decoded = Base64::decode_vec(value)
        .map_err(|_| ApiError::BadRequest("content_sha256_b64 must be valid base64".to_string()))?;
    if decoded.len() != 32 {
        return Err(ApiError::BadRequest(
            "content_sha256_b64 must decode to 32 bytes".to_string(),
        ));
    }

    Ok(())
}

fn validate_grant_operation(value: &str) -> Result<(), ApiError> {
    if matches!(value, "download" | "upload") {
        Ok(())
    } else {
        Err(ApiError::BadRequest(
            "operation must be 'download' or 'upload'".to_string(),
        ))
    }
}

fn validate_grant_token(value: &str) -> Result<(), ApiError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ApiError::BadRequest(
            "grant token must be a 64-character hex value".to_string(),
        ));
    }

    Ok(())
}

fn parse_prekey_bundle_upload(
    request: &PublishPreKeyBundleRequest,
) -> Result<ParsedPreKeyBundleUpload, ApiError> {
    if request.one_time_prekeys_b64.len() > 256 {
        return Err(ApiError::BadRequest(
            "one_time_prekeys_b64 must not contain more than 256 entries".to_string(),
        ));
    }

    let identity_signing_key = decode_fixed_base64::<32>(
        &request.identity_signing_key_b64,
        "identity_signing_key_b64",
    )?;
    let identity_exchange_key = decode_fixed_base64::<32>(
        &request.identity_exchange_key_b64,
        "identity_exchange_key_b64",
    )?;
    let signed_prekey =
        decode_fixed_base64::<32>(&request.signed_prekey_b64, "signed_prekey_b64")?;
    let signed_prekey_signature = decode_fixed_base64::<64>(
        &request.signed_prekey_signature_b64,
        "signed_prekey_signature_b64",
    )?;

    let signing_key = VerifyingKey::from_bytes(&identity_signing_key).map_err(|_| {
        ApiError::BadRequest("identity_signing_key_b64 contains invalid Ed25519 bytes".to_string())
    })?;
    let exchange_key = X25519PublicKey::from(identity_exchange_key);
    let signed_prekey_public = X25519PublicKey::from(signed_prekey);
    let signed_prekey_sig = Signature::from_bytes(&signed_prekey_signature);
    let signed_prekey_expires_at = request
        .signed_prekey_expires_at
        .unwrap_or_else(default_signed_prekey_expiry);

    validate_signed_prekey_expiry(signed_prekey_expires_at)?;

    let bundle = PreKeyBundle {
        identity_pub: IdentityPublicKeys {
            signing_key,
            exchange_key,
        },
        signed_prekey_pub: signed_prekey_public,
        signed_prekey_sig,
        one_time_prekey_pub: None,
    };
    bundle.verify().map_err(|_| {
        ApiError::BadRequest(
            "signed_prekey_signature_b64 does not verify against the identity key".to_string(),
        )
    })?;

    let mut one_time_prekeys = Vec::with_capacity(request.one_time_prekeys_b64.len());
    let mut seen_one_time_prekeys = BTreeSet::new();
    for encoded in &request.one_time_prekeys_b64 {
        let public_key = decode_fixed_base64::<32>(encoded, "one_time_prekeys_b64 entry")?;
        if !seen_one_time_prekeys.insert(public_key) {
            return Err(ApiError::BadRequest(
                "one_time_prekeys_b64 contains duplicates".to_string(),
            ));
        }
        one_time_prekeys.push(public_key);
    }

    Ok(ParsedPreKeyBundleUpload {
        identity_signing_key,
        identity_exchange_key,
        signed_prekey,
        signed_prekey_signature,
        one_time_prekeys,
        signed_prekey_expires_at,
    })
}

fn decode_fixed_base64<const N: usize>(value: &str, field_name: &str) -> Result<[u8; N], ApiError> {
    let decoded = Base64::decode_vec(value)
        .map_err(|_| ApiError::BadRequest(format!("{field_name} must be valid base64")))?;
    decoded.try_into().map_err(|_| {
        ApiError::BadRequest(format!("{field_name} must decode to exactly {N} bytes"))
    })
}

fn default_signed_prekey_expiry() -> DateTime<Utc> {
    Utc::now() + Duration::days(7)
}

fn validate_signed_prekey_expiry(value: DateTime<Utc>) -> Result<(), ApiError> {
    if value <= Utc::now() {
        return Err(ApiError::BadRequest(
            "signed_prekey_expires_at must be in the future".to_string(),
        ));
    }

    Ok(())
}

fn random_hex_token(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn random_token_bytes(byte_len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; byte_len];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn suggested_codename(mailbox_id: &str) -> String {
    let prefixes = [
        "Ghost", "Vector", "Cipher", "Nova", "Rook", "Echo", "Onyx", "Kite",
        "Delta", "Signal", "Atlas", "Shade",
    ];
    let digest = Sha256::digest(mailbox_id.as_bytes());
    let prefix = prefixes[(digest[0] as usize) % prefixes.len()];
    let suffix = u16::from(digest[1]) * 10 + u16::from(digest[2] % 10);
    format!("{prefix}-{suffix}")
}

fn sha256_bytes(value: &[u8]) -> Vec<u8> {
    Sha256::digest(value).to_vec()
}

async fn build_s3_client(config: &RelayConfig) -> S3Client {
    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(config.media_object_store_region.clone()))
        .credentials_provider(Credentials::new(
            config.media_object_store_access_key_id.clone(),
            config.media_object_store_secret_access_key.clone(),
            None,
            None,
            "simy-relay",
        ))
        .load()
        .await;

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .endpoint_url(config.media_object_store_endpoint.clone())
        .force_path_style(true)
        .build();

    S3Client::from_conf(s3_config)
}

async fn presign_upload_request(
    client: &S3Client,
    config: &RelayConfig,
    object_key: &str,
    media_type: &str,
    ttl_seconds: i64,
) -> Result<PresignedRequestResponse, ApiError> {
    let presigned = client
        .put_object()
        .bucket(&config.media_object_store_bucket)
        .key(object_key)
        .content_type(media_type)
        .presigned(presigning_config(ttl_seconds)?)
        .await
        .map_err(|error| ApiError::Dependency(format!("media upload presign failed: {error}")))?;

    Ok(serialize_presigned_request(presigned))
}

async fn presign_media_access_request(
    client: &S3Client,
    bucket: &str,
    object_key: &str,
    media_type: &str,
    operation: &str,
    expires_at: DateTime<Utc>,
) -> Result<PresignedRequestResponse, ApiError> {
    let now = Utc::now();
    let ttl_seconds = (expires_at - now).num_seconds().max(1);

    let presigned = match operation {
        "download" => client
            .get_object()
            .bucket(bucket)
            .key(object_key)
            .response_content_type(media_type)
            .presigned(presigning_config(ttl_seconds)?)
            .await
            .map_err(|error| ApiError::Dependency(format!("media download presign failed: {error}")))?,
        "upload" => client
            .put_object()
            .bucket(bucket)
            .key(object_key)
            .content_type(media_type)
            .presigned(presigning_config(ttl_seconds)?)
            .await
            .map_err(|error| ApiError::Dependency(format!("media upload grant presign failed: {error}")))?,
        _ => {
            return Err(ApiError::BadRequest(
                "unsupported grant operation".to_string(),
            ));
        }
    };

    Ok(serialize_presigned_request(presigned))
}

fn presigning_config(ttl_seconds: i64) -> Result<PresigningConfig, ApiError> {
    PresigningConfig::expires_in(StdDuration::from_secs(ttl_seconds as u64))
        .map_err(|error| ApiError::BadRequest(format!("invalid presign ttl: {error}")))
}

fn serialize_presigned_request(request: PresignedRequest) -> PresignedRequestResponse {
    let headers = request
        .headers()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();

    PresignedRequestResponse {
        method: request.method().to_string(),
        url: request.uri().to_string(),
        headers,
    }
}

fn hash_mailbox_token(mailbox_id: &str, token: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(mailbox_id.as_bytes());
    hasher.update(b":");
    hasher.update(token);
    hasher.finalize().to_vec()
}

async fn ping_redis(client: &redis::Client) -> Result<(), ApiError> {
    let mut connection = redis_connection(client).await?;
    let response: String = redis::cmd("PING")
        .query_async(&mut connection)
        .await
        .map_err(|error| ApiError::Dependency(format!("redis ping failed: {error}")))?;

    if response == "PONG" {
        Ok(())
    } else {
        Err(ApiError::Dependency("redis ping returned unexpected response".to_string()))
    }
}

async fn enforce_rate_limit(
    client: &redis::Client,
    namespace: &str,
    limit: u32,
    window_seconds: usize,
) -> Result<(), ApiError> {
    let key = format!("relay:limit:{namespace}");
    let mut connection = redis_connection(client).await?;
    let count: i64 = redis::cmd("INCR")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .map_err(|error| ApiError::Dependency(format!("redis rate-limit increment failed: {error}")))?;

    if count == 1 {
        let _: bool = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(window_seconds)
            .query_async(&mut connection)
            .await
            .map_err(|error| ApiError::Dependency(format!("redis rate-limit expiry failed: {error}")))?;
    }

    if count > i64::from(limit) {
        return Err(ApiError::TooManyRequests(
            "rate limit exceeded".to_string(),
        ));
    }

    Ok(())
}

async fn reserve_replay_token(
    client: &redis::Client,
    mailbox_id: &str,
    replay_token: &str,
    ttl_seconds: i64,
) -> Result<bool, ApiError> {
    let key = format!("relay:replay:{mailbox_id}:{replay_token}");
    let mut connection = redis_connection(client).await?;
    let response: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg("1")
        .arg("EX")
        .arg(ttl_seconds)
        .arg("NX")
        .query_async(&mut connection)
        .await
        .map_err(|error| ApiError::Dependency(format!("redis replay reservation failed: {error}")))?;

    Ok(matches!(response.as_deref(), Some("OK")))
}

async fn redis_connection(client: &redis::Client) -> Result<MultiplexedConnection, ApiError> {
    client
        .get_multiplexed_async_connection()
        .await
        .map_err(|error| ApiError::Dependency(format!("redis connection failed: {error}")))
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("23505"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use comm_core::{IdentityKeyPair, OneTimePreKey, SignedPreKey};

    #[test]
    fn mailbox_id_rules_are_enforced() {
        assert!(validate_mailbox_id("short").is_err());
        assert!(validate_mailbox_id("valid_mailbox_id_123").is_ok());
        assert!(validate_mailbox_id("invalid/mailbox/id").is_err());
    }

    #[test]
    fn mailbox_token_requires_entropy() {
        let weak = Base64::encode_string(&[7u8; 12]);
        let strong = Base64::encode_string(&[9u8; 32]);

        assert!(decode_mailbox_token(&weak).is_err());
        assert_eq!(decode_mailbox_token(&strong).unwrap().len(), 32);
    }

    #[test]
    fn mailbox_token_hash_is_mailbox_scoped() {
        let token = [3u8; 32];
        let left = hash_mailbox_token("mailbox_a", &token);
        let right = hash_mailbox_token("mailbox_b", &token);

        assert_ne!(left, right);
    }

    #[test]
    fn replay_token_character_policy_is_enforced() {
        assert!(validate_replay_token("good-token_123456789").is_ok());
        assert!(validate_replay_token("bad token with spaces").is_err());
    }

    #[test]
    fn media_type_must_look_like_mime_type() {
        assert!(validate_media_type("image/jpeg").is_ok());
        assert!(validate_media_type("bad value").is_err());
    }

    #[test]
    fn sha256_base64_requires_32_bytes() {
        let valid = Base64::encode_string(&[1u8; 32]);
        let invalid = Base64::encode_string(&[1u8; 16]);

        assert!(validate_sha256_b64(&valid).is_ok());
        assert!(validate_sha256_b64(&invalid).is_err());
    }

    #[test]
    fn grant_operation_is_restricted() {
        assert!(validate_grant_operation("download").is_ok());
        assert!(validate_grant_operation("invalid").is_err());
    }

    #[test]
    fn grant_token_must_be_hex() {
        assert!(validate_grant_token(&"a".repeat(64)).is_ok());
        assert!(validate_grant_token("not-hex").is_err());
    }

    #[test]
    fn signed_prekey_expiry_must_be_future() {
        assert!(validate_signed_prekey_expiry(Utc::now() - Duration::seconds(1)).is_err());
        assert!(validate_signed_prekey_expiry(Utc::now() + Duration::days(7)).is_ok());
    }

    #[test]
    fn prekey_upload_parser_accepts_valid_bundle() {
        let identity = IdentityKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(&identity);
        let one_time_prekey = OneTimePreKey::generate();
        let request = PublishPreKeyBundleRequest {
            identity_signing_key_b64: Base64::encode_string(&identity.verifying_key.to_bytes()),
            identity_exchange_key_b64: Base64::encode_string(identity.exchange_public_key.as_bytes()),
            signed_prekey_b64: Base64::encode_string(signed_prekey.public_key.as_bytes()),
            signed_prekey_signature_b64: Base64::encode_string(&signed_prekey.signature.to_bytes()),
            signed_prekey_expires_at: Some(Utc::now() + Duration::days(7)),
            one_time_prekeys_b64: vec![Base64::encode_string(one_time_prekey.public_key.as_bytes())],
        };

        let parsed = parse_prekey_bundle_upload(&request).unwrap();

        assert_eq!(parsed.one_time_prekeys.len(), 1);
        assert_eq!(parsed.signed_prekey, *signed_prekey.public_key.as_bytes());
    }

    #[test]
    fn prekey_upload_parser_rejects_duplicate_one_time_prekeys() {
        let identity = IdentityKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(&identity);
        let one_time_prekey = OneTimePreKey::generate();
        let encoded_one_time_prekey = Base64::encode_string(one_time_prekey.public_key.as_bytes());
        let request = PublishPreKeyBundleRequest {
            identity_signing_key_b64: Base64::encode_string(&identity.verifying_key.to_bytes()),
            identity_exchange_key_b64: Base64::encode_string(identity.exchange_public_key.as_bytes()),
            signed_prekey_b64: Base64::encode_string(signed_prekey.public_key.as_bytes()),
            signed_prekey_signature_b64: Base64::encode_string(&signed_prekey.signature.to_bytes()),
            signed_prekey_expires_at: Some(Utc::now() + Duration::days(7)),
            one_time_prekeys_b64: vec![encoded_one_time_prekey.clone(), encoded_one_time_prekey],
        };

        assert!(matches!(
            parse_prekey_bundle_upload(&request),
            Err(ApiError::BadRequest(message)) if message.contains("duplicates")
        ));
    }

    #[test]
    fn prekey_upload_parser_rejects_bad_signature() {
        let identity = IdentityKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(&identity);
        let attacker = IdentityKeyPair::generate();
        let request = PublishPreKeyBundleRequest {
            identity_signing_key_b64: Base64::encode_string(&attacker.verifying_key.to_bytes()),
            identity_exchange_key_b64: Base64::encode_string(identity.exchange_public_key.as_bytes()),
            signed_prekey_b64: Base64::encode_string(signed_prekey.public_key.as_bytes()),
            signed_prekey_signature_b64: Base64::encode_string(&signed_prekey.signature.to_bytes()),
            signed_prekey_expires_at: Some(Utc::now() + Duration::days(7)),
            one_time_prekeys_b64: Vec::new(),
        };

        assert!(matches!(
            parse_prekey_bundle_upload(&request),
            Err(ApiError::BadRequest(message)) if message.contains("does not verify")
        ));
    }
}
