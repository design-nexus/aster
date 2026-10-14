mod app;
mod aurora;
mod config;
mod editor;
mod presets;
mod preview;
mod theme;
mod ui;
mod wizard;
mod workspace;

use anyhow::Context;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io::{self, IsTerminal},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
struct TerminalGuard {
    active: bool,
}
impl TerminalGuard {
    fn enter(mouse: bool) -> anyhow::Result<Self> {
        enable_raw_mode()?;
        let guard = Self { active: true };
        execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
        if mouse {
            execute!(io::stdout(), EnableMouseCapture)?;
        }
        Ok(guard)
    }
    fn leave(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        Self::restore();
    }
    fn restore() {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.leave();
    }
}
fn main() -> anyhow::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|s| s == "--help" || s == "-h") {
        println!("Aster — Starship prompt studio\n\naster [--edit] [--wizard] [--no-mouse]\n\nOpens the current prompt on Design. --edit opens Modules.\n--wizard starts staged guided creation. --no-mouse permits terminal selection.\nConfig: $STARSHIP_CONFIG, or ~/.config/starship.toml\n\nTab screens · F6 panes · F2 Navigation · Ctrl+K actions · Ctrl+S Review");
        return Ok(());
    }
    if args.iter().any(|s| s == "--version") {
        println!("aster {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    for arg in &args {
        anyhow::ensure!(
            ["--edit", "--wizard", "--no-mouse"].contains(&arg.as_str()),
            "Unknown argument: {arg}"
        );
    }
    anyhow::ensure!(
        io::stdout().is_terminal() && io::stdin().is_terminal(),
        "Aster runs in a terminal"
    );
    crossterm::style::force_color_output(true);
    let mut app = app::App::new(
        config::config_path(),
        args.iter().any(|s| s == "--edit"),
        args.iter().any(|s| s == "--wizard"),
    );
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        TerminalGuard::restore();
        hook(info);
    }));
    let stopped = Arc::new(AtomicBool::new(false));
    let terminated = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTSTP, stopped.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, terminated.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGHUP, terminated.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, terminated.clone())?;
    let mouse = !args.iter().any(|s| s == "--no-mouse");
    let mut guard = TerminalGuard::enter(mouse).context("could not start terminal")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || loop {
        let result = match event::poll(Duration::from_millis(80)) {
            Ok(true) => event::read(),
            Ok(false) => continue,
            Err(e) => Err(e),
        };
        let failed = result.is_err();
        if events_tx.send(result).is_err() || failed {
            break;
        }
    });
    while !app.quit {
        let mut input = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut input, 1, 0);
        }
        if input.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            break;
        }
        if terminated.swap(false, Ordering::SeqCst) {
            app.dispatch(app::Intent::Quit);
        }
        if stopped.swap(false, Ordering::SeqCst) {
            guard.leave();
            unsafe {
                libc::raise(libc::SIGSTOP);
            }
            guard = TerminalGuard::enter(mouse)?;
            terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        }
        app.tick();
        terminal.draw(|f| ui::draw(f, &mut app))?;
        let next = match events_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => result?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match next {
            Event::Key(k) if k.kind == KeyEventKind::Press || k.kind == KeyEventKind::Repeat => {
                app.on_key(k)
            }
            Event::Mouse(m) if mouse => app.mouse(m),
            Event::Paste(text) => {
                if let Some(app::Overlay::Input(input)) = &mut app.overlay {
                    input.text.insert_str(input.cursor, &text);
                    input.cursor += text.len();
                } else if app.raw_editing {
                    app.workspace.raw.insert_str(app.raw_cursor, &text);
                    app.raw_cursor += text.len();
                    app.workspace.error = app
                        .workspace
                        .raw
                        .parse::<toml_edit::DocumentMut>()
                        .err()
                        .map(|e| e.to_string());
                }
            }
            _ => {}
        }
    }
    drop(terminal);
    drop(guard);
    Ok(())
}
