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
    ./bin/r-emsctl status
    ```
    

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
        

---

## 📦 Installer

Install via helper script:

```bash
./scripts/install.sh
```

Supports Docker, systemd, and bare-metal deployments.

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
