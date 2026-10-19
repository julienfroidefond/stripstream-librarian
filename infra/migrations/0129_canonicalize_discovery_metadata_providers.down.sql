-- The original discovery source id (`senscritique_bd`, `sc_trending_bd`, …) is
-- not recoverable once links are merged onto `senscritique`, and duplicate links
-- deleted by the up migration cannot be restored. Nothing to do.
SELECT 1;
