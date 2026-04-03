import type { Config } from 'tailwindcss';

const config: Config = {
  darkMode: 'class',
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      colors: {
        background: '#0d0d0f',
        surface: '#1a1a1f',
        primary: '#6c63ff',
        code: '#12121a',
        success: '#22c55e',
        danger: '#ef4444'
      },
      fontFamily: {
        sans: ['IBM Plex Sans', 'ui-sans-serif', 'system-ui', 'sans-serif'],
        mono: ['IBM Plex Mono', 'ui-monospace', 'SFMono-Regular', 'monospace']
      },
      boxShadow: {
        glow: '0 0 0 1px rgba(108, 99, 255, 0.25), 0 10px 30px rgba(108, 99, 255, 0.2)'
      }
    }
  },
  plugins: []
};

export default config;
