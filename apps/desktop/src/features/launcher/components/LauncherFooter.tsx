import type { LauncherFooterViewModel } from "@/features/launcher/types";

export function LauncherFooter({ error, message, muted }: LauncherFooterViewModel) {
  return (
    <section
      aria-live="polite"
      className="grid gap-1 rounded-[16px] border border-[var(--output-border)] bg-[var(--output-bg)] px-4 py-3"
    >
      {message ? (
        <p className={muted ? "m-0 text-sm text-[var(--muted)]" : "m-0 text-sm"}>{message}</p>
      ) : null}
      {error ? <p className="m-0 text-sm text-[var(--error)]">{error}</p> : null}
    </section>
  );
}
