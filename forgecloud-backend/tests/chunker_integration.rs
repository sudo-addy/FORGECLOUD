use forgecloud_backend::engine::chunker::process_upload_stream;
use forgecloud_backend::engine::crypto::decrypt_chunk;
use forgecloud_backend::engine::storage::{StorageProvider, StoredChunkInfo};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use rand::RngExt;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncRead;

// A mock storage provider to verify what the chunker uploads
struct MockStorageProvider {
    chunks: Mutex<HashMap<String, Vec<u8>>>,
}

impl MockStorageProvider {
    fn new() -> Self {
        Self {
            chunks: Mutex::new(HashMap::new()),
        }
    }

    fn get_chunk(&self, id: &str) -> Option<Vec<u8>> {
        self.chunks.lock().unwrap().get(id).cloned()
    }
}

#[async_trait]
impl StorageProvider for MockStorageProvider {
    async fn upload_chunk(
        &self,
        chunk_id: &str,
        mut stream: Pin<Box<dyn AsyncRead + Send>>,
        size: u64,
    ) -> Result<StoredChunkInfo, anyhow::Error> {
        let mut buf = Vec::with_capacity(size as usize);
        tokio::io::copy(&mut stream, &mut buf).await?;

        self.chunks
            .lock()
            .unwrap()
            .insert(chunk_id.to_string(), buf);

        Ok(StoredChunkInfo {
            backend_chunk_id: chunk_id.to_string(),
            size_bytes: size,
        })
    }

    async fn download_chunk(
        &self,
        _backend_chunk_id: &str,
    ) -> Result<
        Pin<Box<dyn futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
        anyhow::Error,
    > {
        unimplemented!()
    }

    async fn delete_chunk(&self, _backend_chunk_id: &str) -> Result<(), anyhow::Error> {
        unimplemented!()
    }
}

fn get_random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill(&mut key);
    key
}

#[tokio::test]
async fn test_process_upload_stream_integration() {
    let key = get_random_key();
    let target_chunk_size: u64 = 10 * 1024 * 1024; // 10MB chunk target for testing

    // Let's create a 25MB file to trigger 3 chunks (10MB, 10MB, 5MB)
    let total_size = 25 * 1024 * 1024;
    let mut original_data = vec![0u8; total_size];
    rand::rng().fill(original_data.as_mut_slice());

    // We simulate a stream that yields 1MB chunks
    let stream_chunks = original_data
        .chunks(1024 * 1024)
        .map(|chunk| Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(chunk)));

    let byte_stream = stream::iter(stream_chunks);
    let mock_storage = Arc::new(MockStorageProvider::new());

    let uploaded_chunks =
        process_upload_stream(byte_stream, mock_storage.clone(), target_chunk_size, &key)
            .await
            .unwrap();

    // Should be exactly 3 chunks
    assert_eq!(uploaded_chunks.len(), 3);

    let mut reconstructed_data = Vec::with_capacity(total_size);

    for chunk_info in uploaded_chunks {
        // Retrieve encrypted bytes from mock storage
        let encrypted = mock_storage
            .get_chunk(&chunk_info.backend_chunk_id)
            .unwrap();

        // Decrypt it
        let decrypted = decrypt_chunk(&encrypted, &key).unwrap();

        // Append to reconstructed
        reconstructed_data.extend_from_slice(&decrypted);
    }

    // Verify the entire pipeline roundtrip
    assert_eq!(original_data.len(), reconstructed_data.len());
    assert_eq!(original_data, reconstructed_data);
}
