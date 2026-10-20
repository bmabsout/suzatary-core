package node

import (
	"context"
	"crypto/subtle"
	"encoding/base64"
	"io"
	"net"
	"net/http"
	"time"
)

// gate is who the proxy admits and where it lets them go. Loopback is every
// app's on a phone, so the password is what makes the proxy the portal's: a
// fresh one each session, asked by Basic auth, which only the portal's
// WebView answers. The one place it goes is the box, on 443, so TLS runs end
// to end between the WebView and the box's own certificate.
type gate struct {
	box      string
	user     string
	password string
}

func (g gate) admits(r *http.Request) bool {
	want := "Basic " + base64.StdEncoding.EncodeToString([]byte(g.user+":"+g.password))
	got := r.Header.Get("Proxy-Authorization")
	return subtle.ConstantTimeCompare([]byte(got), []byte(want)) == 1
}

func (g gate) allows(r *http.Request) bool {
	return r.Method == http.MethodConnect && r.Host == net.JoinHostPort(g.box, "443")
}

// proxy answers CONNECT for the gate's one destination by dialling it over
// the tailnet and splicing the two connections; it never reads what flows
// through them.
type proxy struct {
	gate gate
	dial func(ctx context.Context, network, addr string) (net.Conn, error)
}

func (p proxy) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if !p.gate.admits(r) {
		w.Header().Set("Proxy-Authenticate", `Basic realm="suzatary"`)
		http.Error(w, "this proxy is the Suzatary portal's", http.StatusProxyAuthRequired)
		return
	}
	if !p.gate.allows(r) {
		http.Error(w, "the portal reaches "+p.gate.box+" over https and nothing else", http.StatusForbidden)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()
	far, err := p.dial(ctx, "tcp", r.Host)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	near, buffered, err := http.NewResponseController(w).Hijack()
	if err != nil {
		far.Close()
		return
	}
	if _, err := near.Write([]byte("HTTP/1.1 200 Connection established\r\n\r\n")); err != nil {
		near.Close()
		far.Close()
		return
	}
	// What the client sent after its CONNECT is in the hijacked buffer, whose
	// reader goes on into the connection itself. Either side ending ends both.
	go func() {
		io.Copy(far, buffered)
		far.Close()
		near.Close()
	}()
	io.Copy(near, far)
	near.Close()
	far.Close()
}
