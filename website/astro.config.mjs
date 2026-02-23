// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const site = process.env.SITE_URL || 'https://example.com';
const repoName = process.env.GITHUB_REPOSITORY?.split('/')[1];
const defaultBase = process.env.CI && repoName ? `/${repoName}/` : '/';
const rawBase = process.env.BASE_PATH || defaultBase;
const base = rawBase.endsWith('/') ? rawBase : `${rawBase}/`;

// https://astro.build/config
export default defineConfig({
	site,
	base,
	integrations: [
		starlight({
			title: 'OpenLink',
			description: 'ACARS/CPDLC over NATS — reference implementation, SDK, and operations documentation.',
			logo: {
				src: './src/assets/logo-small.png',
				alt: 'OpenLink',
				replacesTitle: true,
			},
			customCss: ['./src/styles/custom.css'],
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/pluce/openlink' }],
			sidebar: [
				{
					label: 'Start Here',
					items: [
						{ label: 'Overview', slug: '' },
						{ label: 'Platform Overview', slug: 'generated/overview' },
						{ label: 'Guides', slug: 'guides' },
						{ label: 'Reference', slug: 'reference-home' },
					],
				},
				{
					label: 'SDK & Integration',
					autogenerate: { directory: 'generated/docs/sdk' },
				},
				{
					label: 'Runtime & Crates',
					autogenerate: { directory: 'generated/crates' },
				},
				{
					label: 'Specifications',
					autogenerate: { directory: 'generated/spec' },
				},
			],
		}),
	],
});
