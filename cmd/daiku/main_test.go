package main

import (
	"bytes"
	"strings"
	"testing"
)

func TestRun(t *testing.T) {
	cases := []struct {
		name     string
		args     []string
		code     int
		stdout   string
		stderr   string
		noStdout bool
		noStderr bool
	}{
		{name: "version", args: []string{"version"}, code: 0, stdout: "daiku ", noStderr: true},
		{name: "help", args: []string{"help"}, code: 0, stdout: "usage: daiku", noStderr: true},
		{name: "no args", args: nil, code: 0, stdout: "usage: daiku", noStderr: true},
		{name: "unknown", args: []string{"plan"}, code: 2, stderr: `daiku: unknown command "plan"`, noStdout: true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			var out, errOut bytes.Buffer
			if got := run(tc.args, &out, &errOut); got != tc.code {
				t.Fatalf("exit code = %d, want %d", got, tc.code)
			}
			if !strings.Contains(out.String(), tc.stdout) {
				t.Errorf("stdout = %q, want it to contain %q", out.String(), tc.stdout)
			}
			if !strings.Contains(errOut.String(), tc.stderr) {
				t.Errorf("stderr = %q, want it to contain %q", errOut.String(), tc.stderr)
			}
			if tc.noStdout && out.Len() > 0 {
				t.Errorf("stdout = %q, want empty", out.String())
			}
			if tc.noStderr && errOut.Len() > 0 {
				t.Errorf("stderr = %q, want empty", errOut.String())
			}
		})
	}
}

func TestVersionLine(t *testing.T) {
	var out bytes.Buffer
	run([]string{"version"}, &out, &bytes.Buffer{})
	if got := out.String(); got != "daiku dev\n" {
		t.Errorf("version output = %q, want %q under go test", got, "daiku dev\n")
	}
}
