package main

import (
	"encoding/json"
	"errors"
	"testing"
)

func TestRejectsInvalidControlServer(t *testing.T) {
	for _, control := range []string{"file:///private/state", "http://example.com", "https://user:secret@example.com", "https://example.com/#fragment"} {
		if _, err := newEngine(config{Directory: t.TempDir(), Hostname: "herdr-test", ControlURL: control}, nil); !errors.Is(err, errInvalidConfig) {
			t.Errorf("control %q: got %v, want invalid configuration", control, err)
		}
	}
}

func TestSnapshotDoesNotStartNetwork(t *testing.T) {
	engine, err := newEngine(config{Directory: t.TempDir(), Hostname: "herdr-test"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer engine.close()
	data, err := engine.snapshot()
	if err != nil {
		t.Fatal(err)
	}
	var state snapshot
	if err := json.Unmarshal(data, &state); err != nil {
		t.Fatal(err)
	}
	if state.State != "disconnected" || len(state.Peers) != 0 || state.AuthURL != "" {
		t.Fatalf("unexpected initial snapshot: %+v", state)
	}
}

func TestClosedEngineCannotStartOrDial(t *testing.T) {
	engine, err := newEngine(config{Directory: t.TempDir(), Hostname: "herdr-test"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	engine.close()
	engine.close()
	if err := engine.start(); !errors.Is(err, errClosed) {
		t.Fatalf("start after close: %v", err)
	}
	if _, err := engine.dial("peer-id", 22); !errors.Is(err, errClosed) {
		t.Fatalf("dial after close: %v", err)
	}
}

func TestBrowserLoginRejectsAmbientAuthentication(t *testing.T) {
	t.Setenv("TS_AUTHKEY", "private-test-key")
	t.Setenv("TS_CONTROL_URL", "http://unvalidated.invalid")
	e, err := newEngine(config{Directory: t.TempDir(), Hostname: "test"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer e.close()
	if e.server.ControlURL != "https://controlplane.tailscale.com" {
		t.Fatal("default control server was not explicit")
	}
	if err := e.start(); !errors.Is(err, errEnvironment) {
		t.Fatalf("ambient authentication was not refused: %v", err)
	}
}
