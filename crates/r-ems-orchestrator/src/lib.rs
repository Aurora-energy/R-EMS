//! ---
//! ems_section: "04-configuration-orchestration"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "High-level orchestration kernel coordinating grids and redundancy."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use r_ems_common::config::{ControllerConfig, ControllerRole};
use r_ems_redundancy::{ControllerContext, HeartbeatStatus, RedundancySupervisor};
use thiserror::Error;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Default cadence used by the redundancy supervisor to evaluate controller health.
pub const DEFAULT_SUPERVISOR_EVALUATION: Duration = Duration::from_millis(100);

/// Configuration describing the orchestration layout.
#[derive(Debug, Clone, Default)]
pub struct OrchestratorSpec {
    /// All grids that should be launched by the kernel.
    pub grids: Vec<GridSpec>,
    /// Optional override for the redundancy supervisor evaluation cadence.
    pub evaluation_interval: Option<Duration>,
}

impl OrchestratorSpec {
    /// Convenience builder for tests to construct a multi-grid environment.
    pub fn with_grid(mut self, grid: GridSpec) -> Self {
        self.grids.push(grid);
        self
    }
}

/// Configuration for a single simulated grid.
#[derive(Debug, Clone)]
pub struct GridSpec {
    /// Stable identifier for the grid.
    pub grid_id: String,
    /// Controller instances attached to the grid.
    pub controllers: Vec<ControllerSpec>,
}

impl GridSpec {
    /// Create a new grid specification.
    pub fn new(grid_id: impl Into<String>, controllers: Vec<ControllerSpec>) -> Self {
        Self {
            grid_id: grid_id.into(),
            controllers,
        }
    }
}

/// Configuration for an individual controller runtime.
#[derive(Debug, Clone)]
pub struct ControllerSpec {
    /// Identifier for the controller instance.
    pub controller_id: String,
    /// Static configuration applied to the controller loop.
    pub config: ControllerConfig,
}

impl ControllerSpec {
    /// Construct a controller specification with the supplied configuration.
    pub fn new(controller_id: impl Into<String>, config: ControllerConfig) -> Self {
        Self {
            controller_id: controller_id.into(),
            config,
        }
    }

    /// Convenience helper constructing a primary controller with sensible defaults.
    pub fn primary(controller_id: impl Into<String>) -> Self {
        let mut cfg = ControllerConfig::default();
        cfg.role = ControllerRole::Primary;
        Self::new(controller_id, cfg)
    }

    /// Convenience helper constructing a secondary controller with sensible defaults.
    pub fn secondary(controller_id: impl Into<String>) -> Self {
        let mut cfg = ControllerConfig::default();
        cfg.role = ControllerRole::Secondary;
        cfg.failover_order = 1;
        Self::new(controller_id, cfg)
    }
}

/// Runtime handle returned after the kernel has spawned all grids.
#[derive(Debug)]
pub struct OrchestratorHandle {
    grids: HashMap<String, Arc<GridRuntimeHandle>>,
    shutdown: broadcast::Sender<()>,
}

impl OrchestratorHandle {
    /// Retrieve a handle to the requested grid if it exists.
    pub fn grid(&self, grid_id: &str) -> Option<GridView> {
        self.grids.get(grid_id).map(|grid| GridView {
            inner: grid.clone(),
        })
    }

    /// Forcefully terminate a controller task to simulate a fault.
    pub async fn kill_controller(&self, grid_id: &str, controller_id: &str) -> bool {
        match self.grids.get(grid_id) {
            Some(grid) => grid.kill_controller(controller_id).await,
            None => false,
        }
    }

    /// Trigger an emergency stop for the grid, halting all controllers and emitting a bus event.
    pub async fn emergency_stop(&self, grid_id: &str) -> bool {
        let Some(grid) = self.grids.get(grid_id) else {
            return false;
        };
        grid.peripherals.emergency_stop();
        grid.shutdown().await;
        true
    }

    /// Shutdown every grid managed by the orchestrator.
    pub async fn shutdown(&self) {
        let handles: Vec<Arc<GridRuntimeHandle>> = self.grids.values().cloned().collect();
        for grid in handles {
            grid.shutdown().await;
        }
        let _ = self.shutdown.send(());
    }
}

/// Public view over runtime information of a grid.
#[derive(Clone, Debug)]
pub struct GridView {
    inner: Arc<GridRuntimeHandle>,
}

impl GridView {
    /// Access the redundancy supervisor backing the grid.
    pub fn supervisor(&self) -> Arc<RedundancySupervisor> {
        self.inner.supervisor.clone()
    }

    /// Access the in-memory snapshot stub capturing controller state.
    pub fn snapshots(&self) -> Arc<SnapshotStoreStub> {
        self.inner.snapshots.clone()
    }

    /// Access the simulated peripheral bus used for actuator commands.
    pub fn peripherals(&self) -> Arc<PeripheralBus> {
        self.inner.peripherals.clone()
    }
}

/// Primary entrypoint creating the orchestration kernel.
#[derive(Debug, Default)]
pub struct OrchestratorKernel;

impl OrchestratorKernel {
    /// Start all configured grids and return a handle that can manipulate the runtime.
    pub async fn start(spec: OrchestratorSpec) -> OrchestratorHandle {
        let evaluation_interval = spec
            .evaluation_interval
            .unwrap_or(DEFAULT_SUPERVISOR_EVALUATION);
        let (shutdown, _) = broadcast::channel(4);
        let mut grids = HashMap::new();

        for grid in spec.grids {
            let supervisor = Arc::new(RedundancySupervisor::new(grid.grid_id.clone()));
            let snapshots = Arc::new(SnapshotStoreStub::default());
            let peripherals =
                Arc::new(PeripheralBus::new(grid.grid_id.clone(), supervisor.clone()));
            let (grid_shutdown, _) = broadcast::channel(4);
            let supervisor_task = spawn_supervisor_task(
                grid.grid_id.clone(),
                supervisor.clone(),
                evaluation_interval,
                grid_shutdown.subscribe(),
            );

            let mut controller_handles = HashMap::new();
            for controller in grid.controllers {
                let runtime = spawn_controller_task(
                    grid.grid_id.clone(),
                    controller,
                    supervisor.clone(),
                    snapshots.clone(),
                    peripherals.clone(),
                    grid_shutdown.subscribe(),
                );
                controller_handles.insert(runtime.controller_id().to_owned(), runtime);
            }

            let handle = Arc::new(GridRuntimeHandle {
                grid_id: grid.grid_id,
                supervisor,
                snapshots,
                peripherals,
                controllers: Mutex::new(controller_handles),
                supervisor_task: Mutex::new(Some(supervisor_task)),
                shutdown: grid_shutdown,
            });
            grids.insert(handle.grid_id.clone(), handle);
        }

        OrchestratorHandle { grids, shutdown }
    }
}

fn spawn_supervisor_task(
    grid_id: String,
    supervisor: Arc<RedundancySupervisor>,
    evaluation_interval: Duration,
    mut shutdown: broadcast::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = interval(evaluation_interval);
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    debug!(%grid_id, "supervisor shutdown signal received");
                    break;
                }
                _ = interval.tick() => {
                    if let Some(event) = supervisor.evaluate(Instant::now()) {
                        info!(grid = %event.grid_id, controller = %event.activated_controller, reason = ?event.reason, "failover event");
                    }
                }
            }
        }
    })
}

fn spawn_controller_task(
    grid_id: String,
    controller: ControllerSpec,
    supervisor: Arc<RedundancySupervisor>,
    snapshots: Arc<SnapshotStoreStub>,
    peripherals: Arc<PeripheralBus>,
    mut shutdown: broadcast::Receiver<()>,
) -> ControllerRuntime {
    let (kill_tx, mut kill_rx) = watch::channel(false);
    let controller_id = controller.controller_id.clone();
    let cfg = controller.config.clone();
    let task = tokio::spawn(async move {
        let context = ControllerContext::from_config(&grid_id, &controller_id, &cfg);
        supervisor.register(context);
        let mut tick: u64 = 0;
        let mut ticker = interval(cfg.heartbeat_interval);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    debug!(grid = %grid_id, controller = %controller_id, "controller shutdown received");
                    break;
                }
                changed = kill_rx.changed() => {
                    match changed {
                        Ok(()) => {
                            if *kill_rx.borrow() {
                                warn!(grid = %grid_id, controller = %controller_id, "controller kill switch triggered");
                                break;
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    tick += 1;
                    let now = Instant::now();
                    let status = supervisor.heartbeat(&controller_id, now);
                    let active = supervisor.is_active(&controller_id);
                    snapshots.record(SnapshotRecord {
                        grid_id: grid_id.clone(),
                        controller_id: controller_id.clone(),
                        tick,
                        active,
                        heartbeat_status: status,
                    });
                    if active {
                        let command = PeripheralCommand::SetPoint {
                            target_kw: 250.0 + tick as f64,
                        };
                        if let Err(err) = peripherals.commit_with_tick(&controller_id, command, Some(tick)) {
                            warn!(grid = %grid_id, controller = %controller_id, error = %err, "failed to commit actuator command");
                        } else {
                            debug!(grid = %grid_id, controller = %controller_id, tick, "actuator command committed");
                        }
                    } else {
                        debug!(grid = %grid_id, controller = %controller_id, tick, "standby heartbeat");
                    }
                }
            }
        }
        debug!(grid = %grid_id, controller = %controller_id, tick, "controller loop exited");
    });

    ControllerRuntime::new(controller_id, kill_tx, task)
}

#[derive(Debug)]
struct GridRuntimeHandle {
    grid_id: String,
    supervisor: Arc<RedundancySupervisor>,
    snapshots: Arc<SnapshotStoreStub>,
    peripherals: Arc<PeripheralBus>,
    controllers: Mutex<HashMap<String, ControllerRuntime>>,
    supervisor_task: Mutex<Option<JoinHandle<()>>>,
    shutdown: broadcast::Sender<()>,
}

impl GridRuntimeHandle {
    async fn kill_controller(&self, controller_id: &str) -> bool {
        let controller = {
            let controllers = self.controllers.lock();
            controllers.get(controller_id).cloned()
        };
        let Some(runtime) = controller else {
            return false;
        };
        runtime.kill();
        runtime.join().await;
        true
    }

    async fn shutdown(&self) {
        let _ = self.shutdown.send(());
        let controllers: Vec<ControllerRuntime> = {
            let controllers = self.controllers.lock();
            controllers.values().cloned().collect()
        };
        for controller in controllers {
            controller.join().await;
        }
        self.controllers.lock().clear();
        if let Some(task) = self.supervisor_task.lock().take() {
            if let Err(err) = task.await {
                error!(grid = %self.grid_id, error = %err, "supervisor join error");
            }
        }
    }
}

/// Runtime controller handle that exposes lifecycle toggles.
#[derive(Clone, Debug)]
struct ControllerRuntime {
    controller_id: String,
    kill_tx: watch::Sender<bool>,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl ControllerRuntime {
    fn new(controller_id: String, kill_tx: watch::Sender<bool>, task: JoinHandle<()>) -> Self {
        Self {
            controller_id,
            kill_tx,
            task: Arc::new(Mutex::new(Some(task))),
        }
    }

    fn controller_id(&self) -> &str {
        &self.controller_id
    }

    fn kill(&self) {
        let _ = self.kill_tx.send(true);
    }

    async fn join(&self) {
        let handle = self.task.lock().take();
        if let Some(task) = handle {
            if let Err(err) = task.await {
                warn!(controller = %self.controller_id, error = %err, "controller join error");
            }
        }
    }
}

/// Simplified snapshot store capturing controller ticks in memory.
#[derive(Debug, Default)]
pub struct SnapshotStoreStub {
    records: Mutex<Vec<SnapshotRecord>>,
}

impl SnapshotStoreStub {
    /// Record a snapshot for later inspection.
    pub fn record(&self, record: SnapshotRecord) {
        self.records.lock().push(record);
    }

    /// Retrieve all captured snapshots.
    pub fn all(&self) -> Vec<SnapshotRecord> {
        self.records.lock().clone()
    }
}

/// Snapshot metadata recorded for diagnostics and tests.
#[derive(Debug, Clone)]
pub struct SnapshotRecord {
    /// Identifier for the grid.
    pub grid_id: String,
    /// Identifier for the controller.
    pub controller_id: String,
    /// Monotonic tick counter.
    pub tick: u64,
    /// Whether the controller was active when the snapshot was recorded.
    pub active: bool,
    /// Observed heartbeat state for the tick.
    pub heartbeat_status: HeartbeatStatus,
}

/// Commands emitted to simulated peripherals.
#[derive(Debug, Clone, PartialEq)]
pub enum PeripheralCommand {
    /// Adjust an actuator set point to the supplied kilowatt target.
    SetPoint { target_kw: f64 },
    /// Emergency stop broadcast halting all motion.
    EmergencyStop,
}

/// Event tracked by the in-memory peripheral bus.
#[derive(Debug, Clone)]
pub struct PeripheralEvent {
    /// Controller responsible for the command. Emergency stop events use `SYSTEM`.
    pub controller_id: String,
    /// Associated grid identifier.
    pub grid_id: String,
    /// Command payload that was attempted.
    pub command: PeripheralCommand,
    /// Tick counter supplied by the issuing controller, if any.
    pub tick: Option<u64>,
}

/// Errors returned when committing to the peripheral bus fails.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PeripheralError {
    /// Controller attempted to commit while not the active primary.
    #[error("controller {controller} is not the active primary")]
    ControllerNotPrimary { controller: String },
}

/// In-memory peripheral bus verifying actuator commits only originate from primaries.
#[derive(Debug)]
pub struct PeripheralBus {
    grid_id: String,
    supervisor: Arc<RedundancySupervisor>,
    events: Mutex<Vec<PeripheralEvent>>,
}

impl PeripheralBus {
    fn new(grid_id: String, supervisor: Arc<RedundancySupervisor>) -> Self {
        Self {
            grid_id,
            supervisor,
            events: Mutex::new(Vec::new()),
        }
    }

    /// Commit an actuator command from the provided controller.
    pub fn commit(
        &self,
        controller_id: &str,
        command: PeripheralCommand,
    ) -> Result<(), PeripheralError> {
        self.commit_with_tick(controller_id, command, None)
    }

    /// Commit an actuator command while attaching a tick reference.
    pub fn commit_with_tick(
        &self,
        controller_id: &str,
        command: PeripheralCommand,
        tick: Option<u64>,
    ) -> Result<(), PeripheralError> {
        if !self.supervisor.is_active(controller_id) {
            return Err(PeripheralError::ControllerNotPrimary {
                controller: controller_id.to_owned(),
            });
        }
        self.events.lock().push(PeripheralEvent {
            controller_id: controller_id.to_owned(),
            grid_id: self.grid_id.clone(),
            tick,
            command,
        });
        Ok(())
    }

    /// Broadcast an emergency stop command.
    pub fn emergency_stop(&self) {
        self.events.lock().push(PeripheralEvent {
            controller_id: "SYSTEM".to_owned(),
            grid_id: self.grid_id.clone(),
            tick: None,
            command: PeripheralCommand::EmergencyStop,
        });
    }

    /// Retrieve a snapshot of all events emitted so far.
    pub fn events(&self) -> Vec<PeripheralEvent> {
        self.events.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use tokio::time::{sleep, timeout};

    fn fast_primary() -> ControllerConfig {
        ControllerConfig {
            role: ControllerRole::Primary,
            heartbeat_interval: Duration::from_millis(40),
            watchdog_timeout: Duration::from_millis(120),
            failover_order: 0,
            sensor_inputs: Vec::new(),
            metadata: Default::default(),
        }
    }

    fn fast_secondary() -> ControllerConfig {
        ControllerConfig {
            role: ControllerRole::Secondary,
            heartbeat_interval: Duration::from_millis(40),
            watchdog_timeout: Duration::from_millis(120),
            failover_order: 1,
            sensor_inputs: Vec::new(),
            metadata: Default::default(),
        }
    }

    fn build_two_by_two_spec() -> OrchestratorSpec {
        let grid_a = GridSpec::new(
            "grid-a",
            vec![
                ControllerSpec::new("ctrl-a-primary", fast_primary()),
                ControllerSpec::new("ctrl-a-secondary", fast_secondary()),
            ],
        );
        let grid_b = GridSpec::new(
            "grid-b",
            vec![
                ControllerSpec::new("ctrl-b-primary", fast_primary()),
                ControllerSpec::new("ctrl-b-secondary", fast_secondary()),
            ],
        );
        OrchestratorSpec {
            grids: vec![grid_a, grid_b],
            evaluation_interval: Some(Duration::from_millis(30)),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn standby_promoted_when_primary_fails() {
        let handle = OrchestratorKernel::start(build_two_by_two_spec()).await;
        let grid = handle.grid("grid-a").expect("grid exists");

        sleep(Duration::from_millis(150)).await;
        assert!(
            grid.supervisor().is_active("ctrl-a-primary"),
            "primary should initially be active"
        );

        assert!(handle.kill_controller("grid-a", "ctrl-a-primary").await);

        sleep(Duration::from_millis(250)).await;
        assert!(
            grid.supervisor().is_active("ctrl-a-secondary"),
            "secondary promoted after primary failure"
        );

        let events = grid.peripherals().events();
        assert!(events
            .iter()
            .any(|event| event.controller_id == "ctrl-a-secondary"));
        handle.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn peripherals_reject_standby_commits() {
        let handle = OrchestratorKernel::start(build_two_by_two_spec()).await;
        let grid = handle.grid("grid-b").expect("grid exists");

        sleep(Duration::from_millis(100)).await;
        let bus = grid.peripherals();
        let err = bus
            .commit(
                "ctrl-b-secondary",
                PeripheralCommand::SetPoint { target_kw: 10.0 },
            )
            .expect_err("standby commit must fail");
        assert_eq!(
            err,
            PeripheralError::ControllerNotPrimary {
                controller: "ctrl-b-secondary".to_owned()
            }
        );
        handle.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn emergency_stop_halts_controllers_and_records_event() {
        let handle = OrchestratorKernel::start(build_two_by_two_spec()).await;
        sleep(Duration::from_millis(120)).await;
        assert!(handle.emergency_stop("grid-a").await);

        // Ensure no controller tasks remain by waiting on shutdown twice (idempotent).
        let shutdown = handle.shutdown().map(|_| ());
        timeout(Duration::from_secs(1), shutdown)
            .await
            .expect("shutdown completes promptly");

        let events = handle
            .grid("grid-a")
            .expect("grid view persists")
            .peripherals()
            .events();
        assert!(events
            .iter()
            .any(|event| event.command == PeripheralCommand::EmergencyStop));
    }
}
