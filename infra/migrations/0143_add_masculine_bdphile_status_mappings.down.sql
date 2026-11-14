-- Series statuses re-normalized by the up migration are left as-is.
DELETE FROM status_mappings
WHERE provider_status IN ('terminé', 'annulé', 'abandonné', 'abandonnée', 'suspendu');
