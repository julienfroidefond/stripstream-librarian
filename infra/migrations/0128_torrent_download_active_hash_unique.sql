-- =============================================================================
-- Migration 0128: one active AllDebrid row per torrent hash
-- =============================================================================
--
-- `POST /alldebrid/add` looks for an existing active row before inserting, but
-- that check-then-act is racy: two concurrent snatches of the same magnet can
-- both pass it and upload the torrent twice. Make the database the arbiter with
-- a partial unique index covering only the states where a duplicate is harmful.
--
-- Finished rows (`error`, `imported`, ...) are deliberately excluded so the same
-- release can be re-snatched after a failure, and qBittorrent rows are left
-- entirely alone (the predicate pins `client = 'alldebrid'`).
--
-- `LOWER(qb_hash)` also serves `find_active_alldebrid_by_hash`, whose
-- case-insensitive lookup had no index before.

-- Clear any duplicate predating the constraint, keeping the newest row (the one
-- the case-insensitive lookup returns) so creating the index cannot fail on an
-- existing database. `id` breaks ties when two rows share a timestamp.
DELETE FROM torrent_downloads AS older
USING torrent_downloads AS newer
WHERE older.client = 'alldebrid'
  AND newer.client = 'alldebrid'
  AND older.qb_hash IS NOT NULL
  AND newer.qb_hash IS NOT NULL
  AND LOWER(older.qb_hash) = LOWER(newer.qb_hash)
  AND older.status IN ('downloading', 'completed', 'importing')
  AND newer.status IN ('downloading', 'completed', 'importing')
  AND (older.created_at, older.id) < (newer.created_at, newer.id);

CREATE UNIQUE INDEX torrent_downloads_alldebrid_active_hash_key
    ON torrent_downloads (LOWER(qb_hash))
    WHERE client = 'alldebrid'
      AND qb_hash IS NOT NULL
      AND status IN ('downloading', 'completed', 'importing');
