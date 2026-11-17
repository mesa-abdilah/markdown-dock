import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, FolderPlus, Upload, X } from "lucide-react";
import { isTauriRuntime } from "../../lib/ipc";
import { useDockStore } from "../../stores/dockStore";
import { useSearchStore } from "../../stores/searchStore";

export function FilterChips() {
  const roots = useSearchStore((s) => s.roots);
  const createFolder = useSearchStore((s) => s.createFolder);
  const uploadFiles = useSearchStore((s) => s.uploadFiles);
  const markTyping = useDockStore((s) => s.markTyping);

  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  const enabledRoots = roots.filter((r) => r.enabled);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = newName.trim();
    if (!trimmed) return;
    const created = await createFolder(trimmed);
    if (created) {
      setNewName("");
      setCreating(false);
    }
  };

  const handleUploadPick = async () => {
    if (!isTauriRuntime()) return;
    const selected = await open({
      directory: false,
      multiple: true,
      title: "Select Markdown (.md) Files to Upload",
      filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
    });
    const paths = Array.isArray(selected)
      ? selected
      : typeof selected === "string"
        ? [selected]
        : [];
    if (paths.length > 0) {
      await uploadFiles(paths, enabledRoots[0]?.id ?? null);
    }
  };

  return (
    <div className="shrink-0 space-y-1.5 px-3 pb-2">
      <div className="flex items-center justify-between gap-2">
        <button
          type="button"
          onClick={() => {
            setCreating((v) => !v);
            markTyping();
          }}
          title="Create grouping folder"
          aria-label="Create grouping folder"
          className="inline-flex h-6 items-center gap-1 rounded-lg bg-white/[0.08] px-2.5 text-[10.5px] font-medium text-zinc-200 hover:bg-white/[0.15]"
        >
          <FolderPlus className="h-3 w-3 text-indigo-300" />
          <span>New Folder</span>
        </button>

        <button
          type="button"
          onClick={() => void handleUploadPick()}
          title="Upload .md file(s)"
          aria-label="Upload Markdown files"
          className="inline-flex h-6 items-center gap-1 rounded-lg bg-indigo-500/80 px-2.5 text-[10.5px] font-medium text-white hover:bg-indigo-500"
        >
          <Upload className="h-3 w-3" />
          <span>Upload .md</span>
        </button>
      </div>

      {creating && (
        <form onSubmit={handleCreate} className="flex items-center gap-1.5">
          <input
            type="text"
            value={newName}
            onChange={(e) => {
              setNewName(e.target.value);
              markTyping();
            }}
            onFocus={() => markTyping()}
            placeholder="New folder name (e.g. Work, Study)..."
            autoFocus
            maxLength={64}
            className="h-7 min-w-0 flex-1 rounded-lg border border-indigo-400/40 bg-white/[0.07] px-2.5 text-[11px] text-zinc-100 placeholder-zinc-500 focus:outline-none"
          />
          <button
            type="submit"
            disabled={!newName.trim()}
            aria-label="Confirm create folder"
            className="inline-flex h-7 items-center gap-1 rounded-lg bg-indigo-500/90 px-2 text-[11px] font-medium text-white hover:bg-indigo-500 disabled:opacity-45"
          >
            <Check className="h-3 w-3" />
            <span>Create</span>
          </button>
          <button
            type="button"
            onClick={() => {
              setCreating(false);
              setNewName("");
            }}
            aria-label="Cancel folder creation"
            className="inline-flex h-7 w-7 items-center justify-center rounded-lg bg-white/[0.06] text-zinc-400 hover:text-zinc-200"
          >
            <X className="h-3 w-3" />
          </button>
        </form>
      )}
    </div>
  );
}
