CREATE TABLE IF NOT EXISTS libraries (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL,
  root_path TEXT NOT NULL UNIQUE,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS books (
  id UUID PRIMARY KEY,
  library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('ebook', 'comic', 'bd')),
  title TEXT NOT NULL,
  author TEXT,
  series TEXT,
  volume TEXT,
  language TEXT,
  page_count INT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS book_files (
  id UUID PRIMARY KEY,
  book_id UUID NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  format TEXT NOT NULL CHECK (format IN ('pdf', 'cbz', 'cbr')),
  abs_path TEXT NOT NULL UNIQUE,
  size_bytes BIGINT NOT NULL,
  mtime TIMESTAMPTZ NOT NULL,
  fingerprint TEXT NOT NULL,
  checksum_opt TEXT,
  parse_status TEXT NOT NULL DEFAULT 'pending' CHECK (parse_status IN ('pending', 'ok', 'error')),
  parse_error_opt TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS index_jobs (
  id UUID PRIMARY KEY,
  library_id UUID REFERENCES libraries(id) ON DELETE SET NULL,
  type TEXT NOT NULL CHECK (type IN ('scan', 'rebuild')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'failed')),
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ,
  stats_json JSONB,
  error_opt TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS api_tokens (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL,
  prefix TEXT NOT NULL UNIQUE,
  token_hash TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('admin', 'read')),
  last_used_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_books_library_id ON books(library_id);
CREATE INDEX IF NOT EXISTS idx_book_files_book_id ON book_files(book_id);
CREATE INDEX IF NOT EXISTS idx_book_files_parse_status ON book_files(parse_status);
CREATE INDEX IF NOT EXISTS idx_index_jobs_status ON index_jobs(status);
CREATE INDEX IF NOT EXISTS idx_api_tokens_scope ON api_tokens(scope);
