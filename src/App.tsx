import { MotionConfig } from "motion/react";
import { DockShell } from "./components/dock/DockShell";
import { useDockMachine } from "./hooks/useDockMachine";

export function App() {
  useDockMachine();

  return (
    <MotionConfig reducedMotion="user">
      <main className="h-screen w-screen overflow-hidden bg-transparent">
        <DockShell />
      </main>
    </MotionConfig>
  );
}

export default App;
