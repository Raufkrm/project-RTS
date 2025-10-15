mod app;
mod loading; 
mod menu;      
mod game;      
mod prelude;   
mod core;

fn main() {
    let mut app = app::build_app();
    app.run();
}