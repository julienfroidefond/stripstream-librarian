-- Backfill volume_type for books whose title was cleaned by the parser
-- (HS pattern removed from title) but the original filename still contains it.

UPDATE books SET volume_type = 'hs'
WHERE volume_type = 'regular'
  AND EXISTS (
    SELECT 1 FROM book_files bf WHERE bf.book_id = books.id
    AND (
      bf.abs_path ~* '\mhs\M'
      OR bf.abs_path ~* '\mhors[- ]?s[eé]rie\M'
      OR bf.abs_path ~* '\msp[eé]cial\M'
      OR bf.abs_path ~* '\mbonus\M'
    )
  );
