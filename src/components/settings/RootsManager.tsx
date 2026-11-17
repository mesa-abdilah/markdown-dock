import { useState } from "react";
import { Check, Cloud, FolderPlus, Pencil, RefreshCw, Trash2, X } from "lucide-react";
import { defaultDockIpc } from "../../lib/ipc";
import { useSearchStore } from "../../stores/searchStore";
import { CustomCheckbox } from "../ui/CustomCheckbox";

export function RootsManager() {
  const roots = useSearchStore((s) => s.roots);
  const createFolder = useSearchStore((s) => s.createFolder);
  const renameFolder = useSearchStore((s) => s.renameFolder);
  const removeRoot = useSearchStore((s) => s.removeRoot);
  const setRootEnabled = useSearchStore((s) => s.setRootEnabled);
  const rescan = useSearchStore((s) => s.rescan);
  const loadRoots = useSearchStore((s) => s.loadRoots);

  const [newFolderName, setNewFolderName] = useState("");
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameDraft, setRenameDraft] = useState("");

  const handleCreateFolder = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = newFolderName.trim();
    if (!trimmed) return;
    const created = await createFolder(trimmed);
    if (created) {
      setNewFolderName("");
    }
  };

  return (
    <div className="space-y-2.5 border-b border-white/[0.07] pb-3">
      <div className="flex items-center justify-between">
        <span className="font-semibold text-zinc-200">In-App Grouping Folders</span>
        <button
          type="button"
          onClick={() => void rescan(null)}
          title="Rescan all folders"
          className="inline-flex h-6 w-6 items-center justify-center rounded-md bg-white/[0.07] text-zinc-300 hover:bg-white/15"
        >
          <RefreshCw className="h-3 w-3" />
        </button>
      </div>

      <form onSubmit={handleCreateFolder} className="flex gap-1.5">
        <input
          type="text"
          value={newFolderName}
          onChange={(e) => setNewFolderName(e.target.value)}
          placeholder="New folder name..."
          maxLength={64}
          className="h-7 min-w-0 flex-1 rounded-md border border-white/10 bg-white/[0.06] px-2.5 text-[11px] text-zinc-100 placeholder-zinc-500 focus:border-indigo-400/60 focus:outline-none"
        />
        <button
          type="submit"
          disabled={!newFolderName.trim()}
          className="inline-flex h-7 items-center gap-1 rounded-md bg-indigo-500/85 px-2.5 text-[11px] font-medium text-white hover:bg-indigo-500 disabled:opacity-45"
        >
          <FolderPlus className="h-3 w-3" />
          <span>Create</span>
        </button>
      </form>

      {roots.length === 0 ? (
        <p className="rounded-lg border border-dashed border-white/10 px-2.5 py-2 text-[11px] text-zinc-400">
          No folders yet. Create a folder above, then drag &amp; drop `.md` files onto the dock.
        </p>
      ) : (
        <ul className="space-y-1.5">
          {roots.map((root) => (
            <li
              key={root.id}
              className="flex items-center justify-between gap-2 rounded-lg border border-white/[0.07] bg-white/[0.03] px-2.5 py-1.5"
            >
              <div className="min-w-0 flex-1">
                {renamingId === root.id ? (
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      const next = renameDraft.trim();
                      if (next) {
                        void renameFolder(root.id, next).then((ok) => {
                          if (ok) setRenamingId(null);
                        });
                      }
                    }}
                    className="flex items-center gap-1"
                  >
                    <input
                      type="text"
                      value={renameDraft}
                      onChange={(e) => setRenameDraft(e.target.value)}
                      autoFocus
                      maxLength={64}
                      className="h-6 min-w-0 flex-1 rounded border border-indigo-400/50 bg-white/[0.08] px-2 text-[11px] text-zinc-100 focus:outline-none"
                    />
                    <button
                      type="submit"
                      aria-label="Save folder rename"
                      className="inline-flex h-6 w-6 items-center justify-center rounded bg-indigo-500/80 text-white"
                    >
                      <Check className="h-3 w-3" />
                    </button>
                    <button
                      type="button"
                      onClick={() => setRenamingId(null)}
                      aria-label="Cancel folder rename"
                      className="inline-flex h-6 w-6 items-center justify-center rounded bg-white/10 text-zinc-300"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </form>
                ) : (
                  <label className="flex cursor-pointer items-center gap-2">
                    <CustomCheckbox
                      checked={root.enabled}
                      onChange={(checked) => void setRootEnabled(root.id, checked)}
                      ariaLabel={`Enable ${root.label}`}
                    />
                    <span className="truncate text-[11px] font-medium text-zinc-100">
                      {root.label}
                    </span>
                    <span className="rounded bg-white/[0.07] px-1.5 py-0.2 text-[9.5px] text-zinc-400 tabular-nums">
                      {root.doc_count}
                    </span>
                  </label>
                )}
              </div>

              <button
                type="button"
                onClick={() => {
                  setRenamingId(root.id);
                  setRenameDraft(root.label);
                }}
                title="Rename folder"
                aria-label={`Rename ${root.label}`}
                className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-white/10 hover:text-zinc-200"
              >
                <Pencil className="h-3 w-3" />
              </button>

              <button
                type="button"
                onClick={() =>
                  void defaultDockIpc.rootsSetBackup(root.id, !root.backup).then(() => loadRoots())
                }
                title={
                  root.backup
                    ? "Auto-backup enabled (1-way to Drive)"
                    : "Enable 1-way auto-backup"
                }
                aria-pressed={root.backup}
                className={`inline-flex h-6 shrink-0 items-center gap-1 rounded px-1.5 text-[10px] font-medium transition-colors ${
                  root.backup
                    ? "bg-indigo-500/25 text-indigo-200 ring-1 ring-indigo-400/35"
                    : "bg-white/[0.05] text-zinc-500 hover:text-zinc-200"
                }`}
              >
                <Cloud className="h-2.5 w-2.5" />
                <span>Backup</span>
              </button>

              <button
                type="button"
                onClick={() => void removeRoot(root.id)}
                title="Delete folder and its stored files"
                aria-label={`Remove ${root.label}`}
                className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded text-zinc-500 hover:bg-rose-500/15 hover:text-rose-300"
              >
                <Trash2 className="h-3 w-3" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
