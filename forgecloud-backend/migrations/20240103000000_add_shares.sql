CREATE TABLE shares (
    id UUID PRIMARY KEY,
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    password_hash TEXT,
    expires_at TIMESTAMPTZ,
    max_downloads INT,
    download_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_shares_token ON shares(token);
CREATE INDEX idx_shares_file_id ON shares(file_id);
