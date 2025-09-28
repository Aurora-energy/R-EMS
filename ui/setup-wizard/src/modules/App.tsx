/**
---
ems_section: "12-gui"
ems_subsection: "06-setup-wizard"
ems_type: "frontend-component"
ems_scope: "gui"
ems_description: "Interactive setup wizard and dashboard for R-EMS operators."
ems_version: "v0.2.0"
ems_owner: "tbd"
ems_references:
  - /docs/VERSIONING.md
  - /README.md
---
*/
import { useCallback, useEffect, useMemo, useState } from "react";

const statusEndpoint = "/api/status";
const configEndpoint = "/api/config";
const logsEndpoint = "/api/logs";
const githubIssuesUrl = "https://github.com/kentthoresen/R-EMS/issues/new";

const wizardSteps = [
  "Welcome",
  "Configuration",
  "Diagnostics",
  "Dashboard",
] as const;

const docsLinks = [
  {
    title: "Operations Guide",
    description: "Covers day-to-day orchestration, licence management, and maintenance windows.",
    href: "/docs/OPERATIONS.md",
  },
  {
    title: "Deployment Reference",
    description: "Documented environment variables, container expectations, and rolling updates.",
    href: "/docs/DEPLOYMENT.md",
  },
  {
    title: "Troubleshooting",
    description: "Collects common failure patterns, log signatures, and recovery procedures.",
    href: "/docs/TROUBLESHOOTING.md",
  },
];

type WizardStep = (typeof wizardSteps)[number];

type StatusResponse = {
  mode: string;
  version: string;
  git_commit: string;
  uptime_seconds: number;
  grid_count: number;
  controller_count: number;
  features: Record<string, boolean>;
};

type LogFileSummary = {
  name: string;
  path: string;
  size_bytes: number;
  modified: string | null;
};

type LogErrorEntry = {
  timestamp: string;
  level: string;
  message: string;
  file?: string;
  line?: number;
  target?: string;
  source: string;
  raw: string;
};

type LogOverview = {
  files: LogFileSummary[];
  recent_errors: LogErrorEntry[];
};

type FeedbackState = "idle" | "pending" | "success" | "error";

const formatDuration = (seconds: number): string => {
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
};

const formatBytes = (bytes: number): string => {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes;
  let unitIndex = -1;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size.toFixed(1)} ${units[unitIndex]}`;
};

const buildIssueUrl = (
  error: LogErrorEntry | null,
  status: StatusResponse | null,
  configFeedback: string | null,
): string => {
  const url = new URL(githubIssuesUrl);
  url.searchParams.set("labels", "bug,gui");
  const title = error
    ? `[GUI] ${error.message}`.slice(0, 80)
    : "[GUI] Setup wizard feedback";
  url.searchParams.set("title", title);

  const lines: string[] = ["### Summary", "", error ? error.message : "Describe the issue"].filter(Boolean);

  lines.push("", "### Environment", "");
  if (status) {
    lines.push(
      `- Version: ${status.version} (${status.git_commit})`,
      `- Mode: ${status.mode}`,
      `- Uptime: ${formatDuration(status.uptime_seconds)}`,
      `- Grids: ${status.grid_count} / Controllers: ${status.controller_count}`,
    );
  } else {
    lines.push("- Status information unavailable");
  }

  if (configFeedback) {
    lines.push("", `> Last configuration action: ${configFeedback}`);
  }

  if (error) {
    lines.push(
      "",
      "### Relevant Log Entry",
      "",
      "```json",
      error.raw,
      "```",
    );
    lines.push("", `Source: ${error.source}`);
    if (error.file) {
      lines.push(`File: ${error.file}${error.line ? `:${error.line}` : ""}`);
    }
  }

  lines.push("", "### Steps to Reproduce", "", "1. ", "2. ", "3. ");

  url.searchParams.set("body", lines.join("\n"));
  return url.toString();
};

export const App: React.FC = () => {
  const [activeStep, setActiveStep] = useState<WizardStep>("Welcome");
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [configDraft, setConfigDraft] = useState("");
  const [configBaseline, setConfigBaseline] = useState("");
  const [isSavingConfig, setIsSavingConfig] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(false);
  const [configFeedback, setConfigFeedback] = useState<string | null>(null);
  const [configFeedbackState, setConfigFeedbackState] = useState<FeedbackState>("idle");
  const [logs, setLogs] = useState<LogOverview | null>(null);
  const [logsError, setLogsError] = useState<string | null>(null);
  const [selectedError, setSelectedError] = useState<LogErrorEntry | null>(null);

  const loadStatus = useCallback(async () => {
    try {
      setStatusError(null);
      const response = await fetch(statusEndpoint, { cache: "no-store" });
      if (!response.ok) {
        throw new Error(`Status request failed (${response.status})`);
      }
      const payload = (await response.json()) as StatusResponse;
      setStatus(payload);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to load status";
      setStatusError(message);
    }
  }, []);

  const loadConfig = useCallback(async () => {
    try {
      setIsLoadingConfig(true);
      const response = await fetch(configEndpoint, { cache: "no-store" });
      if (!response.ok) {
        throw new Error(`Config request failed (${response.status})`);
      }
      const payload = await response.json();
      const serialised = JSON.stringify(payload, null, 2);
      setConfigBaseline(serialised);
      setConfigDraft(serialised);
      setConfigFeedback("Configuration loaded from daemon");
      setConfigFeedbackState("success");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to load configuration";
      setConfigFeedback(message);
      setConfigFeedbackState("error");
    } finally {
      setIsLoadingConfig(false);
    }
  }, []);

  const loadLogs = useCallback(async () => {
    try {
      setLogsError(null);
      const response = await fetch(logsEndpoint, { cache: "no-store" });
      if (!response.ok) {
        throw new Error(`Log request failed (${response.status})`);
      }
      const payload = (await response.json()) as LogOverview;
      setLogs(payload);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to load log overview";
      setLogsError(message);
    }
  }, []);

  useEffect(() => {
    loadStatus();
    const timer = window.setInterval(() => {
      loadStatus();
    }, 15_000);
    return () => window.clearInterval(timer);
  }, [loadStatus]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    loadLogs();
    const timer = window.setInterval(() => {
      loadLogs();
    }, 20_000);
    return () => window.clearInterval(timer);
  }, [loadLogs]);

  useEffect(() => {
    if (!selectedError && logs?.recent_errors.length) {
      setSelectedError(logs.recent_errors[0]);
    }
  }, [logs, selectedError]);

  const handleConfigChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    setConfigDraft(event.target.value);
    setConfigFeedbackState("idle");
    setConfigFeedback(null);
  };

  const handleConfigReset = () => {
    setConfigDraft(configBaseline);
    setConfigFeedback("Reverted to last saved configuration snapshot");
    setConfigFeedbackState("success");
  };

  const handleConfigSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    let parsed: unknown;
    try {
      parsed = JSON.parse(configDraft);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown parse error";
      setConfigFeedback(`JSON parse error: ${message}`);
      setConfigFeedbackState("error");
      return;
    }

    try {
      setIsSavingConfig(true);
      setConfigFeedback("Applying configuration…");
      setConfigFeedbackState("pending");
      const response = await fetch(configEndpoint, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(parsed),
      });
      if (!response.ok) {
        const details = await response.json().catch(() => ({ message: response.statusText }));
        throw new Error(details.message ?? "Failed to apply configuration");
      }
      setConfigFeedback("Configuration applied successfully");
      setConfigFeedbackState("success");
      setConfigBaseline(configDraft);
      await Promise.all([loadStatus(), loadLogs()]);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to apply configuration";
      setConfigFeedback(message);
      setConfigFeedbackState("error");
    } finally {
      setIsSavingConfig(false);
    }
  };

  const issueUrl = useMemo(
    () => buildIssueUrl(selectedError, status, configFeedback),
    [selectedError, status, configFeedback],
  );

  const stepIndex = wizardSteps.indexOf(activeStep);
  const canGoBack = stepIndex > 0;
  const canGoNext = stepIndex < wizardSteps.length - 1;

  const renderStatusSummary = () => (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
        <p className="text-xs uppercase tracking-widest text-slate-400">Mode</p>
        <p className="mt-2 text-xl font-semibold text-white">{status?.mode ?? "unknown"}</p>
        <p className="mt-1 text-xs text-slate-400">Runtime orchestration mode</p>
      </div>
      <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
        <p className="text-xs uppercase tracking-widest text-slate-400">Version</p>
        <p className="mt-2 text-xl font-semibold text-white">{status?.version ?? "n/a"}</p>
        <p className="mt-1 text-xs text-slate-400">Commit {status?.git_commit.slice(0, 8) ?? "-"}</p>
      </div>
      <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
        <p className="text-xs uppercase tracking-widest text-slate-400">Uptime</p>
        <p className="mt-2 text-xl font-semibold text-white">
          {status ? formatDuration(status.uptime_seconds) : "-"}
        </p>
        <p className="mt-1 text-xs text-slate-400">Since last daemon restart</p>
      </div>
      <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
        <p className="text-xs uppercase tracking-widest text-slate-400">Topology</p>
        <p className="mt-2 text-xl font-semibold text-white">
          {status ? `${status.grid_count} grids` : "-"}
        </p>
        <p className="mt-1 text-xs text-slate-400">
          {status ? `${status.controller_count} controllers` : "Awaiting status"}
        </p>
      </div>
    </div>
  );

  const renderWelcome = () => (
    <div className="space-y-8">
      <section className="space-y-4">
        <h2 className="text-3xl font-semibold text-white">Welcome to R-EMS Setup</h2>
        <p className="text-sm leading-relaxed text-slate-300">
          The setup wizard guides operators through applying configuration, validating controller health, and
          launching the dashboard. Use the navigation to revisit steps as needed&mdash;changes can be applied at any
          time thanks to the REST-backed configuration API.
        </p>
        {statusError ? (
          <div className="rounded-lg border border-red-500/40 bg-red-500/10 p-4 text-sm text-red-200">
            {statusError}
          </div>
        ) : (
          renderStatusSummary()
        )}
      </section>
      <section className="space-y-4">
        <h3 className="text-xl font-semibold text-white">Documentation</h3>
        <div className="grid gap-4 lg:grid-cols-3">
          {docsLinks.map((link) => (
            <a
              key={link.title}
              href={link.href}
              className="group block rounded-xl border border-slate-800 bg-slate-900/50 p-5 transition hover:border-sky-500/60 hover:bg-slate-900"
              target="_blank"
              rel="noreferrer"
            >
              <p className="text-sm font-semibold text-sky-300">{link.title}</p>
              <p className="mt-2 text-xs leading-relaxed text-slate-300">{link.description}</p>
              <span className="mt-4 inline-flex items-center gap-2 text-xs font-medium text-sky-400">
                Open guide
                <svg
                  className="h-4 w-4 transition group-hover:translate-x-0.5"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 19.5l15-15M9 4.5h10.5V15" />
                </svg>
              </span>
            </a>
          ))}
        </div>
      </section>
    </div>
  );

  const renderConfiguration = () => (
    <form onSubmit={handleConfigSubmit} className="space-y-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 className="text-2xl font-semibold text-white">Configuration Editor</h2>
          <p className="text-xs text-slate-400">
            The editor interacts with <code className="rounded bg-slate-800 px-1 py-0.5">/api/config</code>. Changes are
            validated server-side before being written back to disk.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <button
            type="button"
            onClick={loadConfig}
            className="rounded-lg border border-slate-700 px-3 py-1 text-slate-200 transition hover:border-sky-500/60 hover:text-sky-200"
            disabled={isLoadingConfig}
          >
            {isLoadingConfig ? "Refreshing…" : "Reload from daemon"}
          </button>
          <button
            type="button"
            onClick={handleConfigReset}
            className="rounded-lg border border-slate-700 px-3 py-1 text-slate-200 transition hover:border-emerald-500/60 hover:text-emerald-200"
            disabled={configDraft === configBaseline}
          >
            Reset edits
          </button>
        </div>
      </header>
      <textarea
        value={configDraft}
        onChange={handleConfigChange}
        className="min-h-[28rem] w-full rounded-xl border border-slate-800 bg-slate-950/80 p-4 font-mono text-sm text-slate-100 shadow-inner shadow-slate-950 focus:border-sky-500 focus:outline-none"
        spellCheck={false}
        aria-label="Configuration JSON"
      />
      <div className="flex flex-col items-start gap-3 sm:flex-row sm:items-center sm:justify-between">
        <button
          type="submit"
          className="inline-flex items-center gap-2 rounded-lg bg-sky-500 px-4 py-2 text-sm font-semibold text-slate-950 transition hover:bg-sky-400 disabled:cursor-not-allowed disabled:bg-slate-700 disabled:text-slate-400"
          disabled={isSavingConfig}
        >
          {isSavingConfig ? "Saving…" : "Apply configuration"}
        </button>
        {configFeedback && (
          <p
            className={`text-sm ${
              configFeedbackState === "error"
                ? "text-red-300"
                : configFeedbackState === "success"
                ? "text-emerald-300"
                : configFeedbackState === "pending"
                ? "text-sky-300"
                : "text-slate-300"
            }`}
          >
            {configFeedback}
          </p>
        )}
      </div>
    </form>
  );

  const renderDiagnostics = () => (
    <div className="space-y-6">
      <header className="flex flex-col gap-2">
        <h2 className="text-2xl font-semibold text-white">Diagnostics</h2>
        <p className="text-xs text-slate-400">
          Review recent log files, inspect the most recent errors, and raise GitHub issues with one click. The data is
          sourced from <code className="rounded bg-slate-800 px-1 py-0.5">/api/logs</code>.
        </p>
        {logsError && (
          <div className="rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-200">{logsError}</div>
        )}
      </header>
      <section className="space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-white">Log files</h3>
          <button
            type="button"
            onClick={loadLogs}
            className="rounded-lg border border-slate-700 px-3 py-1 text-xs text-slate-200 transition hover:border-sky-500/60 hover:text-sky-200"
          >
            Refresh
          </button>
        </div>
        <div className="overflow-hidden rounded-xl border border-slate-800">
          <table className="min-w-full divide-y divide-slate-800 text-left text-xs text-slate-300">
            <thead className="bg-slate-900/80 uppercase text-slate-400">
              <tr>
                <th className="px-4 py-2">File</th>
                <th className="px-4 py-2">Size</th>
                <th className="px-4 py-2">Modified</th>
                <th className="px-4 py-2">Path</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800 bg-slate-950/60">
              {logs?.files.length ? (
                logs.files.map((file) => (
                  <tr key={file.path}>
                    <td className="px-4 py-2 font-medium text-slate-100">{file.name}</td>
                    <td className="px-4 py-2">{formatBytes(file.size_bytes)}</td>
                    <td className="px-4 py-2">{file.modified ?? "unknown"}</td>
                    <td className="px-4 py-2 font-mono text-[11px] text-slate-400">{file.path}</td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td className="px-4 py-6 text-center text-slate-500" colSpan={4}>
                    No log files discovered yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>
      <section className="grid gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <div className="space-y-3">
          <h3 className="text-lg font-semibold text-white">Recent errors</h3>
          <div className="space-y-2">
            {logs?.recent_errors.length ? (
              logs.recent_errors.map((entry) => (
                <button
                  key={`${entry.timestamp}-${entry.source}-${entry.raw}`}
                  type="button"
                  onClick={() => setSelectedError(entry)}
                  className={`w-full rounded-lg border px-4 py-3 text-left transition ${
                    selectedError?.raw === entry.raw
                      ? "border-sky-500/70 bg-sky-500/10"
                      : "border-slate-800 bg-slate-900/40 hover:border-slate-700"
                  }`}
                >
                  <p className="flex items-center justify-between text-xs text-slate-400">
                    <span>{entry.timestamp}</span>
                    <span className="font-semibold text-red-300">{entry.level}</span>
                  </p>
                  <p className="mt-1 text-sm font-medium text-slate-100">{entry.message}</p>
                  <p className="mt-1 text-xs text-slate-400">
                    {entry.file ? `${entry.file}${entry.line ? `:${entry.line}` : ""}` : entry.source}
                  </p>
                </button>
              ))
            ) : (
              <p className="rounded-lg border border-slate-800 bg-slate-900/50 p-4 text-sm text-slate-400">
                No errors logged within the captured window.
              </p>
            )}
          </div>
        </div>
        <div className="space-y-3">
          <h3 className="text-lg font-semibold text-white">Error detail</h3>
          {selectedError ? (
            <div className="space-y-3 rounded-xl border border-slate-800 bg-slate-900/50 p-4 text-sm text-slate-200">
              <p className="text-xs uppercase tracking-widest text-slate-400">Timestamp</p>
              <p className="text-sm font-medium text-white">{selectedError.timestamp}</p>
              <p className="text-xs uppercase tracking-widest text-slate-400">Message</p>
              <p>{selectedError.message}</p>
              <p className="text-xs uppercase tracking-widest text-slate-400">Source</p>
              <p className="font-mono text-xs text-slate-300">{selectedError.source}</p>
              {selectedError.file && (
                <p className="font-mono text-xs text-slate-300">
                  {selectedError.file}
                  {selectedError.line ? `:${selectedError.line}` : ""}
                </p>
              )}
              <a
                href={issueUrl}
                target="_blank"
                rel="noreferrer"
                className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-lg bg-emerald-500 px-4 py-2 text-sm font-semibold text-slate-950 transition hover:bg-emerald-400"
              >
                Report issue on GitHub
                <svg className="h-4 w-4" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                  <path d="M10 .5a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48l-.01-1.68c-2.78.6-3.37-1.34-3.37-1.34-.45-1.16-1.1-1.47-1.1-1.47-.9-.62.07-.61.07-.61 1 .07 1.52 1.03 1.52 1.03.89 1.52 2.34 1.08 2.91.82.09-.66.35-1.09.63-1.34-2.22-.25-4.56-1.11-4.56-4.95 0-1.09.39-1.99 1.03-2.69-.1-.25-.45-1.28.1-2.67 0 0 .84-.27 2.75 1.02a9.56 9.56 0 0 1 5 0c1.9-1.29 2.74-1.02 2.74-1.02.55 1.39.2 2.42.1 2.67.64.7 1.02 1.6 1.02 2.69 0 3.85-2.35 4.7-4.59 4.95.36.31.68.91.68 1.84l-.01 2.73c0 .26.18.58.69.48A10 10 0 0 0 10 .5Z" />
                </svg>
              </a>
            </div>
          ) : (
            <p className="rounded-xl border border-slate-800 bg-slate-900/50 p-4 text-sm text-slate-400">
              Select an error to view full context and publish a GitHub issue template.
            </p>
          )}
        </div>
      </section>
    </div>
  );

  const renderDashboard = () => (
    <div className="space-y-8">
      <section className="space-y-3">
        <h2 className="text-2xl font-semibold text-white">Operator Dashboard</h2>
        <p className="text-sm text-slate-300">
          A consolidated view of orchestration state after setup. Metrics refresh alongside the configuration editor
          so operators always have the latest data.
        </p>
        {renderStatusSummary()}
      </section>
      <section className="grid gap-6 lg:grid-cols-2">
        <div className="space-y-3 rounded-xl border border-slate-800 bg-slate-900/50 p-5">
          <h3 className="text-lg font-semibold text-white">Feature Flags</h3>
          <p className="text-xs text-slate-400">Derived from the validated licence bundle.</p>
          <ul className="mt-3 space-y-2">
            {status && Object.keys(status.features).length > 0 ? (
              Object.entries(status.features).map(([feature, enabled]) => (
                <li key={feature} className="flex items-center justify-between rounded-lg border border-slate-800 bg-slate-950/60 px-3 py-2 text-sm">
                  <span className="font-medium text-slate-200">{feature}</span>
                  <span className={enabled ? "text-emerald-300" : "text-slate-500"}>{enabled ? "Enabled" : "Disabled"}</span>
                </li>
              ))
            ) : (
              <li className="rounded-lg border border-slate-800 bg-slate-950/60 px-3 py-3 text-sm text-slate-400">
                Feature information unavailable; ensure licence validation succeeded.
              </li>
            )}
          </ul>
        </div>
        <div className="space-y-3 rounded-xl border border-slate-800 bg-slate-900/50 p-5">
          <h3 className="text-lg font-semibold text-white">Latest Diagnostics</h3>
          <p className="text-xs text-slate-400">
            Highlights from the recent log scan with direct links to troubleshooting docs.
          </p>
          <ul className="space-y-2 text-sm text-slate-200">
            <li className="rounded-lg border border-slate-800 bg-slate-950/60 px-3 py-2">
              {logs?.recent_errors.length ? (
                <span>
                  {logs.recent_errors.length} error entries captured. Review details in the diagnostics tab.
                </span>
              ) : (
                <span>No error entries detected in the latest scan.</span>
              )}
            </li>
            <li className="rounded-lg border border-slate-800 bg-slate-950/60 px-3 py-2">
              {logs?.files.length ? (
                <span>{logs.files.length} log files tracked under the configured logging directory.</span>
              ) : (
                <span>Log directory empty or not yet initialised.</span>
              )}
            </li>
          </ul>
          <a
            href="/docs/TROUBLESHOOTING.md"
            className="inline-flex items-center gap-2 text-xs font-semibold text-sky-300 hover:text-sky-200"
            target="_blank"
            rel="noreferrer"
          >
            Troubleshooting playbook
            <svg className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.5" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 19.5l15-15M9 4.5h10.5V15" />
            </svg>
          </a>
        </div>
      </section>
    </div>
  );

  const renderStep = () => {
    switch (activeStep) {
      case "Welcome":
        return renderWelcome();
      case "Configuration":
        return renderConfiguration();
      case "Diagnostics":
        return renderDiagnostics();
      case "Dashboard":
        return renderDashboard();
      default:
        return null;
    }
  };

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <header className="border-b border-slate-800 bg-slate-900/70 backdrop-blur">
        <div className="mx-auto flex max-w-6xl flex-col gap-4 px-6 py-6 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <p className="text-xs uppercase tracking-[0.35em] text-sky-400">R-EMS</p>
            <h1 className="text-2xl font-semibold text-white">Setup Wizard &amp; Operator Console</h1>
            <p className="text-xs text-slate-400">Interact with the daemon via the REST API and build confidence before production go-live.</p>
          </div>
          <div className="flex items-center gap-3 text-xs">
            <button
              type="button"
              onClick={loadStatus}
              className="rounded-lg border border-slate-700 px-3 py-1 text-slate-200 transition hover:border-sky-500/60 hover:text-sky-200"
            >
              Refresh status
            </button>
            <a
              href="/docs/VERSIONING.md"
              className="rounded-lg border border-slate-700 px-3 py-1 text-slate-200 transition hover:border-sky-500/60 hover:text-sky-200"
              target="_blank"
              rel="noreferrer"
            >
              Versioning policy
            </a>
          </div>
        </div>
      </header>
      <main className="mx-auto flex max-w-6xl flex-1 flex-col gap-8 px-6 py-10 lg:flex-row">
        <nav className="lg:w-64">
          <ol className="space-y-3">
            {wizardSteps.map((step) => {
              const index = wizardSteps.indexOf(step);
              const isActive = step === activeStep;
              const isComplete = index < stepIndex;
              return (
                <li key={step}>
                  <button
                    type="button"
                    onClick={() => setActiveStep(step)}
                    className={`w-full rounded-xl border px-4 py-3 text-left transition ${
                      isActive
                        ? "border-sky-500/60 bg-sky-500/10 text-sky-200"
                        : isComplete
                        ? "border-emerald-500/60 bg-emerald-500/10 text-emerald-200 hover:border-emerald-400/80"
                        : "border-slate-800 bg-slate-900/40 hover:border-slate-700"
                    }`}
                  >
                    <p className="text-xs uppercase tracking-widest text-slate-400">Step {index + 1}</p>
                    <p className="text-sm font-semibold text-white">{step}</p>
                  </button>
                </li>
              );
            })}
          </ol>
        </nav>
        <section className="flex-1 space-y-8">
          {renderStep()}
          <footer className="flex flex-col gap-4 border-t border-slate-800 pt-6 sm:flex-row sm:items-center sm:justify-between">
            <div className="text-xs text-slate-400">
              Need support? Email <a href="mailto:ems@company.invalid" className="text-sky-300 hover:text-sky-200">ems@company.invalid</a>
              .
            </div>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={() => canGoBack && setActiveStep(wizardSteps[stepIndex - 1])}
                className="rounded-lg border border-slate-700 px-3 py-1 text-xs text-slate-200 transition hover:border-slate-500/80 hover:text-slate-100 disabled:cursor-not-allowed disabled:border-slate-900 disabled:text-slate-600"
                disabled={!canGoBack}
              >
                Back
              </button>
              <button
                type="button"
                onClick={() => canGoNext && setActiveStep(wizardSteps[stepIndex + 1])}
                className="rounded-lg border border-slate-700 px-3 py-1 text-xs text-slate-200 transition hover:border-sky-500/60 hover:text-sky-200 disabled:cursor-not-allowed disabled:border-slate-900 disabled:text-slate-600"
                disabled={!canGoNext}
              >
                {canGoNext ? "Next" : "On dashboard"}
              </button>
            </div>
          </footer>
        </section>
      </main>
      <footer className="border-t border-slate-800 bg-slate-900/70 px-6 py-4 text-center text-xs text-slate-500">
        R-EMS Setup Wizard • Powered by React &amp; Axum • Review /docs/VERSIONING.md for release cadence.
      </footer>
    </div>
  );
};
