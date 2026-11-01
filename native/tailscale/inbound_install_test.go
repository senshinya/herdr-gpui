package main

import "testing"

func TestEngineInstallsInboundHandler(t *testing.T) {
	e, err := newEngine(config{Directory: t.TempDir(), Hostname: "test"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer e.close()
	if e.server.NetstackHandler == nil {
		t.Fatal("the embedded node has no inbound handler")
	}
}
