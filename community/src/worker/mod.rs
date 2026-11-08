use crate::{
    core::compiler::build_dir_tree_string,
    output::OutputStyle,
};
use std::{
    fs, io::{BufWriter, Write},
    path::PathBuf,
    thread,
};

pub enum Msg {
    Progress { done: usize, total: usize, file: String },
    Log(String),
    Done { path: PathBuf, count: usize },
    Err(String),
}

/// Accepts a generic callback so the caller can forward Msg into any channel.
pub fn spawn_worker(
    root:          PathBuf,
    out:           PathBuf,
    paths:         Vec<PathBuf>,
    patterns:      Vec<String>,
    dir_structure: bool,
    style:         OutputStyle,
    send:          impl Fn(Msg) + Send + 'static,
) {
    thread::spawn(move || {
        let total = paths.len();
        send(Msg::Log(format!("Writing → {}", out.display())));
        if let Some(p) = out.parent() { let _ = fs::create_dir_all(p); }

        let file = match fs::File::create(&out) {
            Ok(f)  => f,
            Err(e) => { send(Msg::Err(e.to_string())); return; }
        };
        let mut w   = BufWriter::new(file);

        if dir_structure {
            let tree = build_dir_tree_string(&root, &patterns);
            let _ = writeln!(w, "{}\n\n{tree}\n", style.dir_header());
        }

        for (i, path) in paths.iter().enumerate() {
            let rel = path.strip_prefix(&root).unwrap_or(path)
                .to_string_lossy().replace('\\', "/");
            send(Msg::Progress { done: i, total, file: rel.clone() });
            let _ = writeln!(w, "{}\n", style.file_header(&rel));
            if let Ok(content) = fs::read_to_string(path) {
                let _ = writeln!(w, "{content}\n");
            }
        }
        send(Msg::Done { path: out, count: total });
    });
}