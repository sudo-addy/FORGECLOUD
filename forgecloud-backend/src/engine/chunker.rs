use std::pin::Pin;
use std::sync::Arc;

use futures_util::StreamExt;
use tokio::io::AsyncRead;
use uuid::Uuid;

use crate::engine::crypto;
use crate::engine::storage::{StorageProvider, StoredChunkInfo};

/// Consumes a stream of bytes, buffering them up to `target_chunk_size`,
/// encrypts each chunk with the provided key, and uploads the ciphertext
/// using the provided `StorageProvider`.
pub async fn process_upload_stream<S, E>(
    mut stream: S,
    storage: Arc<dyn StorageProvider>,
    target_chunk_size: u64,
    master_key: &[u8; 32],
) -> Result<Vec<StoredChunkInfo>, anyhow::Error>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut uploaded_chunks = Vec::new();

    // Bounded memory buffer to accumulate chunks sequentially
    let mut buffer = Vec::with_capacity(target_chunk_size as usize);

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(anyhow::Error::new)?;
        buffer.extend_from_slice(&chunk);

        // Once accumulated bytes reach the target memory boundary
        if buffer.len() as u64 >= target_chunk_size {
            let info = encrypt_and_upload(&buffer, &storage, master_key).await?;
            uploaded_chunks.push(info);

            // Clear the buffer memory cleanly without reallocation
            buffer.clear();
        }
    }

    // Process any final remaining bytes as the last chunk
    if !buffer.is_empty() {
        let info = encrypt_and_upload(&buffer, &storage, master_key).await?;
        uploaded_chunks.push(info);

        buffer.clear();
    }

    Ok(uploaded_chunks)
}

/// Encrypts a plaintext buffer and uploads the ciphertext to the storage backend.
async fn encrypt_and_upload(
    plaintext: &[u8],
    storage: &Arc<dyn StorageProvider>,
    master_key: &[u8; 32],
) -> Result<StoredChunkInfo, anyhow::Error> {
    let encrypted = crypto::encrypt_chunk(plaintext, master_key)?;
    let chunk_id = Uuid::new_v4().to_string();
    let size = encrypted.len() as u64;

    // Wrap the owned encrypted Vec into a Cursor for async reading.
    let reader = std::io::Cursor::new(encrypted);
    let reader_box: Pin<Box<dyn AsyncRead + Send>> = Box::pin(reader);

    storage.upload_chunk(&chunk_id, reader_box, size).await
}
