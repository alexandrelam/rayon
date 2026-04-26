import * as React from "react";

import { cn } from "@/lib/utils";

const Input = React.forwardRef<HTMLInputElement, React.ComponentProps<"input">>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        ref={ref}
        type={type}
        data-slot="input"
        className={cn(
          "flex h-14 w-full min-w-0 rounded-[15px] border border-[var(--input-border)] bg-[var(--input-bg)] px-4 py-3 text-base text-[var(--panel-foreground)] shadow-none transition-[border-color,box-shadow,background-color] outline-none placeholder:text-[var(--placeholder)] selection:bg-[var(--selected-bg)] selection:text-[var(--panel-foreground)] file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
          "focus-visible:border-[var(--input-focus-border)] focus-visible:bg-[var(--input-focus-bg)] focus-visible:ring-4 focus-visible:ring-[var(--input-focus-ring)]",
          className,
        )}
        {...props}
      />
    );
  },
);

Input.displayName = "Input";

export { Input };
