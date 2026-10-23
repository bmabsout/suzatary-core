/*
 * THE BOX THIS APP OPENS. An APK built with a profile that names a box opens
 * that box and asks nothing (Profile.java, written by nix/apk.nix). The
 * generic APK names none: it asks at first launch (MainActivity's pairing
 * screen), asks that box how it wakes the phone (Pairing), and keeps both
 * answers here. Every class that reaches the box asks this one.
 */
package com.bmabsout.suzatary;

import android.content.Context;
import android.content.SharedPreferences;
import android.net.Uri;

import java.util.regex.Pattern;

final class Box {

    /** The box's MagicDNS name, as the box's `app_url` names it. */
    final String host;

    /** The Firebase project's number Play Services registers against; null when the box sends no push. */
    final String sender;

    private static final String PREFS = "box";
    private static final String KEY_HOST = "host";
    private static final String KEY_SENDER = "sender";

    /** A DNS name of two or more labels, lower case. No scheme, port or path. */
    private static final Pattern HOST =
            Pattern.compile("[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+");

    Box(String host, String sender) {
        this.host = host;
        this.sender = sender;
    }

    /** The home page. No port: Android verifies an app's links over https on 443 only. */
    String https() {
        return "https://" + host + "/";
    }

    /** Whether `url` is one of this box's pages, over either scheme. */
    boolean holds(Uri url) {
        String scheme = url.getScheme();
        return ("https".equals(scheme) || "http".equals(scheme)) && host.equals(url.getHost());
    }

    /** The box this app opens, or null while a generic APK has not been given one. */
    static Box of(Context context) {
        if (Profile.BOX != null) {
            return Profile.BOX;
        }
        SharedPreferences prefs = prefs(context);
        String host = prefs.getString(KEY_HOST, "");
        return host.isEmpty() ? null : new Box(host, prefs.getString(KEY_SENDER, null));
    }

    /**
     * Keep `box` as this app's box. Called only once the box has answered
     * Pairing, so a name that is not a Suzatary box is never kept. commit,
     * not apply: the app opens the box right after, and a process killed
     * before the write lands would ask again with nothing to show for it.
     */
    static void keep(Context context, Box box) {
        SharedPreferences.Editor edit = prefs(context).edit().putString(KEY_HOST, box.host);
        if (box.sender == null) {
            edit.remove(KEY_SENDER);
        } else {
            edit.putString(KEY_SENDER, box.sender);
        }
        edit.commit();
    }

    /** The host a person typed, lower case and trimmed, or null when it is no DNS name. */
    static String hostOf(CharSequence typed) {
        String host = typed.toString().trim().toLowerCase(java.util.Locale.ROOT);
        return host.length() <= 253 && HOST.matcher(host).matches() ? host : null;
    }

    private static SharedPreferences prefs(Context context) {
        return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }
}
