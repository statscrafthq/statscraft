# Install and update

Statscraft runs on Windows, macOS and Linux. Each package holds both programs: the window (`statscraft-gui`) and the command line (`statscraft`).

## Download

Download the package for your system from the [download page](https://statscraft.org/download.html). In the file names, `X` stands for the version:

| File | For |
|---|---|
| `statscraft-X-windows-x86_64-setup.exe` | Windows, with an installer |
| `statscraft-X-windows-x86_64.zip` | Windows, without an installer |
| `statscraft-X-macos-universal.tar.gz` | macOS, on Apple silicon and Intel |
| `statscraft_X_amd64.deb` | Debian and Ubuntu |
| `statscraft-X-linux-x86_64.tar.gz` | any other Linux |

The download page also explains how to check a file against its published checksum. The files for AI agents are described in [Connect your AI agent](connect.md).

## Windows

Run `statscraft-X-windows-x86_64-setup.exe`. It installs for your user only and needs no administrator rights.

- The Start menu gets **Statscraft**, which opens the window.
- `statscraft` works in any terminal you open after the installation.
- .sav and .zsav files open in the window, and .sps files in its syntax editor. Where another program already opens one of these types, it stays the default, and Statscraft is offered under **Open with**.

If Windows warns that the installer comes from an unknown publisher, choose **More info**, then **Run anyway**.

To uninstall, open **Settings > Apps > Installed apps**, find Statscraft and choose **Uninstall**.

## Windows without an installer

Use the .zip where you may not install programs. Unpack it anywhere you can write, for example to a folder in Documents or on a USB stick (in Explorer: right-click, **Extract All**). The folder holds:

- `statscraft-gui.exe`, the window: double-click it to start Statscraft;
- `statscraft.exe`, the command line: run it in a terminal from that folder, or add the folder to your `PATH`;
- `LICENSE.txt` and `README.md`.

There is no Start menu entry, and files do not open with a double-click. Open them with **File > Open data...**, or drag them onto `statscraft-gui.exe`. Keep `LICENSE.txt` next to the programs, so updates know where to go. To remove Statscraft, delete the folder.

## macOS

Double-click `statscraft-X-macos-universal.tar.gz` in Finder to unpack it. Drag `Statscraft.app` to **Applications**.

The first time you open it, macOS says it cannot verify the app. Click **Done** (not **Move to Trash**), then:

1. Choose Apple menu > **System Settings**, then **Privacy & Security**.
2. Under **Security**, find the message about Statscraft and click **Open Anyway**.
3. Enter your login password and click **OK**.

After that, Statscraft opens with a double-click like any other app. Finder offers it under **Open With** for .sav, .zsav and .sps files.

The command line is the file `statscraft` in the same folder. Move it to a folder on your `PATH`, for example:

```
mkdir -p ~/.local/bin
mv ~/Downloads/statscraft-X-macos-universal/statscraft ~/.local/bin/
```

If `~/.local/bin` is not on your `PATH` yet, add `export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc`. The first `statscraft --version` is blocked in the same way as the app: allow it under **Privacy & Security** and run it again.

To uninstall, move Statscraft.app to the Bin and delete the `statscraft` file.

## Debian and Ubuntu

```
sudo apt install ./statscraft_X_amd64.deb
```

Statscraft is then in the applications menu, and .sav, .zsav and .sps files open in the window. To uninstall, run `sudo apt remove statscraft`.

## Other Linux

The tarball holds a `bin` and a `share` folder. Unpack it into `~/.local`, then refresh the file types and the applications menu:

```
tar -xzf statscraft-X-linux-x86_64.tar.gz --strip-components=1 -C ~/.local
update-mime-database ~/.local/share/mime
update-desktop-database ~/.local/share/applications
```

If `update-desktop-database` is missing, skip that line. `~/.local/bin` must be on your `PATH`, which it is on most distributions once the folder exists.

To uninstall, delete what the tarball put there:

```
cd ~/.local
rm bin/statscraft bin/statscraft-gui share/applications/statscraft.desktop share/mime/packages/statscraft.xml
rm share/icons/hicolor/*/apps/statscraft.png share/icons/hicolor/scalable/apps/statscraft.svg
rm -r share/doc/statscraft
update-mime-database ~/.local/share/mime
```

## Updates

When a new version is out, the window shows a bar under the menu. **Update now** installs it, and **Later** hides the bar until the next check. **Help > Check for updates** checks at once.

What **Update now** does depends on how you installed Statscraft:

| Installed with | What happens |
|---|---|
| the Windows installer | The window asks about unsaved work, closes, installs the new version and opens again. |
| the Windows .zip | The two programs in the folder are replaced. **Restart now** opens the new version. |
| the Linux tarball | The two programs in `~/.local/bin` are replaced. **Restart now** opens the new version. |
| the .deb | The new package is saved in `~/Downloads`, and the system asks for your password to install it. Then **Restart now**. |
| macOS | The bar has a **Download** link. Unpack the new package and replace Statscraft.app (and the `statscraft` file) with the new ones. The first start needs **Open Anyway** again. |

Every download is checked against the checksums published with the release before it is used.

The window checks at most once a day. The check sends nothing about you or your data. To turn it off, clear **Edit > Settings > Updates > Check for updates automatically**.

From the command line:

```
statscraft update --check     is there a newer version?
statscraft update             install it
```

On Debian and Ubuntu, `statscraft update` downloads the package and prints the one command that installs it. On macOS it names the download page instead. [The command line](command-line.md#update) lists the options.
