import type { LauncherHeaderViewModel } from "@/features/launcher/types";

export function LauncherHeader({ title, subtitle }: LauncherHeaderViewModel) {
  return (
    <header className="grid min-w-0 gap-0.5">
      <p className="m-0 text-[0.73rem] font-semibold tracking-[0.16em] text-[var(--eyebrow)] uppercase">
        rayon
      </p>
      <h1 className="m-0 text-[1.12rem] font-[640] tracking-[-0.02em]">{title}</h1>
      {subtitle ? (
        <p className="m-0 min-w-0 text-[0.82rem] text-[var(--result-id)] [overflow-wrap:anywhere]">
          {subtitle}
        </p>
      ) : null}
    </header>
  );
}
