# Troubleshooting

This guide collects quick fixes for the most common issues that block a fresh
R-EMS development environment from starting. When the setup helper reports a
problem, locate the matching symptom below and follow the resolution steps.

## Freeing a conflicting port during setup

**Symptom**: `scripts/setup.sh` or `scripts/setup_web.sh` logs messages similar to:

```
[ERROR] Port 7100 is currently in use. Free the port before launching the web services.
```

The setup helpers check the three ports used by the local stack:

| Port | Service     | Default endpoint                    |
| ---- | ----------- | ----------------------------------- |
| 7100 | Supervisor  | http://127.0.0.1:7100/healthz        |
| 8000 | Registry    | http://127.0.0.1:8000/healthz        |
| 8080 | GUI         | http://127.0.0.1:8080/healthz        |

When any of these ports are occupied, Docker cannot bind the new container and
the scripts abort. This usually happens when:

* A previous `run_core_stack.sh up` session is still running.
* A stale container was left behind after an interrupted setup.
* Another local application coincidentally uses the same port.

### 1. Identify the process that owns the port

Pick one of the commands below (all are safe to run as a normal user):

```bash
# Linux (requires the iproute2 `ss` utility, installed on most distros)
ss -ltnp 'sport = :7100'

# macOS and many Linux distributions
lsof -i :7100

# Fallback when the above tools are unavailable
netstat -ltnp | grep ':7100'
```

The output shows the PID/command that is bound to the port. If it is an
R-EMS container the command column will mention `docker` and the container name
(e.g. `profiles-supervisor-1`).

### 2. Stop the conflicting service

* To shut down a running R-EMS stack, execute:

  ```bash
  ./scripts/run_core_stack.sh down --remove-orphans
  ```

* To remove leftover containers manually, list the running containers and stop
  the one that exposes the conflicting port:

  ```bash
  docker ps --format 'table {{.ID}}\t{{.Names}}\t{{.Ports}}'
  docker stop <container-id>
  ```

* If the process belongs to another application, close that application or stop
  its service. On Linux you can also terminate it directly:

  ```bash
  sudo kill <pid>
  ```

### 3. Retry the setup

Run the relevant setup helper again after the port is free:

```bash
./scripts/setup.sh      # verifies the ports and rebuilds images
./scripts/setup_web.sh  # restarts the compose stack and runs health checks
```

If the port remains in use, double-check that the process is gone and that no
system service restarts it automatically.

### Optional: change the exposed port

If you cannot free the port because another critical application uses it,
you can temporarily change the published port in
`compose/profiles/profile.core.yml`. For example, to expose the supervisor on
port 7200 instead of 7100, update the mapping to `"7200:7100"` and rerun the
setup script. Remember to visit `http://localhost:7200/healthz` for health
checks afterward.

## Need more help?

Collect the relevant logs from `scripts/setup.log` and `scripts/setup_web.log`,
note the commands you ran, and open a discussion or support ticket with the
details. The maintainers can usually point to the next debugging steps once
they see the log output.
