# Stripstream Librarian

Gestionnaire auto-hébergé de bibliothèque de BD, mangas et ebooks : indexation automatique
(CBZ/CBR/PDF/EPUB), recherche plein texte, enrichissement de métadonnées multi-providers,
découverte et téléchargement.

## Écosystème

- **Lecteur officiel** — [julienfroidefond/stripstream](https://github.com/julienfroidefond/stripstream) : application de lecture séparée qui consomme l'API Client (scope `read`).

## Documentation

La documentation de référence vit dans [`apps/docs/`](apps/docs) (site Astro/Starlight), servie
sur http://localhost:7084. Principales entrées :

- [Introduction](apps/docs/src/content/docs/getting-started/introduction.md)
- [Installation](apps/docs/src/content/docs/getting-started/installation.md)
- [Configuration](apps/docs/src/content/docs/getting-started/configuration.md)
- [Variables d'environnement](apps/docs/src/content/docs/reference/environment.md)
- [Référence API](apps/docs/src/content/docs/reference/api.md)

## Démarrage rapide

```bash
cp .env.example .env   # renseigner API_BOOTSTRAP_TOKEN, ADMIN_USERNAME, ADMIN_PASSWORD, SESSION_SECRET
docker compose up -d
```

| Service | URL |
|---------|-----|
| Backoffice | http://localhost:7082 |
| API (Swagger UI) | http://localhost:7080/swagger-ui |
| Documentation | http://localhost:7084 |

## Développement

Voir [`AGENTS.md`](AGENTS.md) pour les commandes de build, lint et test, ainsi que les
conventions du dépôt.

## Licence

[MIT](LICENSE).
