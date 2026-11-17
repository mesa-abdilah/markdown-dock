import type { Transition } from "motion/react";

export const SPRINGS = {
  // Zero-overshoot Apple deceleration curve (eliminates spring bounce / "mantul")
  morph: {
    type: "tween",
    duration: 0.22,
    ease: [0.16, 1, 0.3, 1],
  },
  // Clean rapid crossfade for retiring layers so old and new shells never clash
  exit: {
    type: "tween",
    duration: 0.14,
    ease: [0.4, 0, 1, 1],
  },
  // Smooth inner content glide
  content: {
    type: "tween",
    duration: 0.18,
    ease: [0.16, 1, 0.3, 1],
  },
  snap: {
    type: "tween",
    duration: 0.16,
    ease: [0.16, 1, 0.3, 1],
  },
} as const satisfies Record<string, Transition>;

export const REDUCED_MOTION_TRANSITION: Transition = {
  duration: 0.1,
  ease: "easeOut",
};
