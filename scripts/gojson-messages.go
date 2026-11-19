// Prints Go's json.Unmarshal error for each input below, one line per input:
// the input in hex, a tab, then "ok" or Go's message. The output is checked in as
// crates/daiku-core/tests/gojson-go.txt, the expectations for daiku-core's port of
// Go's JSON syntax errors (gojson.rs). Regenerate with the pinned Go:
//
//	GOTOOLCHAIN=go1.26.5 go run scripts/gojson-messages.go > crates/daiku-core/tests/gojson-go.txt
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"
)

func main() {
	inputs := []string{
		"", " ", "{", "[", "{ not json\n", `{"a":1}`, `{"a":1} `, `{"a":1}x`, `{"a":1} x`,
		`{"a" 1}`, `{"a":1 "b":2}`, `{"a":1,}`, `[1 2]`, `[1,]`, `[,]`, `]`, `}`,
		`{1:2}`, `{"a":}`, `"abc`, "\"a\x01b\"", `"\x"`, `"\u12g4"`, `"\u"`, `"\uZ"`,
		`-`, `-a`, `01`, `1.`, `1.e5`, `1e`, `1e+`, `1ex`, `.5`, `+1`,
		`tru`, `trux`, `t`, `tx`, `fals`, `falsy`, `nul`, `nulx`, `nx`, `True`,
		"\xef\xbb\xbf{}", "\xff", "\x80", "{\"a\":\"\xff\"}", `'a'`, "\x7f",
		`{"gates": "auto"}`, `[]`, `"str"`, `null`, `3`, "[[[[]]]]",
		strings.Repeat("[", 10001), strings.Repeat("[", 10000) + strings.Repeat("]", 10000),
		"\t\r\n{}\t\r\n", `{"a":[1,{"b":null}],"c":true}`, `{"a":1}}`, `[1]]`,
	}
	for _, in := range inputs {
		var v any
		msg := "ok"
		if err := json.Unmarshal([]byte(in), &v); err != nil {
			msg = err.Error()
		}
		fmt.Printf("%s\t%s\n", hex.EncodeToString([]byte(in)), msg)
	}
}
