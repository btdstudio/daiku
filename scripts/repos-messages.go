// Prints how Go's daiku decodes each REPOS.json and .daiku/paths.json below, one
// line per input: "repos" or "paths", a tab, the input in hex, a tab, then Go's
// json.Unmarshal error or the decoded value re-encoded as JSON. Its types mirror
// internal/core/repos (Load). Go's package for these types is repos, so "main."
// in a type name is printed as "repos.". The output is checked in as
// crates/daiku-core/tests/repos-go.txt, the expectations for daiku-core's
// decoder (repos.rs). Regenerate with the pinned Go:
//
//	GOTOOLCHAIN=go1.26.5 go run scripts/repos-messages.go > crates/daiku-core/tests/repos-go.txt
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"
)

type Repo struct {
	Remote        string `json:"remote"`
	DefaultBranch string `json:"defaultBranch"`
}

func main() {
	repos := []string{
		`{}`, `null`, `[]`, `"x"`, `1`, `{"repos": null}`, `{"repos": []}`, `{"repos": 1}`,
		`{"repos": {}}`, `{"repos": {"api": null}}`, `{"repos": {"api": 1}}`, `{"repos": {"api": []}}`,
		`{"repos": {"api": {"remote": 1}}}`, `{"repos": {"api": {"defaultBranch": true}}}`,
		`{"repos": {"api": {"remote": "git@h:o/api.git", "defaultBranch": "main", "x": [1]}}}`,
		`{"REPOS": {"api": {"REMOTE": "r", "DefaultBranch": "b"}}, "other": 1e400}`,
		`{"repos": {"a": {"remote": "r1"}}, "repos": {"b": {"remote": "r2"}}}`,
		`{"repos": {"a": {"remote": "r1", "remote": null}}}`,
		`{"repos": {"a": {"remote": "r1"}, "a": {"defaultBranch": "d"}}}`,
	}
	paths := []string{
		`{}`, `null`, `[]`, `"x"`, `1`, `{"api": "/src/api"}`, `{"api": 1}`, `{"api": null}`,
		`{"api": {}}`, `{"api": "a", "api": "b"}`, `{"api": "~/src/api", "web": "../web"}`,
	}
	run := func(kind string, inputs []string, into func() any) {
		for _, in := range inputs {
			v := into()
			out := ""
			if err := json.Unmarshal([]byte(in), v); err != nil {
				out = "error: " + strings.ReplaceAll(err.Error(), "main.", "repos.")
			} else {
				b, _ := json.Marshal(v)
				out = string(b)
			}
			fmt.Printf("%s\t%s\t%s\n", kind, hex.EncodeToString([]byte(in)), out)
		}
	}
	run("repos", repos, func() any {
		return &struct {
			Repos map[string]Repo `json:"repos"`
		}{}
	})
	run("paths", paths, func() any {
		m := map[string]string{}
		return &m
	})
}
