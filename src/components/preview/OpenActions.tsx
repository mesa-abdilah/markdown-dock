import { ExternalLink, FolderSearch } from "lucide-react";

interface OpenActionsProps {
  onOpenEditor: () => void;
  onRevealFolder: () => void;
}

export function OpenActions({ onOpenEditor, onRevealFolder }: OpenActionsProps) {
  return (
    <div className="flex items-center gap-1">
      <button
        type="button"
        onClick={onOpenEditor}
        title="Open in default OS editor (Ctrl+O)"
        className="inline-flex items-center gap-1 rounded-lg bg-white/[0.08] px-2 py-1 text-[11px] font-medium text-zinc-200 transition-colors hover:bg-white/[0.14] focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70"
      >
        <ExternalLink className="h-3 w-3" />
        <span>Open</span>
      </button>
      <button
        type="button"
        onClick={onRevealFolder}
        title="Reveal in Explorer / Finder"
        aria-label="Reveal in folder"
        className="inline-flex h-6 w-6 items-center justify-center rounded-lg text-zinc-400 transition-colors hover:bg-white/10 hover:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70"
      >
        <FolderSearch className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
