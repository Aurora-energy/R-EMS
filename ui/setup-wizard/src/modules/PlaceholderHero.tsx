/**
---
ems_section: "12-gui"
ems_subsection: "01-bootstrap"
ems_type: "frontend-component"
ems_scope: "gui"
ems_description: "Placeholder hero component outlining upcoming wizard, dashboard, and editor steps."
ems_version: "v0.1.0"
ems_owner: "tbd"
ems_references:
  - /docs/VERSIONING.md
---
*/
const ROADMAP_ITEMS = [
  {
    title: "Setup Wizard",
    description: "Guide operators through installation naming, grid controllers, and runtime mode selection."
  },
  {
    title: "Operator Dashboard",
    description: "Summarise system status, controller health, and provide quick operational actions."
  },
  {
    title: "Configuration Editor",
    description: "Enable post-setup adjustments with validation and audit-friendly workflows."
  }
];

export const PlaceholderHero: React.FC = () => {
  return (
    <section className="space-y-6 rounded-2xl border border-slate-800 bg-slate-900/60 p-8 shadow-xl shadow-slate-950/60">
      <h2 className="text-3xl font-semibold text-white">GUI &amp; Setup Wizard Roadmap</h2>
      <p className="max-w-3xl text-sm leading-relaxed text-slate-300">
        This bootstrap milestone establishes the React + Tailwind foundation for the R-EMS operator experience.
        Subsequent steps will deliver the full setup wizard workflow, status dashboard, configuration editor, and the
        supporting API services. Track release compatibility and semantic versioning expectations in
        <span className="font-medium text-sky-300"> /docs/VERSIONING.md</span>.
      </p>
      <div className="grid gap-4 sm:grid-cols-3">
        {ROADMAP_ITEMS.map((item) => (
          <article
            key={item.title}
            className="rounded-xl border border-slate-800 bg-slate-900/70 p-4 transition hover:border-sky-500/60 hover:shadow-lg hover:shadow-sky-950/50"
          >
            <h3 className="text-lg font-semibold text-sky-300">{item.title}</h3>
            <p className="mt-2 text-xs leading-relaxed text-slate-300">{item.description}</p>
          </article>
        ))}
      </div>
      <div className="rounded-lg border border-sky-500/40 bg-sky-500/10 p-4 text-xs text-sky-200">
        <strong className="block text-sm font-semibold uppercase tracking-widest text-sky-300">
          Next Actions
        </strong>
        <ul className="mt-2 list-disc space-y-1 pl-5">
          <li>Implement multi-step setup workflow with API integration.</li>
          <li>Build dashboards, configuration editor, and secure operator auth.</li>
          <li>Expand docs and tests to cover GUI subsystems.</li>
        </ul>
      </div>
    </section>
  );
};
