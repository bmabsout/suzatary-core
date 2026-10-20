package node

import (
	"bufio"
	"context"
	"encoding/base64"
	"io"
	"math/rand/v2"
	"net"
	"net/http"
	"strconv"
	"strings"
	"testing"

	"tailscale.com/ipn"
)

// box stands in for the tailnet: the proxy's dial reaches an echo server,
// whatever address it is asked for, and records the address.
func box(t *testing.T) (dial func(context.Context, string, string) (net.Conn, error), asked *[]string) {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ln.Close() })
	go func() {
		for {
			c, err := ln.Accept()
			if err != nil {
				return
			}
			go func() { io.Copy(c, c); c.Close() }()
		}
	}()
	asked = new([]string)
	return func(ctx context.Context, network, addr string) (net.Conn, error) {
		*asked = append(*asked, addr)
		return new(net.Dialer).DialContext(ctx, network, ln.Addr().String())
	}, asked
}

func serve(t *testing.T, p proxy) string {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	s := &http.Server{Handler: p}
	go s.Serve(ln)
	t.Cleanup(func() { s.Close() })
	return ln.Addr().String()
}

// ask sends one request line with an optional credential and returns the
// connection and the response's status line.
func ask(t *testing.T, at, line, user, password string) (net.Conn, *bufio.Reader, *http.Response) {
	c, err := net.Dial("tcp", at)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { c.Close() })
	req := line + "\r\n"
	if user != "" {
		req += "Proxy-Authorization: Basic " + base64.StdEncoding.EncodeToString([]byte(user+":"+password)) + "\r\n"
	}
	if _, err := io.WriteString(c, req+"\r\n"); err != nil {
		t.Fatal(err)
	}
	r := bufio.NewReader(c)
	resp, err := http.ReadResponse(r, nil)
	if err != nil {
		t.Fatal(err)
	}
	return c, r, resp
}

const theBox = "box.example.ts.net"

func TestProxy(t *testing.T) {
	dial, asked := box(t)
	g := gate{box: theBox, user: "portal", password: "s3cret"}
	at := serve(t, proxy{gate: g, dial: dial})
	connect := "CONNECT " + theBox + ":443 HTTP/1.1\r\nHost: " + theBox + ":443"

	t.Run("a request without the password is asked for it", func(t *testing.T) {
		_, _, resp := ask(t, at, connect, "", "")
		if resp.StatusCode != http.StatusProxyAuthRequired || !strings.HasPrefix(resp.Header.Get("Proxy-Authenticate"), "Basic ") {
			t.Fatalf("got %s %q", resp.Status, resp.Header.Get("Proxy-Authenticate"))
		}
	})
	t.Run("a wrong password is asked again", func(t *testing.T) {
		_, _, resp := ask(t, at, connect, "portal", "guess")
		if resp.StatusCode != http.StatusProxyAuthRequired {
			t.Fatalf("got %s", resp.Status)
		}
	})
	for _, line := range []string{
		"CONNECT elsewhere.example.ts.net:443 HTTP/1.1\r\nHost: elsewhere.example.ts.net:443",
		"CONNECT " + theBox + ":22 HTTP/1.1\r\nHost: " + theBox + ":22",
		"GET http://" + theBox + "/ HTTP/1.1\r\nHost: " + theBox,
	} {
		t.Run("the portal reaches nothing but the box on 443: "+strings.Fields(line)[1], func(t *testing.T) {
			_, _, resp := ask(t, at, line, "portal", "s3cret")
			if resp.StatusCode != http.StatusForbidden {
				t.Fatalf("got %s", resp.Status)
			}
		})
	}
	if len(*asked) != 0 {
		t.Fatalf("a refused request dialled %v", *asked)
	}
	t.Run("the portal's CONNECT to the box is a tunnel", func(t *testing.T) {
		c, r, resp := ask(t, at, connect, "portal", "s3cret")
		if resp.StatusCode != http.StatusOK {
			t.Fatalf("got %s", resp.Status)
		}
		if _, err := io.WriteString(c, "ping"); err != nil {
			t.Fatal(err)
		}
		got := make([]byte, 4)
		if _, err := io.ReadFull(r, got); err != nil || string(got) != "ping" {
			t.Fatalf("echo %q, %v", got, err)
		}
		if want := []string{theBox + ":443"}; strings.Join(*asked, ",") != strings.Join(want, ",") {
			t.Fatalf("dialled %v, want %v", *asked, want)
		}
	})
}

func note(state ipn.State) ipn.Notify  { return ipn.Notify{State: &state} }
func browse(url string) ipn.Notify     { return ipn.Notify{BrowseToURL: &url} }
func failed(message string) ipn.Notify { return ipn.Notify{ErrMessage: &message} }

func TestPhase(t *testing.T) {
	d := door{addr: "127.0.0.1:1", gate: gate{user: "portal", password: "p"}}
	up := ready{"127.0.0.1:1", "portal", "p"}
	for _, c := range []struct {
		name  string
		notes []ipn.Notify
		want  phase
	}{
		{"nothing said yet is connecting", nil, pending{"Connecting to the tailnet"}},
		{"a link before the state is a sign-in", []ipn.Notify{browse("https://login/a"), note(ipn.NeedsLogin)}, login{"https://login/a"}},
		{"a link after the state is a sign-in", []ipn.Notify{note(ipn.NeedsLogin), browse("https://login/a")}, login{"https://login/a"}},
		{"the latest link is the one", []ipn.Notify{note(ipn.NeedsLogin), browse("https://login/a"), browse("https://login/b")}, login{"https://login/b"}},
		{"running is ready", []ipn.Notify{note(ipn.NeedsLogin), browse("https://login/a"), note(ipn.Running)}, up},
		{"running spends the link", []ipn.Notify{browse("https://login/a"), note(ipn.Running), note(ipn.NeedsLogin)}, pending{"Asking Tailscale for a sign-in link"}},
		{"an error is down", []ipn.Notify{note(ipn.Running), failed("control: 401")}, down{"control: 401"}},
		{"a state after an error supersedes it", []ipn.Notify{failed("control: 401"), note(ipn.Running)}, up},
		{"an admin's approval is pending", []ipn.Notify{note(ipn.NeedsMachineAuth)}, pending{"Waiting for the tailnet's admin to approve this device"}},
	} {
		t.Run(c.name, func(t *testing.T) {
			var r reading
			for _, n := range c.notes {
				r = r.with(n)
			}
			if got := r.phase(d); got != c.want {
				t.Fatalf("got %#v, want %#v", got, c.want)
			}
		})
	}
}

// TestReadingLaws: a reading is a product of latest-write registers, so a
// notification said twice is said once, and what it reads as after a run of
// notifications depends on that run alone, from any reading it starts at.
func TestReadingLaws(t *testing.T) {
	states := []ipn.State{ipn.NoState, ipn.NeedsLogin, ipn.NeedsMachineAuth, ipn.Stopped, ipn.Starting, ipn.Running}
	gen := rand.New(rand.NewPCG(1, 2))
	notify := func() ipn.Notify {
		var n ipn.Notify
		if gen.IntN(2) == 0 {
			s := states[gen.IntN(len(states))]
			n.State = &s
		}
		if gen.IntN(3) == 0 {
			u := "https://login/" + strconv.Itoa(gen.IntN(3))
			n.BrowseToURL = &u
		}
		if gen.IntN(4) == 0 {
			e := "error " + strconv.Itoa(gen.IntN(3))
			n.ErrMessage = &e
		}
		return n
	}
	d := door{addr: "127.0.0.1:1", gate: gate{user: "portal", password: "p"}}
	for range 2000 {
		var r reading
		for range gen.IntN(6) {
			r = r.with(notify())
		}
		n := notify()
		if once, twice := r.with(n), r.with(n).with(n); once != twice {
			t.Fatalf("said twice %+v, once %+v", twice, once)
		}
		// A run that ends in a state, a link and an error forgets the start.
		s, u, e := states[gen.IntN(len(states))], "https://login/x", "error x"
		run := []ipn.Notify{{State: &s}, {BrowseToURL: &u}, {ErrMessage: &e}}
		from, fresh := r, reading{}
		for _, n := range run {
			from, fresh = from.with(n), fresh.with(n)
		}
		if from.phase(d) != fresh.phase(d) {
			t.Fatalf("after %+v: %#v from %+v, %#v from nothing", run, from.phase(d), r, fresh.phase(d))
		}
	}
}
