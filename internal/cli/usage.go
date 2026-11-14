package cli

import (
	"fmt"
	"io"
	"os"
	"slices"
	"strings"

	"github.com/btdstudio/daiku/internal/core/repos"
)

// command is one daiku command's usage. Its synopsis is the one source of the
// command's flags: --help, `daiku help`, the usage errors and the scaffold's
// AGENTS.md command table all come from it.
type command struct {
	path     []string // {"adr", "new"}; a shorter shared prefix is a group
	synopsis string   // every flag the command's parser accepts
	summary  string
	notes    string // extra --help lines, such as what a flag means
	raw      string // replaces synopsis and summary in listings (repo, from kiso)
}

var commands = []command{
	{path: []string{"init"}, synopsis: "daiku init [dir] [--no-git] [--force]", summary: "scaffold the planning ICM structure",
		notes: "  --no-git   a local workspace (vcs: none)\n  --force    overwrite files that already exist\n"},
	{path: []string{"update"}, synopsis: "daiku update", summary: "refresh managed files"},
	{path: []string{"status"}, synopsis: "daiku status [<plan-change>]", summary: "show the active change and the board, or a plan's tickets"},
	{path: []string{"settings"}, synopsis: "daiku settings", summary: "show effective settings"},
	{path: []string{"switch"}, synopsis: "daiku switch [--all] [<change>]", summary: "list changes, or switch the active one",
		notes: "  --all      include closed changes in the list\n"},
	{path: []string{"source", "new"}, synopsis: "daiku source new --title <t> [--repo <path>] [--rev <r>] [--body <b>]", summary: "add an intake source to plan/00_intake/"},
	{path: []string{"source", "list"}, synopsis: "daiku source list", summary: "list intake sources (the default verb)"},
	{path: []string{"source", "show"}, synopsis: "daiku source show <id>", summary: "show an intake source"},
	{path: []string{"adr", "new"}, synopsis: "daiku adr new --title <t> [--deciders <d>] [--context <c>] [--decision <d>] [--consequences <c>] [--file <path>]", summary: "create an ADR in plan/04_architecture/adr/",
		notes: "  --file     read the ADR body from a file\n"},
	{path: []string{"adr", "list"}, synopsis: "daiku adr list", summary: "list ADRs (the default verb)"},
	{path: []string{"adr", "show"}, synopsis: "daiku adr show <number>", summary: "show an ADR"},
	{path: []string{"build", "new"}, synopsis: "daiku build new <ticket-id>", summary: "build a ticket in its repo; creates build/ the first time"},
	{path: []string{"change", "new"}, synopsis: "daiku change new <title> [--repo <name|path>]... [--rev <r>] [--body <b>]", summary: "create a change + intake + switch"},
	{path: []string{"change", "close"}, synopsis: "daiku change close <change> --reason <r>", summary: "close a change"},
	{path: []string{"gate"}, synopsis: "daiku gate <phase> <status> [--reason <r>] [--force]", summary: "advance the active change's gate"},
	{path: []string{"reopen"}, synopsis: "daiku reopen <phase> --reason <r> [--force]", summary: "move back to a phase, clearing later approvals"},
	{path: []string{"tickets", "new"}, synopsis: "daiku tickets new <type> <title> [--parent <id>] [--repo <name>] [--depends <id,id>] [--requirements <id,id>] [--inbox]", summary: "create a ticket in the active plan change, or with --inbox (or no open plan change) an N- task in build/tickets/"},
	{path: []string{"tickets", "pull"}, synopsis: "daiku tickets pull <file>", summary: "record a ticket fetched from a PM tool as X-<pm-id> in build/tickets/"},
	{path: []string{"tickets", "check"}, synopsis: "daiku tickets check", summary: "validate every ticket"},
	{path: []string{"tickets", "status"}, synopsis: "daiku tickets status", summary: "list tickets by draft, ready, published and drifted"},
	{path: []string{"tickets", "order"}, synopsis: "daiku tickets order", summary: "list tickets in publish order"},
	{path: []string{"tickets", "ready"}, synopsis: "daiku tickets ready <id>... | --all [--force]", summary: "ready draft tickets once breakdown is approved"},
	{path: []string{"tickets", "mark-published"}, synopsis: "daiku tickets mark-published <id> --external <tool>=<item-id> [--force]", summary: "record a PM tool's id and freeze the ticket",
		notes: "  --force    allow it outside the publish phase\n"},
	{path: []string{"tickets", "unfreeze"}, synopsis: "daiku tickets unfreeze <id> --reason <r>", summary: "clear a ticket's freeze"},
	{path: []string{"overview"}, synopsis: "daiku overview [<change>] [--check] [--stdout]", summary: "write or check the change's README overview"},
	{path: []string{"idea", "new"}, synopsis: "daiku idea new <title>", summary: "create an idea in ideas/, from the summary template"},
	{path: []string{"idea", "list"}, synopsis: "daiku idea list", summary: "list ideas with their status"},
	{path: []string{"idea", "show"}, synopsis: "daiku idea show <id>", summary: "print an idea's summary and status"},
	{path: []string{"idea", "park"}, synopsis: "daiku idea park <id> --reason <r>", summary: "set an open idea aside, with a reason"},
	{path: []string{"idea", "drop"}, synopsis: "daiku idea drop <id> --reason <r>", summary: "drop an open idea, with a reason (the folder stays)"},
	{path: []string{"idea", "reopen"}, synopsis: "daiku idea reopen <id> --reason <r>", summary: "reopen a parked or dropped idea, with a reason"},
	{path: []string{"idea", "promote"}, synopsis: "daiku idea promote <id> --title <t> [--repo <name|path>]... [--rev <r>]", summary: "start a change from an idea"},
	{path: []string{"references", "check"}, synopsis: "daiku references check", summary: "warn about unindexed references and bad index entries"},
	{path: []string{"repo"}, raw: repos.Usage("daiku")},
	{path: []string{"env"}, synopsis: "daiku env", summary: "print harness flags for the active change's repos"},
	{path: []string{"commit"}, synopsis: "daiku commit [git commit args]", summary: "commit what's staged in the active build's repo"},
	{path: []string{"pr"}, synopsis: "daiku pr [title]", summary: "open a pull request from the active build's repo, linking its ticket"},
	{path: []string{"check"}, synopsis: "daiku check", summary: "validate the embedded scaffold"},
	{path: []string{"version"}, synopsis: "daiku version", summary: "print the version"},
	{path: []string{"help"}, synopsis: "daiku help [<command> [<verb>]]", summary: "this help, or one command's"},
}

// summaryColumn is where listings start a command's summary.
const summaryColumn = 43

// listing is c's lines in the two-column `daiku help` layout.
func (c command) listing() string {
	if c.raw != "" {
		return c.raw
	}
	line := "  " + c.synopsis
	if len(line) < summaryColumn {
		return line + strings.Repeat(" ", summaryColumn-len(line)) + c.summary + "\n"
	}
	return line + "\n" + strings.Repeat(" ", summaryColumn) + c.summary + "\n"
}

// synopses is c's synopsis lines; repo has one per kiso usage line.
func (c command) synopses() []string {
	if c.raw == "" {
		return []string{c.synopsis}
	}
	var out []string
	for _, line := range strings.Split(c.raw, "\n") {
		line = strings.TrimSpace(line)
		if !strings.HasPrefix(line, "daiku ") {
			continue
		}
		if i := strings.Index(line, "  "); i >= 0 {
			line = line[:i]
		}
		out = append(out, line)
	}
	return out
}

// Synopses lists every command's synopsis in `daiku help` order.
func Synopses() []string {
	var out []string
	for _, c := range commands {
		out = append(out, c.synopses()...)
	}
	return out
}

// PrintUsage prints the daiku usage summary.
func PrintUsage(w io.Writer) {
	fmt.Fprint(w, "daiku — folder-driven planning workspace\n\nUsage:\n")
	for _, c := range commands {
		fmt.Fprint(w, c.listing())
	}
}

// group returns the commands under prefix, in table order.
func group(prefix []string) []command {
	var out []command
	for _, c := range commands {
		if len(c.path) >= len(prefix) && slices.Equal(c.path[:len(prefix)], prefix) {
			out = append(out, c)
		}
	}
	return out
}

// usageText is the usage for a command or group path: "usage: <synopsis>" for
// one command, or "usage:" and each command's listing for a group.
func usageText(path ...string) string {
	cmds := group(path)
	if len(cmds) == 1 && slices.Equal(cmds[0].path, path) && cmds[0].raw == "" {
		return "usage: " + cmds[0].synopsis + "\n"
	}
	var b strings.Builder
	b.WriteString("usage:\n")
	for _, c := range cmds {
		b.WriteString(c.listing())
	}
	return b.String()
}

// usageLine is usageText without its trailing newline, for errors.
func usageLine(path ...string) string {
	return strings.TrimSuffix(usageText(path...), "\n")
}

// helpText is what --help prints for path: its usage, and for one command its
// summary and notes too.
func helpText(path []string) string {
	text := usageText(path...)
	cmds := group(path)
	if len(cmds) == 1 && cmds[0].raw == "" && slices.Equal(cmds[0].path, path) {
		text += "\n" + cmds[0].summary + "\n" + cmds[0].notes
	}
	return text
}

// Help handles a help request: --help or -h anywhere before a "--", or the
// help command. It prints the usage of the command named by the leading words
// to stdout and returns 0, and ok is false when args aren't a help request or
// name no known command. It reads and writes nothing else, so a command asked
// for help never runs.
func Help(args []string, stdout io.Writer) (code int, ok bool) {
	help := false
	var words []string
	for i, a := range args {
		if a == "--" {
			break
		}
		if a == "--help" || a == "-h" || (i == 0 && a == "help") {
			help = true
			continue
		}
		words = append(words, a)
	}
	if !help {
		return 0, false
	}
	var path []string
	for _, w := range words {
		if strings.HasPrefix(w, "-") || len(group(append(slices.Clone(path), w))) == 0 {
			break
		}
		path = append(path, w)
	}
	if len(path) == 0 || slices.Equal(path, []string{"help"}) {
		if len(path) == 0 && len(words) > 0 && !strings.HasPrefix(words[0], "-") {
			if args[0] == words[0] {
				return 0, false // `daiku bogus --help`: dispatch reports it
			}
			fmt.Fprintf(os.Stderr, "daiku: unknown command %q\n", words[0])
			PrintUsage(os.Stderr)
			return 1, true
		}
		PrintUsage(stdout)
		return 0, true
	}
	fmt.Fprint(stdout, helpText(path))
	return 0, true
}
