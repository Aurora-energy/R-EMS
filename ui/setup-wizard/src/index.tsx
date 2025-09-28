/**
---
ems_section: "12-gui"
ems_subsection: "01-bootstrap"
ems_type: "frontend-entry"
ems_scope: "gui"
ems_description: "React entrypoint rendering the setup wizard shell."
ems_version: "v0.1.0"
ems_owner: "tbd"
ems_references:
  - /docs/VERSIONING.md
---
*/
import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import { App } from "./modules/App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
