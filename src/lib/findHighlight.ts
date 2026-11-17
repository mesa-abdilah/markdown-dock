export interface HtmlHighlightResult {
  html: string;
  count: number;
}

export interface TextPart {
  text: string;
  matchIdx: number | null;
}

export interface TextHighlightResult {
  parts: TextPart[];
  count: number;
}

export interface TextMatchOffset {
  start: number;
  end: number;
}

export function findTextMatchOffsets(text: string, query: string): TextMatchOffset[] {
  const needle = query.trim().toLowerCase();
  if (!needle || !text) return [];
  const lower = text.toLowerCase();
  const offsets: TextMatchOffset[] = [];
  let cursor = 0;
  while (cursor < lower.length) {
    const idx = lower.indexOf(needle, cursor);
    if (idx === -1) break;
    offsets.push({ start: idx, end: idx + needle.length });
    cursor = idx + needle.length;
  }
  return offsets;
}

/**
 * Splits plain text into literal segments + indexed case-insensitive match segments.
 */
export function splitTextMatches(text: string, query: string): TextHighlightResult {
  const needle = query.trim().toLowerCase();
  if (!needle || !text) {
    return { parts: text ? [{ text, matchIdx: null }] : [], count: 0 };
  }

  const lower = text.toLowerCase();
  const parts: TextPart[] = [];
  let cursor = 0;
  let count = 0;

  while (cursor < text.length) {
    const idx = lower.indexOf(needle, cursor);
    if (idx === -1) {
      parts.push({ text: text.slice(cursor), matchIdx: null });
      break;
    }
    if (idx > cursor) {
      parts.push({ text: text.slice(cursor, idx), matchIdx: null });
    }
    parts.push({
      text: text.slice(idx, idx + needle.length),
      matchIdx: count,
    });
    count += 1;
    cursor = idx + needle.length;
  }

  return { parts, count };
}

/**
 * Wraps case-insensitive text-node matches inside pre-sanitized HTML with
 * `<mark class="dmd-find-match" data-find-idx="N">` without breaking tags or attributes.
 */
export function highlightHtmlMatches(sanitizedHtml: string, query: string): HtmlHighlightResult {
  const needle = query.trim().toLowerCase();
  if (!needle || !sanitizedHtml || typeof document === "undefined") {
    return { html: sanitizedHtml, count: 0 };
  }

  const tpl = document.createElement("template");
  tpl.innerHTML = sanitizedHtml;

  const walker = document.createTreeWalker(tpl.content, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  let current = walker.nextNode();
  while (current) {
    const parentTag = current.parentElement?.tagName;
    if (parentTag !== "SCRIPT" && parentTag !== "STYLE") {
      textNodes.push(current as Text);
    }
    current = walker.nextNode();
  }

  let matchCount = 0;
  for (const node of textNodes) {
    const val = node.nodeValue ?? "";
    const lower = val.toLowerCase();
    let idx = lower.indexOf(needle);
    if (idx === -1) continue;

    const frag = document.createDocumentFragment();
    let cursor = 0;
    while (idx !== -1) {
      if (idx > cursor) {
        frag.appendChild(document.createTextNode(val.slice(cursor, idx)));
      }
      const mark = document.createElement("mark");
      mark.className = "dmd-find-match";
      mark.setAttribute("data-find-idx", String(matchCount));
      mark.textContent = val.slice(idx, idx + needle.length);
      frag.appendChild(mark);
      matchCount += 1;
      cursor = idx + needle.length;
      idx = lower.indexOf(needle, cursor);
    }
    if (cursor < val.length) {
      frag.appendChild(document.createTextNode(val.slice(cursor)));
    }
    node.parentNode?.replaceChild(frag, node);
  }

  return { html: tpl.innerHTML, count: matchCount };
}
