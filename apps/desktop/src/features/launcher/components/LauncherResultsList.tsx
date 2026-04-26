import { ScrollArea } from "@/components/ui/scroll-area";

import type { LauncherResultItemViewModel } from "../types";
import { LauncherResultItem, LauncherResultSkeleton } from "./LauncherResultItem";

export function LauncherResultsList({
  emptyMessage,
  items,
  onSelect,
  setItemRef,
  showInteractiveSkeleton,
}: {
  emptyMessage: string | null;
  items: LauncherResultItemViewModel[];
  onSelect: (itemId: string) => void;
  setItemRef: (itemId: string, node: HTMLButtonElement | null) => void;
  showInteractiveSkeleton: boolean;
}) {
  return (
    <ScrollArea className="min-w-0 max-h-[var(--results-max-height)] overflow-x-hidden pr-1">
      <ul
        aria-label="Search results"
        className="m-0 grid min-w-0 list-none gap-2 overflow-x-hidden p-[2px_4px_4px_0]"
      >
        {showInteractiveSkeleton
          ? Array.from({ length: 6 }, (_, index) => (
              <LauncherResultSkeleton key={`skeleton-${String(index)}`} index={index} />
            ))
          : items.map((item) => (
              <LauncherResultItem
                key={item.id}
                item={item}
                onSelect={onSelect}
                setRef={(node) => {
                  setItemRef(item.id, node);
                }}
              />
            ))}
        {emptyMessage ? (
          <li className="flex min-h-12 min-w-0 items-center justify-center rounded-[14px] border border-[var(--result-border)] bg-[var(--result-bg)] px-[13px] py-[11px] text-[var(--empty)] [overflow-wrap:anywhere]">
            {emptyMessage}
          </li>
        ) : null}
      </ul>
    </ScrollArea>
  );
}
