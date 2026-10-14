/** Inline SVG becomes a standalone image in Takumi. Resolve host theme colors
 * in tag attributes before separation, without rewriting authored text content. */
export function captureHTML(html: string, theme: Record<string, string>): string {
  return html.replace(/<[a-zA-Z][^>]*>/g, (tag) =>
    tag.replace(/var\(--slop-([a-zA-Z0-9-]+)\)/g, (value, key: string) => theme[key] ?? value),
  );
}
