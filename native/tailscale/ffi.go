package main

/*
#include "bridge.h"
*/
import "C"

import (
	"errors"
	"github.com/sagernet/tailscale/ipn"
	"io"
	"net"
	"time"
	"unsafe"
)

// The ABI returns numeric classifications; diagnostics never contain credentials.
func errorCode(err error) C.int32_t {
	switch {
	case err == nil:
		return 0
	case errors.Is(err, errInvalidConfig):
		return -1
	case errors.Is(err, errEnvironment):
		return -10
	case errors.Is(err, errClosed):
		return -2
	case errors.Is(err, errNotReady):
		return -3
	case errors.Is(err, errPeer):
		return -4
	case errors.Is(err, errBusy):
		return -5
	case errors.Is(err, errLimit):
		return -6
	case errors.Is(err, errStorage):
		return -7
	default:
		var timeout net.Error
		if errors.As(err, &timeout) && timeout.Timeout() {
			return -9
		}
		return -8
	}
}

// recoverCode keeps a Go panic from crossing the C ABI. Callers receive only
// the stable IO classification, never a panic payload containing remote state.
func recoverCode(code *C.int32_t) {
	if recover() != nil {
		*code = -8
	}
}

//export herdr_ts_abi
func herdr_ts_abi() C.uint32_t { return 1 }

//export herdr_ts_create
func herdr_ts_create(data *C.uint8_t, length C.size_t, callback C.herdr_store_fn, context C.uintptr_t, result *C.uint64_t) (code C.int32_t) {
	defer recoverCode(&code)
	if data == nil || result == nil || length == 0 || length > 8192 || callback == nil {
		return -1
	}
	id, err := registerEngine(C.GoBytes(unsafe.Pointer(data), C.int(length)), &hostStore{callback: callback, context: context})
	if err == nil {
		*result = C.uint64_t(id)
	}
	return errorCode(err)
}

//export herdr_ts_command
func herdr_ts_command(id C.uint64_t, action C.uint32_t) (code C.int32_t) {
	defer recoverCode(&code)
	e := findEngine(uint64(id))
	if e == nil {
		return -2
	}
	switch action {
	case 1:
		return errorCode(e.start())
	case 2:
		e.close()
		return 0
	case 3:
		return errorCode(e.logout())
	default:
		return -1
	}
}

//export herdr_ts_destroy
func herdr_ts_destroy(id C.uint64_t) { removeEngine(uint64(id)) }

//export herdr_ts_snapshot
func herdr_ts_snapshot(id C.uint64_t, output *C.uint8_t, capacity C.size_t, length *C.size_t) (code C.int32_t) {
	defer recoverCode(&code)
	e := findEngine(uint64(id))
	if e == nil {
		return -2
	}
	if length == nil || capacity > maxSnapshot {
		return -1
	}
	data, err := e.snapshot()
	if err != nil {
		return errorCode(err)
	}
	*length = C.size_t(len(data))
	if capacity < C.size_t(len(data)) {
		return -6
	}
	if output == nil {
		return -1
	}
	copy(unsafe.Slice((*byte)(unsafe.Pointer(output)), int(capacity)), data)
	return 0
}

//export herdr_ts_dial
func herdr_ts_dial(id C.uint64_t, peer *C.uint8_t, length C.size_t, port C.uint16_t, result *C.uint64_t) (code C.int32_t) {
	defer recoverCode(&code)
	e := findEngine(uint64(id))
	if e == nil {
		return -2
	}
	if peer == nil || length == 0 || length > 256 || port == 0 || result == nil {
		return -1
	}
	stream, err := e.dial(string(C.GoBytes(unsafe.Pointer(peer), C.int(length))), uint16(port))
	if err == nil {
		*result = C.uint64_t(stream)
	}
	return errorCode(err)
}

//export herdr_ts_read
func herdr_ts_read(id C.uint64_t, stream C.uint64_t, output *C.uint8_t, capacity C.size_t, length *C.size_t) (code C.int32_t) {
	defer recoverCode(&code)
	e := findEngine(uint64(id))
	if e == nil {
		return -2
	}
	if output == nil || capacity == 0 || capacity > maxState || length == nil {
		return -1
	}
	conn, err := e.stream(uint64(stream))
	if err != nil {
		return errorCode(err)
	}
	if err := conn.SetReadDeadline(time.Now().Add(200 * time.Millisecond)); err != nil {
		return errorCode(err)
	}
	n, err := conn.Read(unsafe.Slice((*byte)(unsafe.Pointer(output)), int(capacity)))
	*length = C.size_t(n)
	if n > 0 || errors.Is(err, io.EOF) {
		return 0
	}
	return errorCode(err)
}

//export herdr_ts_write
func herdr_ts_write(id C.uint64_t, stream C.uint64_t, data *C.uint8_t, length C.size_t, written *C.size_t) (code C.int32_t) {
	defer recoverCode(&code)
	e := findEngine(uint64(id))
	if e == nil {
		return -2
	}
	if data == nil || length == 0 || length > maxState || written == nil {
		return -1
	}
	conn, err := e.stream(uint64(stream))
	if err != nil {
		return errorCode(err)
	}
	if err := conn.SetWriteDeadline(time.Now().Add(200 * time.Millisecond)); err != nil {
		return errorCode(err)
	}
	n, err := conn.Write(unsafe.Slice((*byte)(unsafe.Pointer(data)), int(length)))
	*written = C.size_t(n)
	if n > 0 {
		return 0
	}
	return errorCode(err)
}

//export herdr_ts_close_stream
func herdr_ts_close_stream(id C.uint64_t, stream C.uint64_t) {
	if e := findEngine(uint64(id)); e != nil {
		e.closeStream(uint64(stream))
	}
}

type hostStore struct {
	callback C.herdr_store_fn
	context  C.uintptr_t
}

func (s *hostStore) ReadState(key ipn.StateKey) ([]byte, error) {
	data := make([]byte, maxState)
	size, err := s.call(0, key, data)
	if err != nil {
		return nil, err
	}
	return data[:size], nil
}

func (s *hostStore) WriteState(key ipn.StateKey, data []byte) error {
	op := uint32(1)
	if data == nil {
		op = 2
	}
	_, err := s.call(op, key, data)
	return err
}

func (s *hostStore) call(op uint32, key ipn.StateKey, data []byte) (int, error) {
	name := []byte(key)
	if len(name) == 0 || len(name) > 256 || len(data) > maxState {
		return 0, errLimit
	}
	var size C.size_t
	status := C.herdr_store_call(s.callback, s.context, C.uint32_t(op), (*C.uint8_t)(unsafe.Pointer(unsafe.SliceData(name))), C.size_t(len(name)), (*C.uint8_t)(unsafe.Pointer(unsafe.SliceData(data))), C.size_t(len(data)), &size)
	if status == 1 {
		return 0, ipn.ErrStateNotExist
	}
	if status != 0 || size > maxState || (op == 0 && size > C.size_t(len(data))) {
		return 0, errStorage
	}
	return int(size), nil
}

func main() {}
