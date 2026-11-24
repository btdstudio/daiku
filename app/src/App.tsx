import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import type { Status } from "@/generated/Success_Status";
import { call, internalFailure } from "@/lib/call";
import { type Envelope, isFailure } from "@/lib/envelope";

export default function App() {
  const [root, setRoot] = useState<string | null>(null);
  const [measure, setMeasure] = useState(false);
  const [result, setResult] = useState<Envelope<Status> | null>(null);

  // A measure build names its workspace; any other build returns null (design D7).
  useEffect(() => {
    invoke<string | null>("measure_root")
      .then((path) => {
        if (path) {
          setMeasure(true);
          setRoot(path);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!root) return;
    let live = true;
    call<Status>("status", { root }).then((e) => {
      if (live) setResult(e);
    });
    return () => {
      live = false;
    };
  }, [root]);

  // In measure mode, report once the first data is on screen.
  useEffect(() => {
    if (!measure || !result) return;
    requestAnimationFrame(() => {
      invoke("measure_ready").catch(() => {});
    });
  }, [measure, result]);

  async function pick() {
    try {
      const chosen = await open({ directory: true });
      if (typeof chosen === "string") {
        setResult(null);
        setRoot(chosen);
      }
    } catch (err) {
      setResult(internalFailure(err));
    }
  }

  return (
    <main className="flex min-h-screen flex-col gap-4 p-6">
      <header className="flex items-center gap-3">
        <h1 className="text-[20px] font-semibold">daiku</h1>
        <Button onClick={pick}>Open workspace…</Button>
      </header>
      {root && !result && <p className="text-ink-muted">Reading…</p>}
      {result && <Result envelope={result} />}
    </main>
  );
}

function Result({ envelope }: { envelope: Envelope<Status> }) {
  if (isFailure(envelope)) {
    return (
      <section className="rounded-md border border-line bg-surface-raised p-4">
        <p className="font-mono text-attention-ink">{envelope.error.kind}</p>
        <p className="whitespace-pre-wrap text-ink-secondary">{envelope.error.message}</p>
      </section>
    );
  }
  const { active, plans, builds } = envelope.data;
  const buildCount = builds.reduce((n, r) => n + r.builds.length, 0);
  return (
    <section className="flex flex-col gap-2 rounded-md border border-line bg-surface-raised p-4">
      {active ? (
        <p>
          <span className="font-mono">{active.change.name}</span>{" "}
          <span className="text-ink-muted">
            {active.change.phase} · {active.change.status}
          </span>
        </p>
      ) : (
        <p className="text-ink-faint">No active change</p>
      )}
      <p className="text-ink-muted">
        {plans.length} open plans · {buildCount} builds
      </p>
    </section>
  );
}

