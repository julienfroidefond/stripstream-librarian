-- Normalize series_metadata.status values from provider-specific strings to standard enum values
UPDATE series_metadata SET status = 'ongoing' WHERE LOWER(status) LIKE '%en cours%';
UPDATE series_metadata SET status = 'ended' WHERE LOWER(status) LIKE '%finie%' OR LOWER(status) LIKE '%terminée%';
UPDATE series_metadata SET status = 'hiatus' WHERE LOWER(status) LIKE '%hiatus%' OR LOWER(status) LIKE '%suspendue%';
UPDATE series_metadata SET status = 'cancelled' WHERE LOWER(status) LIKE '%annulée%' OR LOWER(status) LIKE '%arrêtée%';
UPDATE series_metadata SET status = 'upcoming' WHERE LOWER(status) LIKE '%not_yet_released%';
UPDATE series_metadata SET status = 'ongoing' WHERE LOWER(status) = 'releasing';
UPDATE series_metadata SET status = 'ended' WHERE LOWER(status) = 'finished';
