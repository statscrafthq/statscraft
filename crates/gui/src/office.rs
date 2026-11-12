//! A chart of the output in the office programs: opened as a file of its own, or copied as an
//! editable chart. The files and the clipboard package come from the engine's `office` module.

use statscraft_engine::Chart;
use statscraft_engine::office::{self, Item};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Program {
    PowerPoint,
    Excel,
    Word,
}

impl Program {
    pub const ALL: [Program; 3] = [Program::PowerPoint, Program::Excel, Program::Word];

    pub fn label(self) -> &'static str {
        match self {
            Program::PowerPoint => "Open in PowerPoint",
            Program::Excel => "Open in Excel",
            Program::Word => "Open in Word",
        }
    }
}

/// Files written so far, so that each name is new: a file still open in an office program
/// cannot be written again.
static WRITTEN: AtomicU64 = AtomicU64::new(0);

/// Writes the chart to a presentation, workbook or document in a temporary folder and opens it
/// with the program the system opens such files with.
pub fn open(chart: &Chart, program: Program) -> Result<String, String> {
    let (bytes, extension) = match program {
        Program::PowerPoint => (
            office::presentation(&[Item::Chart(chart)], true, true),
            "pptx",
        ),
        Program::Excel => (office::chart_workbook(chart), "xlsx"),
        Program::Word => (office::document(&[Item::Chart(chart)], true), "docx"),
    };
    let bytes = bytes.map_err(|e| e.to_string())?;
    let dir = std::env::temp_dir().join(format!("statscraft-charts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name: String = chart
        .title
        .chars()
        .map(|c| match c.is_alphanumeric() || " -_".contains(c) {
            true => c,
            false => '_',
        })
        .take(60)
        .collect();
    let n = WRITTEN.fetch_add(1, Ordering::Relaxed) + 1;
    let path = dir.join(format!("{} {n}.{extension}", name.trim()));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    open_with_system(&path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    Ok(format!("chart opened from {}", path.display()))
}

/// Opens a file with the program the system has for its type. The window's links go to the web
/// browser instead, which would download an office file rather than open it.
fn open_with_system(path: &Path) -> std::io::Result<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = std::process::Command::new(program).arg(path).spawn()?;
    // collected when it ends, so no finished process is left behind
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Puts the chart on the clipboard as an editable chart with its workbook, which the
/// presentation and word processing programs paste as their own chart in their theme, with the
/// data table as tab-separated text beside it for every other program. False when the chart
/// is not an office chart.
#[cfg(windows)]
pub fn copy(chart: &Chart, text: &str) -> Result<bool, String> {
    use clipboard_win::{options::NoClear, raw};
    let Some(package) = office::clipboard_package(chart).map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let format = clipboard_win::register_format(office::CLIPBOARD_FORMAT)
        .ok_or("the clipboard format could not be registered")?;
    let failed = |e: clipboard_win::ErrorCode| format!("the clipboard refused the chart: {e}");
    let _open = clipboard_win::Clipboard::new_attempts(10).map_err(failed)?;
    raw::empty().map_err(failed)?;
    raw::set_without_clear(format.get(), &package).map_err(failed)?;
    raw::set_string_with(text, NoClear).map_err(failed)?;
    // shortcut: no picture beside the package until the window draws charts (charts-plan.md
    // step 4); programs that take only pictures paste the text then.
    Ok(true)
}

/// shortcut: on macOS the package goes on the pasteboard as com.microsoft.Art--GVML-ClipFormat,
/// which needs a Mac to test (charts-slice1.md section 7); until then Copy puts the data table
/// as text, as on Linux.
#[cfg(not(windows))]
pub fn copy(_chart: &Chart, _text: &str) -> Result<bool, String> {
    Ok(false)
}
