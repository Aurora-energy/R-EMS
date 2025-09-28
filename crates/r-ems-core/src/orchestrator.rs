//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use r_ems_common::config::{AppConfig, ControllerConfig, GridConfig, Mode, SimulationConfig};
use r_ems_common::license::LicenseValidation;
use r_ems_common::metrics::LoopTimingReporter;
use r_ems_common::time::jitter_us;
use r_ems_common::version::VersionInfo;
use r_ems_metrics::{OrchestratorMetrics, SharedRegistry};
use r_ems_persistence::PersistenceMetrics;
use r_ems_redundancy::{FailoverEvent, RedundancySupervisor};
use r_ems_rt::RateLimiter;
use r_ems_sim::{SimulationMode, TelemetryFrame, TelemetrySimulationEngine};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::integration_persistence::{log_replayed_entry, PersistenceBridge};
use crate::state::{ControllerSnapshot, SnapshotStore};
use crate::update::{UpdateClient, UpdateCommand, UpdateResult};

const DEFAULT_EVALUATION_INTERVAL_MS: u64 = 200;

/// Primary orchestrator entrypoint.
#[derive(Debug)]
pub struct RemsOrchestrator {
    config: Arc<AppConfig>,
    license: LicenseValidation,
    update_client: UpdateClient,
    mode: Mode,
    version: VersionInfo,
    metrics_registry: Option<SharedRegistry>,
}

impl RemsOrchestrator {
    pub fn new(
        config: AppConfig,
        license: LicenseValidation,
        update_client: UpdateClient,
        metrics: Option<SharedRegistry>,
    ) -> Self {
        let version = VersionInfo::current();
        let mode = config.effective_mode();
        Self {
            config: Arc::new(config),
            license,
            update_client,
            mode,
            version,
            metrics_registry: metrics,
        }
    }

    /// Start all grid runtimes and return a handle for lifecycle control.
    pub async fn start(self) -> Result<OrchestratorHandle> {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(16);
        let mut grid_handles = Vec::new();
        let orchestrator_metrics = match &self.metrics_registry {
            Some(registry) => Some(OrchestratorMetrics::new(registry.clone())?),
            None => None,
        };
        let persistence_metrics = match &self.metrics_registry {
            Some(registry) => Some(Arc::new(PersistenceMetrics::new(registry.clone())?)),
            None => None,
        };

        if let Some(metrics) = &orchestrator_metrics {
            metrics.set_grid_count(self.config.grids.len());
        }
        for (grid_id, grid_config) in &self.config.grids {
            let supervisor = Arc::new(RedundancySupervisor::new(grid_id.clone()));
            let snapshot_store =
                Arc::new(SnapshotStore::from_config(&grid_config.snapshot, grid_id)?);
            let persistence = Arc::new(
                PersistenceBridge::for_grid(snapshot_store.root(), persistence_metrics.clone())
                    .with_context(|| {
                        format!(
                            "failed to initialize persistence bridge for grid {}",
                            grid_id
                        )
                    })?,
            );
            let auto_replay = grid_config.snapshot.auto_replay;
            let handle = GridHandle::spawn(
                grid_id.clone(),
                grid_config.clone(),
                supervisor,
                snapshot_store,
                persistence,
                self.mode,
                self.config.simulation.clone(),
                auto_replay,
                shutdown_rx.resubscribe(),
                orchestrator_metrics.clone(),
            );
            grid_handles.push(handle);
        }

        if let Some(meta) = self.license.metadata() {
            info!(
                key_id = %meta.key_id,
                owner = %meta.owner,
                expires_at = %meta.expires_at,
                "license metadata logged"
            );
        } else {
            warn!("running without validated license metadata");
        }

        info!(mode = ?self.mode, version = %self.version.cli_string(), "orchestrator started");

        Ok(OrchestratorHandle {
            shutdown: shutdown_tx,
            grids: grid_handles,
            update_client: self.update_client.clone(),
            license: self.license.clone(),
            config: self.config.clone(),
            mode: self.mode,
            version: self.version,
            metrics_registry: self.metrics_registry.clone(),
            orchestrator_metrics: orchestrator_metrics.clone(),
        })
    }
}

/// Handle returned from the orchestrator startup that can be used by the CLI.
#[derive(Debug)]
pub struct OrchestratorHandle {
    shutdown: broadcast::Sender<()>,
    grids: Vec<GridHandle>,
    update_client: UpdateClient,
    license: LicenseValidation,
    config: Arc<AppConfig>,
    mode: Mode,
    version: VersionInfo,
    metrics_registry: Option<SharedRegistry>,
    orchestrator_metrics: Option<OrchestratorMetrics>,
}

impl OrchestratorHandle {
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn license(&self) -> &LicenseValidation {
        &self.license
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn version(&self) -> &VersionInfo {
        &self.version
    }

    pub fn metrics(&self) -> Option<SharedRegistry> {
        self.metrics_registry.clone()
    }

    pub fn orchestrator_metrics(&self) -> Option<OrchestratorMetrics> {
        self.orchestrator_metrics.clone()
    }

    pub async fn update(&self, command: UpdateCommand) -> Result<Option<UpdateResult>> {
        match command {
            UpdateCommand::Check => {
                let result = self.update_client.check().await?;
                Ok(Some(result))
            }
            UpdateCommand::Apply => {
                let result = self.update_client.check().await?;
                if result.update_available() {
                    self.update_client.apply(&result).await?;
                }
                Ok(Some(result))
            }
        }
    }

    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown.send(());
        for grid in self.grids {
            grid.join().await?;
        }
        info!("orchestrator shutdown complete");
        Ok(())
    }
}

#[derive(Debug)]
pub struct GridHandle {
    grid_id: String,
    supervisor_task: JoinHandle<()>,
    controller_tasks: Vec<JoinHandle<()>>,
}

impl GridHandle {
    #[allow(clippy::too_many_arguments)]
    fn spawn(
        grid_id: String,
        grid_config: GridConfig,
        supervisor: Arc<RedundancySupervisor>,
        snapshot_store: Arc<SnapshotStore>,
        persistence: Arc<PersistenceBridge>,
        mode: Mode,
        simulation: SimulationConfig,
        auto_replay: bool,
        shutdown: broadcast::Receiver<()>,
        metrics: Option<OrchestratorMetrics>,
    ) -> Self {
        let mut controller_tasks = Vec::new();
        for (controller_id, controller_cfg) in grid_config.controllers.clone() {
            let supervisor_clone = supervisor.clone();
            let snapshot = snapshot_store.clone();
            let persistence_clone = persistence.clone();
            let sim_clone = simulation.clone();
            let metrics_clone = metrics.clone();
            let mut shutdown_rx = shutdown.resubscribe();
            let grid_id_clone = grid_id.clone();
            let controller_id_clone = controller_id.clone();
            let handle = tokio::spawn(async move {
                if let Err(err) = run_controller(
                    &grid_id_clone,
                    &controller_id_clone,
                    controller_cfg,
                    supervisor_clone,
                    snapshot,
                    persistence_clone,
                    mode,
                    sim_clone,
                    auto_replay,
                    metrics_clone,
                    &mut shutdown_rx,
                )
                .await
                {
                    error!(grid = %grid_id_clone, controller = %controller_id_clone, error = %err, "controller loop failed");
                }
            });
            controller_tasks.push(handle);
        }

        let grid_for_supervisor = grid_id.clone();
        let supervisor_clone = supervisor.clone();
        let persistence_for_supervisor = persistence.clone();
        let metrics_for_supervisor = metrics.clone();
        let mut supervisor_shutdown = shutdown;
        let supervisor_task = tokio::spawn(async move {
            let mut evaluation_interval = tokio::time::interval(std::time::Duration::from_millis(
                DEFAULT_EVALUATION_INTERVAL_MS,
            ));
            loop {
                tokio::select! {
                    _ = supervisor_shutdown.recv() => {
                        debug!(grid = %grid_for_supervisor, "supervisor shutdown");
                        break;
                    }
                    _ = evaluation_interval.tick() => {
                        if let Some(event) = supervisor_clone.evaluate(Instant::now()) {
                            if let Some(metrics) = &metrics_for_supervisor {
                                metrics.record_failover(
                                    &event.grid_id,
                                    &event.activated_controller,
                                    &format!("{:?}", event.reason),
                                );
                            }
                            if let Err(err) = persistence_for_supervisor.record_failover(
                                &event.grid_id,
                                &event.activated_controller,
                                &format!("{:?}", event.reason),
                            ) {
                                warn!(grid = %event.grid_id, controller = %event.activated_controller, error = %err, "failed to record failover event");
                            }
                            emit_failover(&event);
                        }
                    }
                }
            }
        });

        Self {
            grid_id,
            supervisor_task,
            controller_tasks,
        }
    }

    async fn join(self) -> Result<()> {
        if let Err(err) = self.supervisor_task.await {
            error!(grid = %self.grid_id, error = %err, "supervisor task join error");
        }
        for handle in self.controller_tasks {
            if let Err(err) = handle.await {
                error!(grid = %self.grid_id, error = %err, "controller task join error");
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_controller(
    grid_id: &str,
    controller_id: &str,
    controller_cfg: ControllerConfig,
    supervisor: Arc<RedundancySupervisor>,
    snapshot_store: Arc<SnapshotStore>,
    persistence: Arc<PersistenceBridge>,
    mode: Mode,
    simulation: SimulationConfig,
    auto_replay: bool,
    metrics: Option<OrchestratorMetrics>,
    shutdown: &mut broadcast::Receiver<()>,
) -> Result<()> {
    let context = supervisor_context(grid_id, controller_id, &controller_cfg);
    supervisor.register(context);
    let mut limiter = RateLimiter::new(controller_cfg.heartbeat_interval);
    let reporter = LoopTimingReporter::new(controller_cfg.heartbeat_interval);
    let mut tick: u64 = 0;
    let mode_label = format!("{:?}", mode);

    if let Some((snapshot, path)) = snapshot_store.load_latest(grid_id, controller_id)? {
        if let Some(saved_tick) = snapshot
            .payload
            .get("tick")
            .and_then(|value| value.as_u64())
        {
            tick = saved_tick;
            info!(
                grid_id,
                controller_id, saved_tick, "restored controller tick from snapshot"
            );
        } else {
            info!(
                grid_id,
                controller_id, "restored latest snapshot without tick metadata"
            );
        }
        if let Err(err) = persistence.record_snapshot_restored(grid_id, controller_id, &path) {
            warn!(grid_id, controller_id, error = %err, "failed to record snapshot restore event");
        }
    }

    if auto_replay {
        let mut replayed = 0usize;
        match persistence.replay_for_controller(grid_id, controller_id, |entry| {
            replayed += 1;
            log_replayed_entry(&entry);
            Ok(())
        }) {
            Ok(_) if replayed > 0 => {
                info!(
                    grid_id,
                    controller_id, replayed, "replayed persistence events for controller"
                );
            }
            Ok(_) => {}
            Err(err) => {
                warn!(grid_id, controller_id, error = %err, "failed to replay controller events");
            }
        }
    }

    let mut sim_engine = build_sim_engine(mode, &simulation)?;
    let exit_after_ticks = controller_cfg
        .metadata
        .get("exit_after_ticks")
        .and_then(|value| value.parse::<u64>().ok());
    let mut was_active = supervisor.is_active(controller_id);
    if let Some(metrics) = &metrics {
        metrics.set_active(grid_id, controller_id, was_active);
    }

    'control_loop: loop {
        tokio::select! {
            _ = shutdown.recv() => {
                debug!(grid_id, controller_id, "controller shutdown signal received");
                break;
            }
            instant = limiter.tick() => {
                let scheduled_at = instant.into_std();
                tick += 1;
                reporter.record_tick();

                let now = Instant::now();
                let status = supervisor.heartbeat(controller_id, now);
                let is_active = supervisor.is_active(controller_id);
                if is_active != was_active {
                    if is_active {
                        info!(grid_id, controller_id, "controller assumed primary role");
                    } else {
                        info!(grid_id, controller_id, "controller entering standby");
                    }
                }
                if let Some(metrics) = &metrics {
                    metrics.set_active(grid_id, controller_id, is_active);
                }
                was_active = is_active;

                let jitter = now.duration_since(scheduled_at);
                let jitter_metric = jitter_us(jitter, controller_cfg.heartbeat_interval);

                if is_active {
                    let telemetry = match &mut sim_engine {
                        Some(engine)
                            if mode.is_simulation()
                                || mode.is_hybrid()
                                || simulation.enable_randomized_inputs =>
                        {
                            engine.next_frame(grid_id, controller_id, tick)
                        }
                        _ => TelemetryFrame::synthetic(grid_id, controller_id, 230.0, 50.0, 20.0),
                    };

                    let telemetry_value = serde_json::to_value(&telemetry)
                        .with_context(|| "failed to serialize telemetry frame")?;
                    let snapshot = ControllerSnapshot {
                        grid_id: grid_id.to_owned(),
                        controller_id: controller_id.to_owned(),
                        captured_at: telemetry.timestamp,
                        payload: serde_json::json!({
                            "telemetry": telemetry_value.clone(),
                            "tick": tick,
                            "mode": mode_label,
                        }),
                    };
                    match snapshot_store.write(&snapshot) {
                        Ok(path) => {
                            if let Err(err) = persistence.record_snapshot_saved(grid_id, controller_id, &path) {
                                warn!(grid_id, controller_id, error = %err, "failed to record snapshot event");
                            }
                        }
                        Err(err) => {
                            persistence.record_snapshot_failure(grid_id, controller_id);
                            warn!(grid_id, controller_id, error = %err, "failed to persist snapshot");
                        }
                    }

                    info!(
                        grid_id,
                        controller_id,
                        role = ?controller_cfg.role,
                        mode = ?mode,
                        tick,
                        jitter_us = jitter_metric,
                        heartbeat_status = ?status,
                        active = true,
                        failover_event = false,
                        "controller tick",
                    );
                    if let Err(err) = persistence.record_controller_tick(
                        grid_id,
                        controller_id,
                        tick,
                        mode_label.as_str(),
                        true,
                        &telemetry_value,
                    ) {
                        warn!(grid_id, controller_id, error = %err, "failed to append controller tick event");
                    }
                } else {
                    debug!(
                        grid_id,
                        controller_id,
                        role = ?controller_cfg.role,
                        tick,
                        jitter_us = jitter_metric,
                        heartbeat_status = ?status,
                        active = false,
                        "standby heartbeat",
                    );
                    if let Err(err) = persistence.record_event(
                        grid_id,
                        controller_id,
                        "controller_heartbeat",
                        serde_json::json!({
                            "tick": tick,
                            "mode": mode_label,
                            "active": false,
                        }),
                    ) {
                        warn!(grid_id, controller_id, error = %err, "failed to append heartbeat event");
                    }
                }

                if let Some(limit) = exit_after_ticks {
                    if tick >= limit {
                        warn!(grid_id, controller_id, limit = limit, "controller reached exit_after_ticks; simulating failure");
                        break 'control_loop;
                    }
                }
            }
        }
    }

    if let Some(metrics) = &metrics {
        metrics.set_active(grid_id, controller_id, false);
    }

    if let Some(summary) = reporter.histogram().summary() {
        let path = format!("target/test-logs/{}-{}-jitter.json", grid_id, controller_id);
        if let Err(err) = std::fs::create_dir_all("target/test-logs") {
            warn!(error = %err, "unable to create test-logs directory");
        }
        if let Err(err) = reporter.histogram().write_json(&path) {
            warn!(grid_id, controller_id, error = %err, "failed to write jitter histogram");
        }
        debug!(
            grid_id,
            controller_id,
            samples = summary.samples,
            mean_ns = summary.mean_ns,
            std_dev_ns = summary.std_dev_ns,
            "controller jitter summary"
        );
    }
    Ok(())
}

fn emit_failover(event: &FailoverEvent) {
    info!(
        grid_id = %event.grid_id,
        controller_id = %event.activated_controller,
        failover_event = true,
        reason = ?event.reason,
        "controller promotion"
    );
}

fn build_sim_engine(
    mode: Mode,
    simulation: &SimulationConfig,
) -> Result<Option<TelemetrySimulationEngine>> {
    if simulation.scenario_files.is_empty() {
        if mode.is_simulation() || mode.is_hybrid() || simulation.enable_randomized_inputs {
            return Ok(Some(TelemetrySimulationEngine::new(
                SimulationMode::Randomized,
                simulation.random_seed,
            )?));
        }
        return Ok(None);
    }
    let scenario_path = simulation.scenario_files[0].clone();
    let sim_mode = if mode.is_hybrid() {
        SimulationMode::Hybrid {
            scenario: scenario_path,
            noise_sigma: 0.2,
        }
    } else {
        SimulationMode::Scenario(scenario_path)
    };
    TelemetrySimulationEngine::new(sim_mode, simulation.random_seed).map(Some)
}

fn supervisor_context(
    grid_id: &str,
    controller_id: &str,
    cfg: &ControllerConfig,
) -> r_ems_redundancy::ControllerContext {
    r_ems_redundancy::ControllerContext::from_config(grid_id, controller_id, cfg)
}
