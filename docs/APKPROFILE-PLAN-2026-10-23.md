# One APK build with a profile (2026-10-23)

Today the release workflow writes the box's tailnet name and the Firebase
sender into the APK from this repository's variables. So the APK belongs to
one deployment, and it must stay private. The owner wants two kinds of APK
from one build:

- a **generic APK**, built from suzatary-core and published there, which asks
  for its box at first launch and learns the box's Firebase sender at pairing;
- a **private APK per person**, built from that person's user repository with
  a profile: the box, the sender, extra permissions or native features, the
  icon, and the signing key.

An **APK profile** is a typed file that says what one APK is built with. The
build is a function of the profile. The generic profile is empty.

## The rules every stage keeps

1. **The owner's APK updates in place.** Android identifies an app by its
   package and its signing certificate. The owner's APK keeps
   `com.bmabsout.suzatary` and the key in `ANDROID_KEYSTORE_B64`, and its
   version code keeps growing, at every stage.
2. **The public tree names no deployment.** A box's name, a Firebase project
   number, a keystore and an icon of a person's own live in that person's user
   repository or in repository variables, never in suzatary-core. The generic
   profile is the only profile in the code repository.
3. **The profile is parsed once.** `nix/apk-profile.nix` reads the TOML file
   and refuses an unknown field or a malformed value at evaluation, with the
   field's name. `nix/apk.nix` takes only what it returns.
4. **The APK still holds no credential.** The tailnet admits the phone. A
   profile adds names and features, never a secret, and the signing key stays
   outside the store (`scripts/sign-apk.sh`).

## Stage 1 (this change): the profile, the first launch, and the sender at pairing

**The profile.** `nix/apk-profile.nix` reads a TOML file with two optional
fields:

| field    | value                                   | when absent                         |
|----------|-----------------------------------------|-------------------------------------|
| `box`    | the box's MagicDNS name, lower case     | the APK asks at first launch        |
| `sender` | the box's Firebase project number       | the APK registers for no push       |

A `sender` without a `box` is refused: a sender belongs to one box's Firebase
project. When the profile names the box, the profile is the whole answer, and
the APK never pairs. The result is a sum: no box (the generic APK), or a box
with its sender (`null` for a box that sends no push).

**The build.** `nix/apk.nix` takes the profile. It writes `Profile.java`,
which holds the profile's box or `null`. It keeps the manifest's verified
https link filter only when the profile names a box, because the generic APK
has no host to claim. The flake has three packages:

- `.#apk-unsigned` is the generic APK, from `android/profile/generic.toml`
  (an empty file). It is a pure build.
- `.#apk-profiled-unsigned` reads a profile's path from
  `SUZATARY_APK_PROFILE` under `--impure`.
- `.#apk-tsnet-unsigned`, the tsnet spike, reads the same variable. Its
  profile must name the box, because the node needs its box before the app
  runs.

`release-apk.yml` writes the owner's profile from the same two repository
variables as before (`SUZATARY_PORTAL_HOST`, `SUZATARY_PORTAL_SENDER`) and
builds `.#apk-profiled-unsigned`. The package, the key, the version code and
the release flow do not change. So the owner's phone installs the next
release as an update, and the APK opens the same box with the same sender.
`Push.ensure` compares the sender it registered with, so the phone does not
register again.

**The first launch.** `Box.java` holds the box the app opens: the profile's,
or the one kept at first launch. When there is neither, `MainActivity` shows
the pairing screen (Android widgets, no HTML). The person types the box's
name. `Box.hostOf` accepts a DNS name alone, with no scheme, port or path.
`Pairing.ask` then reads `GET https://<box>/api/portal`. The app keeps the
box only when that box answers 200 with the portal's shape. Until then no page
loads, no route opens and no push is asked for. The widget says "Open
Suzatary and choose its box". To choose another box, the person clears the
app's data.

**The sender at pairing.** `GET /api/portal` answers
`{"push": {"kind": "fcm", "sender": "<digits>"}}` or
`{"push": {"kind": "none"}}` (`wire::PortalPayload`). Its type is
`suzatary_core::portal::Portal`. It is a read behind the gate's viewers, the
same door as the widget's `GET /api/fold`. So the answer is also the proof
that the name is a Suzatary box that admits this phone's login at its own
`app_url`. The sender comes from the instance file:

```toml
[notify]
channels = ["fcm"]
fcm_sender = "<the Firebase project number>"
```

`Portal::of` answers `fcm` only when `channels` holds `fcm` and
`fcm_sender` is set. A sender the box does not push as would register a
phone that is never woken. The server reads the answer when it starts.

The sender moves from secretspec to the instance file, because it is not a
secret and the box now hands it out. `FCM_SENDER_ID` leaves `secretspec.toml`
and the Firebase connection's template. The box never read its value: only
the APK's build used the number, from the repository variable. The owner adds
`fcm_sender` to the box's instance file once this is deployed. Until then the
box answers `none`, which only the generic APK reads.

**Tests.** `wire::tests::the_portal_answer_is_the_json_the_android_portal_reads`
pins the JSON that `Pairing.java` parses, because an installed APK reads it
for months. `instance.golden` pins how `fcm_sender` is read and refused.
`surface.golden` lists the new route. Both APKs were built with
`nix build`, and their manifests were compared with `aapt2 dump`: the
profiled APK has the same package and the same link filter as before, and the
generic APK has only the `suzatary://` filter.

**What stage 1 does not fix.** `Push.java` posts the push token to
`POST /api/act/device` from native code, with no device signature. The
route table marks that route `signed`, so a box that signs (every box) refuses
the report. This was true before this change, and it is true for both APKs.
Stage 2 fixes it.

## Stage 2: the token through the page, and the box by a scan

- **The token goes through the page (done).** Native code cannot sign: the
  device key belongs to the page. `MainActivity` hands the token to the page
  with the platform's `WebView.postWebMessage`, which delivers only to a main
  frame at the box's origin. It does this when a box page loads and when a
  new token is kept, and never before a box is kept. `boot.ts` holds a
  message that arrives before the app module is imported.
  `web/src/app/pushtoken.ts` reports the token with the page's signed
  `http.act` when it is new or a week old. `Push.report` and its nonce are
  deleted.
- **The box by a scan.** Settings shows a QR code of
  `suzatary://box/<host>`. The phone's camera app opens it in the generic APK,
  which fills the pairing screen's field. The person still taps Connect, so a
  link from another app cannot choose the box.
- **The FCM card asks for the sender.** The connection's template gains a
  step that writes `[notify] fcm_sender` through the instance file's edits.

## Stage 3: a person's APK from their user repository

- The flake exports `lib.<system>.apk { profile; }`, and the user template's
  flake builds the person's APK with it. A user repository's workflow builds
  and signs it with that repository's own keystore secret.
- The profile gains `package`, `icon` and `label`. The defaults are the
  generic APK's. The owner's profile names `com.bmabsout.suzatary`, so the
  owner's APK keeps its identity when its build moves to the user
  repository.
- The version code must keep growing when the build moves. It becomes the
  number of the release (MAJOR, MINOR, PATCH packed into one integer), which
  both repositories read the same way, instead of this repository's commit
  count. The first such code must be greater than the last commit count the
  owner installed.
- The release workflow of this repository stops building the owner's APK.

## Stage 4: features and permissions by profile

- The profile gains `features`, a closed list (for example `widget`, `share`,
  `push`). Each feature names its manifest fragment, its permissions and its
  Java classes in `nix/apk.nix`. A feature not listed is not in the APK, so
  its permission is not either.
- An extra permission is only a feature's. The profile cannot name a raw
  permission string.

## Stage 5: the public generic APK

- suzatary-core's release workflow builds `.#apk-unsigned` and signs it with
  the project's public release key, under its own package
  (`com.bmabsout.suzatary.portal`). Then it installs beside a person's
  private APK and never replaces it.
- Obtainium can watch suzatary-core's releases as it watches this
  repository's today.
- This stage is the first public release. Nothing before it publishes the
  generic APK.
