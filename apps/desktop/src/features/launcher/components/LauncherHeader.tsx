import type { LauncherHeaderViewModel } from "@/features/launcher/types";

export function LauncherHeader({ title, subtitle, version }: LauncherHeaderViewModel) {
  return (
    <header className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-0.5">
      <div className="min-w-0">
        <p className="m-0 text-[0.73rem] font-semibold tracking-[0.16em] text-[var(--eyebrow)] uppercase">
          rayon
        </p>
        <h1 className="m-0 text-[1.12rem] font-[640] tracking-[-0.02em]">{title}</h1>
        {subtitle ? (
          <p className="m-0 min-w-0 text-[0.82rem] text-[var(--result-id)] [overflow-wrap:anywhere]">
            {subtitle}
          </p>
        ) : null}
      </div>

      {version ? (
        <p className="m-0 self-start justify-self-end pt-0.5 text-[0.72rem] font-medium tracking-[0.08em] text-[var(--result-id)]">
          v{version}
        </p>
      ) : null}
    </header>
  );
}
