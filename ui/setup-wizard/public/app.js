/*---
ems_section: "06-gui"
ems_subsection: "02-setup-wizard-js"
ems_type: "ui"
ems_scope: "gui"
ems_description: "Client-side controller for the R-EMS setup wizard single-page app."
ems_version: "v0.1.0"
ems_owner: "tbd"
reference: docs/VERSIONING.md
---*/

const statusEndpoint = '/api/status';
const configEndpoint = '/api/config';

const statusField = (id) => document.getElementById(id);
let activityLog;
let configTextarea;
let feedback;
let footerYear;

function logActivity(message, variant = 'info') {
  const entry = document.createElement('li');
  entry.className = `log-${variant}`;
  entry.textContent = `${new Date().toLocaleTimeString()} — ${message}`;
  activityLog.prepend(entry);
  const limit = 20;
  while (activityLog.children.length > limit) {
    activityLog.removeChild(activityLog.lastChild);
  }
}

function formatDuration(seconds) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remaining = seconds % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m ${remaining}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${remaining}s`;
  }
  return `${remaining}s`;
}

async function fetchStatus() {
  try {
    const response = await fetch(statusEndpoint, { cache: 'no-store' });
    if (!response.ok) {
      throw new Error(`Status request failed (${response.status})`);
    }
    const payload = await response.json();
    statusField('status-mode').textContent = payload.mode ?? 'unknown';
    statusField('status-version').textContent = payload.version ?? 'unknown';
    statusField('status-uptime').textContent = formatDuration(payload.uptime_seconds ?? 0);
    statusField('status-grids').textContent = payload.grid_count ?? 0;
    statusField('status-controllers').textContent = payload.controller_count ?? 0;
    logActivity('Status synchronised');
  } catch (error) {
    statusField('status-mode').textContent = 'offline';
    feedback.textContent = error.message;
    feedback.dataset.state = 'error';
    logActivity(error.message, 'error');
  }
}

async function fetchConfig() {
  try {
    const response = await fetch(configEndpoint, { cache: 'no-store' });
    if (!response.ok) {
      throw new Error(`Config request failed (${response.status})`);
    }
    const payload = await response.json();
    const serialised = JSON.stringify(payload, null, 2);
    configTextarea.value = serialised;
    configTextarea.dataset.original = serialised;
    feedback.textContent = 'Configuration loaded from daemon';
    feedback.dataset.state = 'success';
    logActivity('Configuration snapshot refreshed');
  } catch (error) {
    feedback.textContent = error.message;
    feedback.dataset.state = 'error';
    logActivity(error.message, 'error');
  }
}

async function applyConfig(event) {
  event.preventDefault();
  feedback.textContent = 'Applying configuration…';
  feedback.dataset.state = 'pending';
  let payload;
  try {
    payload = JSON.parse(configTextarea.value);
  } catch (error) {
    feedback.textContent = `JSON parse error: ${error.message}`;
    feedback.dataset.state = 'error';
    logActivity(`JSON parse error: ${error.message}`, 'error');
    return;
  }
  try {
    const response = await fetch(configEndpoint, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      const details = await response.json().catch(() => ({ message: response.statusText }));
      throw new Error(details.message ?? 'Failed to apply configuration');
    }
    feedback.textContent = 'Configuration applied successfully';
    feedback.dataset.state = 'success';
    logActivity('Configuration applied');
    configTextarea.dataset.original = configTextarea.value;
    await Promise.all([fetchStatus(), fetchConfig()]);
  } catch (error) {
    feedback.textContent = error.message;
    feedback.dataset.state = 'error';
    logActivity(error.message, 'error');
  }
}

function resetConfig() {
  const baseline = configTextarea.dataset.original;
  if (baseline) {
    configTextarea.value = baseline;
    feedback.textContent = 'Configuration reset to last known snapshot';
    feedback.dataset.state = 'info';
    logActivity('Configuration reset');
  }
}

function bootstrap() {
  activityLog = document.getElementById('activity-log');
  configTextarea = document.getElementById('config-json');
  feedback = document.getElementById('config-feedback');
  footerYear = document.getElementById('footer-year');
  footerYear.textContent = new Date().getFullYear().toString();
  document.getElementById('refresh-status').addEventListener('click', fetchStatus);
  document.getElementById('config-form').addEventListener('submit', applyConfig);
  document.getElementById('reset-config').addEventListener('click', resetConfig);
  fetchStatus();
  fetchConfig();
  setInterval(fetchStatus, 15000);
}

document.addEventListener('DOMContentLoaded', bootstrap);
