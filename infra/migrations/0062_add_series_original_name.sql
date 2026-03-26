-- Track the filesystem-derived series name so the scanner can map
-- renamed series back to their current DB name.
-- When a user renames series "A" → "B", original_name stores "A" (the directory name).
ALTER TABLE series_metadata ADD COLUMN original_name TEXT;

-- Back-fill original_name for series that were already renamed:
-- compare the DB series name with the actual directory name from book_files.abs_path.
-- If they differ, the series was renamed and we record the filesystem name.
UPDATE series_metadata sm
SET original_name = fs.fs_series
FROM (
    SELECT DISTINCT ON (b.library_id, b.series)
        b.library_id,
        b.series,
        -- First path component after the library root = filesystem series name
        split_part(
            ltrim(
                replace(bf.abs_path, l.root_path, ''),
                '/'
            ),
            '/',
            1
        ) AS fs_series
    FROM books b
    JOIN book_files bf ON bf.book_id = b.id
    JOIN libraries l ON l.id = b.library_id
    WHERE b.series IS NOT NULL AND b.series != ''
) fs
WHERE sm.library_id = fs.library_id
  AND sm.name = fs.series
  AND fs.fs_series != ''
  AND fs.fs_series != fs.series;
