//! Damaged data files: a .sav, .zsav, .por, CSV, text or workbook that is empty, cut short, has
//! the wrong magic bytes or a header that declares absurd counts or widths. Each is made by
//! mutating a file the engine wrote itself. Reading must end in a result or an error within the
//! time limit, never in a panic, and no single allocation may be driven by a number of the
//! header that the file's size does not support.

use statscraft_data::Dataset;
use statscraft_engine::session::Session;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::Duration;

/// Notes the largest single allocation of the threads that are being watched: the one that runs
/// a read and the workers of the pool it uses. Other threads of the test binary are not watched.
struct Watch;

thread_local! {
    static WATCHING: Cell<bool> = const { Cell::new(false) };
}

static BIGGEST: AtomicUsize = AtomicUsize::new(0);

fn note(size: usize) {
    if WATCHING.try_with(Cell::get).unwrap_or(false) {
        BIGGEST.fetch_max(size, Ordering::Relaxed);
    }
}

// SAFETY: every call goes to the system allocator unchanged; only a number is noted.
unsafe impl GlobalAlloc for Watch {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static WATCHED: Watch = Watch;

/// One read at a time: the number noted is the biggest of that read.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The pool the reads run on, its workers watched.
fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .stack_size(64 << 20)
            .start_handler(|_| WATCHING.with(|w| w.set(true)))
            .build()
            .unwrap()
    })
}

/// The most one allocation of a read may ask for: the file's size times this, plus a floor for
/// buffers that do not depend on the file.
const PER_BYTE: usize = 64;
const FLOOR: usize = 1 << 20;

/// Runs `read` on the watched pool: Ok when it ends in a result or an error in time and asks for
/// no allocation its file's size does not support.
fn watched(
    label: &str,
    size: usize,
    read: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    BIGGEST.store(0, Ordering::Relaxed);
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pool().install(read)));
        let _ = send.send(result);
    });
    match receive.recv_timeout(Duration::from_secs(60)) {
        Ok(Ok(_)) => {}
        Ok(Err(panic)) => {
            let text = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            return Err(format!("{label}: panic: {text}"));
        }
        Err(_) => return Err(format!("{label}: {HANG}")),
    }
    let biggest = BIGGEST.load(Ordering::Relaxed);
    if biggest > size * PER_BYTE + FLOOR {
        return Err(format!(
            "{label}: one allocation of {biggest} bytes for a file of {size}"
        ));
    }
    Ok(())
}

const HANG: &str = "no end within 60 s";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-damaged-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the files the damaged ones are made from.
fn good_files(dir: &Path) {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let source = format!(
        "CD '{}'.\n\
         DATA LIST LIST /id (F4) grp (F1) score (F6.2) born (ADATE10) name (A12).\n\
         BEGIN DATA\n\
         1 1 4.5 01/02/2020 'Ann Lee'\n2 2 . 03/04/2021 'Bob'\n3 1 7.25 12/31/1999 'Cy'\n\
         4 9 1 02/29/2024 'Dee'\n\
         END DATA.\n\
         VARIABLE LABELS score 'Test score' grp 'Group'.\n\
         VALUE LABELS grp 1 'Yes' 2 'No' 9 'Unknown'.\n\
         MISSING VALUES grp (9) score (-1 THRU 0).\n\
         DOCUMENT A note for the file.\n\
         VARIABLE ATTRIBUTE VARIABLES=score ATTRIBUTE=source('wave 1').\n\
         MRSETS /MDGROUP NAME=$m CATEGORYLABELS=VARLABELS VARIABLES=id grp VALUE=1.\n\
         FORMATS born (DATE11).\n\
         WEIGHT BY grp.\n\
         SAVE OUTFILE='good.sav'.\n\
         SAVE OUTFILE='good.zsav' /ZCOMPRESSED.\n\
         SAVE OUTFILE='good_plain.sav' /UNCOMPRESSED.\n\
         EXPORT OUTFILE='good.por'.\n\
         SAVE TRANSLATE OUTFILE='good.csv' /TYPE=CSV /FIELDNAMES /REPLACE.\n\
         SAVE TRANSLATE OUTFILE='good.xlsx' /TYPE=XLSX /FIELDNAMES /REPLACE.",
        dir.display()
    );
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
}

/// The values a field of the header or the dictionary is set to: zero, small, the edges of the
/// sizes of 8, 16 and 32 bits.
const EXTREME: [i32; 9] = [0, 1, 2, 255, 65_535, 0x10000000, i32::MAX, -1, i32::MIN];

/// DAMAGE_FULL=1 runs every offset and thousands of random byte changes as well (minutes);
/// the default set is the part of it that runs in a few seconds.
fn full() -> bool {
    std::env::var("DAMAGE_FULL").is_ok()
}

/// A xorshift sequence: the same mutants on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The damaged copies of `good`: empty, a byte, wrong magic, cut at many places, noise, and a
/// four-byte field set to each extreme value at every `step`th offset of the first `span` bytes
/// and of the last `tail` bytes.
fn mutants(good: &[u8], span: usize, tail: usize, step: usize) -> Vec<(String, Vec<u8>)> {
    mutants_with(good, span, tail, step, 6000)
}

fn mutants_with(
    good: &[u8],
    span: usize,
    tail: usize,
    step: usize,
    flips: usize,
) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = vec![
        ("empty".into(), Vec::new()),
        ("one byte".into(), good[..1].to_vec()),
        ("nothing but zeros".into(), vec![0; 4096]),
    ];
    let mut magic = good.to_vec();
    magic[..4].copy_from_slice(b"JUNK");
    out.push(("wrong magic".into(), magic));
    let mut rng = Rng(0x5eed_f11e);
    out.push((
        "noise".into(),
        (0..3000).map(|_| rng.next() as u8).collect(),
    ));
    let len = good.len();
    for cut in (1..len)
        .step_by((len / 80).max(1))
        .chain([len - 1, len - 2])
    {
        out.push((format!("cut at {cut}"), good[..cut].to_vec()));
    }
    for cut in 1..=16.min(len) {
        out.push((format!("cut at {cut}"), good[..cut].to_vec()));
    }
    if full() {
        // a few bytes changed at random places, the way a damaged disk or transfer does it
        for k in 0..flips {
            let mut copy = good.to_vec();
            for _ in 0..1 + k % 4 {
                let at = rng.next() as usize % len;
                copy[at] = rng.next() as u8;
            }
            out.push((format!("random change {k}"), copy));
        }
    }
    let step = if full() { 1 } else { step };
    let last = len.saturating_sub(4);
    let places = (0..span.min(last)).chain(len.saturating_sub(tail)..last);
    for at in places.step_by(step) {
        for value in EXTREME {
            let mut copy = good.to_vec();
            copy[at..at + 4].copy_from_slice(&value.to_le_bytes());
            out.push((format!("{value} at {at}"), copy));
        }
    }
    out
}

fn check(
    files: Vec<(String, Vec<u8>)>,
    name: &str,
    dir: &Path,
    read: fn(&Path) -> Result<(), String>,
) {
    let mut troubles = Vec::new();
    let path = dir.join(name);
    for (label, bytes) in files {
        std::fs::write(&path, &bytes).unwrap();
        let copy = path.clone();
        if let Err(trouble) = watched(&format!("{name} {label}"), bytes.len(), move || read(&copy))
        {
            let hang = trouble.ends_with(HANG);
            troubles.push(trouble);
            // the thread that hangs may still read the file the next mutant would replace
            if hang {
                break;
            }
        }
    }
    report(troubles);
}

fn report(troubles: Vec<String>) {
    assert!(
        troubles.is_empty(),
        "{} troubles, the first ones:\n{}",
        troubles.len(),
        troubles
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn read_sav(path: &Path) -> Result<(), String> {
    Dataset::read_sav(path)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn read_por(path: &Path) -> Result<(), String> {
    Dataset::read_por(path)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn damaged_sav_files_give_an_error_and_no_huge_allocation() {
    let dir = temp_dir("sav");
    good_files(&dir);
    // the dictionary of the plain file at every offset, the other two at every third
    for (file, step) in [("good_plain.sav", 2), ("good.sav", 6), ("good.zsav", 6)] {
        let good = std::fs::read(dir.join(file)).unwrap();
        check(
            mutants(&good, 1800, 64, step),
            "damaged.sav",
            &dir,
            read_sav,
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn damaged_portable_files_give_an_error_and_no_huge_allocation() {
    let dir = temp_dir("por");
    good_files(&dir);
    let good = std::fs::read(dir.join("good.por")).unwrap();
    let mut files = mutants(&good, 0, 0, 1);
    // a number made huge, and a letter changed, at every third place
    let mut rng = Rng(0x00b0_d1e5);
    for at in (0..good.len()).step_by(3) {
        let mut copy = good.clone();
        if copy[at].is_ascii_digit() {
            copy[at] = b'9';
        } else {
            copy[at] = b"9/ZQ0"[(rng.next() % 5) as usize];
        }
        files.push((format!("changed at {at}"), copy));
    }
    check(files, "damaged.por", &dir, read_por);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A text file the readers get: damaged, or good but read with a header that declares absurd
/// counts, widths and columns.
fn text_bytes() -> Vec<(String, Vec<u8>)> {
    let good = b"id,name,score\n1,Ann,4.5\n2,Bob,\n3,\"Cy, Jr\",7.25\n".to_vec();
    let mut out = mutants_with(&good, 0, 0, 1, 300);
    out.truncate(if full() { usize::MAX } else { 12 });
    out.push(("good".into(), good));
    out.push(("one long line".into(), "1,".repeat(30_000).into_bytes()));
    out.push(("one long value".into(), "x".repeat(300_000).into_bytes()));
    out
}

/// What reads the file `F`, with numbers of the header at their edges.
const HUGE_READERS: &[&str] = &[
    "GET DATA /TYPE=TXT /FILE='F' /FIRSTCASE=100000000 /VARIABLES=a F8 b A8 c F8.",
    "GET DATA /TYPE=TXT /FILE='F' /IMPORTCASE=FIRST 100000000 /VARIABLES=a F8 b A8 c F8.",
    "GET DATA /TYPE=TXT /FILE='F' /DELCASE=VARIABLES 100000000 /VARIABLES=a F8 b A8.",
    "GET DATA /TYPE=TXT /FILE='F' /VARIABLES=a A65535 b A65535 c A65535.",
    "GET DATA /TYPE=TXT /FILE='F' /VARIABLES=a F65535.9 b F8.",
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=FIXED /FIXCASE=100000000 /VARIABLES=a 0-3 F4.",
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=FIXED /VARIABLES=a 0-65535 A65535.",
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=FIXED /VARIABLES=a 100000000-100000000 A1.",
    "GET DATA /TYPE=TXT /FILE='F' /ARRANGEMENT=FIXED /VARIABLES=/99999 a 0-3 F4.",
    "DATA LIST FILE='F' FIXED /a 1-1000000000 (A).",
    "DATA LIST FILE='F' FIXED /a 1000000000-1000000000 (A).",
    "DATA LIST FILE='F' RECORDS=1000000000 /a 1-2.",
    "DATA LIST FILE='F' FREE /a (A32767) b (A32767).",
    "DATA LIST FILE='F' LIST /a1 TO a10000 (F8).",
    "INPUT PROGRAM.\nDATA LIST FILE='F' /n 1.\nREPEATING DATA STARTS=2 /OCCURS=1000000000 /DATA=v 1 (A).\n\
     END INPUT PROGRAM.\nLIST.",
    "FILE TYPE MIXED FILE='F' RECORD=#r 1000000000-1000000000 (A).\nRECORD TYPE 'a'.\n\
     DATA LIST /x 2-3.\nEND FILE TYPE.\nLIST.",
    "MATRIX DATA VARIABLES=a b c /FILE='F' /N=1000000000 /CONTENTS=CORR N.\nLIST.",
];

fn text_reader(syntax: String, dir: PathBuf) -> impl FnOnce() -> Result<(), String> + Send {
    move || {
        let mut session = Session::default();
        session.set_base_dir(Some(dir));
        session
            .run(&syntax, None)
            .map(|_| ())
            .map_err(|e| format!("{e:?}"))
    }
}

#[test]
fn text_files_with_absurd_header_numbers_give_an_answer_and_no_huge_allocation() {
    // a reader the syntax refuses would test nothing
    let refused: Vec<String> = HUGE_READERS
        .iter()
        .filter_map(|reader| {
            let (_, errors) = statscraft_syntax::parse(reader);
            errors.first().map(|e| format!("{reader}: {}", e.message))
        })
        .collect();
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    let dir = temp_dir("text");
    let mut troubles = Vec::new();
    'files: for (name, bytes) in text_bytes() {
        std::fs::write(dir.join("damaged.txt"), &bytes).unwrap();
        for reader in HUGE_READERS {
            let syntax = reader.replace("'F'", "'damaged.txt'");
            let label = format!("{name}: {syntax}");
            if let Err(trouble) = watched(&label, bytes.len(), text_reader(syntax, dir.clone())) {
                let hang = trouble.ends_with(HANG);
                troubles.push(trouble);
                // the thread that hangs may still read the file the next one would replace
                if hang {
                    break 'files;
                }
            }
        }
    }
    report(troubles);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn damaged_workbooks_give_an_error_and_no_huge_allocation() {
    let dir = temp_dir("book");
    good_files(&dir);
    let good = std::fs::read(dir.join("good.xlsx")).unwrap();
    // the local headers at the start hold the sizes; the directory at the end holds them again
    let files = mutants(&good, 120, 400, 2);
    let mut troubles = Vec::new();
    'books: for (label, bytes) in files {
        std::fs::write(dir.join("damaged.xlsx"), &bytes).unwrap();
        for reader in [
            "GET DATA /TYPE=XLSX /FILE='damaged.xlsx' /READNAMES=ON.",
            "GET DATA /TYPE=XLSX /FILE='damaged.xlsx' /READNAMES=OFF /CELLRANGE=RANGE 'A1:XFD1048576'.",
        ] {
            let name = format!("{label}: {reader}");
            if let Err(trouble) =
                watched(&name, bytes.len(), text_reader(reader.into(), dir.clone()))
            {
                let hang = trouble.ends_with(HANG);
                troubles.push(trouble);
                if hang {
                    break 'books;
                }
            }
        }
    }
    report(troubles);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_watch_sees_the_allocations_of_the_pool_workers() {
    let outcome = watched("probe", 100, || {
        let (big, _) = rayon::join(|| vec![1u8; 8 << 20], || 1);
        std::hint::black_box(big);
        Ok(())
    });
    assert!(outcome.unwrap_err().contains("one allocation"));
    assert!(watched("small", 100, || Ok(())).is_ok());
}
