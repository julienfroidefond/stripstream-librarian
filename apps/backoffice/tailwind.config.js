/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: 'class',
  content: [
    './pages/**/*.{js,ts,jsx,tsx,mdx}',
    './components/**/*.{js,ts,jsx,tsx,mdx}',
    './app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        card: 'hsl(var(--card))',
        line: {
          DEFAULT: 'hsl(var(--line))',
          strong: 'hsl(var(--line-strong))',
        },
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          soft: 'hsl(var(--primary-soft))',
          foreground: 'hsl(var(--foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--text-muted))',
          foreground: 'hsl(var(--text-muted))',
        },
        success: {
          DEFAULT: 'hsl(142 60% 45%)',
          soft: 'hsl(142 60% 90%)',
        },
        warning: {
          DEFAULT: 'hsl(45 93% 47%)',
          soft: 'hsl(45 93% 90%)',
        },
        error: {
          DEFAULT: 'hsl(2 72% 48%)',
          soft: 'hsl(2 72% 90%)',
        },
        cyan: 'hsl(192 85% 55%)',
        pink: 'hsl(338 82% 62%)',
      },
      fontFamily: {
        sans: ['"Avenir Next"', 'Segoe UI', 'Noto Sans', 'system-ui', 'sans-serif'],
      },
      animation: {
        'spin-slow': 'spin 2s linear infinite',
        'pulse-slow': 'pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'pulse-ring': 'pulse-ring 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
      },
      keyframes: {
        'pulse-ring': {
          '0%': { boxShadow: '0 0 0 0 hsl(var(--primary) / 0.4)' },
          '50%': { boxShadow: '0 0 0 4px hsl(var(--primary) / 0)' },
          '100%': { boxShadow: '0 0 0 0 hsl(var(--primary) / 0)' },
        },
      },
      borderRadius: {
        lg: '12px',
        md: '8px',
        sm: '6px',
      },
      boxShadow: {
        'soft': '0 1px 2px 0 rgb(23 32 46 / 0.06)',
        'card': '0 12px 30px -12px rgb(23 32 46 / 0.22)',
        'dark-soft': '0 1px 2px 0 rgb(2 8 18 / 0.35)',
        'dark-card': '0 12px 30px -12px rgb(2 8 18 / 0.55)',
      },
    },
  },
  plugins: [],
}
