import { lazy, Suspense, useEffect, useRef, useState } from "react";
import {
  Check,
  Keyboard,
  Lock,
  LockOpen,
  Monitor,
  Search,
  Settings2,
  Shield,
  X,
} from "lucide-react";
import type { MonitorPolicy } from "../../bindings/commands";
import { defaultDockIpc } from "../../lib/ipc";
import { useDockStore } from "../../stores/dockStore";
import { useSearchStore } from "../../stores/searchStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSyncStore } from "../../stores/syncStore";
import { useVaultStore } from "../../stores/vaultStore";
import { FilterChips } from "../search/FilterChips";
import { ResultList } from "../search/ResultList";
import { CustomCheckbox } from "../ui/CustomCheckbox";
import { CustomSelect } from "../ui/CustomSelect";
import { Kbd } from "../ui/Kbd";
import { PinButton } from "./PinButton";

const PreviewPane = lazy(() =>
  import("../preview/PreviewPane").then((m) => ({ default: m.PreviewPane })),
);
const RootsManager = lazy(() =>
  import("../settings/RootsManager").then((m) => ({ default: m.RootsManager })),
);
const RestoreDialog = lazy(() =>
  import("../vault/RestoreDialog").then((m) => ({ default: m.RestoreDialog })),
);
const SetupWizard = lazy(() =>
  import("../vault/SetupWizard").then((m) => ({ default: m.SetupWizard })),
);
const SyncPanel = lazy(() => import("../vault/SyncPanel").then((m) => ({ default: m.SyncPanel })));
const UnlockDialog = lazy(() =>
  import("../vault/UnlockDialog").then((m) => ({ default: m.UnlockDialog })),
);

function PanelLoadingFallback() {
  return (
    <div className="flex flex-1 items-center justify-center p-6 text-[11px] text-zinc-500">
      Loading panel…
    </div>
  );
}

export function ExpandedPanel() {
  const mode = useDockStore((s) => s.state);
  const pinned = useDockStore((s) => s.pinned);
  const setPinned = useDockStore((s) => s.setPinned);
  const collapse = useDockStore((s) => s.collapse);
  const settingsOpen = useDockStore((s) => s.settingsOpen);
  const toggleSettings = useDockStore((s) => s.toggleSettings);
  const markTyping = useDockStore((s) => s.markTyping);
  const focusSearchSeq = useDockStore((s) => s.focusSearchSeq);
  const syncStatus = useDockStore((s) => s.syncStatus);
  const indexedCount = useDockStore((s) => s.indexedCount);
  const notice = useDockStore((s) => s.notice);
  const clearNotice = useDockStore((s) => s.clearNotice);

  const query = useSearchStore((s) => s.query);
  const setQuery = useSearchStore((s) => s.setQuery);
  const hits = useSearchStore((s) => s.hits);
  const selectedIndex = useSearchStore((s) => s.selectedIndex);
  const moveSelection = useSearchStore((s) => s.moveSelection);
  const openPreview = useSearchStore((s) => s.openPreview);
  const togglePreviewMode = useSearchStore((s) => s.togglePreviewMode);
  const copyDoc = useSearchStore((s) => s.copyDoc);
  const openInEditor = useSearchStore((s) => s.openInEditor);
  const activeDocId = useSearchStore((s) => s.activeDocId);
  const droppedPreview = useSearchStore((s) => s.droppedPreview);
  const indexingProgress = useSearchStore((s) => s.indexingProgress);
  const searchError = useSearchStore((s) => s.error);
  const clearSearchError = useSearchStore((s) => s.clearError);
  const runSearch = useSearchStore((s) => s.runSearch);

  const settings = useSettingsStore((s) => s.settings);
  const monitors = useSettingsStore((s) => s.monitors);
  const updateSettings = useSettingsStore((s) => s.update);
  const refreshMonitors = useSettingsStore((s) => s.refreshMonitors);
  const settingsError = useSettingsStore((s) => s.error);
  const clearSettingsError = useSettingsStore((s) => s.clearError);

  const vaultStatus = useVaultStore((s) => s.status);
  const vaultModal = useVaultStore((s) => s.modalMode);
  const openVaultModal = useVaultStore((s) => s.openModal);
  const lockVault = useVaultStore((s) => s.lock);
  const syncModal = useSyncStore((s) => s.modalMode);

  const [hotkeyOverride, setHotkeyOverride] = useState<string | null>(null);
  const hotkeyDraft = hotkeyOverride ?? settings.hotkey;
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (mode === "Expanded" || mode === "Wide") {
      const id = window.requestAnimationFrame(() => {
        searchInputRef.current?.focus();
      });
      if (hits.length === 0 && !query) {
        void runSearch();
      }
      return () => window.cancelAnimationFrame(id);
    }
  }, [mode, focusSearchSeq, hits.length, query, runSearch]);

  useEffect(() => {
    if (settingsOpen) {
      void refreshMonitors();
    }
  }, [settingsOpen, refreshMonitors]);

  const handlePolicyChange = (value: string) => {
    let policy: MonitorPolicy;
    if (value === "__primary__") {
      policy = { kind: "Primary" };
    } else if (value === "__cursor__") {
      policy = { kind: "FollowCursor" };
    } else {
      policy = { kind: "Specific", name: value };
    }
    void updateSettings({ monitor_policy: policy });
  };

  const policySelectValue =
    settings.monitor_policy.kind === "Primary"
      ? "__primary__"
      : settings.monitor_policy.kind === "FollowCursor"
        ? "__cursor__"
        : settings.monitor_policy.name;

  const showPreviewSplit =
    !settingsOpen &&
    (mode === "Wide" || mode === "Fullscreen" || activeDocId !== null || droppedPreview !== null);
  const isFullscreenPreview = mode === "Fullscreen" && showPreviewSplit;

  useEffect(() => {
    const onCtrlF = () => {
      if (settingsOpen) return;
      if (showPreviewSplit) {
        window.dispatchEvent(new CustomEvent("dmd-open-find"));
      } else {
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };
    window.addEventListener("dmd-ctrl-f", onCtrlF);
    return () => window.removeEventListener("dmd-ctrl-f", onCtrlF);
  }, [settingsOpen, showPreviewSplit]);

  const handlePanelKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (settingsOpen) return;
    const target = e.target as HTMLElement | null;
    const isFindInput = target?.dataset?.dmdFindInput === "true";
    const isEditorTextarea = target instanceof HTMLTextAreaElement;

    if (e.key === "ArrowDown" && !isFindInput && !isEditorTextarea) {
      e.preventDefault();
      moveSelection(1);
    } else if (e.key === "ArrowUp" && !isFindInput && !isEditorTextarea) {
      e.preventDefault();
      moveSelection(-1);
    } else if (e.key === "Enter" && document.activeElement === searchInputRef.current) {
      const hit = hits[selectedIndex];
      if (hit) {
        e.preventDefault();
        void openPreview(hit.doc_id);
      }
    } else if (e.key === "Tab" && showPreviewSplit && !isEditorTextarea) {
      e.preventDefault();
      void togglePreviewMode();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
      const sel = window.getSelection()?.toString();
      if (!sel) {
        const targetId = activeDocId ?? hits[selectedIndex]?.doc_id;
        if (targetId !== undefined) {
          e.preventDefault();
          void copyDoc(targetId, "Raw");
        }
      }
    } else if (e.key === "F11" && showPreviewSplit) {
      e.preventDefault();
      void useDockStore
        .getState()
        .expandTo(isFullscreenPreview ? "Wide" : "Fullscreen", { focusWindow: true });
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
      const targetId = activeDocId ?? hits[selectedIndex]?.doc_id;
      if (targetId !== undefined) {
        e.preventDefault();
        void openInEditor(targetId);
      }
    }
  };

  return (
    <div onKeyDown={handlePanelKeyDown} className="flex h-full w-full flex-col overflow-hidden">
      {/* Header */}
      <header className="flex shrink-0 items-center justify-between border-b border-white/[0.07] px-3.5 py-2.5">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold tracking-tight text-zinc-100">Markdown Dock</span>
          <span className="rounded-full bg-gradient-to-b from-sky-400/20 to-blue-500/15 px-2 py-0.5 text-[10px] font-bold text-sky-100 tabular-nums shadow-[inset_0_1px_0_rgba(255,255,255,0.15)] ring-1 ring-sky-300/30">
            {indexedCount} files
          </span>
          {indexingProgress && (
            <span className="rounded bg-indigo-500/20 px-1.5 py-0.5 text-[10px] font-medium text-indigo-200 tabular-nums">
              Indexing {indexingProgress.done}/{indexingProgress.total}
            </span>
          )}
        </div>

        <div className="flex items-center gap-1">
          <div className="group/vault relative">
            <button
              type="button"
              onClick={() => {
                if (vaultStatus.state === "Uninitialized") {
                  openVaultModal("setup");
                } else if (vaultStatus.state === "Locked") {
                  openVaultModal("unlock");
                } else {
                  void lockVault(false);
                }
              }}
              aria-describedby="vault-hover-help"
              aria-label={
                vaultStatus.state === "Uninitialized"
                  ? "Setup Zero-Knowledge Vault"
                  : vaultStatus.state === "Locked"
                    ? "Unlock Vault"
                    : "Lock Vault"
              }
              className={`inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-[10.5px] font-semibold transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70 ${
                vaultStatus.state === "Unlocked"
                  ? "bg-gradient-to-b from-emerald-400/20 to-emerald-500/15 text-emerald-200 shadow-[0_0_10px_rgba(52,211,153,0.18)] ring-1 ring-emerald-400/40 hover:from-emerald-400/30 hover:to-emerald-500/25"
                  : vaultStatus.state === "Locked"
                    ? "bg-gradient-to-b from-amber-400/25 to-amber-500/15 text-amber-200 shadow-[0_0_10px_rgba(251,191,36,0.2)] ring-1 ring-amber-400/45 hover:from-amber-400/35 hover:to-amber-500/25"
                    : "bg-white/[0.06] text-zinc-300 ring-1 ring-white/15 hover:bg-white/12 hover:text-zinc-100"
              }`}
            >
              <span
                className={`h-1.5 w-1.5 rounded-full ${
                  vaultStatus.state === "Unlocked"
                    ? "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.9)]"
                    : vaultStatus.state === "Locked"
                      ? "animate-pulse bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.9)]"
                      : "bg-zinc-400"
                }`}
                aria-hidden="true"
              />
              {vaultStatus.state === "Unlocked" ? (
                <LockOpen className="h-3 w-3" />
              ) : vaultStatus.state === "Locked" ? (
                <Lock className="h-3 w-3" />
              ) : (
                <Shield className="h-3 w-3" />
              )}
              <span className="hidden sm:inline">
                {vaultStatus.state === "Unlocked"
                  ? "Unlocked"
                  : vaultStatus.state === "Locked"
                    ? "Locked"
                    : "Vault"}
              </span>
            </button>

            <div
              id="vault-hover-help"
              role="tooltip"
              className="pointer-events-none absolute top-full right-0 z-50 mt-1.5 w-64 rounded-xl border border-white/15 bg-zinc-950/95 p-2.5 text-left opacity-0 shadow-xl backdrop-blur-md transition-opacity duration-150 group-focus-within/vault:opacity-100 group-hover/vault:opacity-100"
            >
              <p className="text-[11px] font-semibold text-zinc-100">
                {vaultStatus.state === "Uninitialized"
                  ? "Vault Not Set Up — Click to Setup"
                  : vaultStatus.state === "Locked"
                    ? "Vault Locked — Click to Unlock"
                    : "Vault Unlocked — Click to Lock"}
              </p>
              <p className="mt-1 text-[10.5px] leading-relaxed text-zinc-400">
                {vaultStatus.state === "Uninitialized"
                  ? "Create a Master Password & Recovery Key to encrypt backups before uploading to Google Drive."
                  : vaultStatus.state === "Locked"
                    ? "Encryption keys are cleared from memory. Unlock with your Master Password or Recovery Key to enable Google Drive backup & restore."
                    : "Encryption keys are active in memory for zero-knowledge Google Drive backup & restore. Click to lock and wipe keys from RAM."}
              </p>
            </div>
          </div>
          <PinButton
            pinned={pinned}
            onToggle={() => {
              void setPinned(!pinned);
            }}
          />
          <button
            type="button"
            onClick={() => toggleSettings()}
            aria-pressed={settingsOpen}
            aria-label="Dock settings"
            title="Dock settings & folders"
            className={`mr-1 inline-flex h-7 w-7 items-center justify-center rounded-lg transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70 ${
              settingsOpen
                ? "bg-white/15 text-zinc-100"
                : "text-zinc-400 hover:bg-white/10 hover:text-zinc-100"
            }`}
          >
            <Settings2 className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => void collapse({ force: true })}
            aria-label="Collapse dock"
            title="Collapse dock (Esc)"
            className="inline-flex h-7 w-7 items-center justify-center rounded-lg text-zinc-400 transition-colors hover:bg-white/10 hover:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </header>

      {/* Notice / Error banners */}
      {notice && (
        <div className="mx-3 mt-2 flex shrink-0 items-center justify-between gap-2 rounded-lg border border-amber-400/25 bg-amber-500/10 px-2.5 py-1.5 text-[11px] text-amber-200">
          <span>{notice.text}</span>
          <button
            type="button"
            onClick={clearNotice}
            aria-label="Dismiss notice"
            className="text-amber-300 hover:text-white"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      )}

      {searchError && (
        <div className="mx-3 mt-2 flex shrink-0 items-center justify-between gap-2 rounded-lg border border-rose-500/30 bg-rose-500/15 px-2.5 py-1.5 text-[11px] text-rose-200">
          <span className="truncate">{searchError}</span>
          <button type="button" onClick={clearSearchError}>
            <X className="h-3 w-3" />
          </button>
        </div>
      )}

      {/* Screen-reader live status */}
      <div className="sr-only" aria-live="polite" aria-atomic="true">
        {`${indexedCount} files indexed. Vault ${vaultStatus.state}. Sync ${syncStatus}.`}
      </div>

      {/* Main Split Area */}
      {vaultModal === "setup" ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <Suspense fallback={<PanelLoadingFallback />}>
            <SetupWizard />
          </Suspense>
        </div>
      ) : vaultModal === "unlock" || vaultModal === "change-password" ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <Suspense fallback={<PanelLoadingFallback />}>
            <UnlockDialog />
          </Suspense>
        </div>
      ) : syncModal === "restore" ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <Suspense fallback={<PanelLoadingFallback />}>
            <RestoreDialog />
          </Suspense>
        </div>
      ) : settingsOpen ? (
        <div className="min-h-0 flex-1 overflow-y-auto px-3.5 pb-3.5">
          <div className="space-y-3.5 pt-3 text-xs">
            <Suspense fallback={<PanelLoadingFallback />}>
              <RootsManager />
              <SyncPanel />
            </Suspense>

            {/* Zero-Knowledge Vault Controls */}
            <div className="space-y-2 border-b border-white/[0.07] pb-3">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-200">Zero-Knowledge Vault</span>
                <span
                  className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${
                    vaultStatus.state === "Unlocked"
                      ? "bg-emerald-500/15 text-emerald-300"
                      : vaultStatus.state === "Locked"
                        ? "bg-amber-500/15 text-amber-300"
                        : "bg-white/10 text-zinc-400"
                  }`}
                >
                  {vaultStatus.state}
                </span>
              </div>
              <p className="text-[10.5px] leading-relaxed text-zinc-400">
                Manages local encryption keys for Google Drive backup &amp; restore. Locking clears
                keys from RAM; local `.md` notes remain accessible.
              </p>
              <div className="flex flex-wrap gap-1.5">
                {vaultStatus.state === "Uninitialized" ? (
                  <button
                    type="button"
                    onClick={() => openVaultModal("setup")}
                    className="rounded-md bg-indigo-500/85 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-indigo-500"
                  >
                    Setup Encryption Vault
                  </button>
                ) : vaultStatus.state === "Locked" ? (
                  <button
                    type="button"
                    onClick={() => openVaultModal("unlock")}
                    className="rounded-md bg-amber-500/80 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-amber-500"
                  >
                    Unlock Vault
                  </button>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() => openVaultModal("change-password")}
                      className="rounded-md bg-white/[0.08] px-2.5 py-1 text-[11px] text-zinc-200 hover:bg-white/15"
                    >
                      Change Password
                    </button>
                    <button
                      type="button"
                      onClick={() => void lockVault(false)}
                      className="rounded-md bg-white/[0.08] px-2.5 py-1 text-[11px] text-zinc-200 hover:bg-white/15"
                    >
                      Lock Now
                    </button>
                    {vaultStatus.remember_in_keyring && (
                      <button
                        type="button"
                        onClick={() => void lockVault(true)}
                        className="rounded-md bg-rose-500/15 px-2.5 py-1 text-[11px] text-rose-200 hover:bg-rose-500/25"
                      >
                        Forget Keyring
                      </button>
                    )}
                  </>
                )}
              </div>
            </div>

            <div className="flex items-center justify-between">
              <span className="font-semibold text-zinc-200">Window & Docking</span>
              <button
                type="button"
                onClick={() => void defaultDockIpc.dockRedock()}
                className="rounded-md bg-white/[0.08] px-2 py-1 text-[11px] text-zinc-300 hover:bg-white/15"
              >
                Re-dock edge
              </button>
            </div>

            {settingsError && (
              <div className="flex items-center justify-between rounded-lg border border-rose-500/30 bg-rose-500/15 px-2.5 py-1.5 text-[11px] text-rose-200">
                <span>{settingsError}</span>
                <button type="button" onClick={clearSettingsError}>
                  <X className="h-3 w-3" />
                </button>
              </div>
            )}

            <div className="block space-y-1">
              <span className="mb-2 flex items-center gap-1 text-[11px] font-medium text-zinc-400">
                <Monitor className="h-3.5 w-3.5" /> Target Display
              </span>
              <CustomSelect
                ariaLabel="Target Display"
                value={policySelectValue}
                onChange={handlePolicyChange}
                options={[
                  { value: "__primary__", label: "Primary Display" },
                  { value: "__cursor__", label: "Follow Cursor on Hotkey" },
                  ...monitors.map((m) => ({
                    value: m.name,
                    label: `${m.name} (${Math.round((m.scale_factor ?? 1) * 100)}% DPI${m.is_primary ? ", Primary" : ""})`,
                  })),
                ]}
              />
            </div>

            <label className="block space-y-1">
              <div className="flex justify-between text-[11px] font-medium text-zinc-400">
                <span>Vertical Edge Position</span>
                <span className="text-zinc-300 tabular-nums">
                  {Math.round((settings.vertical_ratio ?? 0.5) * 100)}%
                </span>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={Math.round((settings.vertical_ratio ?? 0.5) * 100)}
                onChange={(e) => {
                  const ratio = Number(e.target.value) / 100;
                  useDockStore.setState({ verticalRatio: ratio });
                  void defaultDockIpc.dockSetVerticalRatio(ratio, false);
                }}
                onMouseUp={(e) => {
                  const ratio = Number((e.target as HTMLInputElement).value) / 100;
                  void updateSettings({ vertical_ratio: ratio });
                }}
                className="w-full accent-indigo-500"
              />
            </label>

            <div className="space-y-1">
              <span className="mb-2 flex items-center gap-1 text-[11px] font-medium text-zinc-400">
                <Keyboard className="h-3.5 w-3.5" /> Global Hotkey
              </span>
              <div className="flex gap-1.5">
                <input
                  type="text"
                  value={hotkeyDraft}
                  onChange={(e) => setHotkeyOverride(e.target.value)}
                  className="min-w-0 flex-1 rounded-lg border border-white/10 bg-white/[0.06] px-2.5 py-1.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => {
                    const nextHotkey = hotkeyDraft;
                    setHotkeyOverride(null);
                    void updateSettings({ hotkey: nextHotkey });
                  }}
                  className="inline-flex items-center gap-1 rounded-lg bg-indigo-500/80 px-2.5 py-1.5 text-[11px] font-medium text-white hover:bg-indigo-500"
                >
                  <Check className="h-3 w-3" /> Apply
                </button>
              </div>
            </div>

            <div className="space-y-2 border-t border-white/[0.07] pt-2.5">
              <label className="flex cursor-pointer items-center justify-between">
                <span className="text-zinc-300">Reduced Motion (Crossfade)</span>
                <CustomCheckbox
                  variant="switch"
                  checked={settings.reduced_motion_override ?? false}
                  onChange={(checked) =>
                    void updateSettings({
                      reduced_motion_override: checked ? true : null,
                    })
                  }
                />
              </label>
              <label className="flex cursor-pointer items-center justify-between">
                <span className="text-zinc-300">Launch at OS Login</span>
                <CustomCheckbox
                  variant="switch"
                  checked={settings.autostart}
                  onChange={(checked) => void updateSettings({ autostart: checked })}
                />
              </label>
            </div>
          </div>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 overflow-hidden">
          {/* Left column: Search + Virtual ResultList (hidden in Fullscreen .md mode) */}
          {!isFullscreenPreview && (
            <div
              className={`flex min-h-0 flex-col ${
                showPreviewSplit ? "w-[300px] shrink-0" : "min-w-0 flex-1"
              }`}
            >
              <div className="shrink-0 px-3 pt-2.5 pb-2">
                <div className="relative flex items-center">
                  <Search className="pointer-events-none absolute left-3 h-3.5 w-3.5 text-zinc-400" />
                  <input
                    ref={searchInputRef}
                    type="text"
                    value={query}
                    onChange={(e) => {
                      setQuery(e.target.value);
                      markTyping();
                    }}
                    onFocus={() => markTyping()}
                    placeholder="Search .md files, tag:name, or content..."
                    aria-label="Search Markdown files"
                    className="h-9 w-full rounded-xl border border-white/10 bg-white/[0.06] pr-14 pl-8 text-xs text-zinc-100 placeholder-zinc-500 transition-colors focus:border-indigo-400/60 focus:bg-white/[0.09] focus:outline-none"
                  />
                  {query ? (
                    <button
                      type="button"
                      onClick={() => {
                        setQuery("");
                        searchInputRef.current?.focus();
                      }}
                      aria-label="Clear search"
                      className="absolute right-2.5 rounded p-1 text-zinc-400 hover:text-zinc-200"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  ) : (
                    <span className="pointer-events-none absolute right-2.5 pb-1.5">
                      <Kbd>{settings.hotkey}</Kbd>
                    </span>
                  )}
                </div>
              </div>

              <FilterChips />

              <div className="flex min-h-0 flex-1 flex-col px-3 pb-2">
                <ResultList onOpenSettings={() => toggleSettings(true)} />
              </div>

              <footer className="flex shrink-0 items-center justify-between border-t border-white/[0.06] px-3 py-1.5 text-[10px] text-zinc-500">
                <div className="flex items-center gap-2">
                  <span>
                    <Kbd>↑↓</Kbd> navigate
                  </span>
                  <span>
                    <Kbd>Enter</Kbd> preview
                  </span>
                  <span>
                    <Kbd>Ctrl+C</Kbd> copy
                  </span>
                </div>
                <span>{pinned ? "Pinned" : "Auto-collapse"}</span>
              </footer>
            </div>
          )}

          {/* Right column: PreviewPane in Wide or Fullscreen mode */}
          {showPreviewSplit && (
            <Suspense fallback={<PanelLoadingFallback />}>
              <PreviewPane />
            </Suspense>
          )}
        </div>
      )}
    </div>
  );
}
