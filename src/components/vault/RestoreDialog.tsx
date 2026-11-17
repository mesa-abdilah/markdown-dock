import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { DownloadCloud, FolderOpen, X } from "lucide-react";
import type { RestoreSelection } from "../../bindings/commands";
import { isTauriRuntime } from "../../lib/ipc";
import { useSyncStore } from "../../stores/syncStore";
import { CustomCheckbox } from "../ui/CustomCheckbox";
import { CustomSelect } from "../ui/CustomSelect";

export function RestoreDialog() {
  const catalog = useSyncStore((s) => s.restoreCatalog);
  const busy = useSyncStore((s) => s.busy);
  const error = useSyncStore((s) => s.error);
  const runRestore = useSyncStore((s) => s.runRestore);
  const openModal = useSyncStore((s) => s.openModal);

  const [selectedIds, setSelectedIds] = useState<Record<string, boolean>>({});
  const [chosenVersions, setChosenVersions] = useState<Record<string, string>>({});
  const [targetDir, setTargetDir] = useState("");
  const [overwrite, setOverwrite] = useState(false);

  const toggleItem = (fileId: string) => {
    setSelectedIds((prev) => ({ ...prev, [fileId]: !prev[fileId] }));
  };

  const selectAll = () => {
    const all: Record<string, boolean> = {};
    for (const item of catalog) {
      if (!item.deleted) all[item.file_id] = true;
    }
    setSelectedIds(all);
  };

  const handlePickTarget = async () => {
    if (!isTauriRuntime()) return;
    const dir = await open({
      directory: true,
      multiple: false,
      title: "Select Restore Destination Folder",
    });
    if (typeof dir === "string") setTargetDir(dir);
  };

  const handleRestore = async () => {
    if (!targetDir.trim()) return;
    const selections: RestoreSelection[] = [];
    for (const item of catalog) {
      if (!selectedIds[item.file_id]) continue;
      const chosenBlob = chosenVersions[item.file_id] ?? item.blob_drive_id;
      const versionObj =
        item.versions.find((v) => v.blob_drive_id === chosenBlob) ?? item.versions[0];
      selections.push({
        file_id: item.file_id,
        rel_path: item.rel_path,
        blob_drive_id: versionObj?.blob_drive_id ?? item.blob_drive_id,
        sha256_hex: versionObj?.sha256_hex ?? item.sha256_hex,
      });
    }
    if (selections.length === 0) return;
    await runRestore(selections, targetDir, overwrite);
  };

  const selectedCount = Object.values(selectedIds).filter(Boolean).length;

  return (
    <div className="flex h-full flex-col justify-between p-3.5 text-xs">
      <div className="flex shrink-0 items-center justify-between border-b border-white/[0.07] pb-2.5">
        <div className="flex items-center gap-2">
          <DownloadCloud className="h-4 w-4 text-indigo-400" />
          <span className="font-semibold text-zinc-100">Restore & Decrypt from Drive</span>
        </div>
        <button
          type="button"
          onClick={() => openModal("none")}
          aria-label="Close restore dialog"
          className="rounded p-1 text-zinc-400 hover:bg-white/10 hover:text-zinc-100"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>

      {error && (
        <div className="mt-2 shrink-0 rounded-lg border border-rose-500/35 bg-rose-500/15 px-2.5 py-1.5 text-[11px] text-rose-200">
          {error}
        </div>
      )}

      <div className="my-2.5 flex shrink-0 items-center justify-between text-[11px]">
        <span className="text-zinc-400">{catalog.length} encrypted file(s) in vault</span>
        <button type="button" onClick={selectAll} className="text-indigo-300 hover:text-indigo-200">
          Select all active
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-white/[0.08] bg-black/20 p-2">
        {busy && catalog.length === 0 ? (
          <p className="py-6 text-center text-zinc-400">Decrypting remote manifests...</p>
        ) : catalog.length === 0 ? (
          <p className="py-6 text-center text-zinc-500">No backed-up files found in Drive vault.</p>
        ) : (
          <ul className="space-y-1.5">
            {catalog.map((item) => (
              <li
                key={item.file_id}
                className="flex items-center justify-between gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-2.5 py-1.5"
              >
                <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2">
                  <CustomCheckbox
                    checked={Boolean(selectedIds[item.file_id])}
                    onChange={() => toggleItem(item.file_id)}
                  />
                  <div className="min-w-0">
                    <p className="truncate text-[11px] font-medium text-zinc-100">
                      {item.rel_path}
                      {item.deleted && (
                        <span className="ml-1.5 rounded bg-rose-500/20 px-1 text-[9px] text-rose-300">
                          deleted
                        </span>
                      )}
                    </p>
                    <p className="text-[10px] text-zinc-500">
                      {item.root_label} • {item.device_name}
                    </p>
                  </div>
                </label>

                {item.versions.length > 1 && (
                  <CustomSelect
                    variant="compact"
                    align="right"
                    ariaLabel={`Select version for ${item.rel_path}`}
                    value={chosenVersions[item.file_id] ?? item.blob_drive_id}
                    onChange={(val) =>
                      setChosenVersions((prev) => ({
                        ...prev,
                        [item.file_id]: val,
                      }))
                    }
                    options={item.versions.map((v, idx) => ({
                      value: v.blob_drive_id,
                      label: `${idx === 0 ? "Latest" : `v-${idx}`} (${new Date(v.at_sec * 1000).toLocaleDateString()})`,
                    }))}
                  />
                )}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="shrink-0 space-y-2 border-t border-white/[0.07] pt-2.5">
        <div className="flex gap-1.5">
          <input
            type="text"
            value={targetDir}
            onChange={(e) => setTargetDir(e.target.value)}
            placeholder="Target restore folder path..."
            className="h-8 min-w-0 flex-1 rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-[11px] text-zinc-100 focus:outline-none"
          />
          <button
            type="button"
            onClick={() => void handlePickTarget()}
            className="inline-flex items-center gap-1 rounded-lg bg-white/10 px-2.5 text-[11px] text-zinc-200 hover:bg-white/20"
          >
            <FolderOpen className="h-3.5 w-3.5" /> Browse
          </button>
        </div>

        <label className="flex cursor-pointer items-center justify-between text-[11px] text-zinc-300">
          <span>Overwrite existing files in destination</span>
          <CustomCheckbox variant="switch" checked={overwrite} onChange={setOverwrite} />
        </label>

        <button
          type="button"
          disabled={busy || selectedCount === 0 || !targetDir.trim()}
          onClick={() => void handleRestore()}
          className="w-full rounded-lg bg-indigo-500/90 py-2 text-xs font-semibold text-white hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-45"
        >
          {busy ? "Decrypting & restoring..." : `Restore ${selectedCount} file(s)`}
        </button>
      </div>
    </div>
  );
}
