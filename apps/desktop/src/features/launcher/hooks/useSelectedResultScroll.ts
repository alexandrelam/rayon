import { type MutableRefObject, useLayoutEffect } from "react";

export function useSelectedResultScroll(
  selectedResultId: string | null,
  resultItemRefs: MutableRefObject<Record<string, HTMLButtonElement | null>>,
) {
  useLayoutEffect(() => {
    if (!selectedResultId) {
      return;
    }

    resultItemRefs.current[selectedResultId]?.scrollIntoView({
      block: "nearest",
    });
  }, [resultItemRefs, selectedResultId]);
}
