-- Backfill volume_type for existing books with HS patterns in their title.
-- Uses PostgreSQL regex word boundaries (\m = start, \M = end) for safe matching.

UPDATE books SET volume_type = 'hs'
WHERE volume_type = 'regular'
  AND (
    title ~* '\mhs\M'
    OR title ~* '\mhors[- ]?s[eé]rie\M'
    OR title ~* '\msp[eé]cial\M'
    OR title ~* '\mbonus\M'
  );
