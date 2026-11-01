package main

import (
	"bytes"
	"encoding/json"
	"github.com/sagernet/tailscale/ipn"
	"io"
	"sync"
)

var registry = struct {
	sync.RWMutex
	next    uint64
	engines map[uint64]*engine
}{engines: make(map[uint64]*engine)}

func registerEngine(data []byte, store ipn.StateStore) (uint64, error) {
	if len(data) > 8192 {
		return 0, errLimit
	}
	var c config
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&c); err != nil {
		return 0, errInvalidConfig
	}
	if decoder.Decode(new(any)) != io.EOF {
		return 0, errInvalidConfig
	}
	e, err := newEngine(c, store)
	if err != nil {
		return 0, err
	}
	registry.Lock()
	defer registry.Unlock()
	if len(registry.engines) >= 4 {
		e.close()
		return 0, errBusy
	}
	registry.next++
	registry.engines[registry.next] = e
	return registry.next, nil
}

func findEngine(id uint64) *engine {
	registry.RLock()
	defer registry.RUnlock()
	return registry.engines[id]
}

func removeEngine(id uint64) {
	registry.Lock()
	e := registry.engines[id]
	delete(registry.engines, id)
	registry.Unlock()
	if e != nil {
		e.close()
	}
}
