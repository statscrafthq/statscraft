# The parts every window shares: buttons of the dialogs, file dialogs, the status bar, the log,
# questions before closing, About.

button-ok = OK
button-paste = Paste
button-reset = Reset
button-cancel = Cancel
button-close = Close
button-save = Save
button-dont-save = Don't save
dialog-syntax = Syntax

filter-data-file = Data file
filter-compressed-data-file = Compressed data file
filter-excel-workbook = Excel workbook
filter-web-page = Web page
filter-pdf-document = PDF document
filter-png-image = PNG image
filter-svg-image = SVG image

window-data = Data
window-untitled = Untitled
window-active = { $title } (active)
window-drop-files = Drop data, workbook, text, .sps or .scout files to open them
window-one-data-one-syntax = one data file and one syntax file open at a time
window-opens = Statscraft opens .sav, .zsav, .xlsx, .xlsm, .xls, .csv, .txt, .tab, .dat, .sps and .scout files
window-not-opened = not opened: { $files } ({ $reasons })

status-ready = Ready
status-saved-syntax = syntax saved to { $file }
status-saved-chart = chart saved to { $file }
status-exported = output exported to { $file }
status-chart-no-picture = this kind of chart has no picture yet
status-undone = undone: { $syntax }
status-redone = redone: { $syntax }
status-wait-to-delete-output = wait until the run ends to delete output
status-opening = Opening { $file }
status-running = Running { $what }
status-waiting = { $count } waiting
status-warnings =
    { $count ->
        [one] 1 warning, see the log
       *[other] { $count } warnings, see the log
    }
status-split-layered = Split by { $variables } (layered)
status-split-separate = Split by { $variables } (separate)
status-filter = Filter: { $name }
status-weight = Weight: { $name }

log-title = Log
log-copy-syntax = Copy syntax
log-undone = { $summary }; undone
log-warning = warning: { $warning }

unsaved-quit = Save the changes before closing?
unsaved-syntax = Save the changes to the syntax first?
unsaved-output = Save the output first?
unsaved-data = Save the changes to the data first?
unsaved-data-item = Data: { $name }
unsaved-dataset-no-file = { $name }, not saved to a file yet
unsaved-no-file = not saved to a file yet
unsaved-syntax-item = Syntax: { $name }
unsaved-output-file = Output: { $name } ({ $file })
unsaved-output-no-file = Output: { $name }, not saved to a file yet

about-title = About Statscraft
about-version = Version { $version }
about-what = Statistics with a command language, on .sav data files.
about-license = License: MIT
about-source = Source code
about-themes = Color themes
about-fonts = Fonts

import-sheet = Sheet
import-names-first-row = Read variable names from the first row
import-range = Range
import-whole-sheet = whole sheet
import-delimiter = Delimiter
import-comma = Comma
import-semicolon = Semicolon
import-tab = Tab
import-names-first-line = Read variable names from the first line
import-first-line = First line

# The update bar under the menu.
update-checking = Checking for updates
update-up-to-date = Statscraft { $version } is up to date.
update-no-release = No Statscraft release has been published yet.
update-check-failed = The update check failed: { $error }
update-failed = The update failed: { $error }
update-available = Statscraft { $version } is available (you have { $current }).
update-release-notes = Release notes
update-unmanaged = This copy runs from { $folder }, which Update cannot replace: install the new version the way you installed this one, or use the .deb or tarball from the release notes.
update-download = Download
update-mac-replace = Then replace Statscraft.app with the one in the download.
update-now = Update now
update-windows-restart = The window closes and opens again: save your work first.
update-later = Later
update-downloading = Downloading and checking Statscraft { $version }
update-installed = Statscraft { $version } is installed. Restart the window to use it; save your work first.
update-restart-now = Restart now
update-saved = The new package is checked and saved as { $file }. Install it in a terminal with
update-copy-command = Copy command
update-hint-windows = Downloads the installer and checks it against the release's checksums. The window then closes, the installer updates Statscraft and opens the window again.
update-hint-programs = Downloads the new programs, checks them against the release's checksums and replaces the ones in { $folder }.
update-hint-deb = Downloads the package to your Downloads folder, checks it against the release's checksums and installs it; the system asks for your password.
