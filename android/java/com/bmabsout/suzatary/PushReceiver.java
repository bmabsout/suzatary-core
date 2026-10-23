/*
 * WHERE GOOGLE'S PUSH SERVICE LANDS. Two actions arrive here and nothing else:
 * the answer to the registration this app asked for (Push.ensure), and a
 * message the box sent. The manifest gates this receiver on
 * com.google.android.c2dm.permission.SEND, which only Play Services holds, so
 * "exported" here means exported to GMS and to nothing else on the phone.
 *
 * A RECEIVER IS NOT A BACKGROUND SERVICE. onReceive runs when the OS decides
 * there is something to deliver and the process is torn down again afterwards;
 * this app declares no service, no job and no alarm. That is the whole
 * requirement the owner stated, and it is a property of the manifest rather than
 * of anything this class remembers to do.
 */
package com.bmabsout.suzatary;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.util.Log;

public final class PushReceiver extends BroadcastReceiver {

    /**
     * One channel, so Settings offers one switch for "Suzatary tells me
     * things" rather than a taxonomy nobody asked for. IMPORTANCE_DEFAULT is a
     * sound and a status-bar entry and no heads-up banner: the box pages about
     * work that has already happened, which is worth seeing and never worth
     * interrupting. The owner can raise or silence it in Settings, and that choice
     * outlives every update because the id never changes.
     */
    private static final String CHANNEL = "suzatary";

    /** Remote text. Capped so a malformed payload cannot make a wall of one. */
    private static final int MAX_TEXT = 240;

    @Override
    public void onReceive(Context context, Intent intent) {
        if (intent == null) {
            return;
        }
        if (Push.ACTION_RECEIVE.equals(intent.getAction())) {
            show(context, intent);
            // AND THE WAKE IS THE WIDGET'S TOO (2026-09-10). The box pushes
            // because the chain moved, and the chain moving is exactly when the
            // five most-owed todos on the home screen are wrong — so the one
            // moment this process is reliably alive pays for both. It sends a
            // broadcast and returns; the fetch is Widget.onUpdate's, off this
            // thread, which is the only place in the app allowed to do one.
            Widget.refresh(context);
            return;
        }
        if (!Push.ACTION_REGISTRATION.equals(intent.getAction())) {
            return;
        }
        String token = Push.tokenOf(intent);
        if (token.isEmpty()) {
            return; // Push.tokenOf logged why; no push is a state, not a failure
        }
        // onReceive is the MAIN THREAD and has about ten seconds before the
        // system may kill the process. goAsync holds the process open for the
        // HTTPS post and finish() releases it — the one place this app is
        // allowed to outlive a callback, and it ends by construction.
        final PendingResult pending = goAsync();
        final Context app = context.getApplicationContext();
        new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    Push.accept(app, token);
                } catch (Throwable e) {
                    Log.w(Push.TAG, "reporting the token failed", e);
                } finally {
                    pending.finish();
                }
            }
        }, "suzatary-push").start();
    }

    /**
     * A message from the box, drawn as a notification.
     *
     * THE BOX SENDS A DATA MESSAGE, NOT A NOTIFICATION MESSAGE. An FCM
     * "notification" payload is drawn by Play Services itself while the app is
     * stopped — a picture this app never composed, with an icon and a tap
     * target it did not choose. A data-only message is delivered here instead,
     * so the only notification that exists is the one below. That is the same
     * rule as everywhere else in this repo: one thing draws, and it is ours.
     *
     * `message_type` marks the deliveries that are ABOUT the queue rather than
     * from the box — deleted_messages when Google dropped a backlog, send_error
     * and send_event for upstream sends this app never makes. None of them
     * carries a title, and drawing one would be inventing a notification.
     */
    private void show(Context context, Intent intent) {
        String type = intent.getStringExtra("message_type");
        if (type != null && !"gcm".equals(type)) {
            Log.i(Push.TAG, "ignoring a control delivery: " + type);
            return;
        }
        Box box = Box.of(context);
        if (box == null) {
            Log.i(Push.TAG, "a push arrived after this app's data was cleared");
            return;
        }
        String title = text(intent.getStringExtra("title"), context.getString(R.string.app_name));
        String body = text(intent.getStringExtra("body"), "");
        String click = page(box, intent.getStringExtra("click"));

        NotificationManager manager = context.getSystemService(NotificationManager.class);
        if (manager == null) {
            return;
        }
        NotificationChannel channel =
                new NotificationChannel(CHANNEL, context.getString(R.string.push_channel), NotificationManager.IMPORTANCE_DEFAULT);
        channel.setDescription(context.getString(R.string.push_channel_description));
        // Creating a channel that exists is a no-op that never overwrites the
        // importance or the sound the owner chose, so there is no "first run" to
        // get right and no state to keep about whether this has been done.
        manager.createNotificationChannel(channel);

        // EXPLICIT, not an App Link. The click URL is one of this host's pages
        // and MainActivity would take it either way, but routing it by host
        // would depend on Android having verified assetlinks.json at install
        // time — which needs the site up on 443 and may simply not have
        // happened. Naming the class means a tap lands here always, and means
        // no other app can be the one that answers.
        Intent open = new Intent(Intent.ACTION_VIEW, Uri.parse(click)).setClass(context, MainActivity.class);
        // The URL is the identity of the notification: a second push about the
        // same page REPLACES the first rather than stacking a duplicate, and
        // two pushes about different pages sit side by side. FLAG_IMMUTABLE
        // because nothing may rewrite this intent between here and the tap.
        int id = click.hashCode();
        PendingIntent tap = PendingIntent.getActivity(
                context, id, open, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);

        Notification note = new Notification.Builder(context, CHANNEL)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(title)
                .setContentText(body)
                .setStyle(new Notification.BigTextStyle().bigText(body))
                .setContentIntent(tap)
                .setAutoCancel(true) // a tap dismisses; there is nothing to come back to
                .build();
        // Silently dropped when POST_NOTIFICATIONS was refused (API 33+). That
        // is the correct outcome of a refusal and not an error to report.
        manager.notify(id, note);
    }

    /**
     * Remote text, as TEXT. setContentTitle and setContentText take a
     * CharSequence and a String is never parsed as markup, so this is the same
     * doctrine as web/src/view/dom.ts one client further out — the only escaping
     * needed is a length, because the payload comes from off the box.
     */
    private static String text(String value, String fallback) {
        if (value == null || value.isEmpty()) {
            return fallback;
        }
        return value.length() <= MAX_TEXT ? value : value.substring(0, MAX_TEXT);
    }

    /**
     * The page a tap opens. A push arrives through Google, so the click URL is
     * the one field of it that could point somewhere: anything that is not one
     * of this host's own pages is refused and the home page used instead. Same
     * rule as MainActivity.targetOf, for the same reason — a WebView pointed at
     * a stranger's page by a stranger's payload is the one thing this app must
     * not do.
     */
    private static String page(Box box, String click) {
        if (click == null || click.isEmpty()) {
            return box.https();
        }
        Uri url = Uri.parse(click);
        if (box.holds(url)) {
            return url.buildUpon().scheme("https").build().toString(); // never cleartext
        }
        Log.w(Push.TAG, "a push named a page that is not this app's; opening home");
        return box.https();
    }
}
