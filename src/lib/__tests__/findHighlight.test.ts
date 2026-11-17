import { describe, expect, it } from "vitest";
import { highlightHtmlMatches, splitTextMatches } from "../findHighlight";

describe("findHighlight", () => {
  it("splits plain text matches case-insensitively with sequential indices", () => {
    const res = splitTextMatches("Rust and rustfmt in RUST", "rust");
    expect(res.count).toBe(3);
    expect(res.parts.filter((p) => p.matchIdx !== null)).toEqual([
      { text: "Rust", matchIdx: 0 },
      { text: "rust", matchIdx: 1 },
      { text: "RUST", matchIdx: 2 },
    ]);
  });

  it("highlights text nodes in sanitized HTML without touching tags or attributes", () => {
    const html = `<p>Hello <a href="https://hello.example">Hello World</a></p>`;
    const res = highlightHtmlMatches(html, "hello");
    expect(res.count).toBe(2);
    expect(res.html).toContain('href="https://hello.example"');
    expect(res.html).toContain('data-find-idx="0"');
    expect(res.html).toContain('data-find-idx="1"');
  });

  it("returns original html with zero count when query is blank", () => {
    const html = "<p>Test</p>";
    expect(highlightHtmlMatches(html, "   ")).toEqual({ html, count: 0 });
  });
});
