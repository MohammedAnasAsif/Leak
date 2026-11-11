# Leak on iPhone and iPad

There are two free ways. Pick one.

## Option A: Add to Home Screen (easiest, never expires)

1. Open the Leak website in **Safari**.
2. Tap **Share**, then **Add to Home Screen**.

Leak opens full screen like an app, works offline for stories you've read, and updates itself.

## Option B: Sideload the native app with xtool (free Apple ID)

The native app adds pull-to-refresh, swipe-back and opens outlet links in Safari. [xtool](https://xtool.sh) builds and installs it from Windows (through WSL), Linux or macOS, with no Mac or paid developer account.

**Free Apple ID limits:** sideloaded apps stop opening after **7 days**; plug in and run `xtool dev` again to renew. Free accounts can have a few sideloaded apps at a time. A paid account ($99/year) removes the 7-day limit, but it isn't required.

### 1. Set up xtool

**On Windows** (Leak's apps are built for Windows too, so this is the common path):

1. Install WSL: open PowerShell as administrator and run `wsl --install`, then restart.
2. Install [usbipd-win](https://github.com/dorssel/usbipd-win/releases) so WSL can see your iPhone over USB. Plug in the iPhone, then in an administrator PowerShell:
   ```powershell
   usbipd list                      # find the Apple device's BUSID
   usbipd bind --busid <BUSID>
   usbipd attach --wsl --busid <BUSID>
   ```
3. Inside WSL (Ubuntu), install the Swift toolchain from [swift.org/install/linux](https://swift.org/install/linux) (check [xtool's install guide](https://xtool.sh/documentation/xtooldocs/installation-linux) for the version it needs), then:
   ```bash
   sudo apt-get install -y usbmuxd libimobiledevice-utils
   curl -fL "https://github.com/xtool-org/xtool/releases/latest/download/xtool-$(uname -m).AppImage" -o xtool
   chmod +x xtool && sudo mv xtool /usr/local/bin/
   ```
4. Download **Xcode.xip** in your browser from [developer.apple.com/download/all](https://developer.apple.com/download/all/?q=Xcode) (sign in with any Apple ID; the version to use is in xtool's install guide).
5. Run `xtool setup` and follow the prompts yourself: sign in with your Apple ID (choose **Password** mode for a free account; a spare Apple ID is a sensible precaution) and give it the path to Xcode.xip.

**On macOS:** install Xcode once, then `brew install xtool-org/tap/xtool` and `xtool setup`.

### 2. Build and install Leak

```bash
cd apps/ios
xtool dev
```

The first build takes a while; later builds are quick. On the iPhone:

1. Tap **Trust** when asked and enter your passcode.
2. If prompted, turn on **Developer Mode** (Settings → Privacy & Security → Developer Mode), restart, and run `xtool dev` again.
3. On first launch, if you see "Untrusted Developer": Settings → General → VPN & Device Management → your Apple ID → **Trust**.

### Using your own site address

If you run your own fork, point the app at your site first:

```bash
scripts/configure.sh https://yourname.github.io/YourRepo/
```

### Troubleshooting

- **Device not found:** check `usbipd attach` is still active, the cable is good, and `ideviceinfo` prints device details.
- **Sign-in errors:** `xtool auth logout`, then sign in again with `xtool auth login`.
- **App stopped opening after a week:** that's the free-account limit. Run `xtool dev` again.
