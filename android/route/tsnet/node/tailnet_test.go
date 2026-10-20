package node

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"io"
	"math/big"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"tailscale.com/tsnet"
)

// TestTailnet runs the portal's node against a real control server: headscale
// on loopback, a second tsnet node standing in for the box and serving https
// under its MagicDNS name, and the proxy carrying a client there. It needs a
// headscale binary, named by SUZATARY_HEADSCALE; SUZATARY_BROWSER, if set, is
// a command run with the proxy, its user and password and the box's URL, which
// must print the page (testdata/chromium.mjs drives Chromium so).
func TestTailnet(t *testing.T) {
	headscale := os.Getenv("SUZATARY_HEADSCALE")
	if headscale == "" {
		t.Skip("SUZATARY_HEADSCALE names no headscale binary")
	}
	dir := t.TempDir()
	hs := startHeadscale(t, headscale, dir)
	control, authKey := hs.control, hs.authKey
	t.Setenv("TS_CONTROL_URL", control)
	t.Setenv("TS_AUTHKEY", authKey)

	const page = "the box answers over the tailnet"
	fqdn := "box.suzatary.test"
	pool := serveBox(t, dir, control, authKey, fqdn, page)

	phases := make(chan string, 16)
	if err := Start(filepath.Join(dir, "portal"), "portal", fqdn, told(phases)); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(Stop)
	var proxy, user, password string
	deadline := time.After(60 * time.Second)
	for proxy == "" {
		select {
		case p := <-phases:
			t.Log("phase:", p)
			if f := strings.Fields(p); f[0] == "ready" {
				proxy, user, password = f[1], f[2], f[3]
			}
		case <-deadline:
			t.Fatal("the portal's node never became ready")
		}
	}

	target := "https://" + fqdn + "/"
	via := func(user, password string) (*http.Response, error) {
		c := &http.Client{Transport: &http.Transport{
			Proxy:           http.ProxyURL(&url.URL{Scheme: "http", Host: proxy, User: url.UserPassword(user, password)}),
			TLSClientConfig: &tls.Config{RootCAs: pool},
		}, Timeout: 30 * time.Second}
		return c.Get(target)
	}
	if _, err := via(user, "guess"); err == nil || !strings.Contains(err.Error(), "Proxy Authentication Required") {
		t.Fatalf("a wrong password: %v", err)
	}
	resp, err := via(user, password)
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	if string(body) != page {
		t.Fatalf("the box said %q", body)
	}

	if browser := os.Getenv("SUZATARY_BROWSER"); browser != "" {
		out, err := exec.Command(browser, proxy, user, password, target).CombinedOutput()
		t.Logf("browser: %s", out)
		if err != nil || !strings.Contains(string(out), page) {
			t.Fatalf("the browser did not reach the box: %v", err)
		}
	}

	// The app leaves the foreground and comes back: a request made while the
	// node is stopped waits for the next one, at the same proxy.
	Stop()
	waited := make(chan error, 1)
	go func() {
		resp, err := via(user, password)
		if err == nil {
			resp.Body.Close()
		}
		waited <- err
	}()
	time.Sleep(2 * time.Second)
	if err := Start(filepath.Join(dir, "portal"), "portal", fqdn, told(phases)); err != nil {
		t.Fatal(err)
	}
	if err := <-waited; err != nil {
		t.Fatalf("a request across a restart: %v", err)
	}
	for again := ""; !strings.HasPrefix(again, "ready "); {
		again = <-phases
		t.Log("phase:", again)
		if f := strings.Fields(again); f[0] == "ready" && f[1] != proxy {
			t.Fatalf("the proxy moved from %s to %s", proxy, f[1])
		}
	}

	// Removed from the tailnet, the node reaches nothing. Headscale answers a
	// removed node's polls with 404 and its backend stays Running, so no
	// sign-in is asked for; what the person sees is the page's own error.
	hs.run("nodes", "delete", "--identifier", fmt.Sprint(hs.node("portal")), "--force")
	gone := time.Now().Add(60 * time.Second)
	for {
		if _, err := via(user, password); err != nil {
			t.Log("removed, the proxy answers:", err)
			break
		}
		if time.Now().After(gone) {
			t.Fatal("a removed node still reached the box")
		}
		time.Sleep(time.Second)
	}
	for len(phases) > 0 {
		t.Log("phase:", <-phases)
	}
}

// told is a Reach that writes each case as a line to phases.
type told chan string

func (c told) Pending(why string)             { c <- "pending " + why }
func (c told) Login(url string)               { c <- "login " + url }
func (c told) Ready(proxy, user, pass string) { c <- "ready " + proxy + " " + user + " " + pass }
func (c told) Down(why string)                { c <- "down " + why }

// headscale is a control server on loopback, its CLI, and a reusable key.
type headscale struct {
	t       *testing.T
	bin     string
	config  string
	control string
	authKey string
}

func (h headscale) run(args ...string) []byte {
	var out []byte
	var err error
	for range 50 {
		out, err = exec.Command(h.bin, append(args, "--config", h.config, "--output", "json")...).Output()
		if err == nil {
			return out
		}
		time.Sleep(200 * time.Millisecond)
	}
	h.t.Fatalf("headscale %v: %v", args, err)
	return nil
}

// node is the identifier of the node named name.
func (h headscale) node(name string) uint64 {
	var nodes []struct {
		ID   uint64
		Name string
	}
	if err := json.Unmarshal(h.run("nodes", "list"), &nodes); err != nil {
		h.t.Fatal(err)
	}
	for _, n := range nodes {
		if n.Name == name {
			return n.ID
		}
	}
	h.t.Fatalf("no node %q in %+v", name, nodes)
	return 0
}

func startHeadscale(t *testing.T, bin, dir string) headscale {
	port := func() int {
		ln, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			t.Fatal(err)
		}
		defer ln.Close()
		return ln.Addr().(*net.TCPAddr).Port
	}
	hs := filepath.Join(dir, "headscale")
	if err := os.MkdirAll(hs, 0o700); err != nil {
		t.Fatal(err)
	}
	listen := fmt.Sprintf("127.0.0.1:%d", port())
	config := fmt.Sprintf(`server_url: http://%[1]s
listen_addr: %[1]s
metrics_listen_addr: 127.0.0.1:%[2]d
grpc_listen_addr: 127.0.0.1:%[3]d
noise: {private_key_path: %[4]s/noise.key}
prefixes: {v4: 100.64.0.0/10, v6: "fd7a:115c:a1e0::/48", allocation: sequential}
derp:
  server:
    enabled: true
    region_id: 999
    region_code: local
    region_name: local
    stun_listen_addr: 127.0.0.1:%[5]d
    private_key_path: %[4]s/derp.key
    automatically_add_embedded_derp_region: true
    ipv4: 127.0.0.1
  urls: []
  paths: []
  auto_update_enabled: false
disable_check_updates: true
database: {type: sqlite, sqlite: {path: %[4]s/db.sqlite}}
dns: {magic_dns: true, base_domain: suzatary.test, override_local_dns: false, nameservers: {global: []}}
unix_socket: %[4]s/headscale.sock
unix_socket_permission: "0770"
log: {level: warn}
`, listen, port(), port(), hs, port())
	cfg := filepath.Join(hs, "config.yaml")
	if err := os.WriteFile(cfg, []byte(config), 0o600); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	serve := exec.CommandContext(ctx, bin, "serve", "--config", cfg)
	serve.Stdout, serve.Stderr = os.Stderr, os.Stderr
	if err := serve.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { cancel(); serve.Wait() })
	h := headscale{t: t, bin: bin, config: cfg, control: "http://" + listen}
	var user struct{ ID uint64 }
	if err := json.Unmarshal(h.run("users", "create", "owner"), &user); err != nil {
		t.Fatal(err)
	}
	var key struct{ Key string }
	if err := json.Unmarshal(h.run("preauthkeys", "create", "--user", fmt.Sprint(user.ID), "--reusable"), &key); err != nil {
		t.Fatal(err)
	}
	h.authKey = key.Key
	return h
}

// serveBox starts the box's stand-in, answering page on 443 under a
// certificate for fqdn that only the returned pool trusts.
func serveBox(t *testing.T, dir, control, authKey, fqdn, page string) *x509.CertPool {
	box := &tsnet.Server{Dir: filepath.Join(dir, "box"), Hostname: "box", ControlURL: control, AuthKey: authKey, Logf: func(string, ...any) {}}
	t.Cleanup(func() { box.Close() })
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	if _, err := box.Up(ctx); err != nil {
		t.Fatal(err)
	}
	ln, err := box.Listen("tcp", ":443")
	if err != nil {
		t.Fatal(err)
	}
	cert, pool := selfSigned(t, fqdn)
	go http.Serve(tls.NewListener(ln, &tls.Config{Certificates: []tls.Certificate{cert}}),
		http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { io.WriteString(w, page) }))
	return pool
}

func selfSigned(t *testing.T, host string) (tls.Certificate, *x509.CertPool) {
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	tmpl := &x509.Certificate{
		SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: host}, DNSNames: []string{host},
		NotBefore: time.Now().Add(-time.Hour), NotAfter: time.Now().Add(time.Hour),
		IsCA: true, BasicConstraintsValid: true, KeyUsage: x509.KeyUsageDigitalSignature | x509.KeyUsageCertSign,
		ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, tmpl, &key.PublicKey, key)
	if err != nil {
		t.Fatal(err)
	}
	parsed, _ := x509.ParseCertificate(der)
	pool := x509.NewCertPool()
	pool.AddCert(parsed)
	keyDER, _ := x509.MarshalECPrivateKey(key)
	cert, err := tls.X509KeyPair(
		pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der}),
		pem.EncodeToMemory(&pem.Block{Type: "EC PRIVATE KEY", Bytes: keyDER}))
	if err != nil {
		t.Fatal(err)
	}
	return cert, pool
}
