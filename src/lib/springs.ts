import type { Transition } from "motion/react";

export const SPRINGS = {
  // Snappy yet silky Apple Dynamic Island morph (near-critically damped to prevent OS clip)
  morph: {
    type: "spring",
    stiffness: 290,
    damping: 27,
    mass: 0.88,
  },
  // Smooth blur-crossfade curve harmonized with the morph spring
  content: {
    duration: 0.24,
    ease: [0.22, 1, 0.36, 1],
  },
  snap: {
    type: "spring",
    stiffness: 440,
    damping: 30,
    mass: 0.65,
  },
} as const satisfies Record<string, Transition>;

export const REDUCED_MOTION_TRANSITION: Transition = {
  duration: 0.12,
  ease: "easeOut",
};
