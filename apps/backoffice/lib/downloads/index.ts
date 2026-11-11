/**
 * Deep downloads module: one read model for the three sources.
 *
 * Source DTOs stay in `lib/api`; this module owns their adaptation, merge,
 * filter and grouping semantics, plus the SSE subscription hook.
 */

export * from "./model";
export * from "./available";
export * from "./status";
export * from "./stream";
export * from "./useDownloadsStream";
