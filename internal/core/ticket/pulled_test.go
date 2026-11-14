package ticket

import (
	"strings"
	"testing"
)

func TestPulledID(t *testing.T) {
	cases := map[string]string{
		"X-1234":     "X-1234",
		"X-PROJ-123": "X-PROJ-123",
		"X-#12":      "X-12",
		"#1234":      "X-1234",
		"1234":       "X-1234",
		"PROJ-123":   "X-PROJ-123",
		" #77 ":      "X-77",
		"a--b":       "X-a-b",
		"-a-":        "X-a",
		"../x":       "X-x",
		"x-12":       "X-12",
		"x-PROJ-1":   "X-PROJ-1",
	}
	for in, want := range cases {
		got, err := PulledID(in)
		if err != nil || got != want {
			t.Errorf("PulledID(%q) = %q, %v; want %q", in, got, err, want)
			continue
		}
		if !IDRe.MatchString(got) {
			t.Errorf("PulledID(%q) = %q doesn't match IDRe", in, got)
		}
	}
	for _, in := range []string{"", "#", "X-", "X-#", "--", "/"} {
		if got, err := PulledID(in); err == nil {
			t.Errorf("PulledID(%q) = %q, want an error", in, got)
		}
	}
}

func TestIDRe(t *testing.T) {
	for _, id := range []string{"S-0036", "N-0001", "E-0001", "X-1234", "X-PROJ-123", "X-a"} {
		if !IDRe.MatchString(id) {
			t.Errorf("IDRe rejects %q", id)
		}
	}
	for _, id := range []string{"", "7-gateway", "../x", "X-", "X-#12", "X--1", "X-1-", "X-a/b", "S-1", "Y-0001", "x-1234"} {
		if IDRe.MatchString(id) {
			t.Errorf("IDRe accepts %q", id)
		}
	}
}

// pulled parses a pulled ticket with the given extra frontmatter lines.
func pulled(t *testing.T, id, typ, extra string) *Ticket {
	t.Helper()
	src := "---\nschema: 1\nid: " + id + "\ntype: " + typ + "\ntitle: x\nrepo: api\nstatus: draft\n" + extra + "---\n"
	tk, err := Parse([]byte(src))
	if err != nil {
		t.Fatal(err)
	}
	return tk
}

func TestValidateFilePulled(t *testing.T) {
	cases := []struct {
		name string
		tk   func(t *testing.T) *Ticket
		want string // "" = valid
	}{
		{"X-1234 story", func(t *testing.T) *Ticket {
			return pulled(t, "X-1234", Story, "external:\n  github: btdstudio/api#1234\n")
		}, ""},
		{"X-PROJ-123 any type", func(t *testing.T) *Ticket { return pulled(t, "X-PROJ-123", Feature, "external:\n  jira: PROJ-123\n") }, ""},
		{"no external", func(t *testing.T) *Ticket { return pulled(t, "X-1234", Story, "") }, "exactly one external id, got 0"},
		{"two externals", func(t *testing.T) *Ticket {
			return pulled(t, "X-1234", Story, "external:\n  jira: PROJ-1\n  github: o/r#1\n")
		}, "exactly one external id, got 2"},
		{"origin", func(t *testing.T) *Ticket {
			return pulled(t, "X-1234", Story, "external:\n  jira: PROJ-1\norigin:\n  path: x\n")
		}, "can't have an origin"},
		{"unnormalized id", func(t *testing.T) *Ticket { return pulled(t, "X-#12", Story, "external:\n  github: o/r#12\n") }, "must look like"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			problems := ValidateFile(c.tk(t))
			if c.want == "" {
				if len(problems) != 0 {
					t.Fatalf("problems:\n%s", messages(problems))
				}
				return
			}
			if !strings.Contains(messages(problems), c.want) {
				t.Fatalf("want %q in:\n%s", c.want, messages(problems))
			}
		})
	}
}

func TestPulledTicketCantBeClosed(t *testing.T) {
	tk := pulled(t, "X-1234", Story, "external:\n  jira: PROJ-1\n")
	src := strings.Replace(string(tk.Bytes()), "status: draft", "status: closed", 1)
	closed, err := Parse([]byte(src))
	if err != nil {
		t.Fatal(err)
	}
	if got := messages(ValidateFile(closed)); !strings.Contains(got, "only a native") {
		t.Fatalf("want a refusal to close, got:\n%s", got)
	}
}

// AC5: an N- ticket that isn't a task is still rejected now that X- exists.
func TestNativeStoryStillRejected(t *testing.T) {
	if got := messages(ValidateFile(mk(t, Ticket{ID: "N-0001", Type: Story}))); !strings.Contains(got, "must be a task") {
		t.Fatalf("got:\n%s", got)
	}
}

func TestSequenceIgnoresPulledIDs(t *testing.T) {
	tickets := []*Ticket{
		mk(t, Ticket{ID: "S-0003"}),
		pulled(t, "X-9999", Story, "external:\n  jira: PROJ-9999\n"),
	}
	if got := NextNativeID(tickets); got != "N-0004" {
		t.Errorf("NextNativeID = %q, want N-0004", got)
	}
}
