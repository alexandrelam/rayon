import type { PropsWithChildren, Ref, RefObject } from "react";

export function LauncherShell({
  children,
  shellRef,
}: PropsWithChildren<{
  shellRef: RefObject<HTMLElement | null>;
}>) {
  return (
    <main ref={shellRef as Ref<HTMLElement>} className="min-w-0 w-full bg-transparent p-[14px]">
      <section
        aria-label="Command palette"
        className="grid min-w-0 max-h-[calc(var(--launcher-max-height)-28px)] w-full content-start gap-3 overflow-hidden rounded-[22px] border border-[var(--panel-border)] bg-[linear-gradient(180deg,var(--panel-bg-start)_0%,var(--panel-bg-end)_100%)] p-[18px] shadow-[inset_0_1px_0_var(--panel-inset)] backdrop-blur-[28px] backdrop-saturate-[145%]"
      >
        {children}
      </section>
    </main>
  );
}
