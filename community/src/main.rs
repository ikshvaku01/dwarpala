use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::{io, sync::mpsc};
use dwarpala::{
    Edition,
    cli::Cli,
    config::loader::{config_path, load_or_create_config},
    core::detector::ProjectType,
    ignore::engine::load_gitignore,
    platform::watcher::start_watcher,
    ui::app::{App, events::{run_app, AppEvent, AppExit}},
    utils::{resolve_output_path, scan_files},
};

#[cfg(windows)]
use std::path::PathBuf;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = {
        let p = cli.root_path.canonicalize()
            .map_err(|e| anyhow::anyhow!("Cannot resolve root path: {e}"))?;
        #[cfg(windows)]
        let p = {
            let s = p.to_string_lossy();
            PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s))
        };
        p
    };

    let disable: Vec<String> = cli.disable.into_iter().filter(|s| !s.is_empty()).collect();
    let (config, first_run)  = load_or_create_config()?;
    let ptype                = ProjectType::detect(&root);
    let mut patterns: Vec<String> = config.base_ignore.clone();

    if config.features.git_ignore {
        patterns.extend(load_gitignore(&root));
    }
    patterns.extend(ptype.language_ignores());

    let output_path   = resolve_output_path(&root, &config);
    let dir_structure = config.features.dir_structure
        && !disable.contains(&"DirStructure".to_string());

    let (_paths, rel_paths) = scan_files(&root, &patterns);

    let mut app = App::new(
        root.clone(),
        ptype.as_str(),
        Edition::Community,
        patterns.clone(),
        rel_paths,
        dir_structure,
    );

    if first_run {
        app.log(format!("⚙  First run — config written to {}", config_path().display()));
    }
    app.log(format!(
        "Project: {}  |  {} files  |  {} patterns",
        ptype.as_str(), app.files.len(), app.patterns.len()
    ));
    app.log(format!("Output → {}", output_path.display()));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let _watcher = start_watcher(root.clone(), tx.clone());

    let result = run_app(&mut terminal, app, rx, tx, output_path);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    // Community binary has no license screen, so OpenLicenseScreen is treated as Quit.
    match result? {
        AppExit::Quit | AppExit::OpenLicenseScreen => {}
    }
    Ok(())
}