---
ems_section: "00-meta"
ems_subsection: "readme"
ems_type: "doc"
ems_scope: "meta"
ems_description: "Top-level README for R-EMS repository."
ems_version: "v0.0.0-prealpha"
ems_owner: "tbd"
---

# R-EMS — Rust-based Energy Management System

⚠️ **Status: This project is NOT in release yet — it is pre-alpha, not stable.**  
Development is ongoing. Interfaces, APIs, structure and features will change.

---

## 📖 Project Purpose
R-EMS is an **energy management system written in Rust**.  
It is designed for high-reliability, redundant multi-grid environments, including:  
- Land-based battery installations.  
- Marine systems with parallel grids and controllers.  
- Simulation/testing with replay and synthetic data.  

The architecture emphasizes **resilience, security, observability, and modularity**, with clear boundaries between core, messaging, persistence, orchestration, networking, and simulation.

This project exists to break down the barriers created by proprietary EMS systems, which often make troubleshooting in the field nearly impossible and slow down the transition to sustainable energy solutions. Just as Linux democratized and revolutionized the internet by providing a solid, open foundation, this initiative aims to deliver a similar breakthrough for energy management creating a transparent, reliable, and community-driven platform that helps and accelerates adoption and innovation in the global shift to renewable power.

---

## 🧩 System Architecture (Sections)

The project is organized into 15 Sections, each with sub-steps and full documentation:

1. **Core Functionality**  
2. **Messaging & IPC / Data Model**  
3. **Persistence & Logging**  
4. **Configuration & Orchestration**  
5. **Networking & External Interfaces**  
6. **Security & Access Control**  
7. **Resilience & Fault Tolerance**  
8. **Energy Models & Optimization**  
9. **Integration & Interoperability**  
10. **Deployment & CI/CD Enhancements**  
11. **Simulation & Test Harness**  
12. **GUI & Setup Wizard**  
13. **Documentation & Contributor Guides**  
14. **Versioning & Licensing System**  
15. **Testing, QA & Runbook**

See `/docs/FRONTMATTER-GUIDE.md` for the taxonomy of files.

---

## 🛠️ Build Instructions
```bash
cargo build
cargo test
```

---

## ▶️ Running

- **Daemon:**
    
    ```bash
    ./bin/r-emsd
    ```
    
- **Control CLI:**

    ```bash
    ./bin/r-emsctl setup wizard
    ```

    Interactive first-run workflow that persists a validated configuration
    manifest and assigns controller failover roles.

    ```bash
    ./bin/r-emsctl status
    ```

---

## 📊 Observability (Logging & Metrics)

- **Logging**
  - Structured JSON logs are emitted to stdout by default so container platforms can parse them easily.
  - Daily rotated log files live under the configured `logging.directory` to support long running production analysis.
  - Override verbosity with `R-EMS_LOG=<filter>` (falls back to `RUST_LOG` or defaults to `debug`).

- **Metrics**
  - Enable the Prometheus exporter in `configs/*.toml` under `[metrics]`.
  - Scrape the `/metrics` endpoint to collect counters and histograms for daemon lifecycle and orchestrator health.

---

## 🎛️ Simulation Mode

- Start any device in simulation mode (e.g., Raspberry Pi with Balena).
    
- Generate or replay data into the system.
    
- Useful for testing integrations without real hardware.
    

---

## 🖥️ GUI & Setup Wizard

- Access via web browser after starting R-EMS.
    
- Guides you through first-time installation:

    - Define installation name.

    - Configure grids and controllers.

    - Enable simulation or production mode.

    - Save validated configuration.

- The same manifest is editable later via the CLI (`r-emsctl setup new`)
  or by re-running the wizard. Configuration is persisted under
  `/etc/r-ems` (override with `R_EMS_CONFIG_ROOT`), allowing both the CLI
  and GUI to operate on the same source of truth.
- Wizard actions emit structured success/fault logs via the shared
  logging facade for traceability.
        

---

## 📦 Installer

Install via helper script:

```bash
./scripts/install.sh
```

Supports Docker, systemd, and bare-metal deployments.

- The Docker helper (`scripts/docker-install.sh`) bundles the runtime
  image with the required services (OpenBalena supervisor, SurrealDB, and
  the static setup wizard assets) so a fresh install has all core
  dependencies available out of the box.

---

## 🔒 Security & Licensing

- Development use is free and open.
    
- Commercial/security features require a valid license.
    
- Certificates and PKI integration support full trust chains for national grid and marine class compliance.
    
- See `/docs/LICENSING.md`.
    

---

## 📚 Contributing

Contributions are welcome!

- See `/docs/CONTRIBUTOR.md` for guidelines.
    
- Branching and versioning rules are in `/docs/VERSIONING.md`.
    
- All files must include proper **frontmatter** for discoverability.
    

---

## 📜 License

- Free for development and testing.
    
- Commercial use requires a separate license.
    
- See `/docs/LICENSING.md` for details.
    

---

```
