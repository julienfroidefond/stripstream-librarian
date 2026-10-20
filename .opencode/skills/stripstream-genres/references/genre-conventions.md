# Genre conventions — Stripstream

Snapshot taken on the production library (2026-10-20). Re-run `GET /genres` before
relying on the counts; re-run the reference queries before copying a genre set.

## Taxonomy (25 free tags)

Emoji prefix and accents are part of the name. Copy them verbatim.

| Genre | Series |
| ----- | ------ |
| ☢️ Post-apocalyptique | 12 |
| ⚔️ Action | 119 |
| ⛩️ Isekai | 1 |
| ⭐ Jeunesse | 12 |
| 🌙 Fantastique | 12 |
| 🎭 Drame | 38 |
| 🏆 Sport | 4 |
| 🏛️ Mythologie | 1 |
| 🐉 Heroic Fantasy | 7 |
| 💀 Horreur | 23 |
| 💕 Romance | 1 |
| 💥 Comics | 393 |
| 📜 Historique | 15 |
| 🔍 Mystère | 19 |
| 🔎 Policier | 7 |
| 🔞 Mature | 90 |
| 🕵️ Espionnage | 4 |
| 🗺️ Aventure | 87 |
| 😄 Comédie | 38 |
| 😱 Thriller | 31 |
| 🚀 Science-Fiction | 45 |
| 🥋 Arts martiaux | 4 |
| 🦸 Super-héros | 56 |
| 🧙 Fantasy | 86 |
| 🧠 Psychologique | 22 |

## Decision rules

- **Heroic Fantasy vs Fantasy** — `🐉 Heroic Fantasy` is reserved for the
  D&D-adjacent sword-and-sorcery BDs (`Guerres d'Arran`, `Chroniques de la Lune
  Noire`, `Trolls de Troy`-like). `🧙 Fantasy` is the default for manga and for
  most BD fantasy. Terres d'Arran series are inconsistent today
  (`Guerres d'Arran` → Heroic Fantasy, `Orcs & Gobelins`/`Mages` → Fantasy), so
  `Elfes` and `Nains` carry both.
- **Action + Fantasy + Mature** is the standard manga dark-fantasy trio
  (`Berserk`, `Claymore`, `Chainsaw Man`). `Chainsaw Man` also carries Horreur.
- **Mature** = explicit violence/sex/nihilism, not "adult audience" in the
  marketing sense. Comics with dark content (`Sin City`, `Old man Logan`) carry
  `💥 Comics` + `🔞 Mature`.
- **Horreur** requires monsters/gore as the core premise (`Tokyo Ghoul`,
  `Chainsaw Man`, `Dragon Head`), not just violence.
- **Psychologique** for introspection and mental-health arcs (`Monster`,
  `Le Combat ordinaire`).
- **Policier / Mystère / Thriller / Espionnage** — `Monster` (Thriller, Policier,
  Psychologique, Mystère, Mature) is the canonical multi-axis example. Use
  `🕵️ Espionnage` only for spy fiction (`XIII`, `Spy x Family`).
- **Comics** (`💥 Comics`) is applied to every US comic, including superhero
  books that also carry `🦸 Super-héros`.
- **BD franco-belge** never carries `💥 Comics`; use Drame/Aventure/Historique/
  Fantasy/Science-Fiction instead.
- **Science-Fiction** for space opera, dystopia, hard SF, and near-future tech
  (`L'Incal`, `Noir horizon`, `Sword Art Online`). Add
  `☢️ Post-apocalyptique` only when the apocalypse is the setting.
- **Jeunesse** when the work is genuinely aimed at children (`5 mondes`), not for
  every all-ages BD.

## Reference sets (query before mirroring)

```bash
curl -s --get --data-urlencode "genre=🧙 Fantasy" --data-urlencode "limit=10" \
  -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" "$STRIPSTREAM_API_URL/series"
```

| Reference | Genres |
| --------- | ------ |
| Guerres d'Arran | 🐉 Heroic Fantasy |
| Orcs & Gobelins, Mages | 🧙 Fantasy |
| Berserk, Claymore | ⚔️ Action, 🧙 Fantasy, 🔞 Mature |
| Chainsaw Man | ⚔️ Action, 🧙 Fantasy, 💀 Horreur, 🔞 Mature |
| Monster | 😱 Thriller, 🔎 Policier, 🧠 Psychologique, 🔍 Mystère, 🔞 Mature |
| Vinland Saga | ⚔️ Action, 🗺️ Aventure, 📜 Historique, 🔞 Mature |
| Old man Logan | 💥 Comics, ⚔️ Action, 🦸 Super-héros, 🎭 Drame, ☢️ Post-apocalyptique, 🔞 Mature |
| Batman Saga | 💥 Comics, ⚔️ Action, 🦸 Super-héros, 🗺️ Aventure, 🔎 Policier, 😱 Thriller |

## Decisions already made (2026-10-20 prod pass)

Applied to the 15 series that were untagged:

| Series | Genres |
| ------ | ------ |
| Elfes, Nains | 🐉 Heroic Fantasy, 🧙 Fantasy, 🗺️ Aventure |
| La Confrérie des tempêtes | 🐉 Heroic Fantasy, 🗺️ Aventure |
| L'Incal | 🚀 Science-Fiction, 🗺️ Aventure, 🔞 Mature |
| L'académie Clair-Obscur, La Horde du Contrevent, Horde du contrevent La 04 Alticcio Henninot | 🧙 Fantasy, 🗺️ Aventure |
| Le Combat ordinaire | 🎭 Drame, 🧠 Psychologique |
| Noir horizon | ☢️ Post-apocalyptique, 🚀 Science-Fiction, 😱 Thriller |
| Sin City | 💥 Comics, 🔞 Mature, 🔎 Policier, 😱 Thriller |
| Akame Ga Kill ! Zero, Red Eyes Sword : Akame ga Kill ! | ⚔️ Action, 🧙 Fantasy, 🔞 Mature |
| Sword Art Online | ⛩️ Isekai, ⚔️ Action, 🧙 Fantasy |
| Tokyo Ghoul | ⚔️ Action, 💀 Horreur, 🧠 Psychologique, 🔞 Mature |
| Les Seigneurs Mages | 🧙 Fantasy |

## Known orphan / duplicate candidates

Left untagged-by-decision (tagged, but flagged for cleanup):

- `Les Seigneurs Mages` — 0 book, duplicate of `Mages`.
- `Red Eyes Sword : Akame ga Kill !` — 0 book, mother entry of `Akame Ga Kill ! Zero`.
- `Sword Art Online` — 0 book.
- `Horde du contrevent La 04 Alticcio Henninot` — directory-name artifact,
  duplicate of `La Horde du Contrevent`.

## Gotchas

- `genres` is a free-text array on `series`; deleting a genre from every series
  makes it disappear from `/genres` automatically.
- Metadata refresh overwrites genres unless the `genres` field is locked on the
  series (`locked_fields`) — mention this when a user wants durable manual tags.
- `POST /genres/assign` adds; `PATCH /series/{id}` replaces.
- `GET /genres/untagged-series` caps at 500 results.
