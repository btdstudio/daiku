package icm

import (
	"bufio"
	"bytes"
	"fmt"
	"io/fs"
	"path"
	"regexp"
	"strings"

	"github.com/btdstudio/daiku"
)

// The docs lint keeps each room learnable: a room's docs use only their own
// loop's words (spec N2, architecture D15). The lists hold terms only one loop
// has: its artifacts, personas, room folders and commands. A term on neither
// list is shared — ticket, requirements, design, review, repo — and allowed.
// Bare phase words (intake, breakdown, publish, tasks, ship, …) are left off:
// they're ordinary English, and both rooms use them as such ("the task
// breakdown"). To add a loop-only term, add it here; TestLintTermsAreUsed keeps
// the lists honest.
var (
	planOnlyTerms = []string{
		"functional spec", "functional-spec", "architecture.md", "ADR",
		"researcher", "explorer", "publisher", "synthesis",
		"00_intake", "01_research", "02_explore", "03_spec", "04_architecture", "05_breakdown", "06_publish",
		"change new", "tickets ready", "mark-published",
	}
	buildOnlyTerms = []string{
		"tasks.json", "test-results", "code-review", "security-review", "coder",
		"01_design", "02_implementation", "03_testing", "04_refactoring",
		"build new", "tickets pull", "daiku pr", "daiku commit",
	}
)

// lintSource is a set of docs to lint: every .md file in fsys, or only file when
// it's set. prefix is how its paths are shown, such as "scaffold/build".
type lintSource struct {
	prefix string
	fsys   fs.FS
	file   string
}

// lintRoom is one room's docs and the terms they mustn't use, which belong to
// the loop named owner.
type lintRoom struct {
	name, owner string
	sources     []lintSource
	foreign     []string
}

// termPattern matches term case-insensitively where the characters on either
// side aren't letters, digits or "_", so "ADR-0001" matches ADR and "adrDir"
// doesn't.
func termPattern(term string) *regexp.Regexp {
	return regexp.MustCompile(`(?i)(?:^|[^\pL\pN_])` + regexp.QuoteMeta(term) + `(?:$|[^\pL\pN_])`)
}

// lintDocs reports each line of a room's docs that uses one of the room's
// foreign terms, as "<path>:<line>: <owner>-only term "<term>" in the <room>
// room". Files are read in fs.WalkDir order, so the output is stable.
func lintDocs(rooms []lintRoom) []string {
	var problems []string
	for _, room := range rooms {
		type pat struct {
			term string
			re   *regexp.Regexp
		}
		var pats []pat
		for _, t := range room.foreign {
			pats = append(pats, pat{t, termPattern(t)})
		}
		check := func(display string, data []byte) {
			sc := bufio.NewScanner(bytes.NewReader(data))
			sc.Buffer(make([]byte, 0, 64*1024), 1<<20)
			for n := 1; sc.Scan(); n++ {
				for _, p := range pats {
					if p.re.MatchString(sc.Text()) {
						problems = append(problems, fmt.Sprintf("%s:%d: %s-only term %q in the %s room", display, n, room.owner, p.term, room.name))
					}
				}
			}
		}
		for _, src := range room.sources {
			if src.file != "" {
				data, err := fs.ReadFile(src.fsys, src.file)
				if err != nil {
					problems = append(problems, fmt.Sprintf("%s: %v", path.Join(src.prefix, src.file), err))
					continue
				}
				check(path.Join(src.prefix, src.file), data)
				continue
			}
			err := fs.WalkDir(src.fsys, ".", func(p string, d fs.DirEntry, err error) error {
				if err != nil {
					return err
				}
				if d.IsDir() || !strings.HasSuffix(p, ".md") {
					return nil
				}
				data, err := fs.ReadFile(src.fsys, p)
				if err != nil {
					return err
				}
				check(path.Join(src.prefix, p), data)
				return nil
			})
			if err != nil {
				problems = append(problems, fmt.Sprintf("%s: %v", src.prefix, err))
			}
		}
	}
	return problems
}

// scaffoldRooms are the embedded rooms' docs: each room's tree and its AGENTS.md
// fragment. The root tree and root.md introduce both loops, so they aren't
// linted.
func scaffoldRooms() []lintRoom {
	return []lintRoom{
		{name: BuildRoom, owner: PlanRoom, foreign: planOnlyTerms, sources: []lintSource{
			{prefix: "scaffold/build", fsys: daiku.BuildFS},
			{prefix: "scaffold/agents", fsys: daiku.AgentsFS, file: "build.md"},
		}},
		{name: PlanRoom, owner: BuildRoom, foreign: buildOnlyTerms, sources: []lintSource{
			{prefix: "scaffold/plan", fsys: daiku.PlanFS},
			{prefix: "scaffold/agents", fsys: daiku.AgentsFS, file: "plan.md"},
		}},
	}
}
