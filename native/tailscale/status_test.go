package main

import (
	"github.com/sagernet/tailscale/ipn/ipnstate"
	"github.com/sagernet/tailscale/tailcfg"
	"github.com/sagernet/tailscale/types/key"
	"strings"
	"testing"
)

func TestDisconnectedStateNeverPublishesOldPeers(t *testing.T) {
	status := &ipnstate.Status{BackendState: "NeedsLogin", AuthURL: "https://login.tailscale.com/a/test", Peer: map[key.NodePublic]*ipnstate.PeerStatus{key.NewNode().Public(): {ID: tailcfg.StableNodeID("old"), Online: true}}}
	projected := project(status)
	if len(projected.Peers) != 0 || projected.State != "needs_login" || projected.AuthURL == "" {
		t.Fatalf("stale or incomplete login projection: %+v", projected)
	}
	status.BackendState = "Running"
	if projected = project(status); projected.AuthURL != "" {
		t.Fatal("connected state retained the authorization URL")
	}
}

func TestOversizedPeerIdentityIsRejectedInsteadOfTruncated(t *testing.T) {
	status := &ipnstate.Status{BackendState: "Running", Peer: map[key.NodePublic]*ipnstate.PeerStatus{key.NewNode().Public(): {ID: tailcfg.StableNodeID(strings.Repeat("x", 257)), Online: true}}}
	if len(project(status).Peers) != 0 {
		t.Fatal("truncating an identity can alias a different peer")
	}
}
