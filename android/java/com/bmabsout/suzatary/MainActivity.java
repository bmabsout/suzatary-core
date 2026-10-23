/*
 * SUZATARY ON ANDROID IS A PORTAL, NOT A SECOND CLIENT.
 *
 * The API is the product (suzatary/webapp.py) and web/ is its one client. This
 * app opens that client at the box's tailnet address and then gets out of the
 * way: it holds no fold, no fulfillment, no chain verifier and no HTML. Every
 * one of those already exists exactly once, and a Java copy of any of them
 * would be the duplicate-evaluator failure the repo has already retired twice
 * (typst's evalSpec, 2026-09-02; the browser never computing a fulfillment).
 *
 * IDENTITY IS THE TAILNET'S. The app carries no credential and no token. The
 * server admits a peer because `tailscale whois` names its Tailscale login
 * (instance.WEB_VIEWERS / WEB_ACTORS), so a request from this WebView and a
 * request from Chrome on the same phone are the same actor, and removing the
 * device from the tailnet removes this app's access with it. Nothing here
 * needs revoking separately, because nothing here was ever granted separately.
 * HOW the WebView reaches the tailnet is a Route (android/route/): the phone's
 * own network, or a tailnet node inside this app; either way the box sees a
 * device of the owner's login.
 *
 * WHEN NATIVE WOULD BE EARNED: only OS integration a browser cannot do — a
 * share-sheet target that files a todo, a home-screen widget, a notification on
 * a tip move. Each of those is a native SHELL over the same HTTP API. None of
 * them is a reason to reimplement the semantics on this side.
 *
 * THE FIRST OF THOSE ARRIVED AS DEEP LINKS (2026-09-05). Being the thing that
 * opens https://<your-box>.<tailnet>.ts.net/#/todo/<id> is exactly OS integration
 * a browser cannot grant itself, and it stays a shell: the intent's URI is not
 * parsed into a todo id, a route or a fetch — it is handed to the WebView
 * whole, fragment and all, and the web app's own router reads it. This class
 * still knows nothing about what a todo is.
 *
 * THE SHARE TARGET IS THE SAME TRICK POINTED THE OTHER WAY (2026-09-10).
 * Sharing a link or a sentence from any app on the phone opens this one on the
 * add form with the text in it — OS integration a browser cannot grant itself,
 * and still a shell: what this class does with a share is spell it as
 * `#/add?text=…&title=…` and hand the WebView that URL. It does not parse a
 * URL out of the text, does not decide what a todo's body is, and posts
 * nothing. The write is the one every other todo takes — `POST /api/author`
 * builds the event, the replica in the page seals it — because a second
 * authoring path is a second set of rules about what a todo is.
 *
 * AND THE ROUTE IS IN THE FRAGMENT, for the same reason the deep links are: a
 * fragment never reaches a server, so what the owner shared is not something the
 * box has been told, and no intent filter can match on it either. The text is
 * carried as TEXT — capped, URL-encoded once, never composed into markup or
 * script — so there is no path by which a shared string is parsed as anything.
 *
 * THE BOX IS THE PROFILE'S OR THE PERSON'S (Box). An APK whose profile names
 * a box opens it at once; the generic APK shows the pairing screen first, and
 * keeps the name only once that box has answered Pairing. Until then nothing
 * loads, no route opens and no push is asked for.
 *
 * THIS CLASS AUTHORS NO HTML. webapp.py serves plain text and JSON and lets the
 * one bundle do the rendering; the same rule one client further out means the
 * failure screen below is Android widgets, not a string of markup handed to the
 * WebView. There is therefore no path by which anything this app composes gets
 * parsed as a document.
 */
package com.bmabsout.suzatary;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.Uri;
import android.net.http.SslError;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.util.Log;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup.LayoutParams;
import android.webkit.HttpAuthHandler;
import android.webkit.SslErrorHandler;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebMessage;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.IOException;

public final class MainActivity extends Activity {

    /**
     * The unverifiable half of the link surface (AndroidManifest.xml):
     * `suzatary://todo/<id>` is the same page as the box's home page plus
     * "#/todo/<id>", for the places that cannot linkify a tailnet name. The mapping lives here
     * and nowhere else, so the scheme cannot come to mean a second thing.
     */
    private static final String SCHEME = "suzatary";

    /**
     * The add form, as an address under the box's home page
     * (`web/src/shell/route.ts`'s `add` arm). Written here and nowhere else,
     * like the `suzatary://` mapping above and for the same reason: a second
     * spelling of a route is a second route.
     */
    private static final String ADD = "#/add";

    /**
     * How much shared text this app will carry into a URL, and how much of a
     * subject.
     *
     * CUT, NOT REFUSED. A share is a link or a sentence; somebody sharing a
     * whole article is not something a todo's action line can hold, and the
     * useful thing to do with the overflow is drop it and put the sheet on
     * screen with what fits — the owner finishes it there, with a keyboard. The
     * cap is also what keeps a hostile app from handing this one a megabyte to
     * percent-encode. The same reasoning and the same shape as
     * PushReceiver.MAX_TEXT, one door over.
     */
    private static final int MAX_SHARED = 1000;

    private static final int MAX_SUBJECT = 200;

    private WebView web;
    private View offline;
    private TextView reason;
    private FrameLayout root;

    /** The box this app opens; null while the generic APK shows the pairing screen. */
    private Box box;

    /** The pairing screen, or null once the app has a box. */
    private View pairing;
    private EditText typed;
    private TextView said;
    private Button connect;

    /** Between onStart and onStop: a box paired then opens its route at once. */
    private boolean foreground;

    /**
     * What load() last asked for, https and fragment and all. The retry button
     * reloads THIS rather than the URL the error reports,
     * because a fragment is never sent to the server and so never comes back
     * in an error — and the fragment is the whole address of a deep link.
     *
     * A restored WebView (restoreState) does not go through load(), so after a
     * process death this reads the home page while the screen shows wherever
     * the history left off. That is the behaviour this app had before deep
     * links existed, and it costs one page rather than a wrong page.
     */
    private String current;

    /** The way to the box (android/route/), opened while the activity is in the foreground. */
    private Route route;

    /**
     * The first page — a link, a restored history, or home — until the route
     * reaches the box; null once it has been shown. Nothing loads before then,
     * since a page asked for over a route that is not up is a page that fails.
     */
    private Runnable first;

    /** The request code of the one file chooser this app starts. */
    private static final int CHOOSE_FILES = 2;

    /**
     * The page waiting on the system picker, or null. It is answered exactly
     * once on every path, cancel included: an unanswered callback leaves the
     * page's file input dead until the WebView is rebuilt.
     */
    private ValueCallback<Uri[]> choosing;

    /** Hands each token Play Services gives to the page; held here, since preferences hold listeners weakly. */
    private final SharedPreferences.OnSharedPreferenceChangeListener tokens =
            new SharedPreferences.OnSharedPreferenceChangeListener() {
                @Override
                public void onSharedPreferenceChanged(SharedPreferences prefs, String key) {
                    if (Push.KEY_TOKEN.equals(key)) {
                        handToken();
                    }
                }
            };

    @Override
    protected void onCreate(final Bundle saved) {
        super.onCreate(saved);

        web = new WebView(this);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        // localStorage IS the app's offline cache (web/src/app/cache.ts): the last
        // fold, slim and dated. Without DOM storage the app has no local-first
        // half at all and a dropped tailnet is a blank screen.
        s.setDomStorageEnabled(true);
        // Nothing in web/ reads a file or a content:// URI, so the WebView may
        // not either. Narrowing a capability nobody uses costs nothing.
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        s.setMediaPlaybackRequiresUserGesture(true);
        web.setWebViewClient(new Portal());
        web.setWebChromeClient(new Chooser());

        offline = buildOfflineView();
        offline.setVisibility(View.GONE);

        root = new FrameLayout(this);
        root.addView(web, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        root.addView(offline, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        setContentView(root);

        box = Box.of(this);
        if (box == null) {
            pairing = buildPairingView();
            root.addView(pairing, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            web.setVisibility(View.GONE);
        } else {
            begin(saved);
        }
        Push.kept(this).registerOnSharedPreferenceChangeListener(tokens);
        askToNotify();
    }

    @Override
    protected void onDestroy() {
        Push.kept(this).unregisterOnSharedPreferenceChangeListener(tokens);
        super.onDestroy();
    }

    /**
     * THE TOKEN GOES TO THE PAGE, which reports it signed: this app holds no
     * device key. postWebMessage delivers only to a main frame at the box's
     * origin, so no other page reads the token, and no JavaScript interface
     * is exposed. Before a box is kept there is no origin to hand it to.
     */
    private void handToken() {
        String token = Push.token(this);
        if (box == null || token == null) {
            return;
        }
        try {
            String said = new JSONObject().put("pushToken", token).toString();
            web.postWebMessage(new WebMessage(said), Uri.parse("https://" + box.host));
        } catch (JSONException e) {
            Log.w(Push.TAG, "the token could not be written for the page", e);
        }
    }

    /** The box is known: the first page waits for the route, and push is asked for. */
    private void begin(final Bundle saved) {
        current = box.https();
        // A link the OS handed us wins over restored state: the process may
        // have been killed while the app sat on some other page, and the thing
        // the owner just tapped is what he asked for. With no link, a restore
        // keeps the page and the scroll; with neither, the app opens home.
        final String target = targetOf(getIntent());
        first = new Runnable() {
            @Override
            public void run() {
                if (target != null) {
                    load(target);
                } else if (saved != null) {
                    web.restoreState(saved);
                } else {
                    load(box.https());
                }
            }
        };
        route = new Route(this, root, box.host, new Runnable() {
            @Override
            public void run() {
                reached();
            }
        });

        // PUSH IS A STARTUP ERRAND, NOT A SERVICE. Opening the app is the only
        // moment this process is reliably alive, so it is when the token is
        // refreshed; between two launches nothing of ours runs at all. Off the
        // main thread because SharedPreferences is disk and startService is
        // IPC, and named so a stray trace says whose thread it was.
        new Thread(new Runnable() {
            @Override
            public void run() {
                Push.ensure(getApplicationContext());
            }
        }, "suzatary-register").start();
    }

    @Override
    protected void onStart() {
        super.onStart();
        foreground = true;
        if (route != null) {
            route.open();
        }
    }

    @Override
    protected void onStop() {
        foreground = false;
        if (route != null) {
            route.close();
        }
        super.onStop();
    }

    /**
     * Ask the typed box, off the main thread, and keep it once it answers.
     * The box is kept only from Pairing's answer, so a typo or a stranger's
     * name is never this app's box.
     */
    private void pair() {
        final String host = Box.hostOf(typed.getText());
        if (host == null) {
            said.setText(R.string.pairing_not_a_name);
            return;
        }
        said.setText(getString(R.string.pairing_asking, host));
        connect.setEnabled(false);
        new Thread(new Runnable() {
            @Override
            public void run() {
                Box answered = null;
                String why = "";
                try {
                    answered = Pairing.ask(host);
                    Box.keep(getApplicationContext(), answered);
                } catch (IOException e) {
                    why = String.valueOf(e.getMessage());
                }
                final Box kept = answered;
                final String refused = why;
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        paired(kept, refused);
                    }
                });
            }
        }, "suzatary-pairing").start();
    }

    /** Pairing's outcome, on the main thread: the box opens, or the screen says why not. */
    private void paired(Box answered, String refused) {
        if (isDestroyed()) {
            return;
        }
        connect.setEnabled(true);
        if (answered == null) {
            said.setText(refused);
            return;
        }
        box = answered;
        root.removeView(pairing);
        pairing = null;
        web.setVisibility(View.VISIBLE);
        begin(null);
        if (foreground) {
            route.open();
        }
    }

    /** The route reached the box; the first page, if it is still waiting, is shown now. */
    private void reached() {
        Runnable page = first;
        first = null;
        if (page != null) {
            page.run();
        }
    }

    /**
     * The runtime half of POST_NOTIFICATIONS (API 33+). Asked ONCE per launch
     * and only while it is unanswered; the OS itself stops showing the dialog
     * after two refusals, so there is no counter to keep here and no way for
     * this to become nagging.
     *
     * The system dialog says nothing about what the notifications are, so the
     * one line that does is shown BEFORE the second ask —
     * shouldShowRequestPermissionRationale is true exactly when the owner has
     * already said no once, which is the only moment a reason is worth reading.
     * A toast rather than a screen: this app authors no HTML and, one step
     * further, no UI it can avoid.
     *
     * The permission string is spelled out rather than taken from
     * Manifest.permission, which only exists from API 33 — a compile-time
     * constant would be inlined and work, but the literal is the same string
     * the manifest declares and can be grepped against it.
     */
    private void askToNotify() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return; // below 33 the manifest declaration is the whole of it
        }
        String permission = "android.permission.POST_NOTIFICATIONS";
        if (checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED) {
            return;
        }
        if (shouldShowRequestPermissionRationale(permission)) {
            Toast.makeText(this, R.string.push_rationale, Toast.LENGTH_LONG).show();
        }
        requestPermissions(new String[] {permission}, 1);
    }

    /**
     * A link tapped — OR A SHARE MADE — while the app is already running.
     * singleTask means this is the SAME activity and the same WebView, so the
     * page is loaded into what is already on screen. setIntent keeps
     * getIntent() honest for a later recreation, which is the only reason the
     * launch intent is read twice.
     *
     * A share arriving here is the COMMON case, not the exception: the app is
     * usually already open somewhere behind the sharer. It needs no arm of its
     * own because `targetOf` answers for both actions — one function decides
     * what an intent names, whether the process was just started or not, and
     * "a share is handled twice differently" is a bug this shape cannot have.
     */
    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        if (box == null) {
            return; // begin() reads the intent once the box has answered
        }
        final String target = targetOf(intent);
        if (target != null) {
            Runnable page = new Runnable() {
                @Override
                public void run() {
                    load(target);
                }
            };
            if (first != null) {
                first = page; // the route is not up yet
            } else {
                page.run();
            }
        }
    }

    /**
     * The page an incoming intent names, or null if it names none.
     *
     * An https URL for this host is loaded AS GIVEN — the fragment is where
     * `#/todo/<id>` lives and dropping it would land every link on the home
     * list. The custom scheme is rewritten: `suzatary://todo/<id>` is
     * authority "todo" and path "/<id>", which is exactly the route the web
     * app's hash router reads, so the two spellings are one page by
     * construction rather than by a table of cases. Encoded forms are used
     * throughout: an id that needed escaping must stay escaped.
     *
     * Any other host is refused rather than loaded. A filter this app declares
     * is not the only way an intent arrives here, and a WebView pointed at a
     * stranger's page by a stranger's intent is the one thing this class must
     * not do.
     */
    private String targetOf(Intent intent) {
        if (intent == null) {
            return null;
        }
        if (Intent.ACTION_SEND.equals(intent.getAction())) {
            return shared(intent);
        }
        if (!Intent.ACTION_VIEW.equals(intent.getAction())) {
            return null;
        }
        Uri data = intent.getData();
        if (data == null) {
            return null;
        }
        // An http link to this host is read as its https page: the app never asks for cleartext.
        if (box.holds(data)) {
            return over("https", data.toString());
        }
        if (!SCHEME.equals(data.getScheme())) {
            return null;
        }
        String authority = data.getEncodedAuthority();
        String path = data.getEncodedPath();
        String route = (authority == null ? "" : authority) + (path == null ? "" : path);
        while (route.startsWith("/")) {
            route = route.substring(1); // `suzatary:///todo/x` names the same page
        }
        return route.isEmpty() ? box.https() : box.https() + "#/" + route;
    }

    /**
     * A SHARE, SPELLED AS THE ADD FORM'S ADDRESS.
     *
     * `ACTION_SEND` carries two extras and they are not the same thing:
     * EXTRA_TEXT is what was shared — a URL from a browser, a selection from a
     * reader — and EXTRA_SUBJECT is the page's own title when the sharer had
     * one. Both travel as `#/add`'s query fields and `draft.sharedBody` on the
     * other side decides what they MEAN together, because "what a todo's body
     * is" is a rule the app has exactly one copy of and it is not in Java.
     *
     * getCharSequenceExtra AND NOT getStringExtra. EXTRA_TEXT is declared as a
     * CharSequence, and an app that shares styled text puts a Spanned in it —
     * getStringExtra returns null for that, so a naive read silently loses the
     * share and opens the home page. CONFIDENCE: HIGH; the extra's type is in
     * Android's own documentation, and toString() is the documented way to
     * take the characters out of a Spanned.
     *
     * Uri.encode ONCE, and with no allowed set: it percent-encodes everything
     * outside the unreserved characters, which is exactly `&`, `=`, `#`, `%`,
     * `?`, `/` and a space — every character that is structure in the URL this
     * builds. `decodeURIComponent` on the other side is its inverse
     * (`web/test/route.test.ts` round-trips a URL with all of them in it).
     * Encoding twice would put `%2520` in a todo, which is the failure this
     * comment exists to prevent someone "fixing" back in.
     *
     * A share with nothing in it opens the app, rather than an empty form: an
     * intent this app cannot read is not a reason to show a blank sheet.
     */
    private String shared(Intent intent) {
        String text = capped(intent.getCharSequenceExtra(Intent.EXTRA_TEXT), MAX_SHARED);
        String title = capped(intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT), MAX_SUBJECT);
        if (text.isEmpty() && title.isEmpty()) {
            return box.https();
        }
        StringBuilder url = new StringBuilder(box.https()).append(ADD);
        char join = '?';
        if (!text.isEmpty()) {
            url.append(join).append("text=").append(Uri.encode(text));
            join = '&';
        }
        if (!title.isEmpty()) {
            url.append(join).append("title=").append(Uri.encode(title));
        }
        return url.toString();
    }

    /** Remote text, as TEXT, trimmed and cut to a length. Never parsed. */
    private static String capped(CharSequence value, int most) {
        if (value == null) {
            return "";
        }
        String text = value.toString().trim();
        return text.length() <= most ? text : text.substring(0, most);
    }

    @Override
    protected void onSaveInstanceState(Bundle out) {
        super.onSaveInstanceState(out);
        web.saveState(out);
    }

    /** Back walks the app's own history first; at its root it leaves. */
    @Override
    public void onBackPressed() {
        if (pairing != null || offline.getVisibility() == View.VISIBLE) {
            super.onBackPressed();
        } else if (web.canGoBack()) {
            web.goBack();
        } else {
            super.onBackPressed();
        }
    }

    private void load(String url) {
        current = url;
        offline.setVisibility(View.GONE);
        web.setVisibility(View.VISIBLE);
        web.loadUrl(url);
    }

    /** The same page over another scheme. Uri keeps the fragment; string surgery would not. */
    private static String over(String scheme, String url) {
        return Uri.parse(url).buildUpon().scheme(scheme).build().toString();
    }

    /** Show why, in the OS's own words. Never remote content parsed as markup. */
    private void showOffline(CharSequence why) {
        reason.setText(why);
        web.setVisibility(View.GONE);
        offline.setVisibility(View.VISIBLE);
    }

    private View buildOfflineView() {
        LinearLayout screen = new LinearLayout(this);
        screen.setOrientation(LinearLayout.VERTICAL);
        screen.setGravity(Gravity.CENTER);
        screen.setBackgroundColor(Color.parseColor("#fbfbfa")); // the manifest's theme_color
        int pad = dp(24);
        screen.setPadding(pad, pad, pad, pad);

        TextView title = new TextView(this);
        title.setText(R.string.offline_title);
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 20);
        title.setTextColor(Color.parseColor("#1a1a1a"));

        TextView body = new TextView(this);
        body.setText(R.string.offline_body);
        body.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        body.setTextColor(Color.parseColor("#4a4a48"));
        body.setPadding(0, dp(12), 0, dp(12));

        reason = new TextView(this);
        reason.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        reason.setTextColor(Color.parseColor("#8a8a86"));

        Button retry = new Button(this);
        retry.setText(R.string.offline_retry);
        retry.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                load(over("https", current)); // the page he was after, not home
            }
        });

        screen.addView(title);
        screen.addView(body);
        screen.addView(reason);
        screen.addView(retry);
        return screen;
    }

    private View buildPairingView() {
        LinearLayout screen = new LinearLayout(this);
        screen.setOrientation(LinearLayout.VERTICAL);
        screen.setGravity(Gravity.CENTER);
        screen.setBackgroundColor(Color.parseColor("#fbfbfa"));
        int pad = dp(24);
        screen.setPadding(pad, pad, pad, pad);

        TextView title = new TextView(this);
        title.setText(R.string.pairing_title);
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 20);
        title.setTextColor(Color.parseColor("#1a1a1a"));

        TextView body = new TextView(this);
        body.setText(R.string.pairing_body);
        body.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        body.setTextColor(Color.parseColor("#4a4a48"));
        body.setPadding(0, dp(12), 0, dp(12));

        typed = new EditText(this);
        typed.setHint(R.string.pairing_hint);
        typed.setSingleLine(true);
        typed.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);

        said = new TextView(this);
        said.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        said.setTextColor(Color.parseColor("#8a8a86"));
        said.setPadding(0, dp(8), 0, dp(8));

        connect = new Button(this);
        connect.setText(R.string.pairing_connect);
        connect.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                pair();
            }
        });

        screen.addView(title);
        screen.addView(body);
        screen.addView(typed);
        screen.addView(said);
        screen.addView(connect);
        return screen;
    }

    private int dp(int units) {
        return Math.round(units * getResources().getDisplayMetrics().density);
    }

    /**
     * A WebView opens no picker for `<input type="file">` on its own: without
     * this the composer's attach button did nothing in the app. The system
     * picker hands back content:// grants, so no storage permission is asked.
     */
    private final class Chooser extends WebChromeClient {
        @Override
        public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> callback, FileChooserParams params) {
            answerChooser(null);
            // Built here rather than by params.createIntent(), which some WebView
            // versions give only the first accept type ("image/*,application/pdf").
            Intent pick = new Intent(Intent.ACTION_GET_CONTENT)
                    .addCategory(Intent.CATEGORY_OPENABLE)
                    .setType("*/*")
                    .putExtra(Intent.EXTRA_MIME_TYPES, accepted(params.getAcceptTypes()))
                    .putExtra(Intent.EXTRA_ALLOW_MULTIPLE, params.getMode() == FileChooserParams.MODE_OPEN_MULTIPLE);
            try {
                startActivityForResult(pick, CHOOSE_FILES);
            } catch (ActivityNotFoundException e) {
                callback.onReceiveValue(null);
                Toast.makeText(MainActivity.this, "No app on this phone picks files", Toast.LENGTH_SHORT).show();
                return true;
            }
            choosing = callback;
            return true;
        }
    }

    /** The input's accept list as MIME types; "" and extensions are dropped, and none means any. */
    private static String[] accepted(String[] accept) {
        java.util.ArrayList<String> types = new java.util.ArrayList<>();
        for (String entry : accept) {
            for (String type : entry.split(",")) {
                if (type.trim().contains("/")) {
                    types.add(type.trim());
                }
            }
        }
        return types.isEmpty() ? new String[] {"*/*"} : types.toArray(new String[0]);
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        if (request != CHOOSE_FILES) {
            super.onActivityResult(request, result, data);
            return;
        }
        answerChooser(result == RESULT_OK && data != null ? chosen(data) : null);
    }

    private void answerChooser(Uri[] uris) {
        if (choosing != null) {
            choosing.onReceiveValue(uris);
            choosing = null;
        }
    }

    /** Every file the picker returned: several arrive as ClipData, one as the data URI. */
    private static Uri[] chosen(Intent data) {
        ClipData clip = data.getClipData();
        if (clip != null) {
            Uri[] uris = new Uri[clip.getItemCount()];
            for (int i = 0; i < uris.length; i++) {
                uris[i] = clip.getItemAt(i).getUri();
            }
            return uris;
        }
        return data.getData() == null ? null : new Uri[] {data.getData()};
    }

    private final class Portal extends WebViewClient {

        /**
         * The app's origin stays in the WebView; everything else — a mailto:
         * link in a triaged message, an http link out of a todo — goes to the
         * system, which is where a link to somewhere else belongs.
         */
        @Override
        public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
            Uri url = request.getUrl();
            if (box.host.equals(url.getHost())) {
                return false;
            }
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            } catch (ActivityNotFoundException e) {
                // Nothing on the phone handles the scheme. Staying put is the
                // only honest outcome; swallowing the exception keeps a bad
                // link from killing the app.
            }
            return true;
        }

        /** A box page has loaded and listens: the token goes to it. */
        @Override
        public void onPageFinished(WebView view, String url) {
            if (box != null && box.host.equals(Uri.parse(url).getHost())) {
                handToken();
            }
        }

        /** A challenge only the route can answer (a proxy's); anything else is refused. */
        @Override
        public void onReceivedHttpAuthRequest(WebView view, HttpAuthHandler handler, String host, String realm) {
            if (!route.answers(handler, host)) {
                handler.cancel();
            }
        }

        @Override
        public void onReceivedError(WebView view, WebResourceRequest request, WebResourceError error) {
            if (!request.isForMainFrame()) {
                return; // a missing sub-resource is the page's problem, not ours
            }
            showOffline(error.getDescription() + " — " + request.getUrl());
        }

        /**
         * NEVER proceed(), and NEVER downgrade. An SSL error on the tailnet means
         * the certificate is not the one `tailscale cert` minted, or there is
         * none yet; accepting it, or retrying over plain http, would make this
         * app the weakest reader of a chain whose whole point is that readers
         * check, and would carry the owner's mail in the clear. The offline
         * screen says so, and its retry asks over https again.
         */
        @Override
        public void onReceivedSslError(WebView view, SslErrorHandler handler, SslError error) {
            handler.cancel();
            showOffline("TLS: " + error.getUrl());
        }
    }
}
