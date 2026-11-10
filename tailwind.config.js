/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        bg: {
          primary: "#090d16",
          secondary: "#111827",
          card: "#161f33",
        }
      }
    },
  },
  plugins: [],
}
