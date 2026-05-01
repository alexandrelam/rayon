import type { PropsWithChildren } from "react";
import { AnimatePresence, motion } from "motion/react";

export function LauncherAnimatedPresence({
  children,
  isVisible,
}: PropsWithChildren<{
  isVisible: boolean;
}>) {
  return (
    <AnimatePresence initial={false}>
      {isVisible ? (
        <motion.div
          initial={{ opacity: 0, y: 8, filter: "blur(6px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: -4, filter: "blur(6px)" }}
          transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
        >
          {children}
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
