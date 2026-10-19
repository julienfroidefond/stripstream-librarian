---
title: Téléchargements — Fournisseurs directs
description: Télécharger des fichiers depuis des catalogues externes (Fourtoutici…) directement dans votre bibliothèque
---

Les **fournisseurs de téléchargement direct** interrogent des catalogues externes (API JSON ou pages HTML) pour retrouver un fichier et l'importer automatiquement dans la bibliothèque — sans client torrent.

C'est le même modèle que le [Telegram Monitor](/downloads/telegram) : le fichier est écrit dans le répertoire de la série, puis un scan est déclenché.

:::note[Périmètre actuel]
Le POC couvre **Fourtoutici** (`fourtoutici.cc`), qui expose une API JSON et ne nécessite aucun compte.

Les fournisseurs **Anna's Archive** et **Z-Library** sont prévus ; leurs résultats seront limités aux ouvrages en français (ou proches). Ils nécessiteront probablement un compte, contrairement à Fourtoutici.
:::

## Activer un fournisseur

**Paramètres → onglet Outils de téléchargement → section DDL → carte Téléchargement direct**.

Comme les autres fonctions de téléchargement, les fournisseurs sont **désactivés par défaut** : activez ceux que vous voulez utiliser. Chaque fournisseur dispose de :
- un **interrupteur** pour l'activer ou le désactiver individuellement ;
- un champ **URL de base** pour pointer vers un miroir (laisser la valeur par défaut si le domaine principal fonctionne) ;
- un bouton **Tester** qui vérifie que le catalogue répond.

Les fournisseurs désactivés disparaissent des fenêtres de recherche — aucune fenêtre ne s'ouvre s'ils le sont tous.

Pour Fourtoutici, laisser le domaine par défaut suffit. Le fournisseur utilise un User-Agent de navigateur : sans cela, Cloudflare répond `403`.

## Rechercher depuis une série

Sur la fiche d'une série → *Actions* → **Téléchargement direct**.

Dans la fenêtre de recherche :
- **Sélecteur de fournisseur** quand plusieurs sont activés ;
- champ de recherche pré-rempli avec le nom de la série (article initial retiré, ex. `Astérix`) ;
- **badges rapides** : variantes du nom + tomes manquants (`T41`) pour cibler un tome précis ;
- résultats classés : les tomes manquants de la série sont mis en évidence (vert) et les tomes déjà possédés signalés.

La recherche est aussi disponible depuis la fiche d'un **livre** et depuis la page d'un **tome manquant**, où le tome concerné sert de cible de classement.

## Recherche automatique en arrière-plan

**Paramètres → onglet Outils de téléchargement → section DDL → carte Téléchargement direct → section Recherche automatique**.

Un réglage **global** (commun à toutes les bibliothèques) lance périodiquement une recherche
sur toutes les séries *approuvées* ayant des tomes manquants — exactement le même périmètre
que la [détection des releases](/downloads/overview/) et du Telegram Monitor.

- **Fréquence** : `Désactivée` (défaut), ou toutes les 15, 30, 60 min, 6 h, 24 h.
- **Rechercher maintenant** : lance immédiatement un passage, sans attendre la prochaine échéance
  (désactivé tant qu'aucun fournisseur n'est actif). Le même passage est aussi lançable depuis la
  page **Tâches** (groupe *Téléchargement direct*), et rejouable depuis l'historique des tâches.

Les résultats **ne sont jamais téléchargés automatiquement** : ils apparaissent comme des
**releases** dans la page [Téléchargements](/downloads/overview/) (badge vert, libellé du
fournisseur), à côté des résultats Prowlarr et Telegram. Cliquez sur l'icône de téléchargement
pour importer le fichier.

Une recherche relancée pour une série **remplace ses anciens résultats DDL** (les résultats
Prowlarr/TG en attente sont conservés) : pas d'accumulation entre deux passages.

## Lancer un téléchargement

| Statut | Action disponible |
|--------|-------------------|
| (aucun) | **Télécharger** |
| En attente | Icône horloge |
| Téléchargement | Indicateur de progression |
| Importé | ✓ + **Relancer** |
| Erreur | **Relancer** |
| Ignoré | **Télécharger** (relance) |

Le bouton **Ignorer** masque une entrée sans la supprimer définitivement.

## Flux d'import

1. Clic sur **Télécharger** — l'entrée passe en *En attente*.
2. Le fichier est rapatrié dans le répertoire de la série (résolu comme pour Telegram : fichier existant → répertoire homonyme → nouveau répertoire).
3. Le fichier est renommé selon le [template de renommage](/books/renaming/) si la série n'a pas encore de livre.
4. Un scan est déclenché automatiquement pour intégrer le nouveau fichier.
5. Statut final : **Importé** ou **Erreur**.

Le téléchargement écrit d'abord dans un `.tmp`, renommé une fois complet ; un fichier vide n'est jamais
conservé et est signalé en **Erreur**. Le réglage **Délai maximal par fichier** borne la durée totale :
au-delà, le transfert est abandonné et le `.tmp` supprimé.

## Moteur de téléchargement DDL

**Paramètres → onglet Outils de téléchargement → section Moteur de téléchargement**, carte
**Téléchargements directs (DDL)**. Les trois derniers réglages sont **partagés par tous les moteurs
de téléchargement** (DDL et Telegram) ; les deux premiers ne concernent que les téléchargements
directs.

| Réglage | Clé `limits` | Défaut | Portée |
|---------|--------------|--------|--------|
| Téléchargements DDL simultanés | `concurrent_ddl_provider_downloads` | 2 | DDL |
| Connexions par fichier | `ddl_download_connections` | 4 | DDL |
| Tentatives par bloc | `download_max_attempts` | 5 | partagé |
| Délai d'inactivité (secondes) | `download_stall_timeout_seconds` | 180 | partagé |
| Délai maximal par fichier (minutes) | `download_timeout_minutes` | 30 | partagé |

Quand le serveur répond aux requêtes `Range` (c'est le cas de la plupart des hébergeurs de fichiers),
le fichier est découpé en blocs répartis sur plusieurs connexions parallèles : plus rapide sur les gros
tomes, et un bloc en échec est **relancé sur place** sans repartir de zéro. Chaque bloc est retenté
jusqu'à **Tentatives par bloc** fois, et une connexion qui n'envoie plus rien pendant **Délai
d'inactivité** est considérée comme bloquée puis relancée. Si le serveur ignore `Range` (ou ne
communique pas la taille), Stripstream bascule automatiquement sur un **flux unique** — le
téléchargement reste fonctionnel, seule la parallélisation est perdue.

:::note[Détails techniques]
**Suivi** : chaque requête réelle est persistée dans `ddl_provider_downloads` (`queued` → `downloading` → `imported` / `failed`). Une même entrée (`provider` + `external_id`) est réutilisée : relancer un téléchargement réinitialise la ligne existante.

**Classement** : quand le nom de fichier brut n'est pas exploitable par le parser (Fourtoutici écrit un tome « nu », ex. `BD Astérix le Gaulois 41 Astérix en Lusitanie.pdf`), le fournisseur fournit un `match_name` normalisé (`Astérix le Gaulois T41`) utilisé uniquement pour le classement ; le nom affiché reste l'original.

**Rapprochement de série** : le nom de la série bibliothèque est comparé au catalogue de façon tolérante aux connecteurs (`Boule & Bill` ↔ `Boule et Bill`) et aux articles initiaux (`Les Légendaires` ↔ `Légendaires`) — Prowlarr/Telegram gardent la comparaison stricte. Les packs en plage (`One Piece 01 à T114`) conservent leur borne haute : toute la plage est proposée, pas seulement le premier tome.

**Diagnostic** : le job `ddl_search` écrit dans `stats_json` un entonnoir par étape (`series`, `series_with_results`, `series_without_missing_volumes`, `series_with_missing_volumes`, `files_owned`, `files_other_series`, `files_no_volume_match`, `files_blacklisted`). Un `series_with_results` faible avec beaucoup de `files_other_series` signale un rapprochement de nom trop strict ; beaucoup de `series_without_missing_volumes` indique des métadonnées sans numéro de tome.

**API** : `GET /ddl-providers`, `POST /ddl-providers/{id}/{toggle,test,search,downloads}`, `POST /ddl-providers/search-all`, `GET|POST /ddl-providers/search-settings`, `GET /ddl-provider-downloads`, `DELETE /ddl-provider-downloads/{id}` (scope admin).

**Job** : la recherche planifiée crée un job `ddl_search` (réservé à l'API, sans bibliothèque) traité par le poller de l'API.
:::
