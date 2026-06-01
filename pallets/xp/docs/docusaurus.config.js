// @ts-check
// `@type` JSDoc annotations allow editor autocompletion and type checking
// (when paired with `@ts-check`).
// There are various equivalent ways to declare your Docusaurus config.
// See: https://docusaurus.io/docs/api/docusaurus-config

import {themes as prismThemes} from 'prism-react-renderer';

const config = {
  title: 'Pallet-XP',
  tagline: 'A reputation-driven XP system for tracking contribution, consistency, and participation in non-trusted runtime environments.',
  favicon: 'img/xp-favicon.svg',

  future: {
    v4: true,
  },

  markdown: {
  mermaid: true,
  },

  plugins: [
    [
      '@docusaurus/plugin-client-redirects',
      {
        redirects: [
          {
            from: '/docs',
            to: '/docs/intro',
          },
        ],
      },
    ],
  ],

  url: 'https://auguth.github.io',
  baseUrl: '/frame-suite/pallet-xp/',

  organizationName: 'auguth', 
  projectName: 'frame-suite',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  themes: ['@docusaurus/theme-mermaid'],

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: './sidebars.js',
          editUrl:
            'https://github.com/auguth/frame-suite/tree/master/pallets/xp/docs',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
  ],

  themeConfig: {
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    mermaid: {
      theme: {
        light: 'dark',
        dark: 'dark',
      },
      options: {
        themeVariables: {
          primaryColor: '#1a1428',
          primaryTextColor: '#F5F5FA',
          primaryBorderColor: '#7f5fff',
          lineColor: '#A1A1B5',
          secondaryColor: '#0E0E18',
          tertiaryColor: '#151524',
          background: '#07070D',
          mainBkg: '#0E0E18',
          nodeBorder: '#7f5fff',
          clusterBkg: '#151524',
          titleColor: '#F5F5FA',
          edgeLabelBackground: '#07070D',
          fontFamily: 'Fira Sans, sans-serif',

          /*--- Sequence diagram specific ---*/
          actorBkg: '#0E0E18',
          actorBorder: '#7f5fff',
          actorTextColor: '#F5F5FA',
          actorLineColor: '#7f5fff',
          signalColor: '#A1A1B5',
          signalTextColor: '#F5F5FA',
          labelBoxBkgColor: '#0E0E18',
          labelBoxBorderColor: '#7f5fff',
          labelTextColor: '#F5F5FA',
          loopTextColor: '#F5F5FA',
          noteBorderColor: '#7f5fff',
          noteBkgColor: '#151524',
          noteTextColor: '#F5F5FA',
          activationBorderColor: '#7f5fff',
          activationBkgColor: '#1a1428',
          sequenceNumberColor: '#F5F5FA',
        },
      },
    },
  },
  
};

export default config;
