// Package inbox owns the build room's ticket inbox: one <ID>.md per unplanned
// ticket under build/tickets/, in the shared schema. A native ticket (N-) is
// created here; a pulled one (X-) is recorded from a PM tool's item. Planned
// tickets aren't copied in: a build reads them in place under plan/specs/.
package inbox

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/btdstudio/daiku/internal/build/layout"
	"github.com/btdstudio/daiku/internal/core/frontmatter"
	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/core/workspace"
)

// Dir is the inbox folder under root.
func Dir(root string) string { return filepath.Join(root, filepath.FromSlash(layout.TicketsDir)) }

// Path is where ticket id lives in the inbox.
func Path(root, id string) string { return filepath.Join(Dir(root), id+".md") }

// List returns every ticket in the inbox, sorted by id, and the problems found:
// a file that can't be parsed, and a planned id, which belongs under plan/specs/.
func List(root string) ([]*ticket.Ticket, []ticket.Problem, error) {
	tickets, problems, err := ticket.LoadDir(Dir(root))
	if err != nil {
		return nil, nil, err
	}
	sort.Slice(tickets, func(i, j int) bool { return tickets[i].ID < tickets[j].ID })
	for _, t := range tickets {
		if planned(t.ID) {
			problems = append(problems, ticket.Problem{ID: t.ID, Message: "planned tickets live under plan/specs/, not in " + layout.TicketsDir + "/"})
		}
	}
	return tickets, problems, nil
}

// planned reports whether id is a planned ticket's (E-, F-, S- or T-).
func planned(id string) bool {
	prefix, _, ok := strings.Cut(id, "-")
	return ok && prefix != ticket.NativePrefix && prefix != ticket.PulledPrefix
}

// NormalizeID accepts a full id, or the number of a native ticket: "1" and "N-0001"
// are the same ticket.
func NormalizeID(id string) string {
	if n, err := strconv.Atoi(id); err == nil {
		return fmt.Sprintf("%s-%04d", ticket.NativePrefix, n)
	}
	return id
}

// Read reads one ticket from the inbox by id.
func Read(root, id string) (*ticket.Ticket, error) {
	id = NormalizeID(id)
	path := Path(root, id)
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("daiku: ticket %s not found at %s", id, filepath.ToSlash(filepath.Join(layout.TicketsDir, id+".md")))
	}
	t, err := ticket.Parse(data)
	if err != nil {
		return nil, fmt.Errorf("daiku: %s: %v", path, err)
	}
	t.Path = path
	return t, nil
}

// NewOptions is the payload for New.
type NewOptions struct {
	Title  string
	Repo   string
	Labels []string
	Body   string
}

// New creates a native ticket with the next N- id. It starts as a draft. A native
// ticket is always a task (the schema's rule), and needs a repo to be built in.
func New(root string, opts NewOptions) (*ticket.Ticket, error) {
	if opts.Repo == "" {
		return nil, fmt.Errorf("daiku: an inbox ticket needs a repo (--repo <name>)")
	}
	all, problems, err := ticket.LoadDir(Dir(root))
	if err != nil {
		return nil, err
	}
	if len(problems) > 0 {
		return nil, fmt.Errorf("daiku: can't allocate an id while a ticket is unreadable: %s", problems[0])
	}
	id := ticket.NextNativeID(all)
	body := "\n" + strings.TrimRight(opts.Body, "\n") + "\n"
	t, err := ticket.New(ticket.Ticket{ID: id, Type: ticket.Task, Title: oneLine(opts.Title), Repo: opts.Repo, Labels: opts.Labels, Body: body})
	if err != nil {
		return nil, err
	}
	t.Path = Path(root, id)
	if err := workspace.WriteFile(root, t.Path, t.Bytes(), 0o644); err != nil {
		return nil, err
	}
	return t, nil
}

var lineBreakRe = regexp.MustCompile(`\s*\r?\n+\s*`)

func oneLine(s string) string { return strings.TrimSpace(lineBreakRe.ReplaceAllString(s, " ")) }

// PMID is the PM tool's item id inside an external value: the text after its last
// "#", or the whole value when there's none. So "org/app#1234" and "#1234" are
// 1234, and "PROJ-123" stays PROJ-123. Two repos' "#7" are the same id.
func PMID(value string) string {
	if i := strings.LastIndex(value, "#"); i >= 0 {
		return value[i+1:]
	}
	return value
}

// Pull records a draft ticket fetched from a PM tool as X-<pm-id> in the inbox.
// The draft needs exactly one external entry, whose PM id names the ticket. The
// id is set, origin and change are dropped (a pulled ticket has neither), and
// every other field is kept as the agent wrote it. An existing X- ticket with
// the same id is replaced, so there's never a second one.
func Pull(root string, draft []byte) (*ticket.Ticket, error) {
	parsed, err := ticket.Parse(draft)
	if err != nil {
		return nil, fmt.Errorf("daiku: draft: %v", err)
	}
	if len(parsed.External) != 1 {
		return nil, fmt.Errorf("daiku: a pulled ticket needs exactly one external id, the draft has %d", len(parsed.External))
	}
	var value string
	for _, v := range parsed.External {
		value = v
	}
	id, err := ticket.PulledID(PMID(value))
	if err != nil {
		return nil, fmt.Errorf("daiku: %v", err)
	}
	doc, err := frontmatter.Parse(draft)
	if err != nil {
		return nil, fmt.Errorf("daiku: draft: %v", err)
	}
	if err := doc.Set("id", id); err != nil {
		return nil, err
	}
	doc.Delete("origin")
	doc.Delete("change")
	data := doc.Bytes()
	t, err := ticket.Parse(data)
	if err != nil {
		return nil, fmt.Errorf("daiku: %s: %v", id, err)
	}
	t.Path = Path(root, id)
	if problems := ticket.ValidateFile(t); len(problems) > 0 {
		return nil, fmt.Errorf("daiku: invalid ticket: %s", problems[0])
	}
	if err := workspace.WriteFile(root, t.Path, data, 0o644); err != nil {
		return nil, err
	}
	return t, nil
}
