-- Migration: Progression temps reel et erreurs d'indexation (T19 - Option 2)

-- Colonnes pour la progression temps reel dans index_jobs
ALTER TABLE index_jobs 
  ADD COLUMN IF NOT EXISTS current_file TEXT,
  ADD COLUMN IF NOT EXISTS progress_percent INTEGER CHECK (progress_percent >= 0 AND progress_percent <= 100),
  ADD COLUMN IF NOT EXISTS total_files INTEGER,
  ADD COLUMN IF NOT EXISTS processed_files INTEGER DEFAULT 0;

-- Table pour stocker les erreurs d'indexation
CREATE TABLE IF NOT EXISTS index_job_errors (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  job_id UUID NOT NULL REFERENCES index_jobs(id) ON DELETE CASCADE,
  file_path TEXT NOT NULL,
  error_message TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_index_job_errors_job_id ON index_job_errors(job_id);
CREATE INDEX IF NOT EXISTS idx_index_job_errors_created_at ON index_job_errors(created_at);

COMMENT ON TABLE index_job_errors IS 'Stocke uniquement les erreurs d indexation pour debug';
COMMENT ON COLUMN index_jobs.current_file IS 'Fichier en cours de traitement (pour affichage temps reel)';
COMMENT ON COLUMN index_jobs.progress_percent IS 'Pourcentage de completion estime';
COMMENT ON COLUMN index_jobs.total_files IS 'Nombre total de fichiers a traiter (estimation)';
COMMENT ON COLUMN index_jobs.processed_files IS 'Nombre de fichiers deja traites';
