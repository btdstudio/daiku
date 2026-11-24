import { useEffect, useState } from "react";
import type { ArtifactList } from "@/generated/ArtifactList";
import type { Status } from "@/generated/Status";
import { call } from "@/lib/call";
import { isFailure } from "@/lib/envelope";

interface Props {
  root: string;
  onPick: (path: string) => void;
}

/** Dev only (rendered under `import.meta.env.DEV`): lists the active change's
 *  artifacts until the real lists exist (S-0021, S-0022). */
export function DevPicker({ root, onPick }: Props) {
  const [change, setChange] = useState<string | null | undefined>(undefined);
  const [typed, setTyped] = useState("");
  const [list, setList] = useState<{ change: string; result: ArtifactList | string } | null>(
    null,
  );

  useEffect(() => {
    let live = true;
    call<Status>("status", { root }).then((e) => {
      if (live) setChange(isFailure(e) ? null : (e.data.active?.change.name ?? null));
    });
    return () => {
      live = false;
    };
  }, [root]);

  const name = change ?? (typed.trim() || null);
  useEffect(() => {
    if (name === null) return;
    let live = true;
    call<ArtifactList>("artifacts", { root, change: name }).then((e) => {
      if (live) setList({ change: name, result: isFailure(e) ? e.error.message : e.data });
    });
    return () => {
      live = false;
    };
  }, [root, name]);

  const shown = list !== null && list.change === name ? list.result : null;
  return (
    <div data-dev-picker className="flex flex-col gap-2 p-4 text-list text-ink-muted">
      <p className="text-label uppercase tracking-[var(--tracking-label)]">Dev picker</p>
      {change === undefined && <p>Reading status…</p>}
      {change === null && (
        <input
          aria-label="Change name"
          className="rounded-sm border border-line bg-surface-raised px-2 py-1 font-mono text-ink"
          placeholder="change name"
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
        />
      )}
      {typeof shown === "string" && <p>{shown}</p>}
      {shown !== null && typeof shown !== "string" && (
        <ul className="flex flex-col">
          {shown.artifacts.map((a) => (
            <li key={a.path}>
              <button
                type="button"
                onClick={() => onPick(a.path)}
                className="w-full rounded-md px-2 py-1 text-left font-mono text-ink-secondary hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring"
              >
                {a.path}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
