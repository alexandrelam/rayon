import type { LauncherArgumentPanelViewModel } from "@/features/launcher/types";

export function LauncherArgumentPanel({
  currentStep,
  totalSteps,
  flagLabel,
  defaultValue,
}: LauncherArgumentPanelViewModel) {
  return (
    <section
      aria-live="polite"
      className="grid gap-1 rounded-[16px] border border-[var(--output-border)] bg-[var(--output-bg)] px-4 py-3"
    >
      <p className="m-0 text-sm">
        Step {currentStep} of {totalSteps}
      </p>
      <p className="m-0 text-sm text-[var(--muted)]">{flagLabel}</p>
      {defaultValue ? <p className="m-0 text-sm text-[var(--muted)]">Default: {defaultValue}</p> : null}
    </section>
  );
}
