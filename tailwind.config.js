/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      // Same token names as the web app, so copied editor components keep working.
      colors: {
        ns: {
          bg: "var(--ns-bg)",
          surface: "var(--ns-surface)",
          "surface-hover": "var(--ns-surface-hover)",
          elevated: "var(--ns-elevated)",
          ink: "var(--ns-ink)",
          "ink-secondary": "var(--ns-ink-secondary)",
          "ink-muted": "var(--ns-ink-muted)",
          accent: "var(--ns-accent)",
          "accent-hover": "var(--ns-accent-hover)",
          "accent-subtle": "var(--ns-accent-subtle)",
          gold: "var(--ns-gold)",
          border: "var(--ns-border)",
          "border-strong": "var(--ns-border-strong)",
          destructive: "var(--ns-destructive)",
          success: "var(--ns-success)",
        },
      },
      fontFamily: {
        body: ["Georgia", "Iowan Old Style", "serif"],
        ui: ["system-ui", "-apple-system", "Segoe UI", "sans-serif"],
      },
      borderRadius: { ns: "0.5rem", "ns-lg": "0.75rem" },
    },
  },
  plugins: [],
};
