# crabwire

`crabwire` is a tiny dependency registry for Rust.

You register concrete values once, then fetch shared references by type. It is
meant for simple app wiring, not for scoped lifetimes or per-request containers.

## Basic usage

```rust
use crabwire::{Registry, inject, register};

struct Config {
    app_name: String,
}

struct Logger;

impl Logger {
    fn log(&self, message: &str) {
        println!("[log] {message}");
    }
}

#[inject(config: &Config, logger: &Logger)]
fn run() {
    logger.log(&format!("{} started", config.app_name));
}

fn main() {
    let registry = 
        Registry::new()
            .insert(Config {
                app_name: "demo".to_owned(),
            })
            .insert(Logger);

    register!(registry);

    run();
}
```

`#[inject]` only accepts shared references:

```rust
#[inject(config: &Config)]
fn run() {
    println!("{}", config.app_name);
}
```

The registry stores the owned `Config`. The injected value is `&Config`.

## Modules

Use `Module` when a crate wants to expose its wiring as one value.

```rust
use crabwire::{Module, Registry};

struct Config {
    app_name: String,
}

struct AppModule;

impl Module for AppModule {
    fn register(self, registry: &mut Registry) {
        registry
            .try_insert(Config {
                app_name: "demo".to_owned(),
            })
            .unwrap();
    }
}

let registry = Registry::new().module(AppModule);
```

Modules can carry config too:

```rust
use crabwire::{Module, Registry};

struct Config {
    app_name: String,
}

struct AppModule {
    app_name: String,
}

impl Module for AppModule {
    fn register(self, registry: &mut Registry) {
        registry
            .try_insert(Config {
                app_name: self.app_name,
            })
            .unwrap();
    }
}

let registry = Registry::new().module(AppModule {
    app_name: "demo".to_owned(),
});
```

## Gotchas

`register!` can only be called once per process. It installs a global registry
backed by `OnceLock`, so the second call panics.

`#[inject]` looks up values when the function runs. If you call an injected
function before `register!`, it panics because no global registry exists yet.

Tests need a little care. Plain `cargo test` runs many tests in the same process,
so multiple tests that call `register!` can fight over the same global registry.
`cargo nextest` runs each test in its own process by default, so each test can
install its own registry:

```sh
cargo nextest run
```

There is one global registry per resolved `crabwire` crate instance in the final
binary. If a binary crate and several library crates all use the same `crabwire`
version from the same source, they share one registry. If Cargo resolves multiple
versions or sources of `crabwire`, each resolved crate instance has its own
registry.
