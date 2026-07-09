//! A tiny global dependency registry.
//!
//! Register values once, then fetch shared references by type.

use std::{
    any::{Any, TypeId, type_name},
    error::Error as StdError,
    fmt,
};

#[cfg(not(feature = "testing"))]
use std::sync::OnceLock;
#[cfg(feature = "testing")]
use std::sync::RwLock;

pub use crabwire_macros::inject;
use foldhash::HashMap;

#[cfg(not(feature = "testing"))]
static GLOBAL_REGISTRY: OnceLock<Registry> = OnceLock::new();

#[cfg(feature = "testing")]
static TEST_REGISTRY: RwLock<Option<&'static Registry>> = RwLock::new(None);

/// Errors returned by registry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A value of this type is already registered.
    AlreadyRegistered { type_name: &'static str },
    /// The global registry was already installed.
    AlreadyInstalled,
    /// No global registry has been installed yet.
    NoRegistry,
    /// No value of this type is registered.
    Missing { type_name: &'static str },
    /// The stored value had the wrong type.
    TypeMismatch { type_name: &'static str },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered { type_name } => {
                write!(f, "dependency already registered: {type_name}")
            }
            Self::AlreadyInstalled => {
                write!(f, "global dependency registry is already installed")
            }
            Self::NoRegistry => {
                write!(f, "no global dependency registry has been installed")
            }
            Self::Missing { type_name } => {
                write!(f, "dependency not registered: {type_name}")
            }
            Self::TypeMismatch { type_name } => {
                write!(
                    f,
                    "dependency type mismatch for {type_name}; registry invariant was violated"
                )
            }
        }
    }
}

impl StdError for Error {}

/// A group of dependencies that can register itself into a [`Registry`].
///
/// This is useful when a crate wants to expose its wiring as one value.
///
/// ```rust
/// use crabwire::{Module, Registry};
///
/// struct Logger;
/// struct LoggingModule;
///
/// impl Module for LoggingModule {
///     fn register(self, registry: &mut Registry) {
///         registry.try_insert(Logger).unwrap();
///     }
/// }
///
/// let registry = Registry::new().module(LoggingModule);
/// assert!(registry.contains::<Logger>());
/// ```
pub trait Module {
    /// Register this module's dependencies into the registry.
    fn register(self, registry: &mut Registry);
}

/// Stores dependencies by their concrete Rust type.
///
/// Values must be `Send + Sync + 'static`. Lookups return shared references, so
/// stored values do not need to implement `Clone`.
#[derive(Default)]
pub struct Registry {
    deps: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dependency and panic if that type is already registered.
    ///
    /// Use this for simple setup code where duplicates are a bug.
    ///
    /// ```rust
    /// use crabwire::Registry;
    ///
    /// struct Config;
    ///
    /// let registry = Registry::new().insert(Config);
    /// assert!(registry.contains::<Config>());
    /// ```
    pub fn insert<T>(mut self, value: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self.try_insert(value)
            .unwrap_or_else(|error| panic!("{error}"));

        self
    }

    /// Try to add a dependency.
    ///
    /// Returns [`Error::AlreadyRegistered`] if the registry already has a value
    /// of this type.
    pub fn try_insert<T>(&mut self, value: T) -> Result<(), Error>
    where
        T: Send + Sync + 'static,
    {
        let id = TypeId::of::<T>();

        if self.deps.contains_key(&id) {
            return Err(Error::AlreadyRegistered {
                type_name: type_name::<T>(),
            });
        }

        self.deps.insert(id, Box::new(value));
        Ok(())
    }

    /// Add or replace a dependency.
    ///
    /// This is useful when later setup should override an earlier value.
    pub fn set<T>(mut self, value: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self.deps.insert(TypeId::of::<T>(), Box::new(value));
        self
    }

    /// Register all dependencies from a module.
    ///
    /// ```rust
    /// use crabwire::{Module, Registry};
    ///
    /// struct Service;
    /// struct AppModule;
    ///
    /// impl Module for AppModule {
    ///     fn register(self, registry: &mut Registry) {
    ///         registry.try_insert(Service).unwrap();
    ///     }
    /// }
    ///
    /// let registry = Registry::new().module(AppModule);
    /// assert!(registry.contains::<Service>());
    /// ```
    pub fn module<M>(mut self, module: M) -> Self
    where
        M: Module,
    {
        module.register(&mut self);
        self
    }

    /// Get a dependency by type.
    ///
    /// Returns [`Error::Missing`] if no value of this type is registered.
    ///
    /// ```rust
    /// use crabwire::Registry;
    ///
    /// struct Config {
    ///     name: &'static str,
    /// }
    ///
    /// let registry = Registry::new().insert(Config { name: "demo" });
    /// let config = registry.get::<Config>().unwrap();
    ///
    /// assert_eq!(config.name, "demo");
    /// ```
    pub fn get<T>(&self) -> Result<&T, Error>
    where
        T: Send + Sync + 'static,
    {
        let value = self
            .deps
            .get(&TypeId::of::<T>())
            .ok_or_else(|| Error::Missing {
                type_name: type_name::<T>(),
            })?;

        value
            .downcast_ref::<T>()
            .ok_or_else(|| Error::TypeMismatch {
                type_name: type_name::<T>(),
            })
    }

    /// Check whether a dependency type is registered.
    pub fn contains<T>(&self) -> bool
    where
        T: Send + Sync + 'static,
    {
        self.deps.contains_key(&TypeId::of::<T>())
    }

    /// Return the number of registered dependency types.
    pub fn len(&self) -> usize {
        self.deps.len()
    }

    /// Return `true` if the registry has no dependencies.
    pub fn is_empty(&self) -> bool {
        self.deps.is_empty()
    }
}

#[doc(hidden)]
pub mod macro_utils {
    use super::{Error, Registry};

    #[cfg(not(feature = "testing"))]
    use super::GLOBAL_REGISTRY;
    #[cfg(feature = "testing")]
    use super::TEST_REGISTRY;

    #[cfg(not(feature = "testing"))]
    pub fn install_global(registry: Registry) -> Result<(), Error> {
        GLOBAL_REGISTRY
            .set(registry)
            .map_err(|_| Error::AlreadyInstalled)
    }

    #[cfg(feature = "testing")]
    pub fn install_global(registry: Registry) -> Result<(), Error> {
        let mut global = TEST_REGISTRY.write().unwrap();

        if global.is_some() {
            return Err(Error::AlreadyInstalled);
        }

        *global = Some(Box::leak(Box::new(registry)));
        Ok(())
    }

    #[cfg(feature = "testing")]
    pub fn reregister_global_for_tests(registry: Registry) {
        *TEST_REGISTRY.write().unwrap() = Some(Box::leak(Box::new(registry)));
    }

    #[cfg(not(feature = "testing"))]
    pub fn global_registry() -> Result<&'static Registry, Error> {
        GLOBAL_REGISTRY.get().ok_or(Error::NoRegistry)
    }

    #[cfg(feature = "testing")]
    pub fn global_registry() -> Result<&'static Registry, Error> {
        (*TEST_REGISTRY.read().unwrap()).ok_or(Error::NoRegistry)
    }

    pub fn global_get<T>() -> Result<&'static T, Error>
    where
        T: Send + Sync + 'static,
    {
        global_registry()?.get::<T>()
    }
}

/// Install a registry as the global registry.
///
/// This panics if a global registry has already been installed.
///
/// ```rust,ignore
/// use crabwire::{Registry, register};
///
/// struct Config;
///
/// register!(Registry::new().insert(Config));
/// ```
#[macro_export]
macro_rules! register {
    ($registry:expr $(,)?) => {{
        $crate::macro_utils::install_global($registry).unwrap_or_else(|error| panic!("{}", error));
    }};
}

/// Try to install a registry as the global registry.
///
/// This returns [`Error::AlreadyInstalled`] if a global registry has already
/// been installed.
///
/// ```rust,ignore
/// use crabwire::{Registry, try_register};
///
/// struct Config;
///
/// try_register!(Registry::new().insert(Config))?;
/// # Ok::<(), crabwire::Error>(())
/// ```
#[macro_export]
macro_rules! try_register {
    ($registry:expr $(,)?) => {{ $crate::macro_utils::install_global($registry) }};
}

/// Replace the global registry in builds compiled with the `testing` feature.
///
/// This intentionally leaks the provided registry. Future global lookups use
/// the latest reregistered registry, while references returned before this call
/// remain valid.
#[cfg(feature = "testing")]
#[macro_export]
macro_rules! reregister {
    ($registry:expr $(,)?) => {{
        $crate::macro_utils::reregister_global_for_tests($registry);
    }};
}

/// Get a dependency from the global registry.
///
/// This panics if the global registry is missing or the dependency is not
/// registered.
///
/// ```rust,ignore
/// use crabwire::{Registry, get, register};
///
/// struct Config {
///     name: &'static str,
/// }
///
/// register!(Registry::new().insert(Config { name: "demo" }));
///
/// let config: &'static Config = get!(Config);
/// assert_eq!(config.name, "demo");
/// ```
#[macro_export]
macro_rules! get {
    ($ty:ty $(,)?) => {{ $crate::macro_utils::global_get::<$ty>().unwrap_or_else(|error| panic!("{}", error)) }};
}
