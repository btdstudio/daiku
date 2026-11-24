/** `daiku init` for a folder, the path in POSIX single quotes (design D12). */
export function initCommand(path: string): string {
  return "daiku init '" + path.replaceAll("'", "'\\''") + "'";
}
