import { AlertCircle } from "lucide-react";
import { useSyncStore } from "../../stores/syncStore";

export function ConflictList() {
  const conflicts = useSyncStore((s) => s.conflicts);
  const resolveConflict = useSyncStore((s) => s.resolveConflict);

  if (conflicts.length === 0) return null;

  return (
    <div className="space-y-1.5 rounded-lg border border-amber-400/25 bg-amber-500/10 p-2.5">
      <div className="flex items-center gap-1.5 text-[11px] font-semibold text-amber-200">
        <AlertCircle className="h-3.5 w-3.5" />
        <span>{conflicts.length} Sync Conflict(s)</span>
      </div>
      <ul className="space-y-1.5">
        {conflicts.map((c) => (
          <li
            key={c.id}
            className="rounded-md border border-white/10 bg-black/25 p-2 text-[10.5px]"
          >
            <p className="truncate font-medium text-zinc-100">{c.rel_path}</p>
            <p className="mt-0.5 text-zinc-400">Modified also on: {c.remote_device}</p>
            <div className="mt-1.5 flex gap-1">
              <button
                type="button"
                onClick={() => void resolveConflict(c.id, "keep_both")}
                className="rounded bg-indigo-500/75 px-1.5 py-0.5 text-[10px] text-white hover:bg-indigo-500"
              >
                Keep Both
              </button>
              <button
                type="button"
                onClick={() => void resolveConflict(c.id, "keep_local")}
                className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-zinc-200 hover:bg-white/20"
              >
                Keep Local
              </button>
              <button
                type="button"
                onClick={() => void resolveConflict(c.id, "keep_remote")}
                className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-zinc-200 hover:bg-white/20"
              >
                Keep Remote
              </button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
