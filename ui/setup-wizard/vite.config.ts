/**
---
ems_section: "12-gui"
ems_subsection: "01-bootstrap"
ems_type: "build-config"
ems_scope: "gui"
ems_description: "Vite configuration for the setup wizard frontend."
ems_version: "v0.1.0"
ems_owner: "tbd"
ems_references:
  - /docs/VERSIONING.md
---
*/
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173
  }
});
