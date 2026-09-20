-- bedetheque.com is Cloudflare-blocked and the provider has been removed from
-- the codebase. Migration 0113 rewrote library settings but left existing
-- external_metadata_links rows pointing at the retired provider, which would
-- still be re-scraped by refresh/sync paths. Force-unlink them here.
--
-- Series metadata fields are intentionally left untouched: only the link is
-- removed, so no denormalized data is lost.
DELETE FROM external_metadata_links WHERE provider = 'bedetheque';
