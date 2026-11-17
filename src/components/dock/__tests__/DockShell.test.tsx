import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DockShell } from "../DockShell";
import { useDockStore } from "../../../stores/dockStore";
import { useSearchStore } from "../../../stores/searchStore";

describe("DockShell", () => {
  beforeEach(() => {
    useDockStore.setState({
      state: "Collapsed",
      preCollapseMode: "Peek",
      anchor: "TopRight",
      pinned: false,
      seq: 1,
      lastTypingAt: 0,
      focusSearchSeq: 0,
      settingsOpen: false,
      visualLogical: { width: 40, height: 148 },
      shadowPad: 8,
      verticalRatio: 0.5,
      indexedCount: 42,
      syncStatus: "idle",
      droppedPaths: [],
      notice: null,
    });
    useSearchStore.setState({
      query: "",
      rootFilter: null,
      roots: [
        {
          id: 1,
          root_uuid: "r1",
          path: "D:\\Notes",
          label: "Notes",
          enabled: true,
          backup: false,
          exclude_globs: [],
          doc_count: 42,
        },
      ],
      hits: [],
      snippets: {},
      selectedIndex: 0,
      activeDocId: null,
      preview: null,
      droppedPreview: null,
    });
  });

  it("renders minimalist EdgeSliver in Stage 1 (Collapsed) state", () => {
    render(<DockShell />);
    expect(screen.getByTestId("dock-shell")).toHaveAttribute("data-dock-state", "Collapsed");
    expect(screen.getByTestId("dock-edge-sliver")).toBeInTheDocument();
  });

  it("renders compact Hover Capsule with icon & count badge when hovered (Peek), and opens Expanded panel on click", async () => {
    const expandSpy = vi.spyOn(useDockStore.getState(), "expandTo");
    useDockStore.setState({
      state: "Peek",
      visualLogical: { width: 58, height: 96 },
    });
    render(<DockShell />);
    expect(screen.queryByLabelText("Search Markdown files")).not.toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    const openBtn = screen.getByRole("button", { name: "Open Dynamic Markdown Dock" });
    await act(async () => {
      fireEvent.click(openBtn);
    });
    expect(expandSpy).toHaveBeenCalledWith("Expanded", { focusWindow: true });
    expandSpy.mockRestore();
  });

  it("renders search input in Expanded state", async () => {
    useDockStore.setState({
      state: "Expanded",
      visualLogical: { width: 400, height: 640 },
    });
    await act(async () => {
      render(<DockShell />);
    });
    expect(screen.getByLabelText("Search Markdown files")).toBeInTheDocument();
    expect(screen.getByText("42 files")).toBeInTheDocument();
  });

  it("renders drop zone overlay in DropTarget state", () => {
    useDockStore.setState({
      state: "DropTarget",
      visualLogical: { width: 240, height: 320 },
    });
    render(<DockShell />);
    expect(screen.getByText("Drop Markdown files to upload")).toBeInTheDocument();
  });
});
