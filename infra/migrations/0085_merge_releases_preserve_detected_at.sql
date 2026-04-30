-- Update merge_releases to preserve detected_at from old releases.
-- New releases get their own detected_at; old releases that are kept retain theirs.
-- When a new release has the same title as an old one, the new release wins
-- but inherits detected_at from the old one (it's the same release, just refreshed).
CREATE OR REPLACE FUNCTION merge_releases(old_releases JSONB, new_releases JSONB)
RETURNS JSONB AS $$
DECLARE
    result JSONB := '[]';
    seen_titles TEXT[] := '{}';
    release JSONB;
    title TEXT;
    old_detected_at TEXT;
BEGIN
    -- Build a lookup of old detected_at by title
    -- First add all new releases (they take priority)
    IF new_releases IS NOT NULL AND jsonb_typeof(new_releases) = 'array' THEN
        FOR release IN SELECT * FROM jsonb_array_elements(new_releases) LOOP
            title := release->>'title';
            IF title IS NOT NULL AND NOT (title = ANY(seen_titles)) THEN
                -- If the same title existed in old releases, preserve its detected_at
                old_detected_at := NULL;
                IF old_releases IS NOT NULL AND jsonb_typeof(old_releases) = 'array' THEN
                    SELECT r->>'detected_at' INTO old_detected_at
                    FROM jsonb_array_elements(old_releases) AS r
                    WHERE r->>'title' = title
                    LIMIT 1;
                END IF;
                IF old_detected_at IS NOT NULL THEN
                    release := jsonb_set(release, '{detected_at}', to_jsonb(old_detected_at));
                END IF;
                result := result || jsonb_build_array(release);
                seen_titles := array_append(seen_titles, title);
            END IF;
        END LOOP;
    END IF;

    -- Then add old releases that aren't already in new (keep their detected_at as-is)
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
