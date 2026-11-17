import { Pin, PinOff } from "lucide-react";

interface PinButtonProps {
  pinned: boolean;
  onToggle: () => void;
}

export function PinButton({ pinned, onToggle }: PinButtonProps) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-pressed={pinned}
      aria-label={pinned ? "Unpin panel" : "Pin panel open"}
      title={pinned ? "Unpin panel (auto-collapse enabled)" : "Pin panel open"}
      style={{
        backgroundColor: pinned ? "var(--accent-primary)" : "var(--bg-surface)",
        color: pinned ? "var(--accent-foreground)" : "var(--text-primary)",
        borderColor: "var(--border-color)",
        borderWidth: "var(--border-width)",
        borderStyle: "solid",
        borderRadius: "var(--radius-md)",
        boxShadow: pinned ? "var(--shadow-inset)" : "var(--shadow-button)",
      }}
      className="dmd-interactive-btn mr-1 ml-1 inline-flex h-7 w-7 items-center justify-center transition-all focus:outline-none"
    >
      {pinned ? <Pin className="h-3.5 w-3.5" /> : <PinOff className="h-3.5 w-3.5" />}
    </button>
  );
}
