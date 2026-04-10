-- Backfill volume_type for intégrale books (title or filename patterns).

UPDATE books SET volume_type = 'integral'
WHERE volume_type IN ('regular', 'hs')
  AND (
    title ~* '\mint[eé]grale?\M'
    OR title ~* '\minths?\M'
    OR title ~* '\mint\M'
    OR EXISTS (
      SELECT 1 FROM book_files bf WHERE bf.book_id = books.id
      AND (
        bf.abs_path ~* '\mint[eé]grale?\M'
        OR bf.abs_path ~* '\minths?\M'
        OR bf.abs_path ~* '\mint\M'
      )
    )
  );
