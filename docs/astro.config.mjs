// @ts-check

import starlight from '@astrojs/starlight';
import { defineConfig } from 'astro/config';
import starlightImageZoom from 'starlight-image-zoom';

// https://astro.build/config
export default defineConfig({
  site: 'https://docspring.github.io',
  base: '/cigen',

  integrations: [
    starlight({
      title: 'Cigen',
      description:
        'Universal CI pipeline configuration generator - write once, run anywhere',
      logo: {
        src: './src/assets/logo.svg',
      },
      customCss: ['./src/styles/custom.css'],
      editLink: {
        baseUrl: 'https://github.com/DocSpring/cigen/edit/main/docs',
      },
      plugins: [starlightImageZoom()],
      components: {
        Footer: './src/components/Footer.astro',
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/DocSpring/cigen',
        },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Installation', slug: 'installation' },
            { label: 'Quick Start', slug: 'quick-start' },
            { label: 'Philosophy', slug: 'philosophy' },
          ],
        },
        {
          label: 'Commands',
          items: [
            { label: 'generate', slug: 'commands/generate' },
            { label: 'validate', slug: 'commands/validate' },
          ],
        },
        {
          label: 'Configuration',
          items: [
            { label: 'Overview', slug: 'configuration/overview' },
            { label: 'Checkout', slug: 'configuration/checkout' },
            { label: 'Cache System', slug: 'configuration/cache' },
            { label: 'Package Management', slug: 'configuration/packages' },
          ],
        },
        {
          label: 'Providers',
          items: [{ label: 'CircleCI', slug: 'providers/circleci' }],
        },
        {
          label: 'Advanced Features',
          items: [
            { label: 'OR Dependencies', slug: 'advanced/or-dependencies' },
            { label: 'Job Skipping', slug: 'advanced/job-skipping' },
          ],
        },
        {
          label: 'Examples',
          autogenerate: { directory: 'examples' },
        },
        {
          label: 'Reference',
          items: [{ label: 'Requirements', slug: 'reference/requirements' }],
        },
      ],
    }),
  ],
});
