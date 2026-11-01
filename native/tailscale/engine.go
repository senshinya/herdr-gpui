package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/sagernet/tailscale/envknob"
	"github.com/sagernet/tailscale/ipn"
	"github.com/sagernet/tailscale/tsnet"
)

const maxSnapshot = 4 * 1024 * 1024
const maxPeers = 1024
const maxStreams = 64
const maxState = 64 * 1024

var (
	errInvalidConfig = errors.New("invalid Tailscale configuration")
	errClosed        = errors.New("Tailscale connection is closed")
	errNotReady      = errors.New("Tailscale is not connected")
	errPeer          = errors.New("Tailscale peer is unavailable")
	errBusy          = errors.New("Tailscale connection limit reached")
	errLimit         = errors.New("Tailscale response exceeds its limit")
	errEnvironment   = errors.New("ambient Tailscale authentication is unsupported")
	errStorage       = errors.New("Tailscale identity storage failed")
)

type config struct {
	Directory  string `json:"directory"`
	Hostname   string `json:"hostname"`
	ControlURL string `json:"control_url"`
}

type engine struct {
	mu         sync.Mutex
	server     *tsnet.Server
	ctx        context.Context
	cancel     context.CancelFunc
	started    bool
	closed     bool
	closedDone chan struct{}
	inbound    *inbound
	done       chan struct{}
	latest     snapshot
	streams    map[uint64]net.Conn
	nextStream uint64
}

func newEngine(c config, store ipn.StateStore) (*engine, error) {
	if !filepath.IsAbs(c.Directory) || len(c.Directory) > 4096 || strings.ContainsRune(c.Directory, 0) || len(c.Hostname) == 0 || len(c.Hostname) > 63 {
		return nil, errInvalidConfig
	}
	for _, ch := range c.Hostname {
		if !(ch >= 'a' && ch <= 'z' || ch >= 'A' && ch <= 'Z' || ch >= '0' && ch <= '9' || ch == '-') {
			return nil, errInvalidConfig
		}
	}
	if strings.HasPrefix(c.Hostname, "-") || strings.HasSuffix(c.Hostname, "-") {
		return nil, errInvalidConfig
	}
	if c.ControlURL != "" {
		u, err := url.Parse(c.ControlURL)
		if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.Fragment != "" || u.RawQuery != "" {
			return nil, errInvalidConfig
		}
	}
	if c.ControlURL == "" {
		c.ControlURL = "https://controlplane.tailscale.com"
	}
	envknob.SetNoLogsNoSupport()
	ctx, cancel := context.WithCancel(context.Background())
	discard := func(string, ...any) {}
	server := &tsnet.Server{Dir: c.Directory, Hostname: c.Hostname, ControlURL: c.ControlURL, Store: store, Logf: discard, UserLogf: discard}
	inbound := newInbound(ctx, server.TailscaleIPs, inboundUDPIdle)
	server.NetstackHandler = inbound
	return &engine{
		server: server, inbound: inbound, closedDone: make(chan struct{}),
		ctx: ctx, cancel: cancel, done: make(chan struct{}),
		latest: snapshot{State: "disconnected", Peers: []peer{}, SelfIPs: []string{}}, streams: make(map[uint64]net.Conn),
	}, nil
}

func (e *engine) start() error {
	for _, key := range []string{"TS_AUTHKEY", "TS_AUTH_KEY", "TS_CLIENT_SECRET", "TS_CLIENT_ID", "TS_ID_TOKEN", "TS_AUDIENCE"} {
		if os.Getenv(key) != "" {
			return errEnvironment
		}
	}
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed {
		return errClosed
	}
	if e.started {
		return nil
	}
	e.started = true
	e.latest.State = "connecting"
	go e.run()
	return nil
}

func (e *engine) run() {
	defer close(e.done)
	defer e.server.Close()
	if err := e.server.Start(); err != nil {
		e.fail("start", err)
		return
	}
	client, err := e.server.LocalClient()
	if err != nil {
		e.fail("status", err)
		return
	}
	signal := make(chan struct{}, 1)
	schedule := func() {
		select {
		case signal <- struct{}{}:
		default:
		}
	}
	projected := make(chan struct{})
	defer func() { <-projected }()
	go func() {
		defer close(projected)
		schedule()
		for {
			select {
			case <-e.ctx.Done():
				return
			case <-signal:
				ctx, cancel := context.WithTimeout(e.ctx, 2*time.Second)
				status, err := client.Status(ctx)
				cancel()
				if err != nil {
					if e.ctx.Err() == nil {
						e.fail("status", err)
					}
					continue
				}
				state := project(status)
				e.mu.Lock()
				if !e.closed {
					e.latest = state
				}
				e.mu.Unlock()
			}
		}
	}()
	for e.ctx.Err() == nil {
		watcher, err := client.WatchIPNBus(e.ctx, ipn.NotifyInitialState|ipn.NotifyPeerPatches|ipn.NotifyNoPrivateKeys)
		if err == nil {
			for e.ctx.Err() == nil {
				notification, err := watcher.Next()
				if err != nil {
					break
				}
				if notification.ErrMessage != nil {
					e.fail("network", errors.New(*notification.ErrMessage))
				}
				schedule()
			}
			watcher.Close()
		}
		select {
		case <-e.ctx.Done():
			return
		case <-time.After(time.Second):
			schedule()
		}
	}
}

func (e *engine) fail(kind string, err error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if !e.closed {
		e.latest = snapshot{State: "failed", ErrorKind: kind, Error: "Tailscale could not establish the network connection", Peers: []peer{}, SelfIPs: []string{}}
	}
}

func (e *engine) snapshot() ([]byte, error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	data, err := json.Marshal(e.latest)
	if len(data) > maxSnapshot {
		return nil, errLimit
	}
	return data, err
}

func (e *engine) logout() error {
	e.mu.Lock()
	started, closed := e.started, e.closed
	e.mu.Unlock()
	if closed {
		return errClosed
	}
	if !started {
		return errNotReady
	}
	client, err := e.server.LocalClient()
	if err != nil {
		return err
	}
	ctx, cancel := context.WithTimeout(e.ctx, 10*time.Second)
	defer cancel()
	if err := client.Logout(ctx); err != nil {
		return err
	}
	e.close()
	return nil
}

func (e *engine) close() {
	e.mu.Lock()
	if e.closed {
		done := e.closedDone
		e.mu.Unlock()
		<-done
		return
	}
	e.closed = true
	e.cancel()
	started := e.started
	streams := e.streams
	e.streams = make(map[uint64]net.Conn)
	e.latest = snapshot{State: "disconnected", Peers: []peer{}, SelfIPs: []string{}}
	e.mu.Unlock()
	defer close(e.closedDone)
	for _, stream := range streams {
		if stream != nil {
			stream.Close()
		}
	}
	e.inbound.close()
	if started {
		<-e.done
	}
}

// A stable ID is resolved from the live backend, never from caller-supplied DNS.
func (e *engine) dial(id string, port uint16) (uint64, error) {
	e.mu.Lock()
	if e.closed {
		e.mu.Unlock()
		return 0, errClosed
	}
	if !e.started || e.latest.State != "connected" {
		e.mu.Unlock()
		return 0, errNotReady
	}
	if port == 0 || len(id) > 256 {
		e.mu.Unlock()
		return 0, errPeer
	}
	if len(e.streams) >= maxStreams {
		e.mu.Unlock()
		return 0, errBusy
	}
	e.nextStream++
	streamID := e.nextStream
	e.streams[streamID] = nil
	e.mu.Unlock()
	release := func() { e.mu.Lock(); delete(e.streams, streamID); e.mu.Unlock() }
	ctx, cancel := context.WithTimeout(e.ctx, 10*time.Second)
	defer cancel()
	client, err := e.server.LocalClient()
	if err != nil {
		release()
		return 0, err
	}
	status, err := client.Status(ctx)
	if err != nil {
		release()
		return 0, err
	}
	if status.BackendState != "Running" {
		release()
		return 0, errNotReady
	}
	var address string
	for _, p := range status.Peer {
		if string(p.ID) == id && p.Online && !p.Expired && len(p.TailscaleIPs) > 0 {
			address = net.JoinHostPort(p.TailscaleIPs[0].String(), fmt.Sprint(port))
			break
		}
	}
	if address == "" {
		release()
		return 0, errPeer
	}
	conn, err := e.server.Dial(ctx, "tcp", address)
	if err != nil {
		release()
		return 0, err
	}
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed {
		conn.Close()
		delete(e.streams, streamID)
		return 0, errClosed
	}
	e.streams[streamID] = conn
	return streamID, nil
}

func (e *engine) stream(id uint64) (net.Conn, error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	conn := e.streams[id]
	if conn == nil {
		return nil, errClosed
	}
	return conn, nil
}

func (e *engine) closeStream(id uint64) {
	e.mu.Lock()
	conn := e.streams[id]
	delete(e.streams, id)
	e.mu.Unlock()
	if conn != nil {
		conn.Close()
	}
}

func bounded(s string, limit int) string {
	r := []rune(s)
	if len(r) > limit {
		return string(r[:limit])
	}
	return s
}
