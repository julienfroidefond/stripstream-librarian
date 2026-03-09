## ADDED Requirements

### Requirement: Demande de conversion d'un livre CBR
L'API SHALL exposer un endpoint `POST /books/:id/convert` qui crée un job de type `cbr_to_cbz` pour le livre spécifié. L'endpoint SHALL retourner une erreur `409 Conflict` si le livre n'est pas au format CBR. L'endpoint SHALL retourner une erreur `409 Conflict` si un fichier `{stem}.cbz` existe déjà au même emplacement que le CBR.

#### Scenario: Conversion demandée sur un livre CBR
- **WHEN** l'utilisateur envoie `POST /books/:id/convert` sur un livre avec `file_format = 'cbr'`
- **THEN** un job `cbr_to_cbz` est créé en statut `pending` avec `book_id` = id du livre
- **THEN** la réponse HTTP 200 contient le job créé

#### Scenario: Conversion demandée sur un livre non-CBR
- **WHEN** l'utilisateur envoie `POST /books/:id/convert` sur un livre avec `file_format != 'cbr'`
- **THEN** l'API retourne HTTP 409 avec un message d'erreur explicite

#### Scenario: CBZ déjà présent au même emplacement
- **WHEN** l'utilisateur envoie `POST /books/:id/convert` et qu'un fichier `{stem}.cbz` existe déjà sur le disque
- **THEN** l'API retourne HTTP 409 avec un message indiquant le conflit de fichier

### Requirement: Exécution sécurisée de la conversion
L'indexer SHALL exécuter la conversion CBR→CBZ de manière sécurisée : le CBR original SHALL être supprimé uniquement après que le CBZ a été créé, vérifié, et que la base de données a été mise à jour. En cas d'échec à n'importe quelle étape, le CBR SHALL rester intact et le job SHALL passer en statut `failed`.

#### Scenario: Conversion réussie
- **WHEN** l'indexer traite un job `cbr_to_cbz`
- **THEN** il extrait les images du CBR vers un dossier temporaire
- **THEN** il crée `{stem}.cbz.tmp` dans le même dossier que le CBR
- **THEN** il vérifie que le CBZ contient le même nombre d'images que le CBR original
- **THEN** il renomme `{stem}.cbz.tmp` → `{stem}.cbz`
- **THEN** il met à jour `books.file_path` et `books.file_format = 'cbz'` en DB
- **THEN** il supprime le fichier CBR original
- **THEN** le job passe en statut `success`

#### Scenario: Échec pendant la création du CBZ
- **WHEN** une erreur survient avant la mise à jour DB (extraction, pack, vérification)
- **THEN** le fichier `.cbz.tmp` est supprimé si présent
- **THEN** le CBR original reste intact
- **THEN** le job passe en statut `failed` avec un message d'erreur

#### Scenario: Échec de la suppression du CBR après conversion réussie
- **WHEN** la suppression du CBR échoue après que le CBZ est valide et la DB mise à jour
- **THEN** le job passe quand même en statut `success`
- **THEN** l'erreur de suppression est loguée en avertissement

### Requirement: Vérification du CBZ généré
Le système SHALL vérifier l'intégrité du CBZ créé avant de modifier la base de données. La vérification SHALL confirmer que le nombre d'images dans le CBZ est égal au nombre d'images dans le CBR source.

#### Scenario: CBZ valide avec le bon nombre d'images
- **WHEN** le CBZ est créé avec N images
- **THEN** l'ouverture du ZIP et le décompte des entrées image retourne N
- **THEN** la vérification passe et la conversion continue

#### Scenario: CBZ invalide (décompte incorrect)
- **WHEN** le CBZ créé contient un nombre d'images différent du CBR source
- **THEN** la vérification échoue
- **THEN** le fichier `.cbz.tmp` est supprimé
- **THEN** le job échoue avec une erreur de vérification

### Requirement: Mise à jour de la base de données après conversion
L'indexer SHALL mettre à jour le livre en base de données après une conversion réussie : `file_path` SHALL être mis à jour (`.cbr` → `.cbz`), `file_format` SHALL être mis à jour à `'cbz'`.

#### Scenario: Mise à jour DB réussie
- **WHEN** le CBZ est vérifié et renommé
- **THEN** `books.file_path` est mis à jour pour pointer vers le nouveau fichier `.cbz`
- **THEN** `books.file_format` est mis à jour à `'cbz'`
- **THEN** `books.updated_at` est mis à jour

### Requirement: Bouton de conversion dans le backoffice
Le backoffice SHALL afficher un bouton "Convert to CBZ" sur la page détail d'un livre, visible uniquement si `book.file_format === 'cbr'`. Le clic SHALL appeler `POST /books/:id/convert` et SHALL afficher un feedback (succès ou erreur).

#### Scenario: Affichage du bouton sur un livre CBR
- **WHEN** l'utilisateur consulte la page détail d'un livre avec `file_format = 'cbr'`
- **THEN** le bouton "Convert to CBZ" est visible

#### Scenario: Bouton absent sur un livre non-CBR
- **WHEN** l'utilisateur consulte la page détail d'un livre avec `file_format != 'cbr'`
- **THEN** aucun bouton de conversion n'est affiché

#### Scenario: Conversion lancée depuis le bouton
- **WHEN** l'utilisateur clique sur "Convert to CBZ"
- **THEN** l'API est appelée et un job est créé
- **THEN** un message de confirmation avec le lien vers le job est affiché
