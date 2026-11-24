import { useState } from "react";
import { Button } from "@/components/ui/button";
import { initCommand } from "./initCommand";
import type { WorkspaceRef } from "./viewState";

/** The not-a-workspace message with a copyable `daiku init` (R3, D12). The path is
 *  the ref's `shown`, never the raw root. */
export function NotWorkspace({ folder }: { folder: WorkspaceRef }) {
  const [failed, setFailed] = useState(false);
  const command = initCommand(folder.shown);
  async function copy() {
    try {
      await navigator.clipboard.writeText(command);
      setFailed(false);
    } catch {
      setFailed(true);
    }
  }
  return (
    <section className="m-6 flex max-w-xl flex-col gap-3 rounded-md border border-line bg-surface-raised p-4">
      <h2 className="text-body font-semibold">This folder isn't a daiku workspace</h2>
      <p className="break-all font-mono text-label text-ink-muted">{folder.shown}</p>
      <p className="text-ink-secondary">To make it one, run:</p>
      <pre className="overflow-x-auto rounded-sm bg-surface-window p-3 font-mono text-list text-ink select-all">
        {command}
      </pre>
      <div>
        <Button variant="secondary" size="sm" onClick={() => void copy()}>
          {failed ? "Select and copy" : "Copy"}
        </Button>
      </div>
    </section>
  );
}
