-- Revert 0124: drop the service liveness heartbeats.
DROP TABLE IF EXISTS service_heartbeats;
