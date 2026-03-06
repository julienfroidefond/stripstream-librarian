-- Migration: Ajout du watcher de fichiers

ALTER TABLE libraries 
  ADD COLUMN IF NOT EXISTS watcher_enabled BOOLEAN NOT NULL DEFAULT FALSE;

COMMENT ON COLUMN libraries.watcher_enabled IS 'Active le watcher de fichiers en temps reel (notify)';