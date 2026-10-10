/**
 * Client API : types DTO et fonctions fetch vers l'API Rust.
 *
 * Barrel reconstituant l'ancien module monolithique `lib/api`. Les implémentations
 * sont découpées par domaine ; importer depuis `@/lib/api` reste la norme.
 */

export * from "./client";
export * from "./libraries";
export * from "./jobs";
export * from "./tokens";
export * from "./users";
export * from "./books";
export * from "./series";
export * from "./settings";
export * from "./stats";
export * from "./metadata";
export * from "./komga";
export * from "./anilist";
export * from "./reading-lists";
export * from "./archive";
export * from "./downloads";
export * from "./telegram";
