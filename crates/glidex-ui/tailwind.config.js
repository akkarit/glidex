/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.rs",
    "./index.html",
  ],
  theme: {
    extend: {
      colors: {
        'vm-running': '#22c55e',
        'vm-stopped': '#ef4444',
        'vm-paused': '#eab308',
        'vm-created': '#3b82f6',
      },
    },
  },
  plugins: [],
}
