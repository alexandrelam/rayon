import type { ReactNode } from "react";
import React from "react";
import { motion, type Variants } from "motion/react";

type AnimatedItemState = {
  hidden?: Record<string, string | number>;
  visible?: Record<string, string | number>;
};

export type AnimatedGroupPreset = "fade" | "slide" | "scale" | "blur-slide";

export type AnimatedGroupProps = {
  ariaLabel?: string;
  children: ReactNode;
  className?: string;
  variants?: {
    container?: Variants;
    item?: Variants;
  };
  preset?: AnimatedGroupPreset;
};

const defaultContainerVariants: Variants = {
  hidden: {},
  visible: {
    transition: {
      staggerChildren: 0.045,
      delayChildren: 0.02,
    },
  },
};

const defaultItemState: Required<AnimatedItemState> = {
  hidden: { opacity: 0 },
  visible: { opacity: 1 },
};

const presetStates: Record<AnimatedGroupPreset, AnimatedItemState> = {
  fade: {},
  slide: {
    hidden: { y: 10 },
    visible: { y: 0 },
  },
  scale: {
    hidden: { scale: 0.96 },
    visible: { scale: 1 },
  },
  "blur-slide": {
    hidden: { opacity: 0, y: 12, filter: "blur(4px)" },
    visible: { opacity: 1, y: 0, filter: "blur(0px)" },
  },
};

function buildItemVariants(state: AnimatedItemState): Variants {
  return {
    hidden: { ...defaultItemState.hidden, ...state.hidden },
    visible: { ...defaultItemState.visible, ...state.visible },
  };
}

export function AnimatedGroup({
  ariaLabel,
  children,
  className,
  variants,
  preset = "blur-slide",
}: AnimatedGroupProps) {
  const containerVariants = variants?.container ?? defaultContainerVariants;
  const itemVariants = variants?.item ?? buildItemVariants(presetStates[preset]);

  return (
    <motion.ul
      aria-label={ariaLabel}
      initial="hidden"
      animate="visible"
      variants={containerVariants}
      className={className}
    >
      {React.Children.map(children, (child, index) => (
        <motion.li
          key={React.isValidElement(child) && child.key !== null ? child.key : index}
          variants={itemVariants}
        >
          {child}
        </motion.li>
      ))}
    </motion.ul>
  );
}
