import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Folder,
  FolderPlus,
  Pencil,
  SearchX,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import type { RootInfo, SearchHit } from "../../bindings/commands";
import { isTauriRuntime } from "../../lib/ipc";
import { useDockStore } from "../../stores/dockStore";
import { useSearchStore } from "../../stores/searchStore";
import { ResultRow } from "./ResultRow";

interface ResultListProps {
  onOpenSettings: () => void;
}

type FlatEntry =
  | {
      kind: "folder";
      root: RootInfo;
      docCount: number;
      collapsed: boolean;
    }
  | {
      kind: "doc";
      hit: SearchHit;
      hitIndex: number;
    }
  | {
      kind: "empty";
      root: RootInfo;
    };

export function ResultList({ onOpenSettings }: ResultListProps) {
  const hits = useSearchStore((s) => s.hits);
  const roots = useSearchStore((s) => s.roots);
  const rootFilter = useSearchStore((s) => s.rootFilter);
  const query = useSearchStore((s) => s.query);
  const snippets = useSearchStore((s) => s.snippets);
  const selectedIndex = useSearchStore((s) => s.selectedIndex);
  const activeDocId = useSearchStore((s) => s.activeDocId);
  const collapsedFolderIds = useSearchStore((s) => s.collapsedFolderIds);
  const toggleFolderCollapsed = useSearchStore((s) => s.toggleFolderCollapsed);
  const setSelectedIndex = useSearchStore((s) => s.setSelectedIndex);
  const openPreview = useSearchStore((s) => s.openPreview);
  const copyDoc = useSearchStore((s) => s.copyDoc);
  const openInEditor = useSearchStore((s) => s.openInEditor);
  const fetchSnippets = useSearchStore((s) => s.fetchSnippets);
  const uploadFiles = useSearchStore((s) => s.uploadFiles);
  const moveDocToFolder = useSearchStore((s) => s.moveDocToFolder);
  const deleteDoc = useSearchStore((s) => s.deleteDoc);
  const renameFolder = useSearchStore((s) => s.renameFolder);
  const removeRoot = useSearchStore((s) => s.removeRoot);
  const markTyping = useDockStore((s) => s.markTyping);

  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);

  const parentRef = useRef<HTMLDivElement>(null);

  const { flatEntries, hitToFlatIndex } = useMemo(() => {
    const enabledRoots = roots.filter(
      (r) => r.enabled && (rootFilter === null || r.id === rootFilter),
    );
    const byRoot = new Map<number, SearchHit[]>();
    const hitIndexMap = new Map<number, number>();
    hits.forEach((h, idx) => {
      hitIndexMap.set(h.doc_id, idx);
      const list = byRoot.get(h.root_id) ?? [];
      list.push(h);
      byRoot.set(h.root_id, list);
    });

    const isSearching = query.trim().length > 0;
    const entries: FlatEntry[] = [];
    const hitToFlat = new Map<number, number>();

    for (const root of enabledRoots) {
      const folderHits = byRoot.get(root.id) ?? [];
      if (isSearching && folderHits.length === 0) {
        continue;
      }
      const collapsed = Boolean(collapsedFolderIds[root.id]) && !isSearching;
      entries.push({
        kind: "folder",
        root,
        docCount: isSearching ? folderHits.length : root.doc_count,
        collapsed,
      });

      if (!collapsed) {
        if (folderHits.length === 0) {
          entries.push({ kind: "empty", root });
        } else {
          for (const hit of folderHits) {
            const flatIdx = entries.length;
            const hitIdx = hitIndexMap.get(hit.doc_id) ?? 0;
            hitToFlat.set(hitIdx, flatIdx);
            entries.push({ kind: "doc", hit, hitIndex: hitIdx });
          }
        }
      }
    }

    return { flatEntries: entries, hitToFlatIndex: hitToFlat };
  }, [roots, rootFilter, hits, query, collapsedFolderIds]);

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: flatEntries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (index) => {
      const entry = flatEntries[index];
      if (!entry) return 64;
      if (entry.kind === "folder") return 34;
      if (entry.kind === "empty") return 38;
      return 64;
    },
    overscan: 8,
  });

  const virtualItems = virtualizer.getVirtualItems();

  useEffect(() => {
    if (virtualItems.length === 0 || hits.length === 0) return;
    const ids: number[] = [];
    for (const item of virtualItems) {
      const entry = flatEntries[item.index];
      if (entry?.kind === "doc") {
        ids.push(entry.hit.doc_id);
      }
    }
    if (ids.length > 0) {
      void fetchSnippets(ids);
    }
  }, [virtualItems, flatEntries, hits.length, fetchSnippets]);

  useEffect(() => {
    if (hits.length > 0 && selectedIndex >= 0 && selectedIndex < hits.length) {
      const flatIdx = hitToFlatIndex.get(selectedIndex);
      if (flatIdx !== undefined) {
        virtualizer.scrollToIndex(flatIdx, { align: "auto" });
      }
    }
  }, [selectedIndex, hits.length, hitToFlatIndex, virtualizer]);

  const handleUploadToFolder = async (folderId: number) => {
    if (!isTauriRuntime()) return;
    const selected = await open({
      directory: false,
      multiple: true,
      title: "Upload Markdown (.md) Files to Folder",
      filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
    });
    const paths = Array.isArray(selected)
      ? selected
      : typeof selected === "string"
        ? [selected]
        : [];
    if (paths.length > 0) {
      await uploadFiles(paths, folderId);
    }
  };

  if (roots.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center rounded-xl border border-white/[0.06] bg-white/[0.02] px-5 py-6 text-center">
        <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-500/15 text-indigo-300 ring-1 ring-indigo-400/25">
          <FolderPlus className="h-5 w-5" />
        </div>
        <p className="text-xs font-semibold text-zinc-200">No folders created yet</p>
        <p className="mt-1 max-w-[240px] text-[11px] leading-relaxed text-zinc-400">
          Create a folder above to organize notes, then drag &amp; drop `.md` files onto the dock.
        </p>
        <button
          type="button"
          onClick={onOpenSettings}
          className="mt-3.5 rounded-lg bg-indigo-500/85 px-3 py-1.5 text-[11px] font-medium text-white hover:bg-indigo-500"
        >
          Manage Folders
        </button>
      </div>
    );
  }

  if (query.trim().length > 0 && hits.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center rounded-xl border border-white/[0.06] bg-white/[0.02] px-5 py-6 text-center">
        <SearchX className="mb-2.5 h-7 w-7 text-zinc-500" />
        <p className="text-xs font-medium text-zinc-300">{`No matches for "${query}"`}</p>
        <p className="mt-1 text-[11px] text-zinc-500">
          Try keywords, <code className="text-zinc-400">tag:name</code>, or{" "}
          <code className="text-zinc-400">path:folder</code>
        </p>
      </div>
    );
  }

  return (
    <div
      ref={parentRef}
      role="listbox"
      aria-label="Grouped Markdown files"
      className="min-h-0 flex-1 overflow-y-auto pr-0.5"
    >
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {virtualItems.map((virtualRow) => {
          const entry = flatEntries[virtualRow.index];
          if (!entry) return null;

          if (entry.kind === "folder") {
            const { root, docCount, collapsed } = entry;
            const isRenaming = renamingId === root.id;
            const isConfirmDelete = confirmDeleteId === root.id;

            return (
              <div
                key={`folder-${root.id}`}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
                className="group/hdr flex items-center justify-between gap-1.5 rounded-lg px-1.5 pt-1 text-[11px]"
              >
                {isRenaming ? (
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
                    className="flex min-w-0 flex-1 items-center gap-1"
                  >
                    <input
                      type="text"
                      value={renameDraft}
                      onChange={(e) => {
                        setRenameDraft(e.target.value);
                        markTyping();
                      }}
                      autoFocus
                      maxLength={64}
                      className="h-6 min-w-0 flex-1 rounded border border-indigo-400/50 bg-white/[0.08] px-2 text-[11px] text-zinc-100 focus:outline-none"
                    />
                    <button
                      type="submit"
                      aria-label="Save folder name"
                      className="inline-flex h-6 w-6 items-center justify-center rounded bg-indigo-500/80 text-white hover:bg-indigo-500"
                    >
                      <Check className="h-3 w-3" />
                    </button>
                    <button
                      type="button"
                      onClick={() => setRenamingId(null)}
                      aria-label="Cancel rename"
                      className="inline-flex h-6 w-6 items-center justify-center rounded bg-white/10 text-zinc-300"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </form>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() => toggleFolderCollapsed(root.id)}
                      className="flex min-w-0 flex-1 items-center gap-1.5 text-left text-zinc-200 hover:text-white"
                    >
                      {collapsed ? (
                        <ChevronRight className="h-3.5 w-3.5 shrink-0 text-zinc-400" />
                      ) : (
                        <ChevronDown className="h-3.5 w-3.5 shrink-0 text-zinc-400" />
                      )}
                      <Folder className="h-3.5 w-3.5 shrink-0 text-indigo-300" />
                      <span className="truncate font-semibold">{root.label}</span>
                      <span className="rounded bg-white/[0.08] px-1.5 py-0.2 text-[9.5px] text-zinc-400 tabular-nums">
                        {docCount}
                      </span>
                    </button>

                    <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover/hdr:opacity-100">
                      <button
                        type="button"
                        onClick={() => void handleUploadToFolder(root.id)}
                        title={`Upload .md into ${root.label}`}
                        aria-label={`Upload Markdown to ${root.label}`}
                        className="inline-flex h-5 w-5 items-center justify-center rounded bg-white/[0.07] text-zinc-300 hover:bg-indigo-500/35 hover:text-white"
                      >
                        <Upload className="h-2.5 w-2.5" />
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setRenamingId(root.id);
                          setRenameDraft(root.label);
                          markTyping();
                        }}
                        title="Rename folder"
                        aria-label={`Rename ${root.label}`}
                        className="inline-flex h-5 w-5 items-center justify-center rounded bg-white/[0.07] text-zinc-300 hover:bg-white/15 hover:text-white"
                      >
                        <Pencil className="h-2.5 w-2.5" />
                      </button>
                      {isConfirmDelete ? (
                        <button
                          type="button"
                          onClick={() => {
                            setConfirmDeleteId(null);
                            void removeRoot(root.id);
                          }}
                          className="rounded bg-rose-500/80 px-1.5 py-0.5 text-[9.5px] font-medium text-white hover:bg-rose-500"
                        >
                          Confirm?
                        </button>
                      ) : (
                        <button
                          type="button"
                          onClick={() => {
                            if (docCount === 0) {
                              void removeRoot(root.id);
                            } else {
                              setConfirmDeleteId(root.id);
                              window.setTimeout(() => {
                                setConfirmDeleteId((cur) => (cur === root.id ? null : cur));
                              }, 3000);
                            }
                          }}
                          title="Delete folder"
                          aria-label={`Delete ${root.label}`}
                          className="inline-flex h-5 w-5 items-center justify-center rounded bg-white/[0.07] text-zinc-400 hover:bg-rose-500/20 hover:text-rose-300"
                        >
                          <Trash2 className="h-2.5 w-2.5" />
                        </button>
                      )}
                    </div>
                  </>
                )}
              </div>
            );
          }

          if (entry.kind === "empty") {
            return (
              <div
                key={`empty-${entry.root.id}`}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
                className="flex items-center justify-between rounded-lg border border-dashed border-white/[0.07] bg-white/[0.015] px-3 text-[10.5px] text-zinc-500"
              >
                <span>Empty folder — drag &amp; drop `.md` files</span>
                <button
                  type="button"
                  onClick={() => void handleUploadToFolder(entry.root.id)}
                  className="text-indigo-300 hover:text-indigo-200 hover:underline"
                >
                  Upload .md
                </button>
              </div>
            );
          }

          const { hit, hitIndex } = entry;
          return (
            <div
              key={hit.doc_id}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
                paddingBottom: "4px",
                paddingLeft: "8px",
              }}
            >
              <ResultRow
                hit={hit}
                folders={roots}
                snippetHtml={snippets[hit.doc_id]}
                selected={hitIndex === selectedIndex}
                active={hit.doc_id === activeDocId}
                onSelect={() => {
                  setSelectedIndex(hitIndex);
                  void openPreview(hit.doc_id);
                }}
                onCopy={() => void copyDoc(hit.doc_id, "Raw")}
                onOpenEditor={() => void openInEditor(hit.doc_id)}
                onMoveToFolder={(targetRootId) => void moveDocToFolder(hit.doc_id, targetRootId)}
                onDelete={() => void deleteDoc(hit.doc_id)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
