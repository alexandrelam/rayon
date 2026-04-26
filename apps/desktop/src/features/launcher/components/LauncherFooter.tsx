import { cn } from "@/lib/utils";

import type { LauncherFooterViewModel } from "@/features/launcher/types";

export function LauncherFooter({ error, message, muted }: LauncherFooterViewModel) {
  return (
    <section
      aria-live="polite"
      className="grid min-w-0 gap-1 rounded-[16px] border border-[var(--output-border)] bg-[var(--output-bg)] px-4 py-3"
    >
      {message ? (
        <p
          className={cn(
            "m-0 min-w-0 text-sm [overflow-wrap:anywhere]",
            muted ? "text-[var(--muted)]" : "",
          )}
        >
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="m-0 min-w-0 text-sm text-[var(--error)] [overflow-wrap:anywhere]">{error}</p>
      ) : null}
    </section>
  );
}
