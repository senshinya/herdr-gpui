package main

import (
	"errors"
	"io"
	"net"
	"testing"
)

func TestCloseReleasesOpenStreamsAndIsIdempotent(t *testing.T) {
	e, err := newEngine(config{Directory: t.TempDir(), Hostname: "test"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	local, remote := net.Pipe()
	defer remote.Close()
	e.streams[1] = local
	completed := make(chan error, 1)
	go func() { _, err := local.Read(make([]byte, 1)); completed <- err }()
	e.close()
	e.close()
	if !errors.Is(<-completed, io.ErrClosedPipe) {
		t.Fatal("closing the engine did not cancel its stream")
	}
	if _, err := e.stream(1); !errors.Is(err, errClosed) {
		t.Fatalf("old stream remained accessible: %v", err)
	}
}
