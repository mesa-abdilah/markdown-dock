import { Pin, PinOff } from "lucide-react";
import { cn } from "../../lib/cn";

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
      className={cn(
        "mr-1 ml-1 inline-flex h-7 w-7 items-center justify-center rounded-lg transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70",
        pinned
          ? "bg-indigo-500/25 text-indigo-300 ring-1 ring-indigo-400/40 hover:bg-indigo-500/35"
          : "text-zinc-400 hover:bg-white/10 hover:text-zinc-100",
      )}
    >
      {pinned ? <Pin className="h-3.5 w-3.5" /> : <PinOff className="h-3.5 w-3.5" />}
    </button>
  );
}
