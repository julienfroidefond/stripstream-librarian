-- =============================================================================
-- Migration 0124: service heartbeats
-- =============================================================================
--
-- The API and the indexer run as separate containers. Until now nothing let the
-- backoffice tell whether the indexer was still alive: an empty job queue looks
-- exactly like a healthy idle system. Each process now upserts its own row every
-- 30 seconds, and the diagnostics page reports a service offline when its
-- `last_seen_at` is older than 90 seconds.

CREATE TABLE service_heartbeats (
    service TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
