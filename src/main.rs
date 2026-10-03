mod app;
mod config;
mod preview;
mod theme;
mod ui;
mod wizard;

use std::io::{self, IsTerminal};
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::config::config_path;
use crate::wizard::Catalog;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "\
starship-editor [--edit]

Opens a prompt wizard. Each step says what it decides and which keys work.
--edit skips the wizard and opens the module editor.

Config: $STARSHIP_CONFIG, or ~/.config/starship.toml
"
        );
        return Ok(());
    }
    if !io::stdout().is_terminal() {
        anyhow::bail!("starship-editor runs in a terminal.");
    }
    let edit = args.iter().any(|arg| arg == "--edit");
    let path = config_path();
    let user_text = crate::config::read_config(&path);
    let catalog = Catalog::load();
    let mut app = App::new(path, user_text, catalog, edit);

    enable_raw_mode().context("could not set the terminal to raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("could not enter the alternate screen")?;
    let mut terminal =
        Terminal::new(CrosstermBackend::new(stdout)).context("could not start the tui")?;
    let result = run(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> anyhow::Result<()> {
    let mut dirty = true;
    loop {
        if app.preview.poll() {
            dirty = true;
        }
        if dirty {
            terminal.draw(|frame| ui::draw(frame, app))?;
            dirty = false;
        }
        if app.quit {
            break;
        }
        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        match event::read()? {
            Event::Key(key)
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat =>
            {
                app.on_key(key);
                dirty = true;
            }
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
    }
    Ok(())
}
