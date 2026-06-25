-- Migration to add upload sessions, pending session chunks tables, and sha256_hash column to files
ALTER TABLE files ADD COLUMN IF NOT EXISTS sha256_hash VARCHAR(64);

CREATE TABLE IF NOT EXISTS upload_sessions (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    total_size BIGINT NOT NULL,
    mime_type TEXT,
    folder_id UUID REFERENCES folders(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'Created' CHECK (status IN ('Created', 'Uploading', 'Paused', 'PendingCommit', 'Completed', 'Failed', 'Abandoned')),
    sha256_hash VARCHAR(64), -- Client-supplied file-level integrity hash (optional comparison)
    server_sha256 VARCHAR(64), -- Server-computed file-level integrity hash
    owner_api_key TEXT NOT NULL, -- Track which API key owns this session
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS pending_session_chunks (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES upload_sessions(id) ON DELETE CASCADE,
    chunk_number INT NOT NULL,
    backend_chunk_id TEXT NOT NULL,
    size_bytes BIGINT NOT NULL, -- Plaintext chunk size
    encrypted_size BIGINT NOT NULL, -- Ciphertext chunk size
    chunk_sha256 VARCHAR(64) NOT NULL, -- Plaintext chunk SHA-256 hash
    storage_provider TEXT NOT NULL, -- Storage backend used
    retry_count INT NOT NULL DEFAULT 0,
    upload_timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(session_id, chunk_number)
);

CREATE INDEX IF NOT EXISTS idx_pending_session_chunks_session_id ON pending_session_chunks(session_id);
