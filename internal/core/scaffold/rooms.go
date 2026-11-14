package scaffold

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"

	"github.com/btdstudio/daiku/internal/core/version"
)

// RoomReport describes what EnsureRoom did.
type RoomReport struct {
	// Created is true when the room was added by this call, and false when its
	// folder was already there.
	Created bool
	// Written lists the files written, slash-separated, relative to the
	// workspace root.
	Written []string
}

// EnsureRoom adds a room to an existing workspace the first time it's needed: it
// writes the room's tree under <room>/ and records those files in the
// workspace's manifest, alongside the root's. A room whose folder already exists
// is left alone and reported as not created; refreshing its files is Update's
// job.
func (e Engine) EnsureRoom(dir, room string) (RoomReport, error) {
	rep := RoomReport{}
	fsys, ok := e.Rooms[room]
	if !ok {
		return rep, fmt.Errorf("%s: unknown room %q", e.Tool, room)
	}
	if !validRoom(room) {
		return rep, fmt.Errorf("%s: invalid room name %q", e.Tool, room)
	}
	if err := e.checkVCS(); err != nil {
		return rep, err
	}
	if roomIsSymlink(dir, room) {
		return rep, fmt.Errorf("%s: %s/ is a symlink, which could lead outside the workspace — replace it with a folder", e.Tool, room)
	}
	if roomExists(dir, room) {
		return rep, nil
	}
	m, err := e.ReadManifest(dir)
	if errors.Is(err, fs.ErrNotExist) {
		if info, statErr := os.Stat(filepath.Join(dir, filepath.FromSlash(e.Marker))); statErr != nil || !info.IsDir() {
			return rep, fmt.Errorf("%s: %s is not a %s workspace (run `%s init`)", e.Tool, dir, e.Tool, e.Tool)
		}
		m, err = Manifest{Version: version.String(), Files: map[string]string{}}, nil
	}
	if err != nil {
		return rep, fmt.Errorf("%s: %v", e.Tool, err)
	}
	// The folder marks the room as present, even if its tree has no files.
	if err := os.MkdirAll(filepath.Join(dir, room), 0o755); err != nil {
		return rep, err
	}
	t := tree{fsys: fsys, prefix: room}
	err = e.walkTree(t, func(rel string, data []byte) error {
		if err := e.write(dir, rel, data); err != nil {
			return err
		}
		m.Files[rel] = hashBytes(data)
		rep.Written = append(rep.Written, rel)
		return nil
	})
	if err != nil {
		return rep, err
	}
	if err := e.keptDirs(dir, []tree{t}); err != nil {
		return rep, err
	}
	if err := e.WriteManifest(dir, m); err != nil {
		return rep, err
	}
	sort.Strings(rep.Written)
	rep.Created = true
	return rep, nil
}
