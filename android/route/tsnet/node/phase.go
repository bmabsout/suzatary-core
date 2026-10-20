package node

import "tailscale.com/ipn"

// reading is what the node's notifications have said so far: the backend's
// state, the last sign-in link and the last error, each the latest write.
type reading struct {
	state ipn.State
	url   string
	err   string
}

// with is the reading after one more notification; a field it does not carry
// keeps its value, a new state supersedes the last error, and reaching
// Running spends the sign-in link.
func (r reading) with(n ipn.Notify) reading {
	if n.State != nil {
		r.state = *n.State
		r.err = ""
		if r.state == ipn.Running {
			r.url = ""
		}
	}
	if n.BrowseToURL != nil {
		r.url = *n.BrowseToURL
	}
	if n.ErrMessage != nil {
		r.err = *n.ErrMessage
	}
	return r
}

// phase is a Reach's case as a comparable value, so a reading is told only
// when what it reads as changes.
type phase interface{ tell(Reach) }

type (
	pending struct{ why string }
	login   struct{ url string }
	ready   struct{ proxy, user, password string }
	down    struct{ why string }
)

func (p pending) tell(r Reach) { r.Pending(p.why) }
func (p login) tell(r Reach)   { r.Login(p.url) }
func (p ready) tell(r Reach)   { r.Ready(p.proxy, p.user, p.password) }
func (p down) tell(r Reach)    { r.Down(p.why) }

func (r reading) phase(d door) phase {
	if r.err != "" {
		return down{r.err}
	}
	switch r.state {
	case ipn.Running:
		return ready{d.addr, d.gate.user, d.gate.password}
	case ipn.NeedsLogin:
		if r.url != "" {
			return login{r.url}
		}
		return pending{"Asking Tailscale for a sign-in link"}
	case ipn.NeedsMachineAuth:
		return pending{"Waiting for the tailnet's admin to approve this device"}
	case ipn.Stopped:
		return down{"The node stopped"}
	case ipn.InUseOtherUser:
		return down{"The node is in use by another user"}
	default:
		return pending{"Connecting to the tailnet"}
	}
}
