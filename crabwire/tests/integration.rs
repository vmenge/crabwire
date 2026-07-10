use crabwire::{Error, Module, Registry, get, inject, register, try_register};

#[cfg(feature = "testing")]
use crabwire::{merge, reregister};

struct NotClone {
    value: String,
}

struct AppModule {
    label: &'static str,
}

struct ModuleValue {
    label: &'static str,
}

trait Logger: Send + Sync {
    fn log(&self, message: &str) -> String;
}

struct PrefixLogger {
    prefix: &'static str,
}

impl Logger for PrefixLogger {
    fn log(&self, message: &str) -> String {
        format!("{}:{message}", self.prefix)
    }
}

impl Module for AppModule {
    fn register(self, registry: &mut Registry) {
        registry
            .try_insert(ModuleValue { label: self.label })
            .expect("module should register ModuleValue");
    }
}

#[test]
fn registry_returns_references_without_requiring_clone() {
    let registry = Registry::new().insert(NotClone {
        value: "stored".to_owned(),
    });

    let value = registry
        .get::<NotClone>()
        .expect("NotClone should be registered");

    assert_eq!(value.value, "stored");
}

#[test]
fn registry_reports_duplicate_insertions() {
    let mut registry = Registry::new();

    registry
        .try_insert(NotClone {
            value: "first".to_owned(),
        })
        .expect("first insert should succeed");

    let error = registry
        .try_insert(NotClone {
            value: "second".to_owned(),
        })
        .expect_err("second insert should fail");

    assert!(matches!(error, Error::AlreadyRegistered { .. }));
}

#[test]
fn module_registers_dependencies() {
    let registry = Registry::new().module(AppModule {
        label: "from module",
    });

    let value = registry
        .get::<ModuleValue>()
        .expect("module should have registered ModuleValue");

    assert_eq!(value.label, "from module");
}

#[test]
fn global_get_returns_static_references() {
    register!(Registry::new().insert(NotClone {
        value: "global".to_owned(),
    }));

    let value: &'static NotClone = get!(NotClone);

    assert_eq!(value.value, "global");
}

#[test]
fn try_register_reports_already_installed() {
    try_register!(Registry::new().insert(NotClone {
        value: "first".to_owned(),
    }))
    .expect("first global registry install should succeed");

    let error = try_register!(Registry::new().insert(ModuleValue { label: "second" }))
        .expect_err("second global registry install should fail");

    assert_eq!(error, Error::AlreadyInstalled);
}

#[test]
fn inject_uses_reference_arguments() {
    struct Config {
        prefix: &'static str,
    }

    #[inject(config: &Config, value: &NotClone)]
    fn render() -> String {
        format!("{}:{}", config.prefix, value.value)
    }

    register!(
        Registry::new()
            .insert(Config { prefix: "injected" })
            .insert(NotClone {
                value: "value".to_owned(),
            })
    );

    assert_eq!(render(), "injected:value");
}

#[test]
fn registry_can_store_trait_objects() {
    let logger = Box::new(PrefixLogger { prefix: "test" }) as Box<dyn Logger>;
    let registry = Registry::new().insert(logger);

    let logger = registry
        .get::<Box<dyn Logger>>()
        .expect("trait object should be registered");

    assert_eq!(logger.log("message"), "test:message");
}

#[test]
fn inject_can_use_trait_object_references() {
    #[inject(logger: &Box<dyn Logger>)]
    fn render() -> String {
        logger.log("injected")
    }

    let logger = Box::new(PrefixLogger { prefix: "dyn" }) as Box<dyn Logger>;

    register!(Registry::new().insert(logger));

    assert_eq!(render(), "dyn:injected");
}

#[cfg(feature = "testing")]
#[test]
fn reregister_replaces_global_registry_for_future_lookups() {
    struct Config {
        value: &'static str,
    }

    #[inject(config: &Config)]
    fn render() -> &'static str {
        config.value
    }

    reregister!(Registry::new().insert(Config { value: "first" }));
    assert_eq!(get!(Config).value, "first");
    assert_eq!(render(), "first");

    reregister!(Registry::new().insert(Config { value: "second" }));
    assert_eq!(get!(Config).value, "second");
    assert_eq!(render(), "second");
}

#[cfg(feature = "testing")]
#[test]
fn merge_layers_registry_over_previous_global_registry() {
    struct Config {
        value: &'static str,
    }

    struct Service {
        value: &'static str,
    }

    reregister!(
        Registry::new()
            .insert(Config { value: "base" })
            .insert(Service { value: "service" })
    );

    merge!(Registry::new().insert(Config { value: "merged" }));

    assert_eq!(get!(Config).value, "merged");
    assert_eq!(get!(Service).value, "service");
}
