// sidebars.js
module.exports = {
  docs: [
    'intro',
    'start',

    {
      type: 'category',
      label: 'Concepts',
      items: [
        'concepts/xp',
        'concepts/identity',
        'concepts/pulse',
        'concepts/lifecycle',
        'concepts/constraints',
      ],
    },

    {
      type: 'category',
      label: 'Architecture',
      items: [
        'architecture/overview',
        'architecture/storage',
        'architecture/traits',
        'architecture/fungible-adapter',
        'architecture/call-surface',
      ],
    },

    {
      type: 'category',
      label: 'Getting Started',
      items: [
        'getting-started/installation',
        'getting-started/configuration',
        'getting-started/first-xp',
      ],
    },

    {
      type: 'category',
      label: 'Core',
      items: [
        'core/extrinsics',
        'core/inspectors',
        'core/events',
        'core/rpc-ui',
      ],
    },
    {
      type: 'category',
      label: 'Advanced',
      items: [
        'advanced/instances',
        'advanced/weights',
        'advanced/listeners',
        'advanced/staking',
        'advanced/upcoming',
      ],
    },
  ],
};