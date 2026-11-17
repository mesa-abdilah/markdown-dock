import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowLeft,
  ChevronDown,
  ChevronUp,
  Code2,
  Eye,
  Folder,
  FolderInput,
  Maximize2,
  Minimize2,
  Redo2,
  Save,
  Search,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import { findTextMatchOffsets, highlightHtmlMatches } from "../../lib/findHighlight";
import { useDockStore } from "../../stores/dockStore";
import { useSearchStore } from "../../stores/searchStore";
import { CustomSelect } from "../ui/CustomSelect";
import { CopyButton } from "./CopyButton";
import { OpenActions } from "./OpenActions";
import { RawView } from "./RawView";
import { RenderedView } from "./RenderedView";

export function PreviewPane() {
  const preview = useSearchStore((s) => s.preview);
  const droppedPreview = useSearchStore((s) => s.droppedPreview);
  const roots = useSearchStore((s) => s.roots);
  const hits = useSearchStore((s) => s.hits);
  const previewMode = useSearchStore((s) => s.previewMode);
  const draftContent = useSearchStore((s) => s.draftContent);
  const undoStack = useSearchStore((s) => s.undoStack);
  const redoStack = useSearchStore((s) => s.redoStack);
  const dirty = useSearchStore((s) => s.dirty);
  const saving = useSearchStore((s) => s.saving);
  const loadingPreview = useSearchStore((s) => s.loadingPreview);
  const copiedFeedback = useSearchStore((s) => s.copiedFeedback);
  const closePreview = useSearchStore((s) => s.closePreview);
  const togglePreviewMode = useSearchStore((s) => s.togglePreviewMode);
  const setDraftContent = useSearchStore((s) => s.setDraftContent);
  const undoDraft = useSearchStore((s) => s.undoDraft);
  const redoDraft = useSearchStore((s) => s.redoDraft);
  const saveCurrentDraft = useSearchStore((s) => s.saveCurrentDraft);
  const copyDoc = useSearchStore((s) => s.copyDoc);
  const openInEditor = useSearchStore((s) => s.openInEditor);
  const revealInFolder = useSearchStore((s) => s.revealInFolder);
  const moveDocToFolder = useSearchStore((s) => s.moveDocToFolder);
  const deleteDoc = useSearchStore((s) => s.deleteDoc);
  const dockState = useDockStore((s) => s.state);
  const expandTo = useDockStore((s) => s.expandTo);
  const markTyping = useDockStore((s) => s.markTyping);

  const isFullscreen = dockState === "Fullscreen";

  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [rawMatchIdx, setRawMatchIdx] = useState(0);
  const [jumpSeq, setJumpSeq] = useState(0);
  const findInputRef = useRef<HTMLInputElement>(null);

  const active = preview ?? droppedPreview;
  const currentHit = preview ? hits.find((h) => h.doc_id === preview.doc_id) : undefined;
  const otherFolders = roots.filter((r) => r.enabled && r.id !== currentHit?.root_id);

  const rawHtml = preview?.html ?? droppedPreview?.html ?? "";
  const rawText = preview ? draftContent : (droppedPreview?.raw ?? "");
  const effectiveQuery = findOpen ? findQuery.trim() : "";

  const htmlResult = useMemo(
    () => highlightHtmlMatches(rawHtml, effectiveQuery),
    [rawHtml, effectiveQuery],
  );
  const textOffsets = useMemo(
    () => findTextMatchOffsets(rawText, effectiveQuery),
    [rawText, effectiveQuery],
  );

  const isRendered = previewMode === "Rendered" && Boolean(rawHtml);
  const matchCount = isRendered ? htmlResult.count : textOffsets.length;
  const safeActiveIdx =
    matchCount === 0 ? 0 : ((rawMatchIdx % matchCount) + matchCount) % matchCount;
  const canUndo = undoStack.length > 0;
  const canRedo = redoStack.length > 0;

  useEffect(() => {
    const onOpenFind = () => {
      setFindOpen(true);
      window.requestAnimationFrame(() => {
        findInputRef.current?.focus();
        findInputRef.current?.select();
      });
    };
    window.addEventListener("dmd-open-find", onOpenFind);
    return () => window.removeEventListener("dmd-open-find", onOpenFind);
  }, []);

  const stepMatch = (delta: number) => {
    if (matchCount === 0) return;
    setRawMatchIdx((cur) => cur + delta);
    setJumpSeq((s) => s + 1);
  };

  return (
    <section
      aria-label="Markdown editor & preview"
      className={
        isFullscreen
          ? "flex h-full min-w-0 flex-1 flex-col bg-black/20"
          : "flex h-full min-w-0 flex-1 flex-col border-l border-white/[0.08] bg-black/20"
      }
    >
      {/* Preview Toolbar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-1.5 border-b border-white/[0.07] px-3 py-2">
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          <button
            type="button"
            onClick={() => void closePreview()}
            title="Close preview"
            aria-label="Close preview"
            className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-zinc-400 hover:bg-white/10 hover:text-zinc-100"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
          </button>
          <span className="truncate text-xs font-semibold text-zinc-100">
            {active ? active.title : "Preview"}
          </span>
          {preview && (
            <span
              className={`shrink-0 rounded px-1.5 py-0.5 text-[9.5px] font-medium ${
                saving
                  ? "bg-indigo-500/20 text-indigo-200"
                  : dirty
                    ? "bg-amber-500/20 text-amber-200"
                    : "bg-emerald-500/15 text-emerald-300"
              }`}
            >
              {saving ? "Saving..." : dirty ? "Unsaved" : "Saved"}
            </span>
          )}
        </div>

        <div className="flex shrink-0 flex-wrap items-center gap-1">
          {preview && previewMode === "Raw" && (
            <>
              <button
                type="button"
                disabled={!canUndo}
                onClick={() => undoDraft()}
                title="Undo (Ctrl+Z)"
                aria-label="Undo edit"
                className="inline-flex h-7 w-7 items-center justify-center rounded-lg bg-white/[0.07] text-zinc-300 hover:bg-white/[0.14] disabled:cursor-not-allowed disabled:opacity-35"
              >
                <Undo2 className="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                disabled={!canRedo}
                onClick={() => redoDraft()}
                title="Redo (Ctrl+Y / Ctrl+Shift+Z)"
                aria-label="Redo edit"
                className="inline-flex h-7 w-7 items-center justify-center rounded-lg bg-white/[0.07] text-zinc-300 hover:bg-white/[0.14] disabled:cursor-not-allowed disabled:opacity-35"
              >
                <Redo2 className="h-3.5 w-3.5" />
              </button>
            </>
          )}

          {preview && (
            <button
              type="button"
              disabled={!dirty || saving}
              onClick={() => void saveCurrentDraft()}
              title="Save changes (Ctrl+S)"
              aria-label="Save changes"
              className={`inline-flex h-7 items-center gap-1 rounded-lg px-2 text-[11px] font-medium transition-colors ${
                dirty
                  ? "bg-emerald-500/85 text-white hover:bg-emerald-500"
                  : "bg-white/[0.06] text-zinc-500"
              } disabled:cursor-not-allowed disabled:opacity-50`}
            >
              <Save className="h-3 w-3" />
              <span>{saving ? "Saving" : "Save"}</span>
            </button>
          )}

          <button
            type="button"
            onClick={() => {
              const next = !findOpen;
              setFindOpen(next);
              if (next) {
                window.requestAnimationFrame(() => {
                  findInputRef.current?.focus();
                  findInputRef.current?.select();
                });
              }
            }}
            aria-pressed={findOpen}
            title="Find text in Markdown (Ctrl+F)"
            aria-label="Find in document"
            className={`inline-flex h-7 w-7 items-center justify-center rounded-lg transition-colors ${
              findOpen
                ? "bg-indigo-500/30 text-indigo-200 ring-1 ring-indigo-400/40"
                : "bg-white/[0.07] text-zinc-300 hover:bg-white/[0.14]"
            }`}
          >
            <Search className="h-3.5 w-3.5" />
          </button>

          <button
            type="button"
            onClick={() =>
              void expandTo(isFullscreen ? "Wide" : "Fullscreen", { focusWindow: true })
            }
            aria-pressed={isFullscreen}
            title={isFullscreen ? "Exit full screen (F11 / Esc)" : "Full screen editor (F11)"}
            aria-label={isFullscreen ? "Exit full screen" : "Full screen preview"}
            className={`inline-flex h-7 w-7 items-center justify-center rounded-lg transition-colors ${
              isFullscreen
                ? "bg-indigo-500/30 text-indigo-200 ring-1 ring-indigo-400/40"
                : "bg-white/[0.07] text-zinc-300 hover:bg-white/[0.14]"
            }`}
          >
            {isFullscreen ? (
              <Minimize2 className="h-3.5 w-3.5" />
            ) : (
              <Maximize2 className="h-3.5 w-3.5" />
            )}
          </button>

          <button
            type="button"
            onClick={() => void togglePreviewMode()}
            title="Toggle Raw / Rendered (Tab)"
            className="inline-flex items-center gap-1 rounded-lg bg-white/[0.07] px-2 py-1 text-[11px] font-medium text-zinc-200 hover:bg-white/[0.14]"
          >
            {previewMode === "Rendered" ? (
              <>
                <Code2 className="h-3 w-3" />
                <span>Raw</span>
              </>
            ) : (
              <>
                <Eye className="h-3 w-3" />
                <span>Preview</span>
              </>
            )}
          </button>

          {preview && (
            <>
              {otherFolders.length > 0 && (
                <CustomSelect
                  variant="action"
                  align="right"
                  value=""
                  menuHeader="Move to folder"
                  ariaLabel="Move document to folder"
                  title="Move file to folder"
                  triggerIcon={<FolderInput className="h-3 w-3" />}
                  triggerLabel="Move"
                  onChange={(val) => {
                    const id = Number(val);
                    if (Number.isFinite(id) && id > 0) {
                      void moveDocToFolder(preview.doc_id, id);
                    }
                  }}
                  options={otherFolders.map((f) => ({
                    value: String(f.id),
                    label: f.label,
                    icon: <Folder className="h-3 w-3" />,
                  }))}
                />
              )}
              <CopyButton
                copied={copiedFeedback === "Raw"}
                onClick={() => void copyDoc(preview.doc_id, "Raw")}
              />
              <OpenActions
                onOpenEditor={() => void openInEditor(preview.doc_id)}
                onRevealFolder={() => void revealInFolder(preview.doc_id)}
              />
              <button
                type="button"
                onClick={() => void deleteDoc(preview.doc_id)}
                title="Delete file"
                aria-label="Delete file"
                className="inline-flex h-7 w-7 items-center justify-center rounded-lg bg-white/[0.07] text-zinc-400 hover:bg-rose-500/20 hover:text-rose-300"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </>
          )}
        </div>
      </div>

      {/* In-Document Find Bar (Ctrl+F) */}
      {findOpen && (
        <div className="flex shrink-0 items-center gap-1.5 border-b border-white/[0.07] bg-white/[0.03] px-3 py-1.5">
          <Search className="h-3 w-3 shrink-0 text-zinc-400" />
          <input
            ref={findInputRef}
            data-dmd-find-input="true"
            type="text"
            value={findQuery}
            onChange={(e) => {
              setFindQuery(e.target.value);
              setRawMatchIdx(0);
              markTyping();
            }}
            onFocus={() => markTyping()}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                e.stopPropagation();
                stepMatch(e.shiftKey ? -1 : 1);
              } else if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                e.nativeEvent.stopImmediatePropagation();
                setFindOpen(false);
              }
            }}
            placeholder="Find text in this Markdown file..."
            aria-label="Find text in Markdown preview"
            className="h-6 min-w-0 flex-1 rounded bg-white/[0.06] px-2 text-[11px] text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-1 focus:ring-indigo-400/60"
          />
          <span className="shrink-0 font-mono text-[10px] text-zinc-400 tabular-nums">
            {effectiveQuery
              ? matchCount === 0
                ? "0/0"
                : `${safeActiveIdx + 1}/${matchCount}`
              : ""}
          </span>
          <button
            type="button"
            disabled={matchCount === 0}
            onClick={() => stepMatch(-1)}
            title="Previous match (Shift+Enter)"
            aria-label="Previous match"
            className="inline-flex h-6 w-6 items-center justify-center rounded text-zinc-300 hover:bg-white/10 disabled:opacity-35"
          >
            <ChevronUp className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            disabled={matchCount === 0}
            onClick={() => stepMatch(1)}
            title="Next match (Enter)"
            aria-label="Next match"
            className="inline-flex h-6 w-6 items-center justify-center rounded text-zinc-300 hover:bg-white/10 disabled:opacity-35"
          >
            <ChevronDown className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => setFindOpen(false)}
            title="Close find bar (Esc)"
            aria-label="Close find bar"
            className="inline-flex h-6 w-6 items-center justify-center rounded text-zinc-400 hover:bg-white/10 hover:text-zinc-200"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      )}

      {/* Metadata Strip */}
      {active && (
        <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-white/[0.05] px-3.5 py-1.5 text-[10px] text-zinc-400">
          <span className="truncate font-mono text-zinc-400">
            {preview ? `${preview.root_label}/${preview.rel_path}` : droppedPreview?.path}
          </span>
          <span>•</span>
          <span>{active.word_count} words</span>
          {active.tags.slice(0, 4).map((t) => (
            <span
              key={t}
              className="rounded bg-indigo-500/15 px-1.5 py-0.2 text-[9.5px] font-medium text-indigo-300"
            >
              #{t}
            </span>
          ))}
        </div>
      )}

      {/* Body */}
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <div className={isFullscreen ? "mx-auto flex h-full w-full max-w-4xl flex-col py-2" : "flex h-full w-full flex-col"}>
          {loadingPreview ? (
            <div className="space-y-2 pt-2">
              <div className="h-4 w-2/3 animate-pulse rounded bg-white/10" />
              <div className="h-3 w-full animate-pulse rounded bg-white/[0.06]" />
              <div className="h-3 w-5/6 animate-pulse rounded bg-white/[0.06]" />
            </div>
          ) : preview ? (
            isRendered ? (
              <>
                {dirty && (
                  <p className="mb-2 rounded-lg border border-amber-400/25 bg-amber-500/10 px-2.5 py-1 text-[10.5px] text-amber-200">
                    Showing last saved render — press Ctrl+S or click Save to save your edits.
                  </p>
                )}
                <RenderedView
                  html={htmlResult.html}
                  activeMatchIdx={safeActiveIdx}
                  findQuery={effectiveQuery}
                />
              </>
            ) : (
              <RawView
                raw={draftContent}
                onChange={(next) => {
                  setDraftContent(next);
                  markTyping();
                }}
                onSaveNow={() => void saveCurrentDraft()}
                onUndo={() => {
                  undoDraft();
                  markTyping();
                }}
                onRedo={() => {
                  redoDraft();
                  markTyping();
                }}
                matchOffsets={textOffsets}
                activeMatchIdx={safeActiveIdx}
                jumpSeq={jumpSeq}
              />
            )
          ) : droppedPreview ? (
            previewMode === "Rendered" ? (
              <RenderedView
                html={htmlResult.html}
                activeMatchIdx={safeActiveIdx}
                findQuery={effectiveQuery}
              />
            ) : (
              <RawView
                raw={droppedPreview.raw}
                readOnly
                matchOffsets={textOffsets}
                activeMatchIdx={safeActiveIdx}
                jumpSeq={jumpSeq}
              />
            )
          ) : (
            <p className="text-xs text-zinc-500">Select a Markdown file to edit.</p>
          )}
        </div>
      </div>
    </section>
  );
}
