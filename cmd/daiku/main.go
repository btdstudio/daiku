// Command daiku plans and builds changes in one workspace.
package main

import (
	"fmt"
	"io"
	"os"

	"github.com/btdstudio/daiku/internal/core/version"
)

const usage = `usage: daiku <command>

commands:
  version   print daiku's version
  help      show this help
`

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

// run executes one command and returns the process exit code.
func run(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprint(stdout, usage)
		return 0
	}
	switch args[0] {
	case "version", "--version", "-v":
		fmt.Fprintf(stdout, "daiku %s\n", version.String())
		return 0
	case "help", "--help", "-h":
		fmt.Fprint(stdout, usage)
		return 0
	}
	fmt.Fprintf(stderr, "daiku: unknown command %q\n\n%s", args[0], usage)
	return 2
}
