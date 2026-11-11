# Releasing the apps

## One click

Actions tab → **Release apps** → **Run workflow**. About 15 minutes later the Releases page has:

- `Leak.apk` (Android 7.0 and newer, 64-bit and 32-bit ARM)
- `Leak-setup.exe` (Windows installer, no admin rights needed)
- `Leak-portable.exe` (runs without installing; needs WebView2, which Windows 10 and 11 include)

Pushing a tag like `v0.2.0` does the same and names the release after the tag.

## Give Android a permanent signing key (do this once)

Android only installs an update over an existing app if both are signed with the same key. Without a key, each build is signed with a throwaway key and people must uninstall before updating.

On any computer with Java installed:

```bash
keytool -genkeypair -v -keystore leak-release.jks -alias leak \
  -keyalg RSA -keysize 2048 -validity 10000
base64 -w0 leak-release.jks > leak-release.b64   # macOS: base64 -i leak-release.jks
```

Then in the repository: Settings → Secrets and variables → Actions → New repository secret:

| Secret | Value |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | contents of `leak-release.b64` |
| `ANDROID_KEYSTORE_PASSWORD` | the password you chose |
| `ANDROID_KEY_ALIAS` | `leak` |

Keep `leak-release.jks` and its password safe and private. Losing it means users must reinstall to update.

## Windows SmartScreen

Unsigned Windows apps show a SmartScreen warning ("More info" → "Run anyway"). Open-source projects can get free code signing from the [SignPath Foundation](https://signpath.org/); once approved, add their signing step to the Windows job.

## Which site the apps open

The apps open `https://<owner>.github.io/<repo>/`. To use a custom domain or another host, set a repository variable `LEAK_SITE_URL` (Settings → Secrets and variables → Actions → Variables).
