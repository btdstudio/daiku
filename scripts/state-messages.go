// Prints how Go's daiku decodes each STATE.json below, one line per input: the
// input in hex, a tab, then Go's json.Unmarshal error, or the decoded state
// re-encoded as JSON. Its types mirror internal/core/state (stateFile, Change).
// Go's package for these types is state, so "main." in a type name is printed
// as "state.". The output is checked in as crates/daiku-core/tests/state-go.txt, the
// expectations for daiku-core's STATE.json decoder (state/file.rs). Regenerate
// with the pinned Go:
//
//	GOTOOLCHAIN=go1.26.5 go run scripts/state-messages.go > crates/daiku-core/tests/state-go.txt
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"
)

type Change struct {
	Name      string            `json:"-"`
	Loop      string            `json:"loop"`
	Phase     string            `json:"phase"`
	Status    string            `json:"status"`
	Gates     map[string]string `json:"gates"`
	Reasons   map[string]string `json:"reasons,omitempty"`
	Sources   []string          `json:"sources"`
	Repos     []string          `json:"repos,omitempty"`
	UpdatedAt string            `json:"updatedAt"`
}

type stateFile struct {
	Schema  int                `json:"schema"`
	Changes map[string]*Change `json:"changes"`
}

func main() {
	inputs := []string{
		`{}`, `null`, `[]`, `"x"`, `1`, `true`,
		`{"schema": 1}`, `{"schema": "1"}`, `{"schema": 1.5}`, `{"schema": 1e3}`, `{"schema": -1}`,
		`{"schema": 1e400}`, `{"schema": 99999999999999999999}`, `{"schema": null}`, `{"schema": true}`,
		`{"schema": [1]}`, `{"schema": {}}`, `{"SCHEMA": 1}`, `{"Schema": 2}`, `{"schema": 1, "schema": 3}`,
		`{"changes": null}`, `{"changes": []}`, `{"changes": "x"}`, `{"changes": 1}`, `{"changes": {}}`,
		`{"changes": {"a": null}}`, `{"changes": {"a": 1}}`, `{"changes": {"a": "x"}}`, `{"changes": {"a": []}}`,
		`{"changes": {"a": {}}}`, `{"changes": {"a": {"loop": 1}}}`, `{"changes": {"a": {"loop": null}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": []}}}`, `{"changes": {"a": {"loop": "plan", "gates": {"x": 1}}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": null}}}}`, `{"changes": {"a": {"loop": "plan", "gates": "x"}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": "x"}}}`, `{"changes": {"a": {"loop": "plan", "sources": [1]}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": [null, "s"]}}}`, `{"changes": {"a": {"loop": "plan", "repos": {}}}}`,
		`{"changes": {"a": {"loop": "plan", "reasons": {"x": 2}}}}`, `{"changes": {"a": {"loop": "plan", "updatedAt": 5}}}`,
		`{"changes": {"a": {"LOOP": "plan", "Phase": "intake", "GATES": {"intake": "draft"}, "UpdatedAT": "t"}}}`,
		`{"changes": {"a": {"loop": "plan", "extra": [1, {"b": 2}], "name": "z"}}}`,
		`{"changes": {"a": {"loop": "plan", "phase": 1, "status": 2}}}`,
		`{"changes": {"a": {"loop": "plan"}, "b": {"loop": 3}}}`,
		`{"schema": "x", "changes": {"a": {"loop": 3}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"a": 1, "b": "x", "c": 2}}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": [1, "x", 2]}}}`,
		`{"changes": {"a": {"loop": "plan", "loop": "build"}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": "a", "x": "b"}}}}`,
		`{"changes": {"a": {"loop": "plan"}, "a": {"loop": "build"}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": "a"}, "gates": {"y": "b"}}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": ["a"], "sources": null}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": "a"}, "gates": null}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": [1.5]}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": {}}}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": [1]}}}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": true}}}}`,
		`{"changes": {"a": {"loop": "plan", "loop": 1}}}`,
		`{"changes": {"a": {"loop": "plan", "ſtatus": "x", "K": 1}}}`,
		`{"changes": {"a": {"loop": "é😀\u0000"}}}`,
		`{"changes": {"é": {"loop": "plan"}}}`,
		`{"changes": {"a": {"loop": "plan", "phase": "x"}}, "extra": 1e400}`,
		`{"changes": {"a": {"loop": "plan", "sources": ["a", "b"], "sources": [null]}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": ["a", "b"], "sources": ["x"], "sources": [null, null]}}}`,
		`{"changes": {"a": {"loop": "plan", "sources": ["a"], "sources": []}}}`,
		`{"changes": {"a": {"loop": "plan", "loop": null}}}`,
		`{"changes": {"a": {"loop": "plan"}}, "changes": {"b": {"loop": "build"}}}`,
		`{"changes": {"a": {"loop": "plan"}, "a": null}}`,
		`{"changes": {"a": {"loop": "plan", "gates": {"x": "a"}}}, "changes": {"a": {"phase": "p"}}}`,
		`{"schema": 1, "schema": null}`,
		"{\"changes\": {\"a\": {\"loop\": \"p\xfflan\\ud800\\udc00\\udc00\\ud800x\xe2\x82a\"}}}",
		`{"changes": {"a": {"loop": "plan", "phase": "\u00"}}}`,
		`{"changes": {"a": {"loop": "plan", "status": "\b\f\n\r\t\/\\\""}}}`,
		`{"schema": 9223372036854775807}`, `{"schema": -9223372036854775808}`, `{"schema": -0}`, `{"schema": 0.0}`,
	}
	for _, in := range inputs {
		var sf stateFile
		out := ""
		if err := json.Unmarshal([]byte(in), &sf); err != nil {
			out = "error: " + strings.ReplaceAll(err.Error(), "main.", "state.")
		} else {
			b, _ := json.Marshal(sf)
			out = string(b)
		}
		fmt.Printf("%s\t%s\n", hex.EncodeToString([]byte(in)), out)
	}
}
