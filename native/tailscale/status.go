package main

import (
	"github.com/sagernet/tailscale/ipn/ipnstate"
	"sort"
)

type snapshot struct {
	NetworkID string   `json:"network_id"`
	State     string   `json:"state"`
	Network   string   `json:"network"`
	SelfName  string   `json:"self_name"`
	SelfIPs   []string `json:"self_ips"`
	AuthURL   string   `json:"auth_url,omitempty"`
	ErrorKind string   `json:"error_kind,omitempty"`
	Error     string   `json:"error,omitempty"`
	Peers     []peer   `json:"peers"`
	Truncated bool     `json:"truncated"`
	Total     int      `json:"total"`
}

type peer struct {
	ID        string   `json:"id"`
	Name      string   `json:"name"`
	DNSName   string   `json:"dns_name"`
	OS        string   `json:"os"`
	Addresses []string `json:"addresses"`
	Online    bool     `json:"online"`
	Expired   bool     `json:"expired"`
	LastSeen  int64    `json:"last_seen"`
	KeyExpiry int64    `json:"key_expiry"`
}

func project(status *ipnstate.Status) snapshot {
	s := snapshot{State: "connecting", Peers: []peer{}, SelfIPs: []string{}}
	switch status.BackendState {
	case "Running":
		s.State = "connected"
	case "NeedsLogin":
		s.State = "needs_login"
	case "NeedsMachineAuth":
		s.State = "needs_approval"
	case "Stopped":
		s.State = "disconnected"
	}
	if s.State == "needs_login" && len(status.AuthURL) <= 4096 {
		s.AuthURL = status.AuthURL
	}
	if status.CurrentTailnet != nil {
		s.Network = bounded(status.CurrentTailnet.Name, 256)
		s.NetworkID = bounded(status.CurrentTailnet.MagicDNSSuffix, 256)
		if s.NetworkID == "" {
			s.NetworkID = s.Network
		}
	}
	if status.Self != nil {
		s.SelfName = bounded(status.Self.HostName, 256)
	}
	for _, ip := range status.TailscaleIPs {
		s.SelfIPs = append(s.SelfIPs, ip.String())
	}
	if s.State != "connected" {
		return s
	}
	for _, p := range status.Peer {
		if p == nil || p.ID == "" || len(p.ID) > 256 {
			continue
		}
		item := peer{ID: bounded(string(p.ID), 256), Name: bounded(p.HostName, 256), DNSName: bounded(p.DNSName, 256), OS: bounded(p.OS, 64), Online: p.Online, Expired: p.Expired, Addresses: []string{}}
		for _, ip := range p.TailscaleIPs {
			if len(item.Addresses) < 8 {
				item.Addresses = append(item.Addresses, ip.String())
			}
		}
		if !p.LastSeen.IsZero() {
			item.LastSeen = p.LastSeen.Unix()
		}
		if p.KeyExpiry != nil {
			item.KeyExpiry = p.KeyExpiry.Unix()
		}
		s.Peers = append(s.Peers, item)
	}
	sort.Slice(s.Peers, func(i, j int) bool {
		a, b := s.Peers[i], s.Peers[j]
		if a.Online != b.Online {
			return a.Online
		}
		if a.Name != b.Name {
			return a.Name < b.Name
		}
		return a.ID < b.ID
	})
	s.Total = len(s.Peers)
	if s.Total > maxPeers {
		s.Peers = s.Peers[:maxPeers]
		s.Truncated = true
	}
	return s
}
