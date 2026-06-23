CREATE TABLE files (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    total_size BIGINT NOT NULL,
    mime_type TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE chunks (
    id UUID PRIMARY KEY,
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    chunk_number INT NOT NULL,
    backend_chunk_id TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(file_id, chunk_number)
);

CREATE INDEX idx_chunks_file_id ON chunks(file_id);
