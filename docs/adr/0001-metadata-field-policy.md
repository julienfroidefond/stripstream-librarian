# Une politique déclarative des champs de métadonnées

Pour garder cohérents l'approbation, l'auto-match, le refresh et la suppression du dernier lien, `apps/api/src/metadata/field_policy.rs` centralise la politique des champs et le catalogue exposé par `GET /metadata/fields`. Ses descripteurs par entité définissent la clé JSON, la colonne, le mode de fusion, la clé de verrou, l'effacement et la présence dans les rapports ; les migrations `0139` et `0140` normalisent puis purgent les verrous hérités.

## Conséquences

- Un nouveau champ synchronisé nécessite un descripteur et, si une colonne est créée, une migration ; son extraction et sa conversion doivent aussi être raccordées à la politique.
- À l'écriture, seules les clés de verrou connues dont la valeur est le booléen `true` sont conservées.
- `cover_url` est verrouillable par l'API, mais le formulaire ne présente pas de cadenas pour ce champ.
