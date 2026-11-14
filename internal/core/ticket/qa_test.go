package ticket

import "testing"

// QA (AC5): a PM id normalized by PulledID gives a ticket that validates, for
// each id form the criterion names.
func TestQAPulledIDsValidateAsTickets(t *testing.T) {
	for in, want := range map[string]string{"X-1234": "X-1234", "X-PROJ-123": "X-PROJ-123", "X-#12": "X-12"} {
		id, err := PulledID(in)
		if err != nil || id != want {
			t.Fatalf("PulledID(%q) = %q, %v; want %q", in, id, err, want)
		}
		tk := pulled(t, id, Task, "external:\n  pm: \""+in+"\"\n")
		if problems := ValidateFile(tk); len(problems) != 0 {
			t.Errorf("%s: problems:\n%s", id, messages(problems))
		}
	}
}
