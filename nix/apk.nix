# The Android portal, built as a PURE DERIVATION.
#
# WHY NO GRADLE. Gradle resolves its own dependencies from the network at build
# time, which makes the build a function of what Maven Central served that
# afternoon — the opposite of everything else here, where the toolchain is
# pinned in the flake and a rebuild is a hash check. The app is one Java file
# and four resources; the Android SDK ships the tools that turn those into an
# APK (aapt2, d8, zipalign, and javac from the JDK), so the whole build is those
# four commands in order. CI then rebuilds only when an input CHANGED, because
# that is what a derivation hash means.
#
# WHY ITS OWN pkgs. The Android SDK is unfree and its licence must be accepted;
# the repo's main pkgs grants unfree to exactly one package BY NAME (claude-code)
# and that decision should stay legible. Importing nixpkgs again HERE confines
# "allowUnfree + accept_license" to the file that needs it, so nothing else in
# the flake silently changes posture.
#
# WHY THE BUILD IS x86_64-linux ON EVERY HOST. Measured against the pinned
# nixpkgs' androidenv/repo.json: Google publishes build-tools for linux (x86_64),
# macosx and windows and for NO aarch64 Linux, at any revision through 37. On
# an aarch64 box the package still deploys, and `aapt2` then says "cannot
# execute binary file: Exec format error" — so a host-native derivation would
# only fail later and less clearly. Pinning the builder to x86_64-linux says the
# true thing instead: this artefact needs an x86_64 Linux builder. It also makes
# the derivation IDENTICAL on every machine, so the store path CI produces and
# the one a laptop produces are the same path — which is what makes the cache in
# .github/workflows/release-apk.yml worth having. The APK itself is bytecode and
# resources; nothing in it is a function of the machine that linked it.
#
# WHY SIGNING IS NOT HERE. A signing key in a derivation is a signing key in the
# store: world-readable to every user of the box and copied to every substituter
# it is pushed to. So this produces an UNSIGNED apk plus its sha256, and
# scripts/sign-apk.sh signs it outside, from a keystore that is a secret.
{
  nixpkgs,
  # The host system. Used ONLY for the signer, which is a JVM program.
  system,
  # The app sources (android/), the web app's icon, and the signing script.
  # android/route/ holds the routes (below); everything else is common.
  src,
  icon,
  signScript,
  # Derived from the tree by flake.nix so the derivation's hash changes exactly
  # when the thing it describes changes.
  versionCode,
  versionName,
  # THE DEPLOYMENT: the box the portal opens (`host`) and the Firebase project
  # number allowed to push to it (`sender`). The sources spell them
  # `@PORTAL_HOST@` and `@PORTAL_SENDER@`, so the code names no box.
  portal,
}:

let
  androidConfig = {
    allowUnfree = true;
    android_sdk.accept_license = true;
  };

  # Pinned, and checked against the pinned nixpkgs' repo.json rather than
  # assumed: 34 is the platform this app targets and 34.0.0 the matching
  # build-tools. "latest" would make the APK a function of the nixpkgs bump.
  buildToolsVersion = "34.0.0";
  platformVersion = "34";

  # Only what compiles an APK. No emulator, no system images, no NDK, no cmake,
  # no sources, and toolsVersion = null drops the legacy `tools` package that
  # build-tools would otherwise link — each of those is hundreds of megabytes of
  # closure this build never opens.
  sdkFor =
    pkgs:
    pkgs.androidenv.composeAndroidPackages {
      platformVersions = [ platformVersion ];
      buildToolsVersions = [ buildToolsVersion ];
      toolsVersion = null;
      includeEmulator = false;
      includeSystemImages = false;
      includeSources = false;
      includeNDK = false;
      includeCmake = false;
    };

  # androidenv deploys every package under $out/libexec/android-sdk/<path>, and
  # neither of these exposes a bin/ — the tools are addressed by their SDK path.
  toolDirOf = sdk: "${builtins.head sdk.build-tools}/libexec/android-sdk/build-tools/${buildToolsVersion}";

  # 17 rather than whatever `jdk` points at: javac's -source/-target 8 (the
  # bytecode level d8 reads with no desugaring) is deprecated in newer JDKs and
  # will eventually be removed. Pinning the JDK keeps a nixpkgs bump from
  # changing the bytecode this app ships.
  jdkOf = pkgs: pkgs.jdk17_headless;

  buildPkgs = import nixpkgs {
    system = "x86_64-linux";
    config = androidConfig;
  };
  buildSdk = sdkFor buildPkgs;
  toolDir = toolDirOf buildSdk;
  androidJar =
    "${builtins.head buildSdk.platforms}/libexec/android-sdk/platforms/android-${platformVersion}/android.jar";

  inherit (buildPkgs) lib;

  # EVERYTHING BUT THE ROUTES, so the portal's derivation does not move when
  # a route it does not use does.
  common = builtins.path {
    name = "android";
    path = src;
    filter = path: _: !(lib.hasPrefix "${toString src}/route" path);
  };

  # A ROUTE is how the WebView reaches the box, and the APK is a function of
  # one. Each names its own Route.java (android/route/<name>/java), the jars
  # that class compiles and runs against, the native libraries it loads, a
  # resource overlay, and the package it installs as. `device` is the phone's
  # own network (the Tailscale app's VPN) and adds nothing; `tsnet` is a
  # tailnet node inside the app (android/route/tsnet, docs/reports).
  routes = {
    device = {
      name = "device";
      java = src + "/route/device/java";
      jars = [ ];
      libs = null;
      res = null;
      package = null;
    };
    tsnet = {
      name = "tsnet";
      java = src + "/route/tsnet/java";
      jars = [
        "${webkit}/classes.jar"
        "${node}/classes.jar"
      ];
      libs = "${node}/jni";
      res = src + "/route/tsnet/res";
      # Its own package, so it installs BESIDE the portal rather than over
      # it, and the owner keeps a working app while he tries this one.
      package = "com.bmabsout.suzatary.tsnet";
    };
  };

  # androidx.webkit, for ProxyController: the one supported way to send a
  # WebView's requests through a proxy. Its classes.jar is all of it the app
  # needs at run time (only WebViewAssetLoader, unused here, reaches into
  # androidx.core; the annotation libraries are compile-time only). 1.12.1 is
  # the newest whose bytecode build-tools 34's d8 reads (1.14.0 crashes it).
  webkit = buildPkgs.runCommand "androidx-webkit-1.12.1" {
    aar = buildPkgs.fetchurl {
      url = "https://dl.google.com/dl/android/maven2/androidx/webkit/webkit/1.12.1/webkit-1.12.1.aar";
      hash = "sha256-GKClGtEp4MhWPwMdRsin5hFp9WroTlYqDXVoaUEmIRE=";
    };
    nativeBuildInputs = [ buildPkgs.unzip ];
  } "mkdir $out && unzip -q $aar classes.jar -d $out";

  # THE NODE: android/route/tsnet/node (Go, tsnet) bound to Java by
  # gomobile, for arm64 phones only. Go 1.27.1 because tailscale.com v1.104.0
  # asks for it; the module cache is a fixed-output derivation of go.sum
  # (proxyVendor), so the build itself reads no network.
  ndkVersion = "29.0.14206865";
  nodeSdk = buildPkgs.androidenv.composeAndroidPackages {
    platformVersions = [ platformVersion ];
    buildToolsVersions = [ buildToolsVersion ];
    toolsVersion = null;
    includeEmulator = false;
    includeSystemImages = false;
    includeSources = false;
    includeNDK = true;
    ndkVersions = [ ndkVersion ];
    includeCmake = false;
  };
  node = (buildPkgs.buildGoModule.override { go = buildPkgs.go_1_27; }) {
    pname = "suzatary-node";
    # tsnet's version, not the app's: the node rebuilds when its own source
    # moves, not on every commit.
    version = "1.104.0";
    src = src + "/route/tsnet/node";
    proxyVendor = true;
    # What gomobile bind reads beyond the build: it writes a module of its own
    # that requires every module of `go list -m all`, flat, and tidies it, so
    # the cache holds what that tidy reads (the same module, built here).
    modPostBuild = ''
      here=$PWD
      mkdir -p "$TMPDIR/gobind" && cd "$TMPDIR/gobind"
      {
        echo 'module gobind'
        echo "go $(go env GOVERSION | sed 's/^go//')"
        GOOS=android GOARCH=arm64 go -C "$here" list -mod=mod -m \
          -f '{{if .Main}}replace {{.Path}} => {{.Dir}}{{else}}require {{.Path}} {{.Version}}{{end}}' all
      } > go.mod
      printf 'package main\n\nimport (\n\t_ "golang.org/x/mobile/bind/java"\n\t_ "golang.org/x/mobile/bind/seq"\n\t_ "suzatary/node"\n)\n\nfunc main() {}\n' > main.go
      GOOS=android GOARCH=arm64 go mod tidy
      cd "$here"
    '';
    vendorHash = "sha256-cvnbtM6+/cmU5HpfKXdOvj1JC12owqvIoupme7RFVqk=";
    nativeBuildInputs = [
      (buildPkgs.gomobile.override { withAndroidPkgs = false; })
      (jdkOf buildPkgs)
      buildPkgs.unzip
    ];
    ANDROID_HOME = "${nodeSdk.androidsdk}/libexec/android-sdk";
    ANDROID_NDK_HOME = "${nodeSdk.androidsdk}/libexec/android-sdk/ndk/${ndkVersion}";
    buildPhase = ''
      runHook preBuild
      gomobile bind -target android/arm64 -androidapi 26 \
        -javapkg com.bmabsout.suzatary -trimpath -ldflags '-s -w -buildid=' \
        -o node.aar .
      runHook postBuild
    '';
    checkPhase = ''
      runHook preCheck
      go test ./...
      runHook postCheck
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p $out
      unzip -q node.aar classes.jar 'jni/*' -d $out
      runHook postInstall
    '';
  };

  apk = route: buildPkgs.stdenvNoCC.mkDerivation {
    pname = "suzatary-apk-unsigned-${route.name}";
    version = versionName;

    dontUnpack = true;
    nativeBuildInputs = [
      (jdkOf buildPkgs)
      buildPkgs.imagemagick # renders the launcher icons; nothing else needs it
      buildPkgs.zip # the one way to add classes.dex to the linked APK
    ];

    buildPhase = ''
      runHook preBuild
      export PATH=${toolDir}:$PATH

      # RESOURCES. The tracked res/ tree, plus launcher icons rendered from the
      # web app's own 512px PNG — so the icon on the home screen and the icon of
      # the installed PWA are the same picture by construction, not by two
      # people exporting the same SVG twice. -strip drops the PNG tIME chunk,
      # which would otherwise put a clock in the output.
      # THE DEPLOYMENT'S NAMES, spelled into a copy of the common sources (a
      # route's carry none); a name missing or a token left over is a build
      # failure, not a portal onto nowhere.
      if [ -z ${lib.escapeShellArg portal.host} ] || [ -z ${lib.escapeShellArg portal.sender} ]; then
        echo "apk: set SUZATARY_PORTAL_HOST and SUZATARY_PORTAL_SENDER and build with --impure" >&2
        exit 1
      fi
      cp -r ${common} android
      chmod -R u+w android
      grep -rlZ '@PORTAL_' android | xargs -0 -r sed -i \
        -e 's|@PORTAL_HOST@|${portal.host}|g' \
        -e 's|@PORTAL_SENDER@|${portal.sender}|g'
      if grep -rq '@PORTAL_' android; then
        echo "apk: a @PORTAL_ token has no value" >&2
        exit 1
      fi

      cp -r android/res res
      chmod -R u+w res
      for spec in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
        density=''${spec%%:*}
        px=''${spec##*:}
        mkdir -p "res/mipmap-$density"
        magick ${icon} -resize "''${px}x''${px}" -strip "res/mipmap-$density/ic_launcher.png"
      done

      aapt2 compile --dir res -o compiled.zip
      ${lib.optionalString (route.res != null) "aapt2 compile --dir ${route.res} -o overlay.zip"}

      # LINK. --version-code/--version-name inject what the manifest omits, so
      # the version in a released APK is a fact about the commit it was built
      # from. --java writes R.java, which MainActivity compiles against, in
      # the classes' own package whatever the route installs as.
      aapt2 link \
        -o base.apk \
        -I ${androidJar} \
        --manifest android/AndroidManifest.xml \
        --java gen \
        --custom-package com.bmabsout.suzatary \
        --version-code ${versionCode} \
        --version-name ${versionName} \
        ${lib.optionalString (route.package != null) "--rename-manifest-package ${route.package}"} \
        compiled.zip \
        ${lib.optionalString (route.res != null) "--auto-add-overlay -R overlay.zip"}

      mkdir -p classes
      javac -encoding UTF-8 -nowarn \
        -source 8 -target 8 -bootclasspath ${androidJar} \
        ${lib.optionalString (route.jars != [ ]) "-classpath ${lib.concatStringsSep ":" route.jars}"} \
        -d classes $(find android/java ${route.java} gen -name '*.java')

      mkdir -p dex
      d8 --release --min-api 26 --lib ${androidJar} \
        --output dex $(find classes -name '*.class') ${toString route.jars}

      # d8 cannot append to an archive and aapt2 cannot be told about dex, so
      # the two halves meet here. -X drops the extra attributes (uid, gid, the
      # high-resolution timestamps); the touch handles the one -X leaves, the
      # DOS date in the central directory, which is otherwise the wall clock and
      # was measured making two builds of identical bytes hash differently. 1980
      # is the DOS epoch and the SOURCE_DATE_EPOCH the rest of this repo uses.
      cp base.apk unaligned.apk
      touch -d @315532800 dex/classes.dex
      ( cd dex && zip -q -X ../unaligned.apk classes.dex )
      ${lib.optionalString (route.libs != null) ''
        # THE ROUTE'S NATIVE CODE, at lib/<abi>/ where the installer looks,
        # deflated (the installer extracts it), dated like the dex.
        mkdir -p native
        cp -r ${route.libs} native/lib
        chmod -R u+w native
        find native -exec touch -d @315532800 {} +
        ( cd native && zip -q -X -r -D ../unaligned.apk lib )
      ''}

      # Android mmaps uncompressed entries straight out of the APK, so they must
      # be 4-byte aligned; -p additionally page-aligns stored .so files, of
      # which this app has none (a route's are deflated). An unaligned APK
      # installs and then runs slower for no reason, which is exactly the kind
      # of thing nobody notices.
      zipalign -p -f 4 unaligned.apk suzatary-unsigned.apk
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      mkdir -p $out
      cp suzatary-unsigned.apk $out/
      # The artefact carries its own identity, so the release notes can say what
      # this is without evaluating the flake a second time or parsing a store
      # path — the same reason every generated todo block carries authoredBy.
      printf '%s\n' ${versionName} > $out/version
      # THE BUILD'S FINGERPRINT. The release workflow compares this against the
      # one already published and skips the upload when they match, so a docs
      # commit does not mint a new APK for people to install. It is the UNSIGNED
      # hash on purpose: it is a fact about the inputs alone and does not depend
      # on a signature algorithm happening to be deterministic.
      ( cd $out && sha256sum suzatary-unsigned.apk > suzatary-unsigned.apk.sha256 )
      runHook postInstall
    '';

    passthru = {
      inherit
        versionCode
        versionName
        buildToolsVersion
        platformVersion
        ;
    };

    meta = {
      description = "Suzatary's Android portal (unsigned): a WebView onto the tailnet address";
      platforms = [ "x86_64-linux" ];
    };
  };

  # THE SIGNER RUNS ANYWHERE A JVM DOES, so it is built for the HOST and not for
  # x86_64-linux. apksigner is a shell script over apksigner.jar; the x86_64
  # aapt2 and zipalign sitting beside it in the same package are never invoked.
  # That distinction is the difference between the owner being able to sign a
  # downloaded APK on an aarch64 box and not.
  hostPkgs = import nixpkgs {
    inherit system;
    config = androidConfig;
  };
  sign = hostPkgs.writeShellScriptBin "suzatary-sign-apk" ''
    export PATH=${hostPkgs.lib.makeBinPath [ (jdkOf hostPkgs) ]}:${toolDirOf (sdkFor hostPkgs)}:"$PATH"
    exec ${hostPkgs.bash}/bin/bash ${signScript} "$@"
  '';
in
{
  unsigned = apk routes.device;
  tsnet = apk routes.tsnet;
  inherit sign;
}
