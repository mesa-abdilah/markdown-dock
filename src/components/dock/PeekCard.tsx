import { ChevronRight, Folder, Lock, LockOpen, Shield } from "lucide-react";
import { useDockStore } from "../../stores/dockStore";
import { useSearchStore } from "../../stores/searchStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVaultStore } from "../../stores/vaultStore";
import { Kbd } from "../ui/Kbd";
import { DockFolderIcon } from "./DockFolderIcon";
import { StatusIndicator } from "./StatusIndicator";

interface PeekCardProps {
  onClickOpen: () => void;
}

export function PeekCard({ onClickOpen }: PeekCardProps) {
  const indexedCount = useDockStore((s) => s.indexedCount);
  const syncStatus = useDockStore((s) => s.syncStatus);
  const hotkey = useSettingsStore((s) => s.settings.hotkey);
  const roots = useSearchStore((s) => s.roots);
  const vaultState = useVaultStore((s) => s.status.state);

  const folderCount = roots.filter((r) => r.enabled).length;

  return (
    <button
      type="button"
      onClick={onClickOpen}
      aria-label="Open Markdown search and file list"
      className="group flex h-full w-full cursor-pointer flex-col justify-between gap-3 p-4 text-left outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70"
    >
      {/* Dynamic Island Live Activity Header */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <DockFolderIcon className="h-8 w-8 shrink-0 drop-shadow-[0_2px_6px_rgba(0,0,0,0.4)]" />
          <div className="min-w-0">
            <span className="block truncate text-xs font-semibold tracking-tight text-zinc-100">
              Markdown Dock
            </span>
          </div>
        </div>

        <span
          className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-semibold shadow-sm ${
            vaultState === "Unlocked"
              ? "bg-gradient-to-b from-emerald-400/20 to-emerald-500/15 text-emerald-200 ring-1 ring-emerald-400/45 shadow-[0_0_8px_rgba(52,211,153,0.2)]"
              : vaultState === "Locked"
                ? "bg-gradient-to-b from-amber-400/25 to-amber-500/15 text-amber-200 ring-1 ring-amber-400/50 shadow-[0_0_8px_rgba(251,191,36,0.25)]"
                : "bg-white/[0.08] text-zinc-300 ring-1 ring-white/15"
          }`}
        >
          <span
            className={`h-1.5 w-1.5 rounded-full ${
              vaultState === "Unlocked"
                ? "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.9)]"
                : vaultState === "Locked"
                  ? "animate-pulse bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.9)]"
                  : "bg-zinc-400"
            }`}
            aria-hidden="true"
          />
          {vaultState === "Unlocked" ? (
            <LockOpen className="h-3 w-3" />
          ) : vaultState === "Locked" ? (
            <Lock className="h-3 w-3" />
          ) : (
            <Shield className="h-3 w-3" />
          )}
          <span>{vaultState === "Uninitialized" ? "Local" : vaultState}</span>
        </span>
      </div>

      {/* Live Stats Strip */}
      <div className="space-y-2 rounded-xl border border-white/[0.08] bg-white/[0.035] px-3.5 py-2.5">
        <div className="flex items-center justify-between gap-3">
          <span className="inline-flex items-center rounded-full bg-gradient-to-b from-sky-400/20 to-blue-500/15 px-2.5 py-0.5 text-[11px] font-bold text-sky-100 tabular-nums ring-1 ring-sky-300/30">
            {indexedCount === 1 ? "1 file indexed" : `${indexedCount} files indexed`}
          </span>
          <span className="inline-flex items-center gap-1.5 rounded-full bg-white/[0.06] px-2 py-0.5 text-[10.5px] font-medium text-zinc-300 tabular-nums ring-1 ring-white/10">
            <Folder className="h-3 w-3 text-amber-300" />
            {folderCount}
          </span>
        </div>
        <StatusIndicator status={syncStatus} showLabel className="pt-0.5" />
      </div>

      {/* Bottom Interactive Open Pill */}
      <div className="flex items-center justify-between rounded-full border border-white/10 bg-gradient-to-r from-indigo-500/20 via-white/[0.06] to-white/[0.04] px-3.5 py-1.5 text-[10.5px] text-zinc-200 transition-all group-hover:border-indigo-400/45 group-hover:from-indigo-500/35 group-hover:text-white">
        <span className="inline-flex items-center gap-1.5 font-medium">
          Open Dock
          <ChevronRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
        </span>
        <Kbd className="border-white/10 bg-black/35 text-[9.5px]">{hotkey}</Kbd>
      </div>
    </button>
  );
}
