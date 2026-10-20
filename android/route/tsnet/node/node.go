// Package node is the portal's own tailnet node: Tailscale's tsnet, run while
// the app is in the foreground, and a proxy on loopback through which the
// WebView reaches one host of the tailnet and nothing else.
//
// gomobile binds it to Java as com.bmabsout.suzatary.node.Node; Route.java (the
// tsnet route) is its one caller.
package node

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"errors"
	"net"
	"net/http"
	"sync"
	"time"

	"tailscale.com/client/local"
	"tailscale.com/ipn"
	"tailscale.com/logtail"
	"tailscale.com/net/netmon"
	"tailscale.com/tsnet"
)

// Reach is how the way to the box stands, a sum given by its eliminator: Java
// implements one method per case, so no case can be left unhandled, and each
// change of the reading calls exactly one.
type Reach interface {
	// Pending: the node is on its way and nothing is asked of the person.
	Pending(why string)
	// Login: Tailscale asks the person to sign in at url, in a browser.
	Login(url string)
	// Ready: the box is reachable through the proxy at proxy (host:port),
	// which admits Basic credentials user and password and nothing else.
	Ready(proxy, user, password string)
	// Down: the node stopped or failed, and why.
	Down(why string)
}

// One session runs at a time; Start replaces it and Stop ends it. A Reach is
// told only by the session that is current, so an old one's late news is
// dropped rather than drawn over a new one's. changed is closed and replaced
// whenever current is, so a dial can wait for the next session.
var (
	mu      sync.Mutex
	current *session
	changed = make(chan struct{})
)

// The proxy outlives sessions: one address and one password for the life of
// the process, so the WebView's setting never goes stale, and a request made
// while the node restarts waits for it instead of failing.
var (
	opening sync.Once
	opened  door
	openErr error
)

// door is the proxy as the WebView is told of it.
type door struct {
	addr string
	gate gate
}

func init() {
	// The owner's phone sends Tailscale no logs: what the node does stays in
	// its own files under Dir.
	logtail.Disable()
	// Android denies an app the netlink socket net.Interfaces reads, and
	// tailscale.com/feature/androidbin, which answers for a raw binary, is
	// not linked into a cgo build such as this one.
	netmon.RegisterInterfaceGetter(interfaces)
}

// Start brings the node up with its state in dir, named hostname on the
// tailnet, and tells r how it stands from now on; the proxy reaches box (a
// MagicDNS name) on 443, the box of the first Start for the life of the
// process. It returns once the node is started, not once it is up.
func Start(dir, hostname, box string, r Reach) error {
	opening.Do(func() { opened, openErr = open(box) })
	if openErr != nil {
		return openErr
	}
	Stop()
	n := &tsnet.Server{Dir: dir, Hostname: hostname}
	if err := n.Start(); err != nil {
		return err
	}
	lc, err := n.LocalClient()
	if err != nil {
		n.Close()
		return err
	}
	ctx, cancel := context.WithCancel(context.Background())
	s := &session{ctx: ctx, cancel: cancel, node: n, local: lc, reach: r, up: make(chan struct{})}
	swap(s)
	go s.watch()
	return nil
}

// Stop ends the session, if one runs: the node leaves the tailnet until the
// next Start, and the proxy answers no request until then. Its state stays
// in dir.
func Stop() {
	if s := swap(nil); s != nil {
		s.cancel()
		s.node.Close()
	}
}

func swap(s *session) (was *session) {
	mu.Lock()
	defer mu.Unlock()
	was, current = current, s
	close(changed)
	changed = make(chan struct{})
	return was
}

// open starts the proxy on loopback with a fresh password.
func open(box string) (door, error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return door{}, err
	}
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return door{}, err
	}
	g := gate{box: box, user: "portal", password: base64.RawURLEncoding.EncodeToString(b)}
	served := &http.Server{Handler: proxy{gate: g, dial: dial}, ReadHeaderTimeout: 10 * time.Second}
	go served.Serve(ln)
	return door{addr: ln.Addr().String(), gate: g}, nil
}

// dial reaches addr through the current session's node once it is up,
// waiting for a session to start and come up if none is.
func dial(ctx context.Context, network, addr string) (net.Conn, error) {
	for {
		mu.Lock()
		s, next := current, changed
		mu.Unlock()
		var up chan struct{}
		if s != nil {
			up = s.up
		}
		select {
		case <-up:
			return s.node.Dial(ctx, network, addr)
		case <-next:
		case <-ctx.Done():
			return nil, errors.New("the portal's tailnet node is not up")
		}
	}
}

type session struct {
	ctx    context.Context
	cancel context.CancelFunc
	node   *tsnet.Server
	local  *local.Client
	reach  Reach
	// up is closed when the node first reads as Running.
	up chan struct{}
}

// watch folds the node's notifications into a reading and tells the Reach
// each time the phase it reads as changes.
func (s *session) watch() {
	w, err := s.local.WatchIPNBus(s.ctx, ipn.NotifyInitialState)
	if err != nil {
		s.tell(down{err.Error()})
		return
	}
	defer w.Close()
	var was reading
	var told phase
	isUp := false
	for {
		n, err := w.Next()
		if err != nil {
			if !errors.Is(s.ctx.Err(), context.Canceled) {
				s.tell(down{err.Error()})
			}
			return
		}
		now := was.with(n)
		// A node removed from the tailnet, or whose key expired, falls from
		// Running back to NeedsLogin; tsnet asks for a sign-in link only when
		// it starts, so the session asks again here.
		if was.state == ipn.Running && now.state == ipn.NeedsLogin {
			go s.local.StartLoginInteractive(s.ctx)
		}
		if now.state == ipn.Running && !isUp {
			isUp = true
			close(s.up)
		}
		if p := now.phase(opened); p != told {
			s.tell(p)
			told = p
		}
		was = now
	}
}

func (s *session) tell(p phase) {
	mu.Lock()
	live := current == s
	mu.Unlock()
	if live {
		p.tell(s.reach)
	}
}
