/** Only a leading `---` block is front matter: the text starts with `---` on its own
 *  line and a later line is exactly `---`. Otherwise it's all body. */
export function splitFrontMatter(text: string): { front: string | null; body: string } {
  const lines = text.split("\n");
  const marker = (line: string) => line.replace(/\r$/, "") === "---";
  if (lines.length < 2 || !marker(lines[0])) return { front: null, body: text };
  const close = lines.findIndex((line, i) => i > 0 && marker(line));
  if (close === -1) return { front: null, body: text };
  return {
    front: lines
      .slice(1, close)
      .map((l) => l.replace(/\r$/, ""))
      .join("\n"),
    body: lines.slice(close + 1).join("\n"),
  };
}
