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
- [Changelog](CHANGELOG.md) — historique des versions, généré à chaque release.

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

## Avertissement légal

Stripstream Librarian est un logiciel d'auto-hébergement : il **ne fournit aucune œuvre** et
n'héberge aucun catalogue. Vous êtes seul responsable des fichiers que vous indexez, de leur
provenance et des téléchargements que vous déclenchez (Prowlarr, qBittorrent, AllDebrid, DDL).

Le logiciel est distribué sous licence MIT, sans garantie. Il ne constitue ni un conseil
juridique ni une autorisation d'usage des œuvres protégées : la reproduction et la communication
au public d'œuvres sans autorisation des ayants droit sont réprimées par le code de la propriété
intellectuelle, y compris à titre gratuit.

Si vous exposez une instance sur Internet, vous devenez éditeur d'un service de communication au
public en ligne : les obligations de la LCEN (mentions légales, identité de l'hébergeur) et du
RGPD s'appliquent. Une instance contenant des œuvres protégées doit rester strictement privée
(authentification, VPN ou accès restreint au foyer).

## Licence

[MIT](LICENSE).
