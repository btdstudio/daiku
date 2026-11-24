/** A leading front-matter block as plain mono text, above the body. */
export function FrontMatter({ text }: { text: string }) {
  return (
    <pre
      data-front-matter
      className="mb-3 overflow-x-auto rounded-md bg-surface-raised p-3 font-mono text-list text-ink-muted"
    >
      {text}
    </pre>
  );
}
