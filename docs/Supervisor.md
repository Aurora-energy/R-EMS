---
creation_date: 2025-08-20
modified_date: 2025-08-22 08:42
doc-type:
---
# R-EMS Supervisor

The **`r-ems-supervisor`** is the **control plane** of the R-EMS system.  
It acts as the **supervisory control process** that coordinates all other services (GUI, Registry, edge agents) and ensures safety, availability, and redundancy.

---

## Core responsibility (control loop)

The Supervisor continuously runs a **desired-vs-actual reconciliation loop**:

1. **Discover desired state**  
   - Fetch policies, device manifests, and service configs from the **Registry (8000)**.  
   - Validate them against **Schemas**.

2. **Observe actual state**  
   - Monitor heartbeats and health checks (`/healthz`) from services.  
   - Collect device/edge summaries via logger and configd.

3. **Compute diff**  
   - Compare desired state vs actual state.

4. **Act to converge**  
   - Issue commands via the Bus.  
   - Push config changes.  
   - Trigger failover when needed.

5. **Verify & repeat**  
   - Confirm the system matches policies.  
   - Loop continues indefinitely.

---

## Major subsystems

### 1. Cluster supervision & failover
- Tracks **heartbeats** from controllers/agents.  
- On missing heartbeat:  
  - Marks a **HeartbeatTimeout**.  
  - Promotes a standby controller to primary.  
  - Broadcasts the new leader.  
- Uses monotonic **epoch/term numbers** to prevent split-brain.

### 2. Service health & readiness
- Probes endpoints:  
  - Supervisor: `http://127.0.0.1:7100/healthz`  
  - Registry:  `http://127.0.0.1:8000/healthz`  
  - GUI:       `http://127.0.0.1:8080/healthz`  
- Only proceeds once all required services are ready.  
- Can enter degraded mode if non-critical services fail.

### 3. Policy & configuration distribution
- Registry holds the source of truth.  
- Supervisor validates updates and pushes them through **configd**.  
- Supports rollouts, staged deployments, and rollbacks.

### 4. Edge orchestration & safety
- Devices authenticate with certificates (mTLS).  
- Supervisor issues bounded commands with ACK/NACK.  
- Local safety logic remains on the edge (contactors, thermal trips).  
- Supports store-and-forward for unreliable networks.

### 5. Observability & audit
- **Metrics endpoint** (`/metrics`) for Prometheus.  
- **Structured logs** with trace IDs.  
- **Audit trail** for every change and command.  
- **Alerts** when service levels are breached.

---

## Lifecycle and modes

### Startup sequence
1. Bind to port **7100**.  
2. Fetch bootstrap config from Registry.  
3. Register as controller candidate.  
4. Join election (primary or follower).  
5. Start probes and enter reconcile loop.

### Failure modes
- **Port conflict (7100)** → cannot start.  
- **Registry unavailable** → degraded mode with retries.  
- **Leader election** ensures only one active primary.

---

## Interfaces

### HTTP APIs
- `GET /healthz` – liveness/readiness  
- `GET /status` – current leader, members, versions  
- `GET /metrics` – Prometheus counters & gauges  
- `POST /control/*` – administrative actions  
- `GET /events/stream` – event log feed (SSE/WebSocket)

### Bus Topics
- `hb.controller.*` – controller heartbeats  
- `hb.agent.*` – agent/edge heartbeats  
- `control.cmd.*` / `control.ack.*` – command routing  
- `policy.update` / `policy.applied` – config rollouts  
- `alarm.*` – safety/fault signals

---

## Safety principles
- **Safety first:** risky operations require strong validation and quorum.  
- **Dead-man’s switch:** missing heartbeats lead to safe defaults.  
- **Rate limiting:** prevents bus floods.  
- **Time-bounded commands:** expired actions are discarded.

---

## Visual models

### Control loop
```mermaid
flowchart TD
    A[Desired state (Registry + Policies)] --> B(r-ems-supervisor)
    B --> C[Actual state (health, heartbeats, telemetry)]
    C --> B
    B -->|Control actions| C
