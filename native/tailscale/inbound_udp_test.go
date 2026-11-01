package main

import (
	"bytes"
	"context"
	"net"
	"net/netip"
	"testing"
	"time"

	tun "github.com/sagernet/sing-tun"
	M "github.com/sagernet/sing/common/metadata"
	N "github.com/sagernet/sing/common/network"
)

type observedInbound struct {
	tun.Handler
	closed chan error
}

func (o observedInbound) NewPacketConnectionEx(ctx context.Context, conn N.PacketConn, source, destination M.Socksaddr, onClose N.CloseHandlerFunc) {
	o.Handler.NewPacketConnectionEx(ctx, conn, source, destination, func(err error) {
		o.closed <- err
		if onClose != nil {
			onClose(err)
		}
	})
}

func TestInboundUDPRestoresTailnetReplyAddressAndDatagramBoundaries(t *testing.T) {
	for _, family := range []struct {
		network string
		local   string
		self    netip.Addr
	}{
		{"udp4", "127.0.0.1:0", inboundTestV4},
		{"udp6", "[::1]:0", inboundTestV6},
	} {
		t.Run(family.network, func(t *testing.T) {
			handler := newInbound(context.Background(), func() (netip.Addr, netip.Addr) {
				return inboundTestV4, inboundTestV6
			}, 2*time.Minute)
			t.Cleanup(handler.close)
			closed := make(chan error, 1)
			stack := inboundTestStack(t, observedInbound{handler, closed})
			server, err := net.ListenUDP(family.network, net.UDPAddrFromAddrPort(netip.MustParseAddrPort(family.local)))
			if err != nil {
				t.Fatal(err)
			}
			defer server.Close()
			if err := server.SetReadBuffer(256 * 1024); err != nil {
				t.Fatal(err)
			}
			if err := server.SetWriteBuffer(256 * 1024); err != nil {
				t.Fatal(err)
			}
			server.SetDeadline(time.Now().Add(3 * time.Second))
			payloads := [][]byte{nil, []byte("one datagram"), bytes.Repeat([]byte{0x5a}, 32*1024), bytes.Repeat([]byte{0xa5}, 65507), nil, []byte("after maximum datagram")}
			done := make(chan error, 1)
			received := make(chan int, len(payloads))
			go func() {
				buffer := make([]byte, 65535)
				for range payloads {
					n, source, err := server.ReadFromUDPAddrPort(buffer)
					if err == nil {
						received <- n
						_, err = server.WriteToUDPAddrPort(buffer[:n], source)
					}
					if err != nil {
						done <- err
						return
					}
				}
				done <- nil
			}()
			port := server.LocalAddr().(*net.UDPAddr).AddrPort().Port()
			destination := netip.AddrPortFrom(family.self, port)
			client, err := stack.DialUDP(netip.AddrPortFrom(family.self.Next(), 0), destination)
			if err != nil {
				t.Fatal(err)
			}
			defer client.Close()
			client.SetDeadline(time.Now().Add(3 * time.Second))
			buffer := make([]byte, 65535)
			for _, payload := range payloads {
				if _, err := client.Write(payload); err != nil {
					t.Fatal(err)
				}
				n, source, err := client.ReadFromUDPAddrPort(buffer)
				if err != nil {
					select {
					case relayErr := <-closed:
						t.Fatalf("payload %d, host received %d datagrams, relay closed with %v: %v", len(payload), len(received), relayErr, err)
					default:
						t.Fatalf("payload %d, host received %d datagrams, relay remains open: %v", len(payload), len(received), err)
					}
				}
				if source != destination || !bytes.Equal(buffer[:n], payload) {
					t.Fatalf("reply from %v with %d bytes, want %v with %d", source, n, destination, len(payload))
				}
			}
			if err := <-done; err != nil {
				t.Fatal(err)
			}
			handler.close()
		})
	}
}
