// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const site = process.env.SITE_URL || 'https://example.com';
const base = process.env.BASE_PATH || '/';

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
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com' }],
			sidebar: [
				{
					label: 'OpenLink',
					items: [
						{ label: 'Overview', slug: '' },
						{ label: 'OpenLink Overview Doc', slug: 'generated/overview' },
					],
				},
				{
					label: 'SDK Docs',
					autogenerate: { directory: 'generated/docs/sdk' },
				},
				{
					label: 'Crates',
					autogenerate: { directory: 'generated/crates' },
				},
				{
					label: 'Spec',
					autogenerate: { directory: 'generated/spec' },
				},
			],
		}),
	],
});
