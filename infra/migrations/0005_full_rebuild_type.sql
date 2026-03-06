-- Migration: Ajout du type de job "full_rebuild" pour réindexation complète

ALTER TABLE index_jobs 
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check 
    CHECK (type IN ('scan', 'rebuild', 'full_rebuild'));
