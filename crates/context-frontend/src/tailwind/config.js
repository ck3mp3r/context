/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/frontend/components/**/*.rs",
    "./src/frontend/pages/**/*.rs",
    "./src/frontend/api/**/*.rs",
    "./src/frontend/models/**/*.rs",
    "./src/frontend/app.rs",
    "./src/frontend/main.rs",
    "./src/frontend/public/index.html",
  ],
  safelist: [
    // Connection indicator colors
    'bg-ctp-green',
    'bg-ctp-yellow',
    'bg-ctp-red',
    // Tab active/hover states
    'bg-ctp-surface2',
    'bg-ctp-surface1',
    'text-ctp-text',
    'text-ctp-subtext1',
    'hover:bg-ctp-surface1',
    'hover:text-ctp-text',
    // Stacked card z-index
    '-z-10',
    '-z-20',
  ],
  theme: {
    extend: {
      colors: {
        ctp: {
          base: 'var(--ctp-base)',
          mantle: 'var(--ctp-mantle)',
          crust: 'var(--ctp-crust)',
          text: 'var(--ctp-text)',
          subtext1: 'var(--ctp-subtext1)',
          subtext0: 'var(--ctp-subtext0)',
          overlay0: 'var(--ctp-overlay0)',
          overlay1: 'var(--ctp-overlay1)',
          overlay2: 'var(--ctp-overlay2)',
          surface0: 'var(--ctp-surface0)',
          surface1: 'var(--ctp-surface1)',
          surface2: 'var(--ctp-surface2)',
          rosewater: 'var(--ctp-rosewater)',
          flamingo: 'var(--ctp-flamingo)',
          pink: 'var(--ctp-pink)',
          mauve: 'var(--ctp-mauve)',
          red: 'var(--ctp-red)',
          maroon: 'var(--ctp-maroon)',
          peach: 'var(--ctp-peach)',
          yellow: 'var(--ctp-yellow)',
          green: 'var(--ctp-green)',
          teal: 'var(--ctp-teal)',
          sky: 'var(--ctp-sky)',
          sapphire: 'var(--ctp-sapphire)',
          blue: 'var(--ctp-blue)',
          lavender: 'var(--ctp-lavender)',
        },
      },
    },
  },
  plugins: [],
}
