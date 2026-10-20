package node

import (
	"errors"
	"net"
	"net/netip"

	"tailscale.com/net/netmon"
)

// interfaces answers netmon with one interface holding the addresses the
// kernel would send from, found by connecting a UDP socket (which sends
// nothing and needs no permission) toward a public address of each family.
// It is enough for the node to find its own endpoints; the phone's real
// interfaces are the platform's to know and an app's not to list.
func interfaces() ([]netmon.Interface, error) {
	var addrs []net.Addr
	for _, toward := range []string{"8.8.8.8:53", "[2001:4860:4860::8888]:53"} {
		if ip, ok := source(toward); ok {
			addrs = append(addrs, &net.IPNet{IP: ip.AsSlice(), Mask: net.CIDRMask(ip.BitLen(), ip.BitLen())})
		}
	}
	if len(addrs) == 0 {
		return nil, errors.New("no route out of this phone")
	}
	return []netmon.Interface{{
		Interface: &net.Interface{Index: 1, MTU: 1500, Name: "android", Flags: net.FlagUp | net.FlagRunning},
		AltAddrs:  addrs,
	}}, nil
}

func source(toward string) (netip.Addr, bool) {
	c, err := net.Dial("udp", toward)
	if err != nil {
		return netip.Addr{}, false
	}
	defer c.Close()
	a, ok := c.LocalAddr().(*net.UDPAddr)
	if !ok {
		return netip.Addr{}, false
	}
	ip, ok := netip.AddrFromSlice(a.IP)
	ip = ip.Unmap()
	return ip, ok && !ip.IsLoopback() && !ip.IsUnspecified()
}
