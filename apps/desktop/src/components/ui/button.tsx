import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 outline-none focus-visible:ring-[3px] focus-visible:ring-[var(--input-focus-ring)] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        ghost: "hover:bg-white/50",
      },
      size: {
        default: "h-9 px-4 py-2",
        icon: "size-9",
      },
    },
    defaultVariants: {
      variant: "ghost",
      size: "default",
    },
  },
);

const Button = React.forwardRef<
  HTMLButtonElement,
  React.ComponentProps<"button"> &
    VariantProps<typeof buttonVariants> & {
      asChild?: boolean;
    }
>(({ className, variant, size, asChild = false, ...props }, ref) => {
  const Comp = asChild ? Slot : "button";

  return <Comp ref={ref} className={cn(buttonVariants({ variant, size, className }))} {...props} />;
});

Button.displayName = "Button";

export { Button };
