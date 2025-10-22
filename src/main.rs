use bevy::prelude::*;
use std::{panic, process};

#[cfg(debug_assertions)]
use std::backtrace::Backtrace;

// bring modules into the crate root so crate::menu / crate::game resolve
mod app;
mod core;
mod game;
mod loading;
mod menu;

use app::AppState;

fn main() {
    install_panic_hook();

    if let Err(payload) = panic::catch_unwind(run_app) {
        report_panic_payload(&payload);
        // ensure we don't continue running with a poisoned render device, etc.
        process::exit(1);
    }
}

fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        error!("panic in thread `{thread_name}`: {info}");

        if let Some(location) = info.location() {
            error!("  at {}:{}", location.file(), location.line());
        }

        #[cfg(debug_assertions)]
        {
            error!("{:#?}", Backtrace::capture());
        }
    }));
}

fn run_app() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(loading::LoadingPlugin)
        .init_state::<AppState>()
        .add_plugins(menu::MenuPlugin)
        .add_plugins(game::GamePlugin)
        .run();
}

fn report_panic_payload(payload: &Box<dyn std::any::Any + Send>) {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        error!("application panicked: {msg}");
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        error!("application panicked: {msg}");
    } else {
        error!("application panicked with a non-string payload");
    }
}
