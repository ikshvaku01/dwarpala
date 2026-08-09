pub mod analysis;
pub mod cli;
pub mod config;
pub mod core;
pub mod edition;
pub mod ignore;
pub mod output;
pub mod platform;
pub mod privacy;
pub mod ui;
pub mod vcs;
pub mod worker;
pub mod utils;

pub use edition::Edition;
pub use ui::app::App;
pub use ui::app::events::run_app;