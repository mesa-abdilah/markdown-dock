import { FileDown } from "lucide-react";

export function DropZoneOverlay() {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center p-4 text-center">
      <div className="flex h-full w-full flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed border-indigo-400/50 bg-indigo-500/[0.08] px-4">
        <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-indigo-500/20 text-indigo-300 ring-1 ring-indigo-400/30">
          <FileDown className="h-5 w-5" />
        </div>
        <div className="space-y-1">
          <p className="text-xs font-semibold text-zinc-100">Drop Markdown files to upload</p>
          <p className="text-[11px] leading-relaxed text-zinc-400">
            Only `.md` files are accepted. Create folders inside the dock to group files.
          </p>
        </div>
      </div>
    </div>
  );
}
