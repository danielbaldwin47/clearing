//! Command line, the --scan and --snapshot exits, and the terminal loop: browse and scan-time keys, and the scan, delete and Trash workers.
mod collector;
mod delete;
mod platform;
/// PROTOTYPE ONLY: the concept mock-up's sample tree, for `--sample`.
mod sample;
mod scan;
mod theme;
mod trash;
mod ui;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{CrosstermBackend, TestBackend},
    style::{Color, Style},
    widgets::Paragraph,
};
use std::{
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

type AppTerminal = Terminal<CrosstermBackend<io::Stdout>>;
struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(e);
        }
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
fn main() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        hook(info)
    }));
    if let Err(e) = run() {
        eprintln!("clearing: {e}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = PathBuf::from(".");
    let mut scan_only = false;
    let mut snapshot = None;
    let mut width = 140;
    let mut height = 44;
    let mut json = false;
    let mut wireframe = false;
    // PROTOTYPE: `--sample` swaps the scan for the mock-up's tree; `--variant`
    // picks today's screen (a) or the ported first proposal (b).
    let mut sample_data = false;
    let mut variant = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_string_lossy().as_ref() {
            "--version" | "-V" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "clearing — find what ate your disk\n\nUsage: clearing [OPTIONS] [PATH]\n\n  --scan             Scan and exit without starting the terminal interface\n  --json, --summary   Print a summary JSON object (with --scan)\n  --snapshot STATE   Render overview, drilled, delete, trash, or collector as ANSI\n                     (also collector-browse, collector-confirm,\n                     collector-errors, collector-empty)\n  --width N          Snapshot columns (default 140)\n  --height N         Snapshot rows (default 44)\n  -V, --version      Show the package version\n  -h, --help         Show this help\n\nKeys: ↑↓ / jk select · Enter open · Backspace back · d delete\n      Home / End first / last · PgUp / PgDn move eight entries\n      t move selected item to Trash · Space collect · c review collector\n      r rescan · ? help · q quit\n      Esc cancels a rescan or dialog; quits otherwise\n\nSizes include allocated file and directory blocks. Symlinks are not followed.\nHard links count once. Deletion is permanent and requires typing delete.\nCollected items use the desktop Trash after typing trash; space is freed\nwhen Trash is emptied. A failed move never falls back to deletion."
                );
                return Ok(());
            }
            "--scan" => scan_only = true,
            "--sample" => sample_data = true,
            "--variant" => {
                variant = Some(
                    args.next()
                        .and_then(|v| ui::prototype::Variant::parse(&v.to_string_lossy()))
                        .ok_or("--variant requires a to i")?,
                )
            }
            "--wireframe" => wireframe = true,
            "--json" | "--summary" => json = true,
            "--snapshot" => snapshot = Some(args.next().ok_or("--snapshot requires a state")?),
            "--width" => {
                width = args
                    .next()
                    .ok_or("--width requires an integer")?
                    .to_string_lossy()
                    .parse()?
            }
            "--height" => {
                height = args
                    .next()
                    .ok_or("--height requires an integer")?
                    .to_string_lossy()
                    .parse()?
            }
            "--" => {
                if let Some(p) = args.next() {
                    path = PathBuf::from(p)
                }
                if args.next().is_some() {
                    return Err("expected one directory path".into());
                }
                break;
            }
            _ if a.as_encoded_bytes().first() == Some(&b'-') => {
                return Err(format!("unknown option: {}", a.to_string_lossy()).into());
            }
            _ => path = PathBuf::from(a),
        }
    }
    let prototype_flags = sample_data || variant.is_some();
    let variant = variant.unwrap_or(ui::prototype::Variant::A);
    if scan_only || snapshot.is_some() {
        let started = Instant::now();
        let root = if sample_data {
            sample::tree()
        } else {
            scan::scan(&path, Arc::new(AtomicU64::new(0)))?
        };
        let seconds = started.elapsed().as_secs_f64();
        if scan_only {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"path":scan::display_path(root.path.as_os_str()),"bytes":root.bytes,"apparent_bytes":root.apparent,"files":root.files,"directories":root.directories,"errors":root.errors,"elapsed_seconds":seconds})
                );
            } else {
                println!(
                    "{}\t{}\t{} files\t{} errors\t{seconds:.6}s",
                    ui::size(root.bytes),
                    scan::display_path(root.path.as_os_str()),
                    root.files,
                    root.errors
                );
            }
            return Ok(());
        }
        if width == 0 || height == 0 || width > 1000 || height > 1000 {
            return Err("snapshot dimensions must be between 1 and 1000".into());
        }
        let mut app = ui::App::new(root, 0.04);
        let mut proto = ui::prototype::Proto::new(variant, sample_data, &app.root.path);
        if sample_data {
            proto.init_sample(&mut app)
        }
        let state = snapshot.unwrap();
        if state == "trash" {
            app.open_single_trash()
        } else if state == "drilled" {
            app.drill()
        } else if state == "delete" {
            app.confirm = true
        } else if let Some(collector_state) =
            state.to_str().and_then(|s| s.strip_prefix("collector"))
        {
            if collector_state != "-empty" {
                snapshot_collect(&mut app);
            }
            match collector_state {
                "-browse" => {}
                "" | "-empty" => app.open_review(),
                "-confirm" => {
                    app.open_review();
                    app.trash_confirm = true
                }
                "-errors" => {
                    // A fabricated report, applied through the real path, shows
                    // how refused and failed items look. Nothing is executed.
                    let outcomes = [
                        trash::Outcome::Failed(
                            "Trashing on system internal mounts is not supported".into(),
                        ),
                        trash::Outcome::Refused(
                            "path now names a different item than the one collected".into(),
                        ),
                    ];
                    let report = trash::Report {
                        outcomes: app
                            .collector
                            .records()
                            .iter()
                            .skip(1)
                            .zip(outcomes)
                            .map(|(r, o)| (r.path.clone(), o))
                            .collect(),
                        attempted: 1,
                        cancelled: false,
                    };
                    app.collector.apply(&report);
                    app.open_review();
                    app.review_selected = 1.min(app.collector.len().saturating_sub(1));
                    app.message = report.summary();
                }
                _ => return Err("unknown collector snapshot state".into()),
            }
        } else if state != "overview" {
            return Err(
                "snapshot state must be overview, drilled, delete, trash, or collector[-browse|-confirm|-errors|-empty]"
                    .into(),
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|f| {
            if wireframe {
                ui::draw_wireframe(f, &app)
            } else if prototype_flags {
                ui::prototype::draw(f, &app, &proto)
            } else {
                ui::draw(f, &app)
            }
        })?;
        let buffer = terminal.backend().buffer();
        let mut out = io::stdout().lock();
        write!(out, "\x1b[2J\x1b[H")?;
        for y in 0..height {
            write!(out, "\x1b[{};1H", y + 1)?;
            for x in 0..width {
                let cell = &buffer[(x, y)];
                ansi_color(&mut out, cell.fg, false)?;
                ansi_color(&mut out, cell.bg, true)?;
                write!(
                    out,
                    "{}{}",
                    if cell.modifier.contains(ratatui::style::Modifier::BOLD) {
                        "\x1b[1m"
                    } else {
                        "\x1b[22m"
                    },
                    cell.symbol()
                )?;
            }
        }
        write!(out, "\x1b[0m\x1b[?25l")?;
        return Ok(());
    }
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        return Err("interactive mode requires a terminal; use --scan".into());
    }
    let guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (root, seconds) = if sample_data {
        (sample::tree(), 0.4)
    } else {
        let Some(scanned) = scan_in_terminal(&mut terminal, &path, ScanKind::Initial)? else {
            return Ok(());
        };
        scanned
    };
    let mut app = ui::App::new(root, seconds);
    let mut proto = ui::prototype::Proto::new(variant, sample_data, &app.root.path);
    if sample_data {
        proto.init_sample(&mut app)
    }
    loop {
        terminal.draw(|f| {
            if wireframe {
                ui::draw_wireframe(f, &app)
            } else {
                ui::prototype::draw(f, &app, &proto)
            }
        })?;
        // No worker is running here; input or resize wakes the next redraw.
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            break;
        }
        // PROTOTYPE: F2 cycles the variants live, from any state.
        if key.code == KeyCode::F(2) {
            proto.cycle();
            continue;
        }
        // PROTOTYPE: the sample tree names nothing on disk. Trash, delete and
        // rescan are refused outright so no real path can ever be touched.
        if proto.sample && !app.help {
            let refused = match key.code {
                KeyCode::Char('t') => true,
                KeyCode::Char('d') | KeyCode::Char('r') => !app.review,
                _ => false,
            };
            if refused {
                app.message = "Sample data · Trash, delete and rescan are switched off".into();
                continue;
            }
        }
        if app.confirm {
            if app.delete_confirm_key(key.code) {
                let name = app.selection().map(|n| n.name.clone()).unwrap_or_default();
                let acted_on = app.selection().map(|node| node.path.clone());
                let root_path = app.root.path.clone();
                let outcome = match delete_in_terminal(&mut terminal, &app, &name) {
                    Ok(outcome) => outcome,
                    Err(e) => {
                        drop(terminal);
                        drop(guard);
                        return Err(e.into());
                    }
                };
                let summary = delete_message(&name, &outcome);
                if matches!(outcome.status, delete::Status::Completed)
                    && acted_on
                        .as_deref()
                        .is_some_and(|path| app.remove_selection(path))
                {
                    app.message = summary;
                    continue;
                }
                // Partial outcomes and shared allocations need a fresh scan.
                match rescan_after_delete(&mut terminal, &root_path, &summary) {
                    Ok(Some((root, seconds))) => {
                        // The collector outlives the tree it was picked from.
                        app = app.rebuild(root, seconds);
                        app.message = summary;
                    }
                    Ok(None) => {
                        drop(terminal);
                        drop(guard);
                        let note = format!("{summary}; exited without a current scan");
                        if matches!(outcome.status, delete::Status::Completed) {
                            eprintln!("clearing: {note}");
                            return Ok(());
                        }
                        return Err(note.into());
                    }
                    Err(e) => {
                        drop(terminal);
                        drop(guard);
                        return Err(format!("{summary}; terminal error: {e}").into());
                    }
                }
            }
            continue;
        }
        if app.single_trash.is_some() {
            if !app.single_trash_key(key.code) {
                continue;
            }
            let record = app
                .single_trash
                .take()
                .expect("confirmed direct Trash record");
            let root_path = app.root.path.clone();
            let report = match trash_in_terminal(&mut terminal, std::slice::from_ref(&record)) {
                Ok(report) => report,
                Err(e) => {
                    drop(terminal);
                    drop(guard);
                    return Err(e.into());
                }
            };
            // Only the same scanned item leaves the collector, and only when it
            // moved. Every other record stays authorized by its own identity.
            let outcome = report.outcomes.first().map(|(_, outcome)| outcome);
            app.settle_single_trash(&record, outcome);
            let label = record
                .path
                .file_name()
                .map(scan::display_path)
                .unwrap_or_default();
            let summary = single_trash_summary(&label, outcome);
            if matches!(outcome, Some(trash::Outcome::Trashed))
                && !report.cancelled
                && app.remove_selection(&record.path)
            {
                app.message = summary;
                continue;
            }
            match rescan_after_delete(&mut terminal, &root_path, &summary) {
                Ok(Some((root, seconds))) => {
                    app = app.rebuild(root, seconds);
                    app.message = summary;
                }
                Ok(None) => {
                    drop(terminal);
                    drop(guard);
                    eprintln!("clearing: {summary}; exited without a current scan");
                    return Ok(());
                }
                Err(e) => {
                    drop(terminal);
                    drop(guard);
                    return Err(format!("{summary}; terminal error: {e}").into());
                }
            }
            continue;
        }
        if app.review {
            if !app.review_key(key.code) {
                continue;
            }
            let root_path = app.root.path.clone();
            let report = match trash_in_terminal(&mut terminal, app.collector.records()) {
                Ok(report) => report,
                Err(e) => {
                    drop(terminal);
                    drop(guard);
                    return Err(e.into());
                }
            };
            // Moved items leave the collector; refused, failed and unprocessed
            // ones stay with their reason. Nothing is ever deleted instead.
            app.collector.apply(&report);
            let summary = report.summary();
            // A refusal can reveal a concurrent filesystem change even when
            // gio never ran. Refresh the view after every confirmed batch.
            match rescan_after_delete(&mut terminal, &root_path, &summary) {
                Ok(Some((root, seconds))) => {
                    app = app.rebuild(root, seconds);
                    app.review = !app.collector.is_empty();
                    app.message = summary;
                }
                Ok(None) => {
                    drop(terminal);
                    drop(guard);
                    eprintln!("clearing: {summary}; exited without a current scan");
                    return Ok(());
                }
                Err(e) => {
                    drop(terminal);
                    drop(guard);
                    return Err(format!("{summary}; terminal error: {e}").into());
                }
            }
            continue;
        }
        if app.help {
            app.help = false;
            continue;
        }
        // PROTOTYPE, variant B only: Tab selects one Tile deeper on the Map and
        // Space collects that nested Tile. Any other key drops back to the
        // List's own selection, so variant A never sees a difference.
        if proto.variant == ui::prototype::Variant::B {
            match key.code {
                KeyCode::Tab => {
                    proto.deeper(&app);
                    continue;
                }
                KeyCode::Char(' ') if !proto.deep.is_empty() => {
                    proto.toggle_deep(&mut app);
                    continue;
                }
                _ => {}
            }
        }
        proto.deep.clear();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Down | KeyCode::Char('j') => app.next(1),
            KeyCode::Up | KeyCode::Char('k') => app.next(-1),
            KeyCode::Home => app.selected = 0,
            KeyCode::End => app.selected = app.current().children.len().saturating_sub(1),
            KeyCode::PageDown => app.next(8),
            KeyCode::PageUp => app.next(-8),
            KeyCode::Enter | KeyCode::Right => app.drill(),
            KeyCode::Backspace | KeyCode::Left => app.back(),
            KeyCode::Char('d') if app.selection().is_some() => {
                app.confirm = true;
                app.typed.clear()
            }
            KeyCode::Char('t') if app.selection().is_some() => app.open_single_trash(),
            KeyCode::Char(' ') => app.toggle_collect(),
            KeyCode::Char('c') => app.open_review(),
            KeyCode::Char('?') => app.help = true,
            KeyCode::Char('r') => {
                let root_path = app.root.path.clone();
                match scan_in_terminal(&mut terminal, &root_path, ScanKind::Rescan) {
                    Ok(Some((root, seconds))) => app = app.rebuild(root, seconds),
                    Ok(None) => app.message = "Rescan cancelled; previous results retained".into(),
                    Err(e) => app.message = format!("Rescan failed: {e}"),
                }
            }
            _ => {}
        }
    }
    Ok(())
}
/// Deterministic collector contents for snapshots: the two largest root
/// entries plus one item from inside the next folder, as a user picking across
/// directories would leave it.
fn snapshot_collect(app: &mut ui::App) {
    let count = app.current().children.len();
    for index in 0..count.min(2) {
        app.selected = index;
        app.toggle_collect();
    }
    let nested = (2..count).find(|&i| !app.current().children[i].children.is_empty());
    if let Some(index) = nested {
        app.selected = index;
        app.drill();
        app.toggle_collect();
        app.back();
    }
    app.selected = 0;
    app.message.clear();
}
fn single_trash_summary(label: &str, outcome: Option<&trash::Outcome>) -> String {
    match outcome {
        Some(trash::Outcome::Trashed) => {
            format!("Moved {label} to Trash · space is freed when Trash is emptied")
        }
        Some(trash::Outcome::Refused(why)) => format!("Trash refused for {label}: {why}"),
        Some(trash::Outcome::Failed(why)) => format!("Trash failed: {why} · {label}"),
        _ => format!("Trash cancelled for {label}; item was not processed"),
    }
}

/// Run the confirmed batch on a worker while this thread keeps drawing. Esc
/// asks the worker to stop before the next item; a gio process that is already
/// running is left to finish, and the worker is always joined.
fn trash_in_terminal(
    terminal: &mut AppTerminal,
    records: &[collector::Record],
) -> io::Result<trash::Report> {
    let cancel = AtomicBool::new(false);
    let done = AtomicUsize::new(0);
    // Mounts can change while the user builds the collection. Recheck current
    // protected locations when executing, retaining the collected identities.
    let guard = trash::Guard::from_system();
    let (report, ui_error) = std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("trash".into())
            .spawn_scoped(scope, || {
                trash::run_batch(records, &guard, &trash::DesktopTrash, &cancel, &done)
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(e) => return (None, Some(e)),
        };
        let mut ui_error = None;
        while !worker.is_finished() {
            if let Err(e) = trash_frame(terminal, records, &cancel, &done) {
                cancel.store(true, Ordering::SeqCst);
                ui_error = Some(e);
                break;
            }
        }
        (worker.join().ok(), ui_error)
    });
    match (report, ui_error) {
        (Some(report), None) => Ok(report),
        (Some(report), Some(e)) => Err(io::Error::other(format!(
            "terminal error while moving items to Trash: {e}; {}",
            report.summary()
        ))),
        (None, Some(e)) => Err(e),
        (None, None) => Err(io::Error::other("trash worker panicked")),
    }
}
fn trash_frame(
    terminal: &mut AppTerminal,
    records: &[collector::Record],
    cancel: &AtomicBool,
    done: &AtomicUsize,
) -> io::Result<()> {
    let finished = done.load(Ordering::SeqCst);
    let hint = if cancel.load(Ordering::SeqCst) {
        "Stopping after the current item; the rest stay collected"
    } else {
        "Esc stop after the current item (items already moved stay in Trash)"
    };
    let current = records
        .get(finished)
        .map(|r| scan::display_path(r.path.as_os_str()))
        .unwrap_or_default();
    terminal.draw(|f| {
        let area = f.area();
        f.render_widget(
            Paragraph::new(format!(
                "\n  Moving items to Trash\n\n  items finished: {finished} of {}\n  {current}\n\n  {hint}",
                records.len()
            ))
            .style(Style::default().fg(theme::FG).bg(theme::BG)),
            area,
        );
    })?;
    if event::poll(Duration::from_millis(40))?
        && let Event::Key(k) = event::read()?
        && k.kind != KeyEventKind::Release
        && (k.code == KeyCode::Esc
            || (k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c')))
    {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}
enum ScanKind {
    Initial,
    Rescan,
}

fn scan_in_terminal(
    terminal: &mut AppTerminal,
    path: &Path,
    kind: ScanKind,
) -> io::Result<Option<(scan::Node, f64)>> {
    let (tx, rx) = mpsc::sync_channel(1);
    let progress = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let p = path.to_path_buf();
    let worker_progress = progress.clone();
    let worker_cancel = cancel.clone();
    std::thread::spawn(move || {
        let now = Instant::now();
        let result = scan::scan_cancellable(&p, worker_progress, worker_cancel)
            .map(|n| (n, now.elapsed().as_secs_f64()));
        let _ = tx.send(result);
    });
    loop {
        match rx.try_recv() {
            Ok(result) => return result.map(Some),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(io::Error::other("scan worker stopped"));
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        terminal.draw(|f| {
            let area = f.area();
            f.render_widget(
                Paragraph::new(format!(
                    "\n  Reading disk allocation\n\n  {} entries scanned\n\n  Esc {}",
                    progress.load(Ordering::Relaxed),
                    match kind {
                        ScanKind::Initial => "quit",
                        ScanKind::Rescan => "cancel",
                    }
                ))
                .style(Style::default().fg(theme::FG).bg(theme::BG)),
                area,
            );
        })?;
        if event::poll(Duration::from_millis(40))?
            && let Event::Key(k) = event::read()?
            && (matches!(k.code, KeyCode::Esc | KeyCode::Char('q'))
                || (k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c')))
        {
            cancel.store(true, Ordering::Relaxed);
            return Ok(None);
        }
    }
}
/// Footer text for a finished deletion. Counts are entries actually unlinked;
/// nothing here claims that disk space was reclaimed.
fn delete_message(name: &str, outcome: &delete::Outcome) -> String {
    let n = outcome.removed;
    match &outcome.status {
        delete::Status::Completed => format!("Deleted {name} · entries removed: {n}"),
        delete::Status::Cancelled if n == 0 => {
            format!("Delete cancelled: {name} untouched · entries removed: 0")
        }
        delete::Status::Cancelled => {
            format!("Delete stopped by user: {name} partially remains · entries removed: {n}")
        }
        delete::Status::Failed(e) if n == 0 => {
            format!("Delete stopped: {e} · entries removed: 0")
        }
        delete::Status::Failed(e) => {
            format!("Delete stopped: {e} · {name} partially remains · entries removed: {n}")
        }
    }
}
/// Run the confirmed deletion on a scoped worker that borrows the scanned tree
/// while this thread keeps drawing and listening for Esc. Any terminal error
/// requests cancellation, and the worker is always joined before returning.
fn delete_in_terminal(
    terminal: &mut AppTerminal,
    app: &ui::App,
    name: &str,
) -> io::Result<delete::Outcome> {
    let cancel = AtomicBool::new(false);
    let removed = AtomicU64::new(0);
    let (outcome, ui_error) = std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("delete".into())
            .stack_size(64 << 20)
            .spawn_scoped(scope, || {
                delete::remove_cancellable(&app.root, &app.route, app.selected, &cancel, &removed)
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(e) => {
                let outcome = delete::Outcome {
                    removed: 0,
                    status: delete::Status::Failed(e),
                };
                return (outcome, None);
            }
        };
        let mut ui_error = None;
        while !worker.is_finished() {
            if let Err(e) = delete_frame(terminal, name, &cancel, &removed) {
                cancel.store(true, Ordering::SeqCst);
                ui_error = Some(e);
                break;
            }
        }
        let outcome = worker.join().unwrap_or_else(|_| delete::Outcome {
            removed: removed.load(Ordering::SeqCst),
            status: delete::Status::Failed(io::Error::other("deletion worker panicked")),
        });
        (outcome, ui_error)
    });
    match ui_error {
        Some(e) => Err(io::Error::other(format!(
            "terminal error during deletion: {e}; {}",
            delete_message(name, &outcome)
        ))),
        None => Ok(outcome),
    }
}
fn delete_frame(
    terminal: &mut AppTerminal,
    name: &str,
    cancel: &AtomicBool,
    removed: &AtomicU64,
) -> io::Result<()> {
    let hint = if cancel.load(Ordering::SeqCst) {
        "Stopping before the next removal"
    } else {
        "Esc stop (entries already removed stay removed)"
    };
    terminal.draw(|f| {
        let area = f.area();
        f.render_widget(
            Paragraph::new(format!(
                "\n  Deleting {name}\n\n  entries removed: {}\n\n  {hint}",
                removed.load(Ordering::Relaxed)
            ))
            .style(Style::default().fg(theme::FG).bg(theme::BG)),
            area,
        );
    })?;
    if event::poll(Duration::from_millis(40))?
        && let Event::Key(k) = event::read()?
        && k.kind != KeyEventKind::Release
        && (k.code == KeyCode::Esc
            || (k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c')))
    {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}
/// Rescan until fresh results exist. A failed or cancelled rescan never falls
/// back to the pre-deletion tree: the only ways out are a retry or quitting.
fn rescan_after_delete(
    terminal: &mut AppTerminal,
    root_path: &Path,
    summary: &str,
) -> io::Result<Option<(scan::Node, f64)>> {
    loop {
        let problem = match scan_in_terminal(terminal, root_path, ScanKind::Rescan) {
            Ok(Some(fresh)) => return Ok(Some(fresh)),
            Ok(None) => "Rescan cancelled".to_string(),
            Err(e) => format!("Rescan failed: {e}"),
        };
        loop {
            terminal.draw(|f| {
                let area = f.area();
                f.render_widget(
                    Paragraph::new(format!(
                        "\n  {summary}\n\n  {problem}\n\n  Earlier sizes are out of date and will not be shown.\n\n  r rescan · q quit"
                    ))
                    .style(Style::default().fg(theme::FG).bg(theme::BG)),
                    area,
                );
            })?;
            // The rescan has stopped, so wait for input or resize without polling.
            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind == KeyEventKind::Release {
                continue;
            }
            if k.code == KeyCode::Char('q')
                || (k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c'))
            {
                return Ok(None);
            }
            if k.code == KeyCode::Char('r') {
                break;
            }
        }
    }
}
fn ansi_color(w: &mut impl Write, c: Color, bg: bool) -> io::Result<()> {
    let code = if bg { 48 } else { 38 };
    match c {
        Color::Rgb(r, g, b) => write!(w, "\x1b[{code};2;{r};{g};{b}m"),
        Color::Reset => write!(w, "\x1b[{}m", if bg { 49 } else { 39 }),
        _ => write!(w, "\x1b[{code};5;7m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_failure_summary_leads_with_reason_before_long_label() {
        let label = "very-long-file-name".repeat(10);
        let reason = "Trashing on system internal mounts is not supported";
        let outcome = trash::Outcome::Failed(reason.into());
        assert_eq!(
            single_trash_summary(&label, Some(&outcome)),
            format!("Trash failed: {reason} · {label}")
        );
    }

    #[test]
    fn trash_failure_reason_is_visible_in_an_80_column_footer() {
        let base =
            std::env::temp_dir().join(format!("clearing-failure-footer-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let root = scan::scan(&base, Arc::new(AtomicU64::new(0))).unwrap();
        let mut app = ui::App::new(root, 0.);
        let reason = "Trashing on system internal mounts is not supported";
        app.message = single_trash_summary(
            &"long-label".repeat(20),
            Some(&trash::Outcome::Failed(reason.into())),
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui::draw(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let screen = (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            screen.contains(&format!("Trash failed: {reason}")),
            "{screen}"
        );
        std::fs::remove_dir(&base).unwrap();
    }
}
