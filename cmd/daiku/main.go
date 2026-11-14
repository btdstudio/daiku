// Command daiku plans and builds changes in one workspace: scaffold a root-only
// workspace (`init`), refresh its managed files (`update`), validate the
// embedded scaffold (`check`), and every workspace command (internal/cli).
package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/btdstudio/daiku/internal/cli"
	"github.com/btdstudio/daiku/internal/core/version"
	"github.com/btdstudio/daiku/internal/icm"
	"github.com/btdstudio/daiku/internal/settings"
)

func main() {
	os.Exit(run(os.Args[1:]))
}

func run(args []string) int {
	if code, ok := cli.Help(args, os.Stdout); ok {
		return code
	}
	if len(args) == 0 {
		cli.PrintUsage(os.Stderr)
		return 1
	}
	switch args[0] {
	case "version", "--version", "-v":
		fmt.Printf("daiku %s\n", version.String())
		return 0
	case "init":
		return runInit(args[1:])
	case "update":
		return runUpdate()
	case "check":
		return runCheck()
	default:
		return cli.Run(args)
	}
}

func runInit(args []string) int {
	dir := "."
	noGit := false
	force := false
	for i := 0; i < len(args); i++ {
		switch a := args[i]; {
		case a == "--no-git":
			noGit = true
		case a == "--force":
			force = true
		case strings.HasPrefix(a, "-"):
			fmt.Fprintf(os.Stderr, "daiku init: unknown flag %s\n", a)
			return 1
		default:
			dir = a
		}
	}
	vcs, err := settings.InitVCS(noGit)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	rep, err := icm.Scaffold(dir, icm.Options{NoGit: noGit, Force: force}, vcs)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	for _, rel := range rep.Written {
		fmt.Println("wrote", rel)
	}
	if rep.GitInit {
		fmt.Println("git init")
	}
	fmt.Printf("scaffolded daiku workspace in %s\n", dir)
	return 0
}

func runUpdate() int {
	rep, err := icm.Update(".")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	for _, rel := range rep.Updated {
		fmt.Println("updated", rel)
	}
	for _, rel := range rep.Merged {
		fmt.Printf("merged %s (daiku's ## sections updated; your sections and the text under the title kept)\n", rel)
	}
	for _, rel := range rep.Skipped {
		if reason, ok := rep.NotMerged[rel]; ok {
			fmt.Printf("skipped (edited) %s: can't merge: %s\n", rel, reason)
			continue
		}
		fmt.Println("skipped (edited)", rel)
	}
	fmt.Printf("update complete (%d updated, %d skipped, %d unchanged, %d merged)\n",
		len(rep.Updated), len(rep.Skipped), len(rep.Unchanged), len(rep.Merged))
	return 0
}

func runCheck() int {
	problems := icm.Check()
	for _, p := range problems {
		fmt.Fprintln(os.Stderr, p)
	}
	if len(problems) > 0 {
		return 1
	}
	fmt.Println("scaffold is clean")
	return 0
}
