mod app;
mod config;
mod screen;
mod theme;
mod tree;
mod ui;

use std::io::{self, IsTerminal};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::{cursor, execute, terminal};

use app::{App, Outcome};
use config::Config;
use screen::Screen;
use theme::THEMES;
use tree::Tree;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("box2cell: {message}");
            ExitCode::from(2)
        }
    }
}

enum Action {
    Run,
    Help,
    Version,
    Init(String),
}

struct Arguments {
    path: Option<PathBuf>,
    theme: Option<String>,
    pick_theme: bool,
    show_hidden: bool,
    action: Action,
}

fn run() -> Result<ExitCode, String> {
    let args = parse_args()?;
    match args.action {
        Action::Help => {
            print!("{}", help());
            return Ok(ExitCode::SUCCESS);
        }
        Action::Version => {
            println!("box2cell {VERSION}");
            return Ok(ExitCode::SUCCESS);
        }
        Action::Init(shell) => {
            print!("{}", shell_init(&shell)?);
            return Ok(ExitCode::SUCCESS);
        }
        Action::Run => {}
    }

    let (mut config, config_exists) = Config::load();
    if args.show_hidden {
        config.show_hidden = true;
    }
    let theme_override = match args.theme.as_deref() {
        Some(name) => Some(theme::by_slug(name).ok_or_else(|| unknown_theme(name))?),
        None => None,
    };

    let root = resolve_path(args.path.as_deref())?;
    let first_run = !config_exists && theme_override.is_none();
    let tree = Tree::new(root, config.show_hidden);
    let mut app = App::new(tree, config, first_run, args.pick_theme, theme_override);

    let outcome = run_ui(&mut app)?;
    if let Some(message) = app.save_error.take() {
        eprintln!("box2cell: could not save config: {message}");
    }

    Ok(match outcome {
        Outcome::Select(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Outcome::Cancel => ExitCode::from(1),
    })
}

fn parse_args() -> Result<Arguments, String> {
    let mut parsed = Arguments {
        path: None,
        theme: None,
        pick_theme: false,
        show_hidden: false,
        action: Action::Run,
    };
    let mut items = std::env::args().skip(1);

    while let Some(item) = items.next() {
        match item.as_str() {
            "-h" | "--help" => parsed.action = Action::Help,
            "-V" | "--version" => parsed.action = Action::Version,
            "-p" | "--pick-theme" => parsed.pick_theme = true,
            "-H" | "--hidden" => parsed.show_hidden = true,
            "-t" | "--theme" => {
                let value = items
                    .next()
                    .ok_or_else(|| "--theme requires a value".to_string())?;
                parsed.theme = Some(value);
            }
            "init" => {
                let shell = items.next().unwrap_or_else(|| "bash".to_string());
                parsed.action = Action::Init(shell);
                if let Some(extra) = items.next() {
                    return Err(format!("unexpected argument: {extra}"));
                }
            }
            _ if item.starts_with("--theme=") => {
                parsed.theme = Some(item["--theme=".len()..].to_string());
            }
            _ if item.starts_with('-') && item != "-" => {
                return Err(format!("unknown option: {item}\ntry `box2cell --help`"));
            }
            _ => {
                if parsed.path.replace(PathBuf::from(&item)).is_some() {
                    return Err(format!("unexpected argument: {item}"));
                }
            }
        }
    }

    Ok(parsed)
}

fn resolve_path(arg: Option<&Path>) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|err| format!("cannot read the current directory: {err}"))?;
    let raw = match arg {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => cwd.join(path),
        None => cwd,
    };
    let resolved = normalize(&raw);
    if !resolved.is_dir() {
        return Err(format!("not a directory: {}", resolved.display()));
    }
    Ok(resolved)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => out.push(name),
        }
    }
    out
}

fn run_ui(app: &mut App) -> Result<Outcome, String> {
    if !io::stderr().is_terminal() {
        return Err("the interface needs a terminal on stderr".to_string());
    }

    terminal::enable_raw_mode().map_err(|err| err.to_string())?;
    let _guard = TerminalGuard;
    execute!(io::stderr(), cursor::Hide).map_err(|err| err.to_string())?;

    let mut screen = Screen::new();
    let mut dirty = true;
    loop {
        if dirty {
            screen.draw(app).map_err(|err| err.to_string())?;
            dirty = false;
        }
        if let Some(outcome) = app.outcome.take() {
            screen.finish().map_err(|err| err.to_string())?;
            return Ok(outcome);
        }
        if event::poll(Duration::from_millis(250)).map_err(|err| err.to_string())?
            && let Event::Key(key) = event::read().map_err(|err| err.to_string())?
        {
            app.on_key(key);
            dirty = true;
        }
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stderr(), cursor::Show);
    }
}

fn unknown_theme(name: &str) -> String {
    let known: Vec<&str> = THEMES.iter().map(|theme| theme.slug).collect();
    format!(
        "unknown theme: {name}\navailable themes: {}",
        known.join(", ")
    )
}

fn help() -> String {
    let themes: Vec<&str> = THEMES.iter().map(|theme| theme.slug).collect();
    format!(
        r#"box2cell {VERSION} — interactive directory navigator

USAGE
    box2cell [OPTIONS] [PATH]
    box2cell init [bash|zsh|fish]

OPTIONS
    -t, --theme <NAME>   start with a theme
    -p, --pick-theme     open the theme picker on start
    -H, --hidden         show hidden directories
    -h, --help           print this help
    -V, --version        print version

THEMES
    {themes}

KEYS
    ↑ ↓ / k j     move
    → / l         expand directory
    ← / h         collapse or go to parent
    ⏎             select directory and exit
    ⌫             go one level up
    g / G         first / last
    ~             jump to home
    .             toggle hidden directories
    t             change theme
    esc / q       cancel
    ctrl+c        cancel

SHELL SETUP
    bash / zsh    eval "$(box2cell init bash)"
    fish          box2cell init fish | source

    Then use `bcd` instead of `cd`. To make it the default,
    add `alias cd=bcd` after the setup line.
"#,
        themes = themes.join(", ")
    )
}

fn shell_init(shell: &str) -> Result<String, String> {
    match shell {
        "bash" | "zsh" => Ok(r#"bcd() {
  local dest
  dest="$(command box2cell "$@")" || return
  [ -n "$dest" ] && builtin cd -- "$dest"
}
"#
        .to_string()),
        "fish" => Ok(r#"function bcd
    set -l dest (command box2cell $argv)
    test $status -eq 0; or return
    test -n "$dest"; and builtin cd -- $dest
end
"#
        .to_string()),
        other => Err(format!(
            "unsupported shell: {other}\nsupported shells: bash, zsh, fish"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dot_segments() {
        assert_eq!(normalize(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
        assert_eq!(normalize(Path::new("/..")), PathBuf::from("/"));
    }

    #[test]
    fn theme_lookup_is_case_insensitive() {
        assert_eq!(theme::by_slug("NORD").map(|item| item.slug), Some("nord"));
        assert!(theme::by_slug("missing").is_none());
    }

    #[test]
    fn shell_init_supports_known_shells() {
        for shell in ["bash", "zsh", "fish"] {
            assert!(shell_init(shell).is_ok());
        }
        assert!(shell_init("nu").is_err());
    }
}
