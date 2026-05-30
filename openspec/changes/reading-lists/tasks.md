# Reading Lists — Tasks

## 1. Migration DB
- [ ] Créer `infra/migrations/XXXX_reading_lists.sql` (up + down)
- [ ] Tables `reading_lists` + `reading_list_items` avec index

## 2. API Rust — types & handlers
- [ ] `apps/api/src/reading_lists/mod.rs` — types DTO + handlers CRUD admin
- [ ] `apps/api/src/reading_lists/client.rs` — handlers lecture client
- [ ] Enregistrer les routes dans `main.rs` (admin + client scopes)
- [ ] Ajouter `#[derive(ToSchema)]` pour OpenAPI dual-spec

## 3. API Next.js — routes proxy
- [ ] `apps/backoffice/app/api/reading-lists/route.ts` (list + create)
- [ ] `apps/backoffice/app/api/reading-lists/[id]/route.ts` (get + patch + delete)
- [ ] `apps/backoffice/app/api/reading-lists/[id]/series/route.ts` (add)
- [ ] `apps/backoffice/app/api/reading-lists/[id]/series/[seriesId]/route.ts` (remove)
- [ ] `apps/backoffice/app/api/reading-lists/[id]/series/reorder/route.ts` (PUT)

## 4. Types TS
- [ ] Ajouter `ReadingListDto`, `ReadingListDetailDto`, `ReadingListSeriesDto` dans `lib/api.ts`

## 5. UI Backoffice
- [ ] Page liste `/reading-lists` — tableau + bouton créer
- [ ] Page détail `/reading-lists/[id]` — items réordonnables (up/down), recherche série, suppression
- [ ] Entrée dans la navigation sidebar

## 6. i18n
- [ ] Clés FR + EN pour les labels reading lists

## 7. Tests
- [ ] Tests unitaires Rust pour le réordonnement
