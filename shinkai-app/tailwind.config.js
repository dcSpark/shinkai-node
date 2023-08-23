/** @type {import('tailwindcss').Config} */
module.exports = {
  // darkMode: ["class"],
  content: ["./src/**/*.tsx", "./src/**/*.html", "./src/**/*.css"],
  theme: {
    container: {
      center: true,
      padding: ["1rem", "1.5rem", "2rem"],
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        primary: "var(--ion-color-primary)",
        secondary: "var(--ion-color-secondary)",
        accent: "rgb(9, 9, 11)",
        muted: "rgb(114, 113, 122)",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};
