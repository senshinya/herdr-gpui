package main

import (
	"context"
	"errors"
	"net"
	"net/netip"
	"sync"
	"testing"
	"time"

	tun "github.com/sagernet/sing-tun"
	"github.com/sagernet/sing/common/bufio"
	M "github.com/sagernet/sing/common/metadata"
	"github.com/sagernet/tailscale/types/ipproto"
)

func TestInboundPolicyRejectsOtherNodesSubnetsAndUnsupportedFlows(t *testing.T) {
	handler := newInbound(context.Background(), func() (netip.Addr, netip.Addr) {
		return inboundTestV4, inboundTestV6
	}, time.Minute)
	defer handler.close()
	for _, entry := range []struct {
		protocol ipproto.Proto
		target   string
		action   tun.FlowAction
	}{
		{ipproto.TCP, "100.64.0.11:22", tun.ActionAccept},
		{ipproto.UDP, "[fd7a:115c:a1e0::11]:53", tun.ActionAccept},
		{ipproto.ICMPv4, "100.64.0.11:0", tun.ActionAccept},
		{ipproto.ICMPv6, "[fd7a:115c:a1e0::11]:0", tun.ActionAccept},
		{ipproto.TCP, "100.64.0.12:22", tun.ActionReject},
		{ipproto.UDP, "192.168.1.1:53", tun.ActionReject},
		{ipproto.TCP, "127.0.0.1:22", tun.ActionReject},
		{ipproto.TCP, "100.64.0.11:0", tun.ActionReject},
		{ipproto.Proto(47), "100.64.0.11:1", tun.ActionReject},
	} {
		target := netip.MustParseAddrPort(entry.target)
		if got := handler.JudgeFlow(uint8(entry.protocol), netip.AddrPort{}, target, nil).Action; got != entry.action {
			t.Errorf("protocol %v destination %v: got %v, want %v", entry.protocol, target, got, entry.action)
		}
	}
	for _, destination := range []M.Socksaddr{
		{Fqdn: "localhost", Port: 22},
		{Addr: inboundTestV4.Next(), Port: 22},
	} {
		if _, err := handler.target(destination); !errors.Is(err, errInboundTarget) {
			t.Fatalf("invalid destination was not rejected: %v", err)
		}
	}
	handler.close()
	if handler.JudgeFlow(uint8(ipproto.TCP), netip.AddrPort{}, netip.AddrPortFrom(inboundTestV4, 22), nil).Action != tun.ActionReject {
		t.Fatal("closed handler still admits new flows")
	}
}

func TestEngineCloseDrainsInboundAndEnforcesSharedSessionLimit(t *testing.T) {
	e, err := newEngine(config{Directory: t.TempDir(), Hostname: "test"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer e.close()
	e.inbound.selfIPs = func() (netip.Addr, netip.Addr) { return inboundTestV4, inboundTestV6 }
	listener := inboundTestListener(t, "tcp4", "127.0.0.1:0")
	destination := M.SocksaddrFrom(inboundTestV4, listener.Addr().(*net.TCPAddr).AddrPort().Port())
	closed := make(chan error, maxInbound)
	for range maxInbound / 2 {
		incoming, peer := net.Pipe()
		defer peer.Close()
		e.inbound.NewConnectionEx(context.Background(), incoming, M.Socksaddr{}, destination, func(err error) { closed <- err })
		backend, err := listener.AcceptTCP()
		if err != nil {
			t.Fatal(err)
		}
		defer backend.Close()
	}
	udp, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer udp.Close()
	udpDestination := M.SocksaddrFrom(inboundTestV4, udp.LocalAddr().(*net.UDPAddr).AddrPort().Port())
	for range maxInbound - maxInbound/2 {
		incoming, peer := net.Pipe()
		defer peer.Close()
		packets := bufio.NewUnbindPacketConnWithAddr(incoming, udpDestination)
		e.inbound.NewPacketConnectionEx(context.Background(), packets, M.Socksaddr{}, udpDestination, func(err error) { closed <- err })
	}
	excess, peer := net.Pipe()
	defer peer.Close()
	rejected := make(chan error, 1)
	e.inbound.NewConnectionEx(context.Background(), excess, M.Socksaddr{}, destination, func(err error) { rejected <- err })
	if err := <-rejected; !errors.Is(err, errBusy) {
		t.Fatalf("excess session: got %v", err)
	}
	var shutdown sync.WaitGroup
	for range 2 {
		shutdown.Add(1)
		go func() { defer shutdown.Done(); e.close() }()
	}
	finished := make(chan struct{})
	go func() { shutdown.Wait(); close(finished) }()
	select {
	case <-finished:
	case <-time.After(3 * time.Second):
		t.Fatal("engine close did not finish with blocked inbound sessions")
	}
	if len(closed) != maxInbound {
		t.Fatalf("shutdown returned before all inbound callbacks finished: %d", len(closed))
	}
	select {
	case <-e.closedDone:
	default:
		t.Fatal("engine closure was acknowledged before cleanup finished")
	}
}

func TestInboundUDPIdleSessionReleasesItsSlot(t *testing.T) {
	handler := newInbound(context.Background(), func() (netip.Addr, netip.Addr) {
		return inboundTestV4, inboundTestV6
	}, 40*time.Millisecond)
	defer handler.close()
	service, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	destination := M.SocksaddrFrom(inboundTestV4, service.LocalAddr().(*net.UDPAddr).AddrPort().Port())
	incoming, peer := net.Pipe()
	defer peer.Close()
	done := make(chan error, 1)
	handler.NewPacketConnectionEx(context.Background(), bufio.NewUnbindPacketConnWithAddr(incoming, destination), M.Socksaddr{}, destination, func(err error) { done <- err })
	select {
	case err := <-done:
		if err == nil {
			t.Fatal("idle flow was not closed with an error")
		}
	case <-time.After(3 * time.Second):
		t.Fatal("idle UDP session remained open")
	}
	handler.mu.Lock()
	remaining := len(handler.active)
	handler.mu.Unlock()
	if remaining != 0 {
		t.Fatalf("idle UDP session retained a slot: %d", remaining)
	}
}
