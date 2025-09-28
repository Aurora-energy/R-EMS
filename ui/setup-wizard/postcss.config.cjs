/**
---
ems_section: "12-gui"
ems_subsection: "01-bootstrap"
ems_type: "build-config"
ems_scope: "gui"
ems_description: "PostCSS configuration enabling TailwindCSS for the setup wizard."
ems_version: "v0.1.0"
ems_owner: "tbd"
ems_references:
  - /docs/VERSIONING.md
---
*/
module.exports = {
  plugins: {
    tailwindcss: {},
    autoprefixer: {}
  }
};
