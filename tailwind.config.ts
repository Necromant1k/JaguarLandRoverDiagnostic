import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: {
          primary: "#1e1e1e",
          secondary: "#252526",
          card: "#2d2d2d",
          hover: "#2a2d2e",
        },
        accent: "#007acc",
        ok: "#4ec9b0",
        err: "#f44747",
        warn: "#cca700",
        pending: "#dcdcaa",
        tx: "#569cd6",
        rx: "#4ec9b0",
      },
    },
  },
  plugins: [],
} satisfies Config;
