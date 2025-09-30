<!--
README_GUI.md documents the R-EMS Core GUI project, providing engineers and operators with a single reference for how the GUI backend and frontend integrate within the broader system.
-->
# R-EMS Core GUI Overview

## Purpose
- Provide a web-based interface for monitoring and configuring the R-EMS platform.
- Offer an operator-friendly summary of health, configuration, high-availability status, and plugin lifecycle information.

## Architecture Summary
- **Backend:** FastAPI service housed in `services/gui/backend`, responsible for aggregating health, plugin, configuration, and help documentation data.
- **Frontend:** React + Vite application located in `services/gui/frontend`, consuming backend APIs and presenting data-rich dashboards.
- **Dockerization:** Backend and frontend are containerized for deployment and orchestrated via compose profiles dedicated to the GUI stack.

## Integration Points
- Backend aggregates status from existing R-EMS microservices (registry, supervisor, bus) via internal APIs.
- Frontend fetches documentation from mounted Markdown files to drive contextual help experiences.
- Docker Compose profile `profile.core.yml` will include both services alongside shared volumes for docs and configuration.

## Next Steps
- Implement the fully commented backend and frontend foundations (Phases B and C) following this scaffold.
- Introduce Docker build definitions (Phase D) and more detailed safety annotations (Phase E) before layering UX refinements (Phase F).


## Running the GUI via Docker Compose

```bash
# From anywhere inside the repository clone, launch the core stack via the helper script.
./scripts/run_core_stack.sh up --build
# After the stack starts, access the frontend at http://localhost:8080 and confirm the backend responds on http://localhost:8000.
```

> [!TIP]
> The helper resolves the repository root automatically. If you prefer to run the compose command manually, first `cd` to the
> repository root and then execute `docker compose -f ./compose/profiles/profile.core.yml up --build` so Docker can locate the
> profile file.

## Help Documentation Wiring
- Mount Markdown files into `/srv/r-ems/docs` so both backend and frontend containers can surface the same documentation set.
- The backend's `/api/gui/help/docs` endpoint lists files by scanning the mounted directory, while `/api/gui/help/docs/{path}` streams raw content.
- The frontend Help page consumes those endpoints and renders markdown via `react-markdown`, keeping the experience synchronized with the source docs.
