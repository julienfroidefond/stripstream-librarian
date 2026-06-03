// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'Stripstream Librarian',
			logo: {
				src: './src/assets/logo.webp',
			},
			social: [{ icon: 'github', label: 'Sources', href: 'https://git.julienfroidefond.com/julienfroidefond/stripstream-librarian' }],
			customCss: ['./src/styles/custom.css'],
			sidebar: [
				{
					label: 'Démarrage',
					items: [
						{ label: 'Introduction', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Configuration', slug: 'getting-started/configuration' },
						{ label: 'Dashboard', slug: 'getting-started/dashboard' },
					],
				},
				{
					label: 'Bibliothèques',
					items: [
						{ label: 'Gestion', slug: 'libraries/management' },
						{ label: 'Scan & Indexation', slug: 'libraries/scanning' },
					],
				},
				{
					label: 'Livres',
					items: [
						{ label: 'Formats supportés', slug: 'books/formats' },
						{ label: 'Métadonnées', slug: 'books/metadata' },
						{ label: 'Auteurs', slug: 'books/authors' },
						{ label: 'Miniatures', slug: 'books/thumbnails' },
						{ label: 'Renommage', slug: 'books/renaming' },
						{ label: 'Conversion CBR→CBZ', slug: 'books/conversion' },
					],
				},
				{
					label: 'Séries',
					items: [
						{ label: 'Gestion des séries', slug: 'series/management' },
						{ label: 'Genres', slug: 'series/genres' },
						{ label: 'Listes de lecture', slug: 'series/reading-lists' },
						{ label: 'Séries liées & Recommandations', slug: 'series/related' },
						{ label: 'Volumes manquants', slug: 'series/missing-volumes' },
						{ label: 'Progression de lecture', slug: 'series/reading-progress' },
						{ label: 'Archives', slug: 'series/archives' },
					],
				},
				{
					label: 'Métadonnées externes',
					items: [
						{ label: 'Providers', slug: 'metadata/providers' },
						{ label: 'Synchronisation', slug: 'metadata/sync' },
						{ label: 'Batch & Refresh', slug: 'metadata/batch-refresh' },
					],
				},
				{
					label: 'Découverte',
					items: [
						{ label: 'Sources', slug: 'discovery/sources' },
						{ label: 'Ajouter à la bibliothèque', slug: 'discovery/add-to-library' },
						{ label: 'Wishlist', slug: 'discovery/wishlist' },
					],
				},
				{
					label: 'Téléchargements',
					items: [
						{ label: 'Vue d\'ensemble', slug: 'downloads/overview' },
					],
				},
				{
					label: 'Jobs',
					items: [
						{ label: 'Vue d\'ensemble', slug: 'jobs/overview' },
						{ label: 'Indexation', slug: 'jobs/indexation' },
						{ label: 'Miniatures', slug: 'jobs/miniatures' },
						{ label: 'Métadonnées', slug: 'jobs/metadonnees' },
						{ label: 'Téléchargements', slug: 'jobs/telechargements' },
						{ label: 'AniList', slug: 'jobs/anilist' },
					],
				},
				{
					label: 'Intégrations',
					items: [
						{ label: 'AniList', slug: 'integrations/anilist' },
						{ label: 'Prowlarr', slug: 'integrations/prowlarr' },
						{ label: 'qBittorrent', slug: 'integrations/qbittorrent' },
						{ label: 'Komga', slug: 'integrations/komga' },
						{ label: 'Telegram', slug: 'integrations/telegram' },
					],
				},
				{
					label: 'Référence',
					items: [
						{ label: 'API', slug: 'reference/api' },
						{ label: 'Utilisateurs & Tokens', slug: 'reference/users-tokens' },
						{ label: 'Variables d\'environnement', slug: 'reference/environment' },
					],
				},
			],
		}),
	],
});
