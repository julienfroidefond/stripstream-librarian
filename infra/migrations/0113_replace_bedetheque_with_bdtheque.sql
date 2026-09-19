-- bedetheque.com is now protected by a Cloudflare JS challenge and can no longer
-- be scraped. Bédéthèque is removed from the exposed providers; migrate remaining
-- configuration to the BDTheque provider (bdtheque.com).
UPDATE libraries
SET metadata_provider = 'bdtheque'
WHERE metadata_provider = 'bedetheque';

UPDATE libraries
SET fallback_metadata_provider = 'bdtheque'
WHERE fallback_metadata_provider = 'bedetheque';

UPDATE app_settings
SET value = jsonb_set(value, '{default_provider}', '"bdtheque"'::jsonb),
    updated_at = CURRENT_TIMESTAMP
WHERE key = 'metadata_providers'
  AND value->>'default_provider' = 'bedetheque';
