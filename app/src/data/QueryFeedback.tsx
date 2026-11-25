import { failureText } from "./envelope";

interface Props {
  /** Core's warnings, in the order its envelope gave them: shown as text, never HTML. */
  warnings: string[];
  /** A query's failure: a `FailureError` becomes "Kind: message". */
  error?: unknown;
  /** One line with no block padding, for the status bar (`S-0019`, T8). */
  inline?: boolean;
}

const BLOCK = "px-3 py-2 text-list text-ink-muted";
const INLINE = "min-w-0 truncate whitespace-nowrap text-ink-muted";
const SEPARATOR = " · ";

/** A query's warnings and failure as text, beside the data they came with (`S-0019`
 *  D8). The caller keeps rendering its data; this shows only the feedback. */
export function QueryFeedback({ warnings, error, inline = false }: Props) {
  const failure = error == null ? null : failureText(error);
  if (inline) {
    const all = failure === null ? warnings : [...warnings, failure];
    return all.length === 0 ? null : (
      <span data-slot="feedback" className={INLINE}>
        {all.join(SEPARATOR)}
      </span>
    );
  }
  return (
    <>
      {failure !== null && (
        <p role="alert" className={BLOCK}>
          {failure}
        </p>
      )}
      {warnings.length > 0 && (
        <ul data-slot="warnings" className={BLOCK}>
          {warnings.map((w, i) => (
            <li key={i}>{w}</li>
          ))}
        </ul>
      )}
    </>
  );
}
