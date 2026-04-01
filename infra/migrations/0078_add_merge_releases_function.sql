-- Merge two JSONB arrays of releases, deduplicating by title.
-- New releases override old ones with the same title.
CREATE OR REPLACE FUNCTION merge_releases(old_releases JSONB, new_releases JSONB)
RETURNS JSONB AS $$
DECLARE
    result JSONB := '[]';
    seen_titles TEXT[] := '{}';
    release JSONB;
    title TEXT;
BEGIN
    -- First add all new releases
    IF new_releases IS NOT NULL AND jsonb_typeof(new_releases) = 'array' THEN
        FOR release IN SELECT * FROM jsonb_array_elements(new_releases) LOOP
            title := release->>'title';
            IF title IS NOT NULL AND NOT (title = ANY(seen_titles)) THEN
                result := result || jsonb_build_array(release);
                seen_titles := array_append(seen_titles, title);
            END IF;
        END LOOP;
    END IF;

    -- Then add old releases that aren't already in new
    IF old_releases IS NOT NULL AND jsonb_typeof(old_releases) = 'array' THEN
        FOR release IN SELECT * FROM jsonb_array_elements(old_releases) LOOP
            title := release->>'title';
            IF title IS NOT NULL AND NOT (title = ANY(seen_titles)) THEN
                result := result || jsonb_build_array(release);
                seen_titles := array_append(seen_titles, title);
            END IF;
        END LOOP;
    END IF;

    RETURN result;
END;
$$ LANGUAGE plpgsql IMMUTABLE;
