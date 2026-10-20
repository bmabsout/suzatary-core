/*
 * PUSH WITHOUT ANYTHING RUNNING IN THE BACKGROUND.
 *
 * The owner, 2026-09-06: "I want our app to create notifications but properly and
 * not dependent on background running." On Android the only thing that can wake
 * a stopped app is Google's own push service, so this app registers with it and
 * then holds no thread, no service, no alarm and no socket. The box sends a
 * message to Google; Google wakes the process; PushReceiver draws a
 * notification; the process goes away again. Between two pushes this app costs
 * nothing at all, which is exactly what ntfy bought before (suzatary/notify.py)
 * at the price of a second app on the phone.
 *
 * NO FIREBASE SDK. firebase-messaging is thirty AARs resolved from Maven at
 * build time, and this APK is a pure derivation built by aapt2/javac/d8 with no
 * Gradle and no network (nix/apk.nix) — vendoring that tree would trade the
 * whole reproducibility argument for a convenience wrapper. What the SDK
 * actually does underneath is send ONE intent to Play Services and read ONE
 * broadcast back, and that is what this file does. The intent is the legacy
 * C2DM/GCM registration protocol, which FCM still speaks because every app
 * built before 2016 depends on it.
 *
 * THE PROTOCOL IS REVERSE-DOCUMENTED, NOT SPECIFIED. Google publishes the SDK,
 * not the wire format; every extra below is named with the source of the claim
 * and how confident it is. If a device answers with no token the app simply has
 * no push — it logs that and carries on, because a portal that crashed over a
 * notification would be worse than a portal with no notifications.
 *
 * IT CARRIES NO CREDENTIAL EITHER. The token is not a secret about the owner: it
 * is a name Google will deliver to, useful only to whoever holds the FCM key on
 * the box. It is reported over the same tailnet address and the same CSRF gate
 * as every other write (`POST /api/act/device`), so the server learns the
 * device from a request it already knows is the owner's phone — the actor is still
 * `tailscale whois`'s answer and never a field of the body.
 */
package com.bmabsout.suzatary;

import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.util.UUID;

import javax.net.ssl.HttpsURLConnection;

final class Push {

    static final String TAG = "SuzataryPush";

    /**
     * The Suzatary Firebase project's NUMBER, under the deployment's own
     * Google account. It is `FCM_SENDER_ID` in secretspec beside
     * the sending key, and it is not a secret: it names who may send to this
     * app, and only the holder of the service-account key can actually send.
     * The build spells it (nix/apk.nix's `portal.sender`).
     */
    private static final String SENDER = "@PORTAL_SENDER@";

    /** Play Services. The one package that answers the registration intent. */
    private static final String GMS = "com.google.android.gms";

    static final String ACTION_REGISTER = "com.google.android.c2dm.intent.REGISTER";
    static final String ACTION_REGISTRATION = "com.google.android.c2dm.intent.REGISTRATION";
    static final String ACTION_RECEIVE = "com.google.android.c2dm.intent.RECEIVE";

    /**
     * A request id GMS echoes back, so a reply can be matched to the request
     * that asked for it. InstanceID's own format is `|ID|<n>|`; this app has
     * exactly one outstanding request ever, so n is 1. CONFIDENCE: MEDIUM —
     * the shape is from decompiled `com.google.android.gms.iid`, and the reply
     * parser below treats the prefix as optional precisely because of that.
     */
    private static final String KID = "|ID|1|";

    private static final String PREFS = "push";
    private static final String KEY_TOKEN = "token";
    private static final String KEY_SENDER = "sender";
    private static final String KEY_REPORTED = "reported_at";
    private static final String KEY_NONCE = "pending_nonce";

    /**
     * How long a reported token is believed. FCM tokens do not expire on a
     * schedule, but they are revoked when the app's data is cleared, when it is
     * restored onto another device, or when Google decides to rotate one — and
     * every one of those is silent. Re-asking weekly is the cheap half of that
     * problem; the other half is the server noticing a token it sends to has
     * gone stale, which is the server's business and not this app's.
     */
    private static final long FRESH_MS = 7L * 24L * 60L * 60L * 1000L;

    /** The route the main session writes; the CSRF triple is `check_act_headers`. */
    private static final String ENDPOINT = MainActivity.HTTPS + "api/act/device";

    private Push() {
    }

    private static SharedPreferences prefs(Context ctx) {
        return ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    /**
     * Ask for a token unless the one on file was reported recently. Called off
     * the main thread from MainActivity.onCreate — SharedPreferences is disk
     * and startService is IPC, and neither belongs on the frame clock.
     *
     * NOTHING HERE BLOCKS ON AN ANSWER. GMS replies with a broadcast whenever
     * it is ready, which may be after this process has gone; that is the whole
     * point of the design and the reason there is no future, callback or
     * timeout in this file.
     */
    static void ensure(Context ctx) {
        try {
            SharedPreferences prefs = prefs(ctx);
            boolean sameSender = SENDER.equals(prefs.getString(KEY_SENDER, ""));
            boolean haveToken = !prefs.getString(KEY_TOKEN, "").isEmpty();
            long since = System.currentTimeMillis() - prefs.getLong(KEY_REPORTED, 0L);
            if (sameSender && haveToken && since >= 0 && since < FRESH_MS) {
                return;
            }
            register(ctx);
        } catch (Throwable e) {
            // No push is a degraded portal; a crash is a broken one.
            Log.w(TAG, "registration could not be started", e);
        }
    }

    /**
     * The registration intent, extra by extra. SOURCES, since none of this is
     * documented by Google:
     *
     * - action REGISTER, package com.google.android.gms, extra `sender`, and
     *   the REGISTRATION broadcast carrying `registration_id`: the original
     *   C2DM/GCM protocol, from the pre-2016 `gcm.jar` helper and independently
     *   reimplemented by microG's PushRegisterService. CONFIDENCE: HIGH.
     * - extra `app`, a PendingIntent: how GMS learns the CALLER's package
     *   without trusting a string in the body — it reads the creator package
     *   off the token. Same source. CONFIDENCE: HIGH. It must be IMMUTABLE:
     *   API 34 refuses a mutable PendingIntent wrapping an implicit intent, and
     *   this one is deliberately empty because it is an identity, never
     *   something to send.
     * - extras `subtype` / `X-subtype`: InstanceID's own bundle. `subtype` is
     *   the sender id for a plain FCM app, and GMS treats an `X-`-prefixed
     *   extra as a message header when the request travels over the legacy
     *   intent transport rather than the Messenger one. CONFIDENCE: MEDIUM —
     *   from decompiled `com.google.android.gms.iid`, not from any spec. A
     *   wrong or ignored extra here costs nothing: GMS ignores what it does not
     *   read.
     * - extra `scope`: "*" is what FirebaseInstanceId asks for (the constant
     *   "GCM" is the older, narrower scope). CONFIDENCE: MEDIUM-HIGH. If a
     *   device ever answers nothing at all, "FCM" and "GCM" are the two other
     *   values worth trying before concluding the protocol has moved.
     *
     * TRANSPORT. GMS declares both a service and a receiver for this action.
     * startService is the SDK's first choice; it returns null when no service
     * matched and throws on API 26+ if the app were in the background, so the
     * broadcast is the fallback for both. Sending it twice would at worst make
     * GMS answer twice with the same token, which the receiver handles.
     */
    private static void register(Context ctx) {
        Intent i = new Intent(ACTION_REGISTER);
        i.setPackage(GMS);
        i.putExtra("sender", SENDER);
        i.putExtra("subtype", SENDER);
        i.putExtra("X-subtype", SENDER);
        i.putExtra("scope", "*");
        i.putExtra("kid", KID);
        i.putExtra("app", PendingIntent.getBroadcast(ctx, 0, new Intent(), PendingIntent.FLAG_IMMUTABLE));
        boolean started = false;
        try {
            started = ctx.startService(i) != null;
        } catch (IllegalStateException | SecurityException e) {
            Log.i(TAG, "startService refused; broadcasting instead: " + e.getMessage());
        }
        if (!started) {
            ctx.sendBroadcast(i);
        }
        Log.i(TAG, "asked Play Services for a token (service=" + started + ")");
    }

    /**
     * The reply. `registration_id` is the token, optionally prefixed with the
     * `kid` we sent and a colon, and an error is delivered in that same field
     * as `error=REASON` as well as in an `error` extra. CONFIDENCE: MEDIUM on
     * the prefix and the inline error, which is why this parses defensively
     * rather than asserting a shape.
     */
    static String tokenOf(Intent intent) {
        String error = intent.getStringExtra("error");
        if (error != null && !error.isEmpty()) {
            Log.w(TAG, "Play Services refused registration: " + error);
            return "";
        }
        String raw = intent.getStringExtra("registration_id");
        if (raw == null || raw.isEmpty()) {
            return "";
        }
        if (raw.startsWith("|")) {
            int mark = raw.indexOf("|:");
            if (mark < 0) {
                return "";
            }
            raw = raw.substring(mark + 2);
        }
        if (raw.startsWith("error=")) {
            Log.w(TAG, "Play Services refused registration: " + raw.substring(6));
            return "";
        }
        return raw;
    }

    /**
     * Store the token and tell the box. Runs on PushReceiver's worker thread.
     * The token is written FIRST and `reported_at` only on a 2xx, so a failed
     * report is retried on the next app start rather than forgotten — which is
     * the same shape as the web app's outbox (web/src/queue.ts): the intent
     * survives the failure and nothing is invented to cover it.
     */
    static void accept(Context ctx, String token) {
        SharedPreferences prefs = prefs(ctx);
        prefs.edit().putString(KEY_TOKEN, token).putString(KEY_SENDER, SENDER).apply();
        String nonce = prefs.getString(KEY_NONCE, "");
        if (nonce.isEmpty()) {
            // A NONCE PER ATTEMPT, NOT PER REQUEST. It is what makes a retry
            // idempotent (suzatary/nonces.py), so the retry of a failed report
            // must carry the SAME one; a re-registration a week later is a new
            // fact about the device and gets a new one.
            nonce = UUID.randomUUID().toString();
            prefs.edit().putString(KEY_NONCE, nonce).apply();
        }
        try {
            if (report(token, nonce)) {
                prefs.edit().putLong(KEY_REPORTED, System.currentTimeMillis()).remove(KEY_NONCE).apply();
                Log.i(TAG, "the box has this device's token");
            }
        } catch (IOException | JSONException e) {
            Log.w(TAG, "could not report the token; will retry next start", e);
        }
    }

    /**
     * HTTPS ONLY, like the WebView: a token posted in the clear is invisible,
     * so this waits for the certificate instead of downgrading. The certificate is a real Let's Encrypt one for the ts.net
     * name, so the platform's own trust store validates it and this class adds
     * no TrustManager of its own.
     */
    private static boolean report(String token, String nonce) throws IOException, JSONException {
        JSONObject body = new JSONObject();
        body.put("platform", "android");
        body.put("token", token);
        body.put("nonce", nonce);
        byte[] bytes = body.toString().getBytes("UTF-8");

        HttpsURLConnection c = (HttpsURLConnection) new URL(ENDPOINT).openConnection();
        try {
            c.setRequestMethod("POST");
            c.setConnectTimeout(15000);
            c.setReadTimeout(15000);
            c.setDoOutput(true);
            c.setFixedLengthStreamingMode(bytes.length);
            // THE CSRF TRIPLE (webapp.check_act_headers). The custom header is
            // the half whois cannot cover; Sec-Fetch-Site is a browser's word
            // and absent here, which the server allows precisely because a
            // native client is not a page some other origin can drive.
            c.setRequestProperty("X-Suzatary-Act", "1");
            c.setRequestProperty("Content-Type", "application/json");
            c.setRequestProperty("Accept", "application/json");
            OutputStream out = c.getOutputStream();
            try {
                out.write(bytes);
            } finally {
                out.close();
            }
            int code = c.getResponseCode();
            if (code / 100 == 2) {
                return true;
            }
            Log.w(TAG, "the box refused the token: " + code + " " + read(c.getErrorStream()));
            return false;
        } finally {
            c.disconnect();
        }
    }

    /** A capped read of a refusal, for the log. Never parsed, never displayed. */
    private static String read(InputStream in) throws IOException {
        if (in == null) {
            return "";
        }
        try {
            byte[] buffer = new byte[512];
            int got = 0;
            int n;
            while (got < buffer.length && (n = in.read(buffer, got, buffer.length - got)) > 0) {
                got += n;
            }
            return new String(buffer, 0, got, "UTF-8");
        } finally {
            in.close();
        }
    }
}
