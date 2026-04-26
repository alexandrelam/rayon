import { type RefObject, useLayoutEffect, useRef } from "react";

import { resizeLauncher } from "@/features/launcher/api";

export function useLauncherAutoResize(shellRef: RefObject<HTMLElement | null>) {
  const requestedWindowHeight = useRef<number | null>(null);

  useLayoutEffect(() => {
    const shell = shellRef.current;
    if (!shell) {
      return;
    }

    let frameId = 0;

    const syncLauncherHeight = () => {
      const measuredHeight = Math.ceil(shell.getBoundingClientRect().height);
      const nextHeight = Math.min(420, Math.max(160, measuredHeight));
      if (requestedWindowHeight.current === nextHeight) {
        return;
      }

      requestedWindowHeight.current = nextHeight;
      void resizeLauncher(nextHeight);
    };

    const scheduleSync = () => {
      cancelAnimationFrame(frameId);
      frameId = requestAnimationFrame(syncLauncherHeight);
    };

    scheduleSync();

    const observer = new ResizeObserver(() => {
      scheduleSync();
    });
    observer.observe(shell);

    return () => {
      cancelAnimationFrame(frameId);
      observer.disconnect();
    };
  }, [shellRef]);
}
