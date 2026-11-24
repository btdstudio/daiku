import { openUrl } from "@tauri-apps/plugin-opener";
import type { ComponentProps } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";

const REMARK_PLUGINS = [remarkGfm, remarkFrontmatter];

/** Only `https:` and in-page `#` URLs survive; anything else becomes "". */
export function allowUrl(url: string): string {
  return url.startsWith("https:") || url.startsWith("#") ? url : "";
}

function Link({ href, children }: ComponentProps<"a">) {
  if (!href) return <span>{children}</span>;
  if (href.startsWith("#")) {
    return (
      <a href={href} onClick={(e) => e.preventDefault()} className="text-prompt underline">
        {children}
      </a>
    );
  }
  return (
    <a
      href={href}
      className="text-prompt underline"
      onClick={(e) => {
        e.preventDefault();
        // Only https: URLs get here (urlTransform); the capability scopes it again.
        openUrl(href).catch(() => {});
      }}
    >
      {children}
    </a>
  );
}

const components: Components = {
  a: Link,
  // An image renders as its alt text and loads nothing.
  img: ({ alt }) => <span>{alt}</span>,
  pre: ({ children }) => (
    <pre className="my-3 overflow-x-auto rounded-md bg-surface-raised p-3 text-list text-ink [&>code]:bg-transparent [&>code]:p-0">
      {children}
    </pre>
  ),
  code: ({ children, className }) => (
    <code className={`rounded-sm bg-surface-raised px-1 font-mono text-list ${className ?? ""}`}>
      {children}
    </code>
  ),
  input: (props) => <input {...props} disabled />,
  table: ({ children }) => (
    <table className="my-3 border-collapse text-list">{children}</table>
  ),
  th: ({ children }) => (
    <th className="border border-line px-2 py-1 text-left font-medium text-ink">{children}</th>
  ),
  td: ({ children }) => <td className="border border-line px-2 py-1">{children}</td>,
  hr: () => <hr className="my-4 border-line" />,
  h1: ({ children }) => <h1 className="mt-4 mb-2 text-title font-semibold text-ink">{children}</h1>,
  h2: ({ children }) => <h2 className="mt-4 mb-2 text-body font-semibold text-ink">{children}</h2>,
  h3: ({ children }) => <h3 className="mt-3 mb-1 text-body font-medium text-ink">{children}</h3>,
  p: ({ children }) => <p className="my-2">{children}</p>,
  ul: ({ children }) => <ul className="my-2 list-disc pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="my-2 list-decimal pl-5">{children}</ol>,
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-line pl-3 text-ink-muted">{children}</blockquote>
  ),
};

/** GFM markdown, read-only and safe: raw HTML is dropped, images show their alt text,
 *  and only `https:` links are links. */
export function Markdown({ source }: { source: string }) {
  return (
    <div className="text-body text-ink-secondary">
      <ReactMarkdown
        remarkPlugins={REMARK_PLUGINS}
        skipHtml
        urlTransform={allowUrl}
        components={components}
      >
        {source}
      </ReactMarkdown>
    </div>
  );
}
