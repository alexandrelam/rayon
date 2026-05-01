import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

import type { LauncherResultItemViewModel } from "../types";

export function LauncherResultItem({
  asListItem = true,
  item,
  onSelect,
  setRef,
}: {
  asListItem?: boolean;
  item: LauncherResultItemViewModel;
  onSelect: (itemId: string) => void;
  setRef: (node: HTMLButtonElement | null) => void;
}) {
  const content = (
    <Button
      type="button"
      variant="ghost"
      ref={setRef}
      data-selected={item.selected}
      className={cn(
        "h-auto min-h-12 min-w-0 w-full whitespace-normal rounded-[14px] border border-[var(--result-border)] bg-[var(--result-bg)] px-[13px] py-[11px] text-left text-[var(--panel-foreground)] transition-[transform,border-color,background-color,box-shadow]",
        "hover:bg-white/65 active:scale-[0.998]",
        "data-[selected=true]:-translate-y-px data-[selected=true]:border-[var(--selected-border)] data-[selected=true]:bg-[var(--selected-bg)] data-[selected=true]:shadow-[0_14px_30px_-24px_rgba(38,120,205,0.58)]",
      )}
      onMouseDown={(event) => {
        event.preventDefault();
      }}
      onClick={() => {
        onSelect(item.id);
      }}
    >
      <span className="flex min-w-0 flex-1 items-start gap-2.5">
        <span className="grid min-w-0 flex-1 gap-[3px]">
          <span className="truncate font-semibold">{item.title}</span>
          <span className="overflow-hidden text-[0.78rem] text-[var(--result-id)] [display:-webkit-box] [overflow-wrap:anywhere] [-webkit-box-orient:vertical] [-webkit-line-clamp:2]">
            {item.meta}
          </span>
        </span>
        <Badge className="max-w-[7rem] shrink-0 truncate">{item.kind}</Badge>
      </span>
    </Button>
  );

  return asListItem ? <li>{content}</li> : content;
}

export function LauncherResultSkeleton({ index }: { index: number }) {
  return (
    <li>
      <div
        aria-hidden="true"
        className="relative overflow-hidden rounded-[14px] border border-[rgba(24,33,43,0.06)] bg-[linear-gradient(180deg,rgba(255,255,255,0.72)_0%,rgba(240,244,249,0.68)_100%)] px-[13px] py-[11px] [animation:launcher-skeleton-breathe_1.9s_ease-in-out_infinite]"
        style={{ animationDelay: `${String(index * 90)}ms` }}
      >
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "linear-gradient(105deg, transparent 0%, rgba(255,255,255,0.18) 32%, rgba(255,255,255,0.42) 48%, rgba(255,255,255,0.18) 64%, transparent 100%)",
            transform: "translateX(-130%)",
            animation: "launcher-skeleton-sweep 1.7s ease-out infinite",
            animationDelay: `${String(index * 90)}ms`,
          }}
        />
        <span className="relative grid min-w-0 gap-[7px]">
          <span className="flex items-center gap-2.5">
            <Skeleton
              className="h-4 rounded-full bg-[linear-gradient(180deg,rgba(255,255,255,0.88)_0%,rgba(223,233,244,0.92)_100%)]"
              style={{
                width: index % 3 === 0 ? "13.5rem" : index % 3 === 1 ? "11.75rem" : "15rem",
              }}
            />
            <Skeleton className="h-5 w-[4.5rem] rounded-full bg-[linear-gradient(180deg,rgba(255,255,255,0.88)_0%,rgba(223,233,244,0.92)_100%)]" />
          </span>
          <Skeleton
            className="h-3.5 rounded-full bg-[linear-gradient(180deg,rgba(255,255,255,0.88)_0%,rgba(223,233,244,0.92)_100%)]"
            style={{
              width: index % 3 === 0 ? "17rem" : index % 3 === 1 ? "13.5rem" : "19rem",
            }}
          />
        </span>
      </div>
    </li>
  );
}
