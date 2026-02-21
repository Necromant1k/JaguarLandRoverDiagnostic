import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: {
          primary: "#0a0e1a",
          secondary: "#111827",
          card: "#1a1f2e",
          hover: "#242a3d",
        },
        accent: "#00d4ff",
        ok: "#00ff88",
        err: "#ff3b3b",
        warn: "#ffaa00",
        pending: "#ffd700",
        tx: "#00d4ff",
        rx: "#00ff88",
      },
    },
  },
  plugins: [],
} satisfies Config;
