# crabwire

`crabwire` is a tiny dependency registry for Rust.

You register concrete values once, then fetch shared references by type. 

This is not zero cost: the global registry stores values behind boxes and uses
type erasure internally, so lookups pay for a `TypeId` map lookup and a downcast.

Usually fine for app setup, but you should probably not be calling functions that use `#[inject]` in a hot loop.

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

Registries can be merged before registration:

```rust
use crabwire::Registry;

struct Config {
    app_name: &'static str,
}

struct Logger;

let registry = Registry::new()
    .insert(Config { app_name: "base" })
    .insert(Logger)
    .merge(Registry::new().insert(Config { app_name: "override" }));

assert_eq!(registry.get::<Config>().unwrap().app_name, "override");
assert!(registry.contains::<Logger>());
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

For tests that need to replace or layer the global registry inside one process,
enable the `testing` feature and use `reregister!` or `merge!`:

```rust
use crabwire::{Registry, get, merge, reregister};

struct Config {
    value: &'static str,
}

struct Logger;

reregister!(
    Registry::new()
        .insert(Config { value: "first" })
        .insert(Logger)
);
assert_eq!(get!(Config).value, "first");

merge!(Registry::new().insert(Config { value: "second" }));
assert_eq!(get!(Config).value, "second");
let _logger = get!(Logger);
```

`reregister!` intentionally leaks each registry it installs. Future lookups use
the latest registry, while references returned before replacement remain valid.
`merge!` also leaks its registry and layers it over the previous registries:
lookups prefer the newest layer and fall back to older layers when a type is
missing. The testing registry is still process-global, so tests that mutate it
can interfere with each other when they run concurrently.

There is one global registry per resolved `crabwire` crate instance in the final
binary. If a binary crate and several library crates all use the same `crabwire`
version from the same source, they share one registry. If Cargo resolves multiple
versions or sources of `crabwire`, each resolved crate instance has its own
registry.
