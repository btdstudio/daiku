import { useState } from "react";
import { FailureError } from "@/data/envelope";
import { useArtifacts, useReadArtifact } from "@/data/hooks";
import { PaneHeader } from "@/frame/PaneHeader";
import { FrontMatter } from "@/viewer/FrontMatter";
import { Markdown } from "@/viewer/Markdown";
import { splitFrontMatter } from "@/viewer/splitFrontMatter";
import { NOT_STARTED, phaseStrip, type StripPhase } from "./phaseStrip";

interface Props {
  root: string | null;
  /** The plan's name. */
  name: string;
  /** A phase to start on (from the nudge); one not in the strip falls back to the current. */
  initialPhase?: string;
  /** The nudge's click count: a repeat click starts on `initialPhase` again. */
  nonce?: number;
  focused: boolean;
  onFill: () => void;
  onClose?: () => void;
}

/** "File too large to show (N MiB)" for the `TooLarge` Failure, from its message. */
export function tooLarge(message: string): string {
  const n = /\(([\d.]+) MiB\)/.exec(message)?.[1];
  return n ? `File too large to show (${n} MiB)` : "File too large to show";
}

/** A query's failure as text: `TooLarge` reworded, others as "Kind: message". */
export function failureText(e: unknown): string {
  if (!(e instanceof FailureError)) return String(e);
  const { kind, message } = e.failure.error;
  return kind === "TooLarge" ? tooLarge(message) : `${kind}: ${message}`;
}

function Dot({ status }: { status: string }) {
  const style =
    status === "approved" || status === "complete"
      ? "bg-ink-secondary border-ink-secondary"
      : status === "skipped"
        ? "border-ink-faint border-dashed"
        : status === NOT_STARTED
          ? "border-line"
          : "border-ink-secondary";
  return <span aria-hidden className={`inline-block size-2 rounded-pill border ${style}`} />;
}

function Chip({ p, chosen, onChoose }: { p: StripPhase; chosen: boolean; onChoose: () => void }) {
  return (
    <button
      type="button"
      onClick={onChoose}
      aria-pressed={chosen}
      aria-current={p.current ? "step" : undefined}
      title={p.skipped && p.reason !== "" ? p.reason : undefined}
      className={`flex items-center gap-1.5 rounded-md border px-2 py-1 text-list outline-none focus-visible:ring-2 focus-visible:ring-ring ${
        chosen ? "border-ink-faint bg-surface-selected" : "border-line bg-surface-raised hover:bg-surface-selected"
      } ${p.skipped ? "text-ink-faint line-through" : "text-ink"}`}
    >
      <Dot status={p.status} />
      <span>{p.name}</span>
      <span className={`text-label no-underline ${p.current ? "text-ink-secondary" : "text-ink-faint"}`}>
        {p.current ? `${p.status} · current` : p.status}
      </span>
    </button>
  );
}

/** A plan in the viewer (design D6): the phase strip, the chosen phase's files, and
 *  the chosen file read through `read_artifact`. The phase and file choice is local,
 *  and starts again when the plan or the workspace changes. */
export function PlanPanel({ root, name, initialPhase, nonce, focused, onFill, onClose }: Props) {
  const key = `${root}\0${name}\0${initialPhase ?? ""}\0${nonce ?? 0}`;
  const start = { key, phase: initialPhase ?? null, file: null };
  const [pick, setPick] = useState<{ key: string; phase: string | null; file: string | null }>(
    start,
  );
  if (pick.key !== key) setPick(start);
  const chosen = pick.key === key ? pick : start;

  const artifacts = useArtifacts(root, name);
  const strip = artifacts.data ? phaseStrip(artifacts.data) : null;
  const phase = strip
    ? (strip.find((p) => p.name === chosen.phase) ?? strip.find((p) => p.current) ?? strip[0])
    : undefined;
  const file = phase ? (phase.files.find((f) => f === chosen.file) ?? phase.files[0] ?? null) : null;
  const read = useReadArtifact(root, name, file);

  let content;
  if (phase && file === null) {
    content = <p className="p-4 text-list text-ink-faint">No files in this phase</p>;
  } else if (read.data !== undefined) {
    const { front, body } = splitFrontMatter(read.data);
    content = (
      <article className="p-4">
        {front !== null && <FrontMatter text={front} />}
        <Markdown source={body} />
      </article>
    );
  } else if (read.isPending && file !== null) {
    content = <p className="p-4 text-list text-ink-faint">Reading…</p>;
  }

  const error = artifacts.error ?? read.error;
  return (
    <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader title={name} focused={focused} onFill={onFill} onClose={onClose} />
      <div className="min-h-0 flex-1 overflow-auto">
        {error && (
          <p role="alert" className="px-4 py-2 text-list text-ink-muted">
            {failureText(error)}
          </p>
        )}
        {strip === null && artifacts.isPending && (
          <p className="p-4 text-list text-ink-faint">Reading…</p>
        )}
        {strip !== null && phase && (
          <>
            <div role="group" aria-label="Phases" className="flex flex-wrap gap-1.5 px-4 pt-3">
              {strip.map((p) => (
                <Chip
                  key={p.name}
                  p={p}
                  chosen={p.name === phase.name}
                  onChoose={() => setPick({ key, phase: p.name, file: null })}
                />
              ))}
            </div>
            {phase.skipped && phase.reason !== "" && (
              <p className="px-4 pt-2 text-list text-ink-muted">
                Skipped: {phase.reason}
              </p>
            )}
            {phase.files.length > 0 && (
              <ul aria-label="Files" className="flex flex-col px-3 pt-2">
                {phase.files.map((f) => (
                  <li key={f}>
                    <button
                      type="button"
                      onClick={() => setPick({ key, phase: phase.name, file: f })}
                      aria-current={f === file ? "true" : undefined}
                      className={`w-full rounded-md px-2 py-1 text-left font-mono text-list outline-none focus-visible:ring-2 focus-visible:ring-ring ${
                        f === file
                          ? "bg-surface-selected text-ink"
                          : "text-ink-secondary hover:bg-surface-raised"
                      }`}
                    >
                      {f}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
        {content}
      </div>
    </section>
  );
}
