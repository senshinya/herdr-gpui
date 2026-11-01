package main

import (
	"encoding/json"
	"testing"
)

func TestRegistryRejectsRemovedHandle(t *testing.T) {
	data, err := json.Marshal(config{Directory: t.TempDir(), Hostname: "herdr-test"})
	if err != nil {
		t.Fatal(err)
	}
	id, err := registerEngine(data, nil)
	if err != nil {
		t.Fatal(err)
	}
	if findEngine(id) == nil {
		t.Fatal("registered engine is missing")
	}
	removeEngine(id)
	if findEngine(id) != nil {
		t.Fatal("destroyed engine remains usable")
	}
	removeEngine(id)
}
