CREATE TABLE IF NOT EXISTS book_reading_progress (
  book_id      UUID PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
  status       TEXT NOT NULL CHECK (status IN ('unread', 'reading', 'read')) DEFAULT 'unread',
  current_page INT,
  last_read_at TIMESTAMPTZ,
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_book_reading_progress_status ON book_reading_progress(status);
