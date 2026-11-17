import { Copy, ExternalLink, Folder, FolderInput, Trash2 } from "lucide-react";
import type { RootInfo, SearchHit } from "../../bindings/commands";
import { cn } from "../../lib/cn";
import { CustomSelect } from "../ui/CustomSelect";

interface ResultRowProps {
  hit: SearchHit;
  folders?: RootInfo[];
  snippetHtml?: string;
  selected: boolean;
  active: boolean;
  onSelect: () => void;
  onCopy: () => void;
  onOpenEditor: () => void;
  onMoveToFolder?: (targetRootId: number) => void;
  onDelete?: () => void;
}

export function ResultRow({
  hit,
  folders = [],
  snippetHtml,
  selected,
  active,
  onSelect,
  onCopy,
  onOpenEditor,
  onMoveToFolder,
  onDelete,
}: ResultRowProps) {
  const otherFolders = folders.filter((f) => f.enabled && f.id !== hit.root_id);

  return (
    <div
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      className={cn(
        "group flex h-[60px] cursor-pointer items-center justify-between gap-2 rounded-xl px-2.5 py-1.5 transition-colors",
        active
          ? "bg-indigo-500/20 ring-1 ring-indigo-400/45"
          : selected
            ? "bg-white/[0.09] ring-1 ring-white/15"
            : "hover:bg-white/[0.06]",
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-zinc-500">
          <span className="truncate font-mono text-zinc-400">{hit.file_name}</span>
          <span>•</span>
          <span className="shrink-0">{hit.word_count} words</span>
        </div>

        {snippetHtml ? (
          <div
            className="dmd-snippet mt-0.5 truncate text-[10.5px] text-zinc-400"
            dangerouslySetInnerHTML={{ __html: snippetHtml }}
          />
        ) : (
          <div className="mt-0.5 truncate text-[10px] text-zinc-500">{hit.rel_path}</div>
        )}
      </div>

      {/* Quick 1-click actions on hover/selection */}
      <div
        className={cn(
          "flex shrink-0 items-center gap-1 transition-opacity",
          selected || active ? "opacity-100" : "opacity-0 group-hover:opacity-100",
        )}
      >
        {onMoveToFolder && otherFolders.length > 0 && (
          <CustomSelect
            variant="action"
            align="right"
            value=""
            menuHeader="Move to folder"
            ariaLabel={`Move ${hit.title} to folder`}
            title="Move file to another folder"
            triggerIcon={<FolderInput className="h-3 w-3" />}
            onChange={(val) => {
              const id = Number(val);
              if (Number.isFinite(id) && id > 0) {
                onMoveToFolder(id);
              }
            }}
            options={otherFolders.map((f) => ({
              value: String(f.id),
              label: f.label,
              icon: <Folder className="h-3 w-3" />,
            }))}
          />
        )}

        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onCopy();
          }}
          title="1-click copy raw Markdown"
          aria-label={`Copy ${hit.title}`}
          className="inline-flex h-6 w-6 items-center justify-center rounded-md bg-white/[0.07] text-zinc-300 hover:bg-indigo-500/30 hover:text-white"
        >
          <Copy className="h-3 w-3" />
        </button>

        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onOpenEditor();
          }}
          title="Open in default editor"
          aria-label={`Open ${hit.title} in editor`}
          className="inline-flex h-6 w-6 items-center justify-center rounded-md bg-white/[0.07] text-zinc-300 hover:bg-white/15 hover:text-white"
        >
          <ExternalLink className="h-3 w-3" />
        </button>

        {onDelete && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
            title="Delete file from folder"
            aria-label={`Delete ${hit.title}`}
            className="inline-flex h-6 w-6 items-center justify-center rounded-md bg-white/[0.07] text-zinc-400 hover:bg-rose-500/20 hover:text-rose-300"
          >
            <Trash2 className="h-3 w-3" />
          </button>
        )}
      </div>
    </div>
  );
}
