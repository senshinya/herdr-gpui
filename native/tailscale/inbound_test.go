package main

import (
	"context"
	"io"
	"net"
	"net/netip"
	"testing"
	"time"

	tun "github.com/sagernet/sing-tun"
	"github.com/sagernet/sing/common/buf"
	"github.com/sagernet/sing/common/logger"
)

var inboundTestV4 = netip.MustParseAddr("100.64.0.11")
var inboundTestV6 = netip.MustParseAddr("fd7a:115c:a1e0::11")

// Two real userspace stacks exchange IP packets in memory. No host interface,
// control server or personal Tailnet is needed to exercise the packet contract.
func inboundTestStack(t *testing.T, handler tun.Handler) *tun.Go {
	t.Helper()
	var clientDevice, serverDevice *tun.MemoryTun
	clientDevice = tun.NewMemoryTun(tun.MemoryTunOptions{MTU: 1280, Outbound: func(packets []*buf.Buffer) {
		for _, packet := range packets {
			_, _ = serverDevice.Write(packet.Bytes())
			packet.Release()
		}
	}})
	serverDevice = tun.NewMemoryTun(tun.MemoryTunOptions{MTU: 1280, Outbound: func(packets []*buf.Buffer) {
		for _, packet := range packets {
			_, _ = clientDevice.Write(packet.Bytes())
			packet.Release()
		}
	}})
	t.Cleanup(func() { clientDevice.Close(); serverDevice.Close() })
	newStack := func(device *tun.MemoryTun, handler tun.Handler) *tun.Go {
		stack, err := tun.NewGo(tun.StackOptions{
			Context: context.Background(), Tun: device, TunOptions: tun.Options{MTU: 1280},
			Handler: handler, Logger: logger.NOP(), UDPTimeout: time.Minute,
		})
		if err != nil {
			t.Fatal(err)
		}
		if err := stack.Start(); err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { stack.Close() })
		return stack
	}
	client := newStack(clientDevice, nil)
	newStack(serverDevice, handler)
	return client
}

func inboundTestListener(t *testing.T, network, address string) *net.TCPListener {
	t.Helper()
	listener, err := net.ListenTCP(network, net.TCPAddrFromAddrPort(netip.MustParseAddrPort(address)))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { listener.Close() })
	if err := listener.SetDeadline(time.Now().Add(3 * time.Second)); err != nil {
		t.Fatal(err)
	}
	return listener
}

func TestInboundTCPForwardsBothFamiliesAndPreservesHalfClose(t *testing.T) {
	for _, family := range []struct {
		network string
		local   string
		self    netip.Addr
	}{
		{"tcp4", "127.0.0.1:0", inboundTestV4},
		{"tcp6", "[::1]:0", inboundTestV6},
	} {
		t.Run(family.network, func(t *testing.T) {
			handler := newInbound(context.Background(), func() (netip.Addr, netip.Addr) {
				return inboundTestV4, inboundTestV6
			}, 2*time.Minute)
			t.Cleanup(handler.close)
			stack := inboundTestStack(t, handler)
			listener := inboundTestListener(t, family.network, family.local)
			port := listener.Addr().(*net.TCPAddr).AddrPort().Port()
			ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
			defer cancel()
			client, err := stack.DialTCP(ctx, family.self.Next(), netip.AddrPortFrom(family.self, port))
			if err != nil {
				t.Fatal(err)
			}
			defer client.Close()
			client.SetDeadline(time.Now().Add(3 * time.Second))
			server, err := listener.AcceptTCP()
			if err != nil {
				t.Fatal(err)
			}
			defer server.Close()
			server.SetDeadline(time.Now().Add(3 * time.Second))
			if !server.RemoteAddr().(*net.TCPAddr).IP.IsLoopback() {
				t.Fatal("inbound dial was not from the host loopback")
			}
			if _, err := client.Write([]byte("request")); err != nil {
				t.Fatal(err)
			}
			if err := client.CloseWrite(); err != nil {
				t.Fatal(err)
			}
			request, err := io.ReadAll(server)
			if err != nil || string(request) != "request" {
				t.Fatalf("request %q: %v", request, err)
			}
			if _, err := server.Write([]byte("response after request EOF")); err != nil {
				t.Fatal(err)
			}
			server.CloseWrite()
			response, err := io.ReadAll(client)
			if err != nil || string(response) != "response after request EOF" {
				t.Fatalf("half-close lost response %q: %v", response, err)
			}
		})
	}
}
