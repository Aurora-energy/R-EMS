# R-EMS Core (Bootstrap)

This repository hosts the early bootstrap of the all-Rust Energy Management
System core. The current phase establishes the workspace structure and provides
heavily-commented skeleton services that will be fleshed out in subsequent
phases as described in the project plan.

## Setup

Use the helper scripts to prepare the repository and launch the dockerized
services in two clear steps.

```sh
./scripts/setup.sh
```

This command ensures every helper under `scripts/` is executable, verifies the
required ports are available, builds the supervisor container image, and records
its progress in `scripts/setup.log`.

After the base setup completes successfully, start the GUI, registry, and
supervisor containers with the dedicated web orchestration helper:

```sh
./scripts/setup_web.sh
```

The script restarts the compose stack so that the containers always run the
latest code, performs health checks for the three HTTP endpoints, and logs its
actions to `scripts/setup_web.log`. Once it finishes, the GUI is available at
<http://localhost:8080> and the registry API responds on <http://localhost:8000>.

Run the configuration preparer to validate redundancy pairings and device
communication definitions before launching the control-plane services:

```sh
./scripts/setup_config.sh
```

This helper ensures `configs/system.yaml` exists (seeding it from the example
if necessary), creates the runtime log directory, and executes
`r-ems-configd validate` with full output captured in
`scripts/setup_config.log`.


```sh
./scripts/run_core_stack.sh up --build
```

The helper resolves the repository root automatically, so you can execute it
from any directory within the clone. The profile builds the Rust services
directly from the workspace sources and wires them together with sensible
defaults:

* `supervisor` – exposes health checks on <http://localhost:7100>.
* `registry` – publishes the bootstrap backend API on
  <http://localhost:8000>.
* `gui` – serves the operator dashboard on <http://localhost:8080> and
  surfaces documentation from the repository's `docs/` directory.

Once the containers report healthy, open <http://localhost:8080> for the GUI
and <http://localhost:8000> to confirm the backend API responds.

## Developing the supervisor service locally

If you prefer to iterate on the supervisor outside of Docker, the primary
service stub can still be launched directly with Cargo:

```sh
cargo run -p supervisor
```

The supervisor exposes a basic health endpoint that can be queried at
<http://127.0.0.1:7100/healthz> to confirm the service is running.
