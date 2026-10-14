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

Settings → onglet **Téléchargements** → carte **Téléchargement direct**.

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

Settings → **Téléchargement direct** → section **Recherche automatique**.

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
2. Le fichier est téléchargé en streaming dans le répertoire de la série (résolu comme pour Telegram : fichier existant → répertoire homonyme → nouveau répertoire).
3. Le fichier est renommé selon le [template de renommage](/books/renaming/) si la série n'a pas encore de livre.
4. Un scan est déclenché automatiquement pour intégrer le nouveau fichier.
5. Statut final : **Importé** ou **Erreur**.

Le nombre de téléchargements simultanés se règle via le paramètre `concurrent_ddl_provider_downloads` (défaut : 2).

:::note[Détails techniques]
**Suivi** : chaque requête réelle est persistée dans `ddl_provider_downloads` (`queued` → `downloading` → `imported` / `failed`). Une même entrée (`provider` + `external_id`) est réutilisée : relancer un téléchargement réinitialise la ligne existante.

**Classement** : quand le nom de fichier brut n'est pas exploitable par le parser (Fourtoutici écrit un tome « nu », ex. `BD Astérix le Gaulois 41 Astérix en Lusitanie.pdf`), le fournisseur fournit un `match_name` normalisé (`Astérix le Gaulois T41`) utilisé uniquement pour le classement ; le nom affiché reste l'original.

**API** : `GET /ddl-providers`, `POST /ddl-providers/{id}/{toggle,test,search,downloads}`, `POST /ddl-providers/search-all`, `GET|POST /ddl-providers/search-settings`, `GET /ddl-provider-downloads`, `DELETE /ddl-provider-downloads/{id}` (scope admin).

**Job** : la recherche planifiée crée un job `ddl_search` (réservé à l'API, sans bibliothèque) traité par le poller de l'API.
:::
