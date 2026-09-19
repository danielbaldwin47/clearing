mod delete;
mod scan;
mod theme;
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
        atomic::{AtomicBool, AtomicU64, Ordering},
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
        eprintln!("tui-disk: {e}");
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
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_string_lossy().as_ref() {
            "--help" | "-h" => {
                println!(
                    "tui-disk — find what ate your disk\n\nUsage: tui-disk [OPTIONS] [PATH]\n\n  --scan             Scan and exit without starting the terminal interface\n  --json, --summary   Print a summary JSON object (with --scan)\n  --snapshot STATE   Render overview, drilled, or delete as ANSI\n  --width N          Snapshot columns (default 140)\n  --height N         Snapshot rows (default 44)\n  -h, --help         Show this help\n\nKeys: ↑↓ / jk select · Enter open · Backspace back · d delete\n      r rescan · ? help · q quit · Esc cancels a scan or dialog\n\nSizes include allocated file and directory blocks. Symlinks are not followed.\nHard links count once. Deletion is permanent and requires typing delete."
                );
                return Ok(());
            }
            "--scan" => scan_only = true,
            "--wireframe" => wireframe = true,
            "--json" | "--summary" => json = true,
            "--no-mouse" => {}
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
    if scan_only || snapshot.is_some() {
        let started = Instant::now();
        let root = scan::scan(&path, Arc::new(AtomicU64::new(0)))?;
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
        let state = snapshot.unwrap();
        if state == "drilled" {
            app.drill()
        } else if state == "delete" {
            app.confirm = true
        } else if state != "overview" {
            return Err("snapshot state must be overview, drilled, or delete".into());
        }
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|f| {
            if wireframe {
                ui::draw_wireframe(f, &app)
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
    let Some((root, seconds)) = scan_in_terminal(&mut terminal, &path)? else {
        return Ok(());
    };
    let mut app = ui::App::new(root, seconds);
    loop {
        terminal.draw(|f| {
            if wireframe {
                ui::draw_wireframe(f, &app)
            } else {
                ui::draw(f, &app)
            }
        })?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            break;
        }
        if app.confirm {
            match key.code {
                KeyCode::Esc => {
                    app.confirm = false;
                    app.typed.clear()
                }
                KeyCode::Backspace => {
                    app.typed.pop();
                }
                KeyCode::Char(c) => {
                    if app.typed.len() < 32 {
                        app.typed.push(c)
                    }
                }
                KeyCode::Enter if app.typed == "delete" => {
                    let name = app.selection().map(|n| n.name.clone()).unwrap_or_default();
                    let current_path = app.current().path.clone();
                    let root_path = app.root.path.clone();
                    app.confirm = false;
                    app.typed.clear();
                    let outcome = match delete_in_terminal(&mut terminal, &app, &name) {
                        Ok(outcome) => outcome,
                        Err(e) => {
                            drop(terminal);
                            drop(guard);
                            return Err(e.into());
                        }
                    };
                    let summary = delete_message(&name, &outcome);
                    // The scanned tree no longer describes the disk, so it is
                    // either replaced by a fresh scan or never shown again.
                    match rescan_after_delete(&mut terminal, &root_path, &summary) {
                        Ok(Some((root, seconds))) => {
                            app = ui::App::new(root, seconds);
                            app.restore_path(&current_path);
                            app.message = summary;
                        }
                        Ok(None) => {
                            drop(terminal);
                            drop(guard);
                            let note = format!("{summary}; exited without a current scan");
                            if matches!(outcome.status, delete::Status::Completed) {
                                eprintln!("tui-disk: {note}");
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
                _ => {}
            }
            continue;
        }
        if app.help {
            app.help = false;
            continue;
        }
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
            KeyCode::Char('?') => app.help = true,
            KeyCode::Char('r') => {
                let current_path = app.current().path.clone();
                let root_path = app.root.path.clone();
                match scan_in_terminal(&mut terminal, &root_path) {
                    Ok(Some((root, seconds))) => {
                        app = ui::App::new(root, seconds);
                        app.restore_path(&current_path)
                    }
                    Ok(None) => app.message = "Rescan cancelled; previous results retained".into(),
                    Err(e) => app.message = format!("Rescan failed: {e}"),
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn scan_in_terminal(
    terminal: &mut AppTerminal,
    path: &Path,
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
                    "\n  Reading disk allocation\n\n  {} entries scanned\n\n  Esc cancel",
                    progress.load(Ordering::Relaxed)
                ))
                .style(Style::default().fg(theme::FG).bg(theme::BG)),
                area,
            );
        })?;
        if event::poll(Duration::from_millis(40))? {
            if let Event::Key(k) = event::read()? {
                if matches!(k.code, KeyCode::Esc | KeyCode::Char('q'))
                    || (k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c'))
                {
                    cancel.store(true, Ordering::Relaxed);
                    return Ok(None);
                }
            }
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
    if event::poll(Duration::from_millis(40))? {
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Release
                && (k.code == KeyCode::Esc
                    || (k.modifiers.contains(KeyModifiers::CONTROL)
                        && k.code == KeyCode::Char('c')))
            {
                cancel.store(true, Ordering::SeqCst);
            }
        }
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
        let problem = match scan_in_terminal(terminal, root_path) {
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
            if !event::poll(Duration::from_millis(100))? {
                continue;
            }
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
