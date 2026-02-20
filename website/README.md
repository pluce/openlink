# OpenLink website

Product site + technical documentation portal for OpenLink, built with Astro + Starlight.

## Documentation sync model

The site does not duplicate authoring manually.

- Source docs live in the repository (workspace README, SDK docs, crate READMEs, protocol references).
- A sync step converts them into website pages under:
	[website/src/content/docs/generated](src/content/docs/generated)
- Sync script:
	[website/scripts/sync-docs.mjs](scripts/sync-docs.mjs)

## Commands

Run from [website](.):

| Command | Action |
| :-- | :-- |
| `npm run sync-docs` | Regenerate website pages from repository docs |
| `npm run dev` | Sync docs, then start local site at `localhost:4321` |
| `npm run build` | Sync docs, then build static output in `dist/` |
| `npm run preview` | Preview the production build |

## GitHub Pages deployment

Workflow file: [.github/workflows/deploy-website-pages.yml](../.github/workflows/deploy-website-pages.yml)

### Required GitHub configuration

1. In repository settings, open **Pages**.
2. Set **Source** to **GitHub Actions**.
3. Push to `main` (or run the workflow manually from **Actions**).

### Optional repository variables

Define these in **Settings → Secrets and variables → Actions → Variables** when needed:

- `GH_PAGES_SITE_URL`
	- default: `https://<owner>.github.io`
	- set this for custom domain scenarios.
- `GH_PAGES_BASE_PATH`
	- default: `/<repo>`
	- use `/` for user/org pages (`<owner>.github.io`).

## Main files

- Site config: [website/astro.config.mjs](astro.config.mjs)
- Landing page: [website/src/content/docs/index.mdx](src/content/docs/index.mdx)
- Generated docs root: [website/src/content/docs/generated](src/content/docs/generated)
