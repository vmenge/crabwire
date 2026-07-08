use crabwire::{Registry, inject, register};

struct Config {
    app_name: String,
}

struct Logger;

impl Logger {
    fn log(&self, msg: &str) {
        println!("[log] {msg}");
    }
}

#[inject(config: &Config, logger: &Logger, phrase: &String)]
fn do_something(n: i32) {
    logger.log(&format!("{} got {n}", config.app_name));
    logger.log(&format!("phrase is {phrase}"));
}

fn main() {
    let registry = Registry::new()
        .insert(Config {
            app_name: "demo".to_owned(),
        })
        .insert(Logger)
        .insert(String::from("whatever"));

    register!(registry);

    do_something(42);
}
