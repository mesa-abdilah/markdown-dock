import { MotionConfig } from "motion/react";
import { DockShell } from "./components/dock/DockShell";
import { ThemeProvider } from "./contexts/ThemeContext";
import { useDockMachine } from "./hooks/useDockMachine";

export function App() {
  useDockMachine();

  return (
    <ThemeProvider>
      <MotionConfig reducedMotion="user">
        <main className="h-screen w-screen overflow-hidden bg-transparent">
          <DockShell />
        </main>
      </MotionConfig>
    </ThemeProvider>
  );
}

export default App;
