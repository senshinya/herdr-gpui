package main

import (
	"context"
	"errors"
	"io"
	"net"
	"net/netip"
	"sync"
	"time"

	tun "github.com/sagernet/sing-tun"
	"github.com/sagernet/sing/common/bufio"
	"github.com/sagernet/sing/common/canceler"
	M "github.com/sagernet/sing/common/metadata"
	N "github.com/sagernet/sing/common/network"
	"github.com/sagernet/tailscale/types/ipproto"
)

const maxInbound = 64
const inboundUDPIdle = 2 * time.Minute
const inboundUDPBuffer = 256 * 1024

var errInboundTarget = errors.New("incoming destination is not this Tailnet node")
var errInboundSocket = errors.New("incoming UDP requires a datagram socket")

type inboundFlow struct {
	client  io.Closer
	backend net.Conn
}

// The WireGuard engine applies Tailnet access rules before these handlers.
// Only this node's own addresses map to loopback; subnet forwarding is refused.
type inbound struct {
	ctx     context.Context
	cancel  context.CancelFunc
	selfIPs func() (netip.Addr, netip.Addr)
	udpIdle time.Duration

	mu         sync.Mutex
	active     map[*inboundFlow]struct{}
	closed     bool
	closedDone chan struct{}
	work       sync.WaitGroup
}

var _ tun.Handler = (*inbound)(nil)

func newInbound(ctx context.Context, selfIPs func() (netip.Addr, netip.Addr), udpIdle time.Duration) *inbound {
	ctx, cancel := context.WithCancel(ctx)
	return &inbound{
		ctx: ctx, cancel: cancel, selfIPs: selfIPs, udpIdle: udpIdle,
		active: make(map[*inboundFlow]struct{}), closedDone: make(chan struct{}),
	}
}

func (h *inbound) owns(address netip.Addr) bool {
	if !address.IsValid() || address.Zone() != "" {
		return false
	}
	v4, v6 := h.selfIPs()
	return address == v4 || address == v6
}

func (h *inbound) target(destination M.Socksaddr) (netip.AddrPort, error) {
	if destination.Fqdn != "" || destination.Port == 0 || !h.owns(destination.Addr) {
		return netip.AddrPort{}, errInboundTarget
	}
	address := netip.AddrFrom4([4]byte{127, 0, 0, 1})
	if destination.Addr.Is6() {
		address = netip.IPv6Loopback()
	}
	return netip.AddrPortFrom(address, destination.Port), nil
}

func (h *inbound) JudgeFlow(protocol uint8, _, destination netip.AddrPort, _ []byte) tun.FlowVerdict {
	if h.ctx.Err() != nil || !h.owns(destination.Addr()) {
		return tun.FlowVerdict{Action: tun.ActionReject}
	}
	switch ipproto.Proto(protocol) {
	case ipproto.TCP, ipproto.UDP:
		if destination.Port() != 0 {
			return tun.FlowVerdict{Action: tun.ActionAccept, UDPTimeout: h.udpIdle}
		}
	case ipproto.ICMPv4:
		if destination.Addr().Is4() {
			return tun.FlowVerdict{Action: tun.ActionAccept}
		}
	case ipproto.ICMPv6:
		if destination.Addr().Is6() {
			return tun.FlowVerdict{Action: tun.ActionAccept}
		}
	}
	return tun.FlowVerdict{Action: tun.ActionReject}
}

// JudgeFlow never requests DNS hijacking. Tailnet's internal DNS stays owned
// by tsnet, while a DNS service on this node uses the ordinary UDP/TCP path.
func (*inbound) NewDNSPacket([]byte, M.Socksaddr, M.Socksaddr, N.PacketWriter) {}

func (h *inbound) NewConnectionEx(ctx context.Context, conn net.Conn, _, destination M.Socksaddr, onClose N.CloseHandlerFunc) {
	h.forward(ctx, conn, destination, "tcp", onClose, func(ctx context.Context, backend net.Conn) error {
		if err := N.ReportConnHandshakeSuccess(conn, backend); err != nil {
			return err
		}
		return bufio.CopyConn(ctx, conn, backend)
	})
}

func (h *inbound) NewPacketConnectionEx(ctx context.Context, conn N.PacketConn, _, destination M.Socksaddr, onClose N.CloseHandlerFunc) {
	h.forward(ctx, conn, destination, "udp", onClose, func(ctx context.Context, backend net.Conn) error {
		socket, ok := backend.(*net.UDPConn)
		if !ok {
			return errInboundSocket
		}
		// Darwin needs room for datagram metadata as well as payload. A 64 KiB
		// receive buffer can silently discard a valid maximum-size reply.
		if err := socket.SetReadBuffer(inboundUDPBuffer); err != nil {
			return err
		}
		if err := socket.SetWriteBuffer(inboundUDPBuffer); err != nil {
			return err
		}
		// A connected UDP socket fixes the local destination and accepts replies
		// only from that service. Restore the Tailnet address on return packets.
		packetBackend := bufio.NewUnbindPacketConnWithAddr(backend, destination)
		if err := N.ReportPacketConnHandshakeSuccess(conn, packetBackend); err != nil {
			return err
		}
		ctx, timed := canceler.NewPacketConn(ctx, conn, h.udpIdle)
		defer timed.Close()
		return bufio.CopyPacketConn(ctx, inboundPacketConn{timed}, inboundPacketConn{packetBackend})
	})
}

// The generic packet copier defaults to 16 KiB, which truncates larger UDP
// replies. Its existing MTU contract reserves a full datagram plus headroom.
type inboundPacketConn struct{ N.PacketConn }

func (inboundPacketConn) ReaderMTU() int  { return 65535 }
func (c inboundPacketConn) Upstream() any { return c.PacketConn }

func (h *inbound) forward(parent context.Context, client io.Closer, destination M.Socksaddr, network string, onClose N.CloseHandlerFunc, bridge func(context.Context, net.Conn) error) {
	if err := parent.Err(); err != nil {
		N.CloseOnHandshakeFailure(client, onClose, err)
		return
	}
	target, err := h.target(destination)
	if err != nil {
		N.CloseOnHandshakeFailure(client, onClose, err)
		return
	}
	h.mu.Lock()
	if h.closed || h.ctx.Err() != nil {
		h.mu.Unlock()
		N.CloseOnHandshakeFailure(client, onClose, errClosed)
		return
	}
	if len(h.active) >= maxInbound {
		h.mu.Unlock()
		N.CloseOnHandshakeFailure(client, onClose, errBusy)
		return
	}
	flow := &inboundFlow{client: client}
	h.active[flow] = struct{}{}
	h.work.Add(1)
	h.mu.Unlock()
	go func() {
		defer h.work.Done()
		ctx, cancel := context.WithCancel(h.ctx)
		stop := context.AfterFunc(parent, cancel)
		var backend net.Conn
		var result error
		defer func() {
			stop()
			cancel()
			client.Close()
			if backend != nil {
				backend.Close()
			}
			h.mu.Lock()
			delete(h.active, flow)
			h.mu.Unlock()
			if onClose != nil {
				onClose(result)
			}
		}()
		dialCtx, dialCancel := context.WithTimeout(ctx, 10*time.Second)
		backend, result = (&net.Dialer{}).DialContext(dialCtx, network, target.String())
		dialCancel()
		if result != nil {
			result = N.CloseOnHandshakeFailure(client, nil, result)
			return
		}
		h.mu.Lock()
		closed := h.closed || ctx.Err() != nil
		if !closed {
			flow.backend = backend
		}
		h.mu.Unlock()
		if closed {
			result = N.CloseOnHandshakeFailure(client, nil, errClosed)
			return
		}
		result = bridge(ctx, backend)
	}()
}

func (h *inbound) close() {
	h.mu.Lock()
	if h.closed {
		done := h.closedDone
		h.mu.Unlock()
		<-done
		return
	}
	h.closed = true
	h.cancel()
	connections := make([]io.Closer, 0, len(h.active)*2)
	for flow := range h.active {
		connections = append(connections, flow.client)
		if flow.backend != nil {
			connections = append(connections, flow.backend)
		}
	}
	h.mu.Unlock()
	for _, conn := range connections {
		conn.Close()
	}
	h.work.Wait()
	close(h.closedDone)
}
