use std::path::PathBuf;
use std::pin::Pin;

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;
use tracing::instrument;

// ---------------------------------------------------------------------------
// StoredChunkInfo — metadata returned after a successful upload
// ---------------------------------------------------------------------------

use serde::Serialize;

/// Metadata returned after a chunk has been persisted to a storage backend.
#[derive(Debug, Clone, Serialize)]
pub struct StoredChunkInfo {
    /// Opaque identifier the backend uses to reference this chunk.
    pub backend_chunk_id: String,
    /// Number of bytes that were actually written.
    pub size_bytes: u64,
}

// ---------------------------------------------------------------------------
// StorageProvider trait
// ---------------------------------------------------------------------------

/// A thread-safe, sendable async trait that abstracts chunk-level storage
/// operations (upload, download, delete) behind a backend-agnostic interface.
#[async_trait]
pub trait StorageProvider: Send + Sync {
    /// Persist a chunk from an async byte stream.
    ///
    /// * `chunk_id`  – logical identifier for the chunk.
    /// * `stream`    – async reader supplying the raw bytes.
    /// * `size`      – expected size in bytes (used for pre-allocation / validation).
    async fn upload_chunk(
        &self,
        chunk_id: &str,
        stream: Pin<Box<dyn AsyncRead + Send>>,
        size: u64,
    ) -> Result<StoredChunkInfo, anyhow::Error>;

    /// Retrieve a chunk as an async byte stream.
    ///
    /// * `backend_chunk_id` – the id returned from a previous `upload_chunk`.
    async fn download_chunk(
        &self,
        backend_chunk_id: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, anyhow::Error>;

    /// Permanently remove a chunk from the backend.
    ///
    /// * `backend_chunk_id` – the id returned from a previous `upload_chunk`.
    async fn delete_chunk(&self, backend_chunk_id: &str) -> Result<(), anyhow::Error>;
}

// ---------------------------------------------------------------------------
// LocalStorageProvider — file-system backed implementation
// ---------------------------------------------------------------------------

/// Stores chunks as individual files inside a configured base directory.
///
/// Uses `tokio::fs::File` for async I/O and `tokio_util::io::ReaderStream`
/// for zero-copy streaming reads.
#[derive(Debug, Clone)]
pub struct LocalStorageProvider {
    base_dir: PathBuf,
}

impl LocalStorageProvider {
    /// Create a new provider rooted at `base_dir`.
    ///
    /// The directory is created (recursively) if it does not already exist.
    pub async fn new(base_dir: impl Into<PathBuf>) -> Result<Self, anyhow::Error> {
        let base_dir = base_dir.into();
        tokio::fs::create_dir_all(&base_dir).await?;
        Ok(Self { base_dir })
    }

    /// Resolve a chunk id to its full filesystem path.
    fn chunk_path(&self, chunk_id: &str) -> PathBuf {
        self.base_dir.join(chunk_id)
    }
}

#[async_trait]
impl StorageProvider for LocalStorageProvider {
    #[instrument(skip(self, stream), fields(provider = "local"))]
    async fn upload_chunk(
        &self,
        chunk_id: &str,
        mut stream: Pin<Box<dyn AsyncRead + Send>>,
        _size: u64,
    ) -> Result<StoredChunkInfo, anyhow::Error> {
        let path = self.chunk_path(chunk_id);

        let mut file = tokio::fs::File::create(&path).await?;
        let bytes_written = tokio::io::copy(&mut stream, &mut file).await?;

        Ok(StoredChunkInfo {
            backend_chunk_id: chunk_id.to_owned(),
            size_bytes: bytes_written,
        })
    }

    #[instrument(skip(self), fields(provider = "local"))]
    async fn download_chunk(
        &self,
        backend_chunk_id: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, anyhow::Error>
    {
        let path = self.chunk_path(backend_chunk_id);
        let file = tokio::fs::File::open(&path).await?;

        // ReaderStream wraps an AsyncRead into a Stream<Item = Result<Bytes, io::Error>>
        // with an internal buffer — no extra copies required.
        let stream = ReaderStream::new(file);

        Ok(Box::pin(stream))
    }

    #[instrument(skip(self), fields(provider = "local"))]
    async fn delete_chunk(&self, backend_chunk_id: &str) -> Result<(), anyhow::Error> {
        let path = self.chunk_path(backend_chunk_id);
        tokio::fs::remove_file(&path).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// TelegramStorageProvider — Telegram Bot API backed implementation
// ---------------------------------------------------------------------------

/// Stores chunks as Telegram documents via the Bot API.
///
/// Each chunk is uploaded as a document to a target chat, and the Telegram
/// `file_id` is returned as the `backend_chunk_id`. Downloads stream the
/// file binary directly from the Telegram file server.
#[derive(Debug, Clone)]
pub struct TelegramStorageProvider {
    client: reqwest::Client,
    bot_token: String,
    chat_id: String,
    api_base_url: String,
}

// -- Telegram Bot API response structures --

#[derive(Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    description: Option<String>,
    result: Option<T>,
}

#[derive(Deserialize)]
struct TelegramDocument {
    file_id: String,
}

#[derive(Deserialize)]
struct TelegramMessage {
    document: Option<TelegramDocument>,
}

#[derive(Deserialize)]
struct TelegramFile {
    file_path: Option<String>,
}

impl TelegramStorageProvider {
    /// Create a new provider pointing at the given Telegram Bot API gateway.
    pub fn new(
        bot_token: impl Into<String>,
        chat_id: impl Into<String>,
        api_base_url: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
            api_base_url: api_base_url.into(),
        }
    }

    /// Build the base URL for a Bot API method.
    fn method_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.api_base_url, self.bot_token, method)
    }

    /// Build the URL for downloading a file by its `file_path`.
    fn file_url(&self, file_path: &str) -> String {
        format!("{}/file/bot{}/{}", self.api_base_url, self.bot_token, file_path)
    }
}

#[async_trait]
impl StorageProvider for TelegramStorageProvider {
    #[instrument(skip(self, stream), fields(provider = "telegram"))]
    async fn upload_chunk(
        &self,
        chunk_id: &str,
        mut stream: Pin<Box<dyn AsyncRead + Send>>,
        size: u64,
    ) -> Result<StoredChunkInfo, anyhow::Error> {
        // Read the chunk data into memory — already bounded by the chunker
        // to a single chunk (≤ 512 MB encrypted).
        let mut buf = Vec::with_capacity(size as usize);
        tokio::io::copy(&mut stream, &mut buf).await?;
        let actual_size = buf.len() as u64;

        // Build a multipart form with the document file and chat_id.
        let file_part = reqwest::multipart::Part::bytes(buf)
            .file_name(chunk_id.to_owned())
            .mime_str("application/octet-stream")?;

        let form = reqwest::multipart::Form::new()
            .text("chat_id", self.chat_id.clone())
            .part("document", file_part);

        let resp = self
            .client
            .post(self.method_url("sendDocument"))
            .multipart(form)
            .send()
            .await?;

        let status = resp.status();
        let body: TelegramResponse<TelegramMessage> = resp.json().await?;

        if !body.ok {
            return Err(anyhow!(
                "Telegram sendDocument failed (HTTP {}): {}",
                status,
                body.description.unwrap_or_default()
            ));
        }

        let file_id = body
            .result
            .and_then(|msg| msg.document)
            .map(|doc| doc.file_id)
            .ok_or_else(|| anyhow!("Telegram response missing document.file_id"))?;

        Ok(StoredChunkInfo {
            backend_chunk_id: file_id,
            size_bytes: actual_size,
        })
    }

    #[instrument(skip(self), fields(provider = "telegram"))]
    async fn download_chunk(
        &self,
        backend_chunk_id: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, anyhow::Error>
    {
        // Step 1: Resolve file_id → file_path via getFile.
        let get_file_url = format!(
            "{}?file_id={}",
            self.method_url("getFile"),
            backend_chunk_id
        );
        let resp: TelegramResponse<TelegramFile> =
            self.client.get(&get_file_url).send().await?.json().await?;

        if !resp.ok {
            return Err(anyhow!(
                "Telegram getFile failed: {}",
                resp.description.unwrap_or_default()
            ));
        }

        let file_path = resp
            .result
            .and_then(|f| f.file_path)
            .ok_or_else(|| anyhow!("Telegram getFile response missing file_path"))?;

        // Step 2: Stream the raw binary from the file download endpoint.
        let download_url = self.file_url(&file_path);
        let file_resp = self.client.get(&download_url).send().await?;

        if !file_resp.status().is_success() {
            return Err(anyhow!(
                "Telegram file download failed with HTTP {}",
                file_resp.status()
            ));
        }

        // Stream the response body directly — zero buffering.
        let stream = file_resp.bytes_stream().map(|result| {
            result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        });

        Ok(Box::pin(stream))
    }

    #[instrument(skip(self), fields(provider = "telegram"))]
    async fn delete_chunk(&self, _backend_chunk_id: &str) -> Result<(), anyhow::Error> {
        // Telegram Bot API does not expose a delete-file endpoint.
        // Chunks are effectively immutable once sent.
        // This is a no-op; cleanup is handled at the chat level if needed.
        tracing::warn!("Telegram does not support file deletion via Bot API — skipping");
        Ok(())
    }
}
