import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { EdgeSliver } from "../../components/dock/EdgeSliver";
import { ThemeSwitcher } from "../../components/ui/ThemeSwitcher";
import { clearStoredTheme, readStoredPeekColor, readStoredTheme, THEMES } from "../../lib/themes";
import { ThemeProvider } from "../ThemeContext";

describe("Multi-Theme Engine", () => {
  beforeEach(() => {
    clearStoredTheme();
    document.documentElement.removeAttribute("data-theme");
  });

  it("defaults to oled-dark and sets data-theme on html", () => {
    render(
      <ThemeProvider>
        <ThemeSwitcher mode="grid" />
      </ThemeProvider>,
    );
    expect(document.documentElement.getAttribute("data-theme")).toBe("oled-dark");
    expect(readStoredTheme()).toBe("oled-dark");
  });

  it("switches across all 6 themes and persists to storage", () => {
    render(
      <ThemeProvider>
        <ThemeSwitcher mode="grid" />
      </ThemeProvider>,
    );

    for (const meta of THEMES) {
      const radio = screen.getByRole("radio", { name: new RegExp(meta.name, "i") });
      act(() => {
        fireEvent.click(radio);
      });
      expect(document.documentElement.getAttribute("data-theme")).toBe(meta.id);
      expect(readStoredTheme()).toBe(meta.id);
      expect(radio).toHaveAttribute("aria-checked", "true");
    }
  });

  it("updates Dock Peek color via color picker and applies it to EdgeSliver", () => {
    render(
      <ThemeProvider>
        <ThemeSwitcher mode="grid" />
        <EdgeSliver onClickOpen={() => {}} />
      </ThemeProvider>,
    );

    const picker = screen.getByLabelText("Custom Dock Peek color picker");
    act(() => {
      fireEvent.change(picker, { target: { value: "#10b981" } });
    });

    expect(readStoredPeekColor()).toBe("#10b981");
    expect(document.documentElement.style.getPropertyValue("--peek-custom-color")).toBe("#10b981");
    const bar = screen.getByTestId("dock-edge-sliver-bar");
    expect(bar).toHaveStyle({ backgroundColor: "#10b981" });
  });
});
