/**
---
ems_section: "12-gui"
ems_subsection: "01-bootstrap"
ems_type: "build-config"
ems_scope: "gui"
ems_description: "TailwindCSS configuration for the setup wizard frontend."
ems_version: "v0.1.0"
ems_owner: "tbd"
ems_references:
  - /docs/VERSIONING.md
---
*/
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {}
  },
  plugins: []
} satisfies Config;
