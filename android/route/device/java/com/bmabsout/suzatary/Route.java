package com.bmabsout.suzatary;

import android.app.Activity;
import android.webkit.HttpAuthHandler;
import android.widget.FrameLayout;

/**
 * HOW THE WEBVIEW REACHES THE BOX: the phone's own network. The Tailscale app's
 * VPN puts the phone on the tailnet, so the box is reached the moment the app
 * opens and there is nothing to start, stop or answer.
 *
 * nix/apk.nix builds the portal with one Route of android/route/; this is the
 * portal's, and android/route/tsnet has the other.
 */
final class Route {

    private final Runnable reached;

    Route(Activity activity, FrameLayout root, Runnable reached) {
        this.reached = reached;
    }

    /** The activity is in the foreground; `reached` runs once the box is reachable. */
    void open() {
        reached.run();
    }

    /** The activity left the foreground. */
    void close() {}

    /** Whether this route answered a request for credentials; none here. */
    boolean answers(HttpAuthHandler handler, String host) {
        return false;
    }
}
