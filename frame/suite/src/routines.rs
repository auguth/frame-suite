// SPDX-License-Identifier: MPL-2.0
//
// Part of Auguth Labs open-source softwares.
// Built for the Substrate framework.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 Auguth Labs (OPC) Pvt Ltd, India

// ===============================================================================
// ``````````````````````````` OFFCHAIN ROUTINES SUITE ```````````````````````````
// ===============================================================================

//! Best-effort execution framework for offchain routines with
//! explicit logging and storage semantics.
//!
//! In FRAME, most logic is executed via dispatchable extrinsics or inherents,
//! both of which execute transactionally and revert on failure.
//!
//! Offchain workers operate outside that model: they run asynchronously,
//! without rollback, and with only best-effort guarantees.
//!
//! This module introduces **routines** as a structured way to write such logic,
//! where each routine is context-driven and multiple routines can be composed
//! and executed under best-effort guarantees.
//!
//! # What routines provide
//!
//! - A disciplined execution model via [`Routines`]
//! - Per-routine semantics, including authorization through [`RoutineOf`]
//!   and domain-specific error policies
//! - Composability, allowing multiple routines to run independently under
//!   best-effort guarantees
//!
//! Unlike extrinsics, routines are not atomic. Failures are local and do not
//! prevent other routines from executing.
//!
//! # Logging over rollback
//!
//! In the absence of transactional guarantees, routines rely on [`Logging`]:
//!
//! - errors are recorded and returned, not used to revert execution
//! - observability replaces rollback as the primary debugging mechanism
//!
//! # Storage as execution semantics
//!
//! Routines can integrate with [`KeyValueStore`] abstractions backed by
//! offchain storage models such as [`Persistent`], [`ForkAware`], and [`Finalized`].
//!
//! These make fork behavior explicit and allow safe state handling across
//! re-orgs and repeated execution.
//!
//! # Summary
//!
//! Routines provide a structured, context-aware model for offchain execution:
//! - best-effort instead of transactional
//! - logging-driven instead of rollback-driven
//! - explicit in both execution and storage semantics

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{Accrete, ForksHandler, base::{Elastic, Portable, Probe, RuntimeEnum, RuntimeError, Time}};

// --- Scale-codec crates ---
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::{
    prelude::{
        format,
        string::{String, ToString},
    },
    TypeInfo,
};

// --- Core / Std ---
use core::marker::PhantomData;

// --- FRAME System ---
use frame_system::{pallet_prelude::BlockNumberFor};

// --- Substrate primitives ---
use sp_core::blake2_256;
use sp_runtime::{
    offchain::storage::{MutateStorageError, StorageValueRef},
    traits::{Debug, One, Saturating, Zero},
    DispatchError,
};

// --- Substrate std (no_std helpers) ---
use sp_std::{collections::{btree_map::BTreeMap, btree_set::BTreeSet}, vec::Vec};

// ===============================================================================
// ``````````````````````````````````` LOGGING ```````````````````````````````````
// ===============================================================================

/// Defines the function signature that can be passed to the logging system
/// to **customize how log messages are formatted**.
///
/// ## Params
/// - `TS`: The type of the timestamp (e.g., block number, system time, or any type
/// implementing [`Debug`]).
/// - `L`: The type representing log level (e.g., [`LogLevel`]).
/// - `target` (`&str`): the logging target, typically the module or subsystem.
/// - `message` (`&str`): the main log message.
///
/// It returns a `String` containing the fully formatted log line.
///
/// This allows the caller to completely customize the log output format, e.g.,
/// changing the order, adding emojis, colors, or any additional context.
pub type LogFormatter<TS, L> = fn(timestamp: TS, level: &L, target: &str, message: &str) -> String;

/// Trait for structured logging in detached, asynchronous routines.
///
/// This trait is intended for use in detached, asynchronous [`Routines`] or functions
/// where errors are handled gracefully rather than propagated. The logging system
/// records errors, warnings, info, or debug messages without affecting control flow.
///
/// ## Type Parameters
/// - `Timestamp`: The type used for timestamps in logs.
pub trait Logging<Timestamp>
where
    Timestamp: Time,
{
    /// The error/logging type propagated through the API.
    ///
    /// Logging responsibility is directional:
    ///
    /// - If a `Logger` is returned from the provider (i.e., received by the caller),
    ///   it has already been logged.
    /// - If a `Logger` is constructed by the caller and returned to the provider,
    ///   it is expected that the provider will perform the logging.
    type Logger: Elastic + RuntimeEnum;

    /// The log level type (Info/Warn/Error/Debug)
    type Level: RuntimeEnum + From<&'static str>;

    /// Default log target if none is provided.
    ///
    /// The **log target** is a label that identifies the source of a log message,
    /// typically a pallet, module, or subsystem. It helps to categorize and filter
    /// logs, making it easier to trace where messages come from in a complex runtime.
    ///
    /// For example, in a log line like:
    /// `[12345][INFO][pallet_template] Templating period has not started`
    ///
    /// - `12345` is the timestamp/block number  
    /// - `INFO` is the log level  
    /// - `pallet_template` is the **log target**  
    /// - The rest is the message
    ///
    /// If the caller does not provide a target, the `FALLBACK_TARGET` constant is
    /// used as a default to ensure all logs have a meaningful source label.
    const FALLBACK_TARGET: &'static str;

    /// Core logging function that all helpers delegate to.
    ///
    /// This central function ensures consistent structure and formatting
    /// across all log messages.  
    ///
    /// The optional `LogFormatter` lets you override the default output style.
    ///
    /// A formatter could add JSON structure, include node metadata, or embed
    /// contextual tags.
    fn log(
        level: Self::Level,
        err: &Self::Logger,
        timestamp: Timestamp,
        target: Option<&str>,
        fmt: Option<LogFormatter<Timestamp, Self::Level>>,
    ) -> Self::Logger;

    /// Logs an info-level message.
    ///
    /// Includes an optional custom formatter and log target.
    #[inline]
    fn info(
        err: &Self::Logger,
        timestamp: Timestamp,
        target: Option<&str>,
        fmt: Option<LogFormatter<Timestamp, Self::Level>>,
    ) -> Self::Logger
    where
        Self: Sized,
    {
        Self::log(Self::Level::from("info"), err, timestamp, target, fmt)
    }

    /// Logs a warning-level message.
    ///
    /// Includes an optional custom formatter and log target.
    #[inline]
    fn warn(
        err: &Self::Logger,
        timestamp: Timestamp,
        target: Option<&str>,
        fmt: Option<LogFormatter<Timestamp, Self::Level>>,
    ) -> Self::Logger
    where
        Self: Sized,
    {
        Self::log(Self::Level::from("warn"), err, timestamp, target, fmt)
    }

    /// Logs an error-level message.
    ///
    /// Includes an optional custom formatter and log target.
    #[inline]
    fn error(
        err: &Self::Logger,
        timestamp: Timestamp,
        target: Option<&str>,
        fmt: Option<LogFormatter<Timestamp, Self::Level>>,
    ) -> Self::Logger
    where
        Self: Sized,
    {
        Self::log(Self::Level::from("error"), err, timestamp, target, fmt)
    }

    /// Logs a debug-level message.
    ///
    /// Includes an optional custom formatter and log target.
    #[inline]
    fn debug(
        err: &Self::Logger,
        timestamp: Timestamp,
        target: Option<&str>,
        fmt: Option<LogFormatter<Timestamp, Self::Level>>,
    ) -> Self::Logger
    where
        Self: Sized,
    {
        Self::log(Self::Level::from("debug"), err, timestamp, target, fmt)
    }
}

/// Blanket implementation of `Logging` for any runtime-type.
impl<T, Time> Logging<Time> for T
where
    Time: crate::Time,
{
    /// The type taken and returned for logging.
    ///
    /// We simply return the same [`DispatchError`] that was logged,
    /// so logging does not change control flow or error propagation.
    ///
    /// `DispatchError` is used because in Substrate it encompasses **all**
    /// runtime errors - including module errors, token errors, arithmetic
    /// issues, and transactional boundaries - making it the universal
    /// substrate-side error representation.
    type Logger = DispatchError;

    /// The log level type.  
    ///
    /// We use the `LogLevel` enum to standardize severity levels
    /// (Info, Warn, Error, Debug) across all routine logs.
    type Level = LogLevel;

    /// Default logging target if none is provided.  
    ///
    /// Most routines, especially offchain workers or background tasks, use this target
    /// for simplicity.
    ///
    /// It allows a consistent place to look for routine logs without requiring every
    /// call to specify a target.
    ///
    /// **Note**: This target is only a conveninence and may be somewhat vague.
    /// To ensure errors can still be traced accurately, the logged messages should
    /// include additional metadata (e.g., module name, error index, or contextual info)
    /// so that the source of the error can be identified even if the target is generic.
    const FALLBACK_TARGET: &str = "routine";

    fn log(
        level: Self::Level,
        err: &Self::Logger,
        timestamp: Time,
        target: Option<&str>,
        fmt: Option<LogFormatter<Time, Self::Level>>,
    ) -> Self::Logger {
        use log::{debug, error, info, warn};

        // Determine the actual logging target
        let actual_target = target.unwrap_or(<Self as Logging<Time>>::FALLBACK_TARGET);

        // Convert the DispatchError into a human-readable message
        let message = match err {
            DispatchError::Other(str) => str.to_string(),
            DispatchError::Module(module_error) => {
                if let Some(msg) = module_error.message {
                    format!("Module({}): {}", module_error.index, msg)
                } else {
                    // fallback to raw bytes if no message available
                    format!(
                        "Module({}) raw error: {:?}",
                        module_error.index, module_error.error
                    )
                }
            }
            DispatchError::CannotLookup => "CannotLookup".to_string(),
            DispatchError::BadOrigin => "BadOrigin".to_string(),
            DispatchError::ConsumerRemaining => "ConsumerRemaining".to_string(),
            DispatchError::NoProviders => "NoProviders".to_string(),
            DispatchError::TooManyConsumers => "TooManyConsumers".to_string(),
            DispatchError::Token(token_error) => match token_error {
                sp_runtime::TokenError::FundsUnavailable => {
                    "TokenError: FundsUnavailable".to_string()
                }
                sp_runtime::TokenError::OnlyProvider => "TokenError: OnlyProvider".to_string(),
                sp_runtime::TokenError::BelowMinimum => "TokenError: BelowMinimum".to_string(),
                sp_runtime::TokenError::CannotCreate => "TokenError: CannotCreate".to_string(),
                sp_runtime::TokenError::UnknownAsset => "TokenError: UnknownAsset".to_string(),
                sp_runtime::TokenError::Frozen => "TokenError: Frozen".to_string(),
                sp_runtime::TokenError::Unsupported => "TokenError: Unsupported".to_string(),
                sp_runtime::TokenError::CannotCreateHold => {
                    "TokenError: CannotCreateHold".to_string()
                }
                sp_runtime::TokenError::NotExpendable => "TokenError: NotExpendable".to_string(),
                sp_runtime::TokenError::Blocked => "TokenError: Blocked".to_string(),
            },
            DispatchError::Arithmetic(arithmetic_error) => match arithmetic_error {
                sp_runtime::ArithmeticError::Underflow => "ArithmeticError: Underflow".to_string(),
                sp_runtime::ArithmeticError::Overflow => "ArithmeticError: Overflow".to_string(),
                sp_runtime::ArithmeticError::DivisionByZero => {
                    "ArithmeticError: DivisionByZero".to_string()
                }
            },
            DispatchError::Transactional(transactional_error) => match transactional_error {
                sp_runtime::TransactionalError::LimitReached => {
                    "TransactionalError: LimitReached".to_string()
                }
                sp_runtime::TransactionalError::NoLayer => {
                    "TransactionalError: NoLayer".to_string()
                }
            },
            DispatchError::Exhausted => "Exhausted".to_string(),
            DispatchError::Corruption => "Corruption".to_string(),
            DispatchError::Unavailable => "Unavailable".to_string(),
            DispatchError::RootNotAllowed => "RootNotAllowed".to_string(),
            DispatchError::Trie(trie_error) => match trie_error {
                frame_support::traits::TrieError::InvalidStateRoot => {
                    "TrieError: InvalidStateRoot".to_string()
                }
                frame_support::traits::TrieError::IncompleteDatabase => {
                    "TrieError: IncompleteDatabase".to_string()
                }
                frame_support::traits::TrieError::ValueAtIncompleteKey => {
                    "TrieError: ValueAtIncompleteKey".to_string()
                }
                frame_support::traits::TrieError::DecoderError => {
                    "TrieError: DecoderError".to_string()
                }
                frame_support::traits::TrieError::InvalidHash => {
                    "TrieError: InvalidHash".to_string()
                }
                frame_support::traits::TrieError::DuplicateKey => {
                    "TrieError: DuplicateKey".to_string()
                }
                frame_support::traits::TrieError::ExtraneousNode => {
                    "TrieError: ExtraneousNode".to_string()
                }
                frame_support::traits::TrieError::ExtraneousValue => {
                    "TrieError: ExtraneousValue".to_string()
                }
                frame_support::traits::TrieError::ExtraneousHashReference => {
                    "TrieError: ExtraneousHashReference".to_string()
                }
                frame_support::traits::TrieError::InvalidChildReference => {
                    "TrieError: InvalidChildReference".to_string()
                }
                frame_support::traits::TrieError::ValueMismatch => {
                    "TrieError: ValueMismatch".to_string()
                }
                frame_support::traits::TrieError::IncompleteProof => {
                    "TrieError: IncompleteProof".to_string()
                }
                frame_support::traits::TrieError::RootMismatch => {
                    "TrieError: RootMismatch".to_string()
                }
                frame_support::traits::TrieError::DecodeError => {
                    "TrieError: DecodeError".to_string()
                }
            },
        };

        // Apply optional custom formatting or default format
        let log_line = if let Some(f) = fmt {
            f(timestamp, &level, actual_target, &message)
        } else {
            format!(
                "[{:?}][{}][{}] {}",
                timestamp,
                level.as_str(),
                actual_target,
                message
            )
        };

        // Emit the log using the appropriate level macro
        match level {
            LogLevel::Error => error!(target: actual_target, "{}", log_line),
            LogLevel::Warn => warn!(target: actual_target, "{}", log_line),
            LogLevel::Debug => debug!(target: actual_target, "{}", log_line),
            LogLevel::Info => info!(target: actual_target, "{}", log_line),
        }

        // Return the original error for convenience
        *err
    }
}

// ===============================================================================
// `````````````````````````````` LOGGING UTILITIES ``````````````````````````````
// ===============================================================================

/// Represents log severity levels.
#[derive(Clone, Copy, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum LogLevel {
    /// Informational messages
    Info,
    /// Warnings
    Warn,
    /// Errors
    Error,
    /// Debug-level messages
    Debug,
}

impl From<&'static str> for LogLevel {
    /// Converts a string literal into a `LogLevel`.
    /// Defaults to `Debug` for unrecognized strings.
    fn from(s: &'static str) -> Self {
        match s {
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            _ => LogLevel::Debug,
        }
    }
}

impl LogLevel {
    /// Returns the string representation of the log level.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }
}

// ===============================================================================
// ``````````````````````````````` KEY-VALUE STORE ```````````````````````````````
// ===============================================================================

/// Trait for a simple key-value storage with integrated logging.
///
/// This trait extends [`Logging`] to ensure that any storage operation
/// (insert or get) automatically logs errors in a standardized way.
///
/// This is generic over `TimeStamp` so logs can carry either a block number,
/// system timestamp, or any other type that implements [`Debug`].
pub trait KeyValueStore<Value, TimeStamp>: Logging<TimeStamp>
where
    TimeStamp: Time,
{
    /// Type of the key, maybe a slice instead of a bounded array.
    type Key: Probe + ?Sized;

    /// Value of the key.
    type Value: Portable;

    /// Inserts a value into the store for a given key.
    ///
    /// ## Parameters
    /// - `key`: The key under which to insert the value.
    /// - `value`: The value to store.
    /// - `target`: Optional log target (e.g., `"runtime::storage::balances"`).
    /// - `fmt`: Optional custom log formatter. If `None`, the default log format is used.
    ///
    /// ## Returns
    /// - `Ok(())` on success.
    /// - `Err(Logger)` if an error occurs; already logged.
    fn insert(
        key: &Self::Key,
        value: &Value,
        target: Option<&str>,
        fmt: Option<LogFormatter<TimeStamp, Self::Level>>,
    ) -> Result<(), Self::Logger>;

    /// Retrieves a value from the store for a given key.
    ///
    /// ## Parameters
    /// - `key`: The key to look up.
    /// - `target`: Optional log target (e.g., `"runtime::storage::balances"`).
    /// - `fmt`: Optional custom log formatter. If `None`, the default log format is used.
    ///
    /// ## Returns
    /// - `Ok(Some(value))` if key exists.
    /// - `Ok(None)` if key does not exist.
    /// - `Err(Logger)` if retrieval fails; already logged.
    fn get(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<TimeStamp, Self::Level>>,
    ) -> Result<Option<Self::Value>, Self::Logger>;

    /// Removes the value associated with the given key.
    ///
    /// If the key exists, the stored value is removed and returned.
    /// If the key does not exist, `Ok(None)` is returned.
    ///
    /// ## Parameters
    /// - `key`: The key whose associated value should be removed.
    /// - `target`: Optional log target (e.g., `"runtime::storage::offchain"`).
    /// - `fmt`: Optional custom log formatter. If `None`, the default log format is used.
    ///
    /// ## Returns
    /// - `Ok(Some(value))` if the key existed and the value was removed.
    /// - `Ok(None)` if the key did not exist.
    /// - `Err(Logger)` if removal fails; already logged.
    fn remove(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<TimeStamp, Self::Level>>,
    ) -> Result<Option<Value>, Self::Logger>;

    /// Mutates the value associated with the given key.
    ///
    /// The mutation closure is invoked with the **current value state**:
    ///
    /// - `Ok(Some(value))` if the key exists and the value was successfully read.
    /// - `Ok(None)` if the key does not exist, allowing the caller to initialize it.
    /// - `Err(Self::Logger)` if reading or decoding the existing value fails
    ///   (this error is already logged).
    ///
    /// The closure **must return a value** to be written back to storage.
    /// Removal is **not supported** via this method; use [`Self::remove`] explicitly
    /// if deletion is required.
    ///
    /// Any error returned by the closure ([`Logging::Logger`]) **must not be logged
    /// by the caller**. This function will automatically log all errors returned from
    /// the closure, ensuring consistent logging behavior.
    ///
    /// ## Parameters
    /// - `key`: The key whose associated value should be mutated.
    /// - `f`: A closure that receives the current value state and returns
    ///   the new value to store, or a domain-level error.
    /// - `target`: Optional log target for storage-related logs.
    /// - `fmt`: Optional custom log formatter.
    ///
    /// ## Returns
    /// - `Ok(())` if the mutation succeeds.
    /// - `Err(Logger)` if mutation fails; already logged.
    fn mutate<F>(
        key: &Self::Key,
        f: F,
        target: Option<&str>,
        fmt: Option<LogFormatter<TimeStamp, Self::Level>>,
    ) -> Result<(), Self::Logger>
    where
        F: FnOnce(Result<Option<Value>, Self::Logger>) -> Result<Value, Self::Logger>;
}

// ===============================================================================
// `````````````````````````` OFFCHAIN-STORAGE UTILITIES `````````````````````````
// ===============================================================================

/// Marker trait for Substrate offchain storage kinds (backends).
///
/// Used exclusively for compile-time specialization and trait bounds.
/// This trait has no methods and implies no behavior.
pub trait SubstrateOffchainStorage {}

/// Defines how **offchain storage failures are reported to callers**.
///
/// Intended for **Substrate FRAME-based runtimes only**.
///
/// This trait assigns that responsibility to the **caller (routine)** by
/// allowing it to define the concrete error values used to represent storage
/// failures in its own domain.
///
/// The `Kind` parameter identifies the offchain storage backend this policy
/// applies to (for example, persistent or fork-aware storage). It may carry
/// additional type parameters, but this trait makes no assumptions about
/// their meaning.
///
/// This trait defines **policy only** and introduces no runtime behavior.
pub trait OffchainStorageError<Kind>
where
    Kind: SubstrateOffchainStorage,
{
    /// Caller-defined error type used for storage failures.
    ///
    /// The same value is logged and returned as a [`DispatchError`].
    type Error: RuntimeError;

    /// Error used when decoding a stored value fails.
    fn decode_failed() -> Self::Error;

    /// Error used when a concurrent mutation of the stored value is detected.
    fn concurrent_mutation() -> Self::Error;
}

// ===============================================================================
// `````````````````````````````` PERSISTENT STORAGE `````````````````````````````
// ===============================================================================

/// Marker type for persistent offchain storage, providing fork-independent,
/// non-reverting state with routine-defined error and logging semantics.
///
/// Intended for **Substrate FRAME-based runtimes only**.
///
/// Persistent offchain storage is **not fork-aware**:
/// - Values persist across block re-organizations.
/// - Values are shared across all forks.
/// - Values are **not reverted** if the current fork is abandoned.
///
/// This marker is used to specialize [`KeyValueStore`] implementations
/// backed by [`StorageValueRef::persistent`].
///
/// ## Error policy requirement
///
/// When [`KeyValueStore`] is **used for this type**, the corresponding
/// `Routine` **must also implement** [`OffchainStorageError`] for the
/// *same specialized* `Persistent<Context, Value, Routine>` type.
///
/// This ensures that:
/// - storage-level failures are surfaced as **caller-defined errors**,
/// - errors are attributed to the routine's domain rather than the
///   storage layer,
/// - and each failure is logged exactly once.
///
/// ## Timestamp semantics
///
/// This storage backend is **explicitly bound to block numbers** as its
/// timestamp source. It does **not** accept a generic timestamp parameter.
///
/// All logging and routine behavior associated with this backend uses
/// [`BlockNumberFor`], reflecting its intended use inside
/// FRAME-based runtimes.
///
/// ## Type parameters
///
/// - `Context`: The active runtime type (i.e. a type implementing
///   [`frame_system::Config`]). This binds the storage to a specific
///   runtime configuration.
/// - `Value`: The value type stored in persistent offchain storage.
/// - `Routine`: A routine type implementing [`Routines`] parameterized
///   by [`BlockNumberFor`], ensuring logging, error handling,
///   and behavior are specialized to block-based execution.
///
/// This type is a **marker only** and carries no runtime data.
#[derive(Clone, Copy, Debug, Default)]
pub struct Persistent<Context, Value, Routine>(PhantomData<(Value, Context, Routine)>)
where
    Context: frame_system::Config,
    Value: Portable,
    Routine: Routines<BlockNumberFor<Context>>;

/// Default backend marker implementation for all valid [`Persistent`] specializations.
///
/// This blanket implementation marks every well-formed [`Persistent<Context, Value, Routine>`]
/// type as a supported Substrate offchain storage backend.
impl<Context, Value, Routine> SubstrateOffchainStorage for Persistent<Context, Value, Routine>
where
    Context: frame_system::Config,
    Value: Portable,
    Routine: Routines<BlockNumberFor<Context>>,
{
}

/// **Peristent Offchain Storage Kind/Backend** Default [`KeyValueStore`]
/// Implementation.
///
/// Intended for Substrate FRAME-based runtimes only.
///
/// The timestamp type is the runtime's block number ([`BlockNumberFor`]),
/// ensuring that all logs are tagged with the block context in which
/// the operation occurred.
impl<T, Value, Routine> KeyValueStore<Value, BlockNumberFor<T>> for Persistent<T, Value, Routine>
where
    T: frame_system::Config,
    Value: Portable,
    Routine: OffchainStorageError<Self> + Routines<BlockNumberFor<T>>,
{
    /// Keys are raw byte slices; allows flexible usage for any encoded identifier.
    type Key = [u8];

    /// Value type implementing [`Encode`] and [`Decode`]
    type Value = Value;

    /// This writes directly to **persistent offchain storage** and is therefore
    /// not reverted on chain re-orgs.
    fn insert(
        key: &Self::Key,
        value: &Value,
        _target: Option<&str>,
        _fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        let storage_ref = StorageValueRef::persistent(key);
        storage_ref.set(value);
        Ok(())
    }

    /// Reads from **persistent offchain storage**, which is shared across forks
    /// and survives re-orgs.
    fn get(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<Option<Value>, Self::Logger> {
        let storage_ref = StorageValueRef::persistent(key);
        let block = frame_system::Pallet::<T>::block_number();

        // Attempt to read from storage.
        let Ok(value) = storage_ref.get() else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                &<Routine as OffchainStorageError<Self>>::decode_failed().into(),
                block,
                target,
                fmt,
            ));
        };

        Ok(value)
    }

    /// Removes the value from **persistent offchain storage**.
    ///
    /// Since persistent storage is not fork-aware, removals are permanent and
    /// are not reverted on chain re-orgs.
    fn remove(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<Option<Value>, Self::Logger> {
        let storage_ref = StorageValueRef::persistent(key);
        let block = frame_system::Pallet::<T>::block_number();

        // Read existing value first
        let Ok(existing) = storage_ref.get::<Value>() else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                &<Routine as OffchainStorageError<Self>>::decode_failed().into(),
                block,
                target,
                fmt,
            ));
        };
        // Remove the value
        let mut storage_ref = storage_ref;
        storage_ref.clear();

        Ok(existing)
    }

    /// Mutates the value associated with the given key in **persistent offchain storage**.
    ///
    /// The closure is invoked with the current value, if any, and **must return**
    /// the new value to store. Removal is not supported by this method.
    ///
    /// Persistent storage is **not fork-aware**: mutations persist across re-orgs
    /// and are visible on all forks.
    fn mutate<F>(
        key: &Self::Key,
        f: F,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger>
    where
        F: FnOnce(Result<Option<Value>, Self::Logger>) -> Result<Value, Self::Logger>,
    {
        let storage_ref = StorageValueRef::persistent(key);
        let block = frame_system::Pallet::<T>::block_number();

        let res = storage_ref.mutate::<Value, Self::Logger, _>(|current| {
            // Decode / retrieval phase
            let current = match current {
                Ok(v) => Ok(v), // v = Option<Value>
                Err(_) => Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &<Routine as OffchainStorageError<Self>>::decode_failed().into(),
                    block,
                    target,
                    fmt,
                )),
            };

            // Delegate domain mutation logic
            f(current)
        });

        match res {
            // Value successfully written
            Ok(_) => Ok(()),

            // Storage-level race
            Err(MutateStorageError::ConcurrentModification(_)) => {
                let logged = <Self as Logging<BlockNumberFor<T>>>::warn(
                    &<Routine as OffchainStorageError<Self>>::concurrent_mutation().into(),
                    block,
                    target,
                    fmt,
                );
                Err(logged)
            }

            // Closure returned a domain error
            Err(MutateStorageError::ValueFunctionFailed(logged)) => {
                Err(<Self as Logging<BlockNumberFor<T>>>::error(
                    &logged, block, target, fmt,
                ))
            }
        }
    }
}

// ===============================================================================
// `````````````````````````````` FORK-AWARE STORAGE `````````````````````````````
// ===============================================================================

/// Marker type for fork-aware offchain storage, enabling re-org-sensitive,
/// fork-scoped state with routine-defined error and logging semantics.
///
/// Intended for **Substrate FRAME-based runtimes only**.
///
/// Fork-aware offchain storage is **re-org sensitive**:
/// - Values are scoped to the current fork.
/// - Values are reverted if the fork is abandoned.
/// - Values are not shared across competing forks.
///
/// This marker is used to specialize [`KeyValueStore`] implementations
/// backed by fork-aware offchain storage via [`StorageValueRef::local`].
///
/// ## Error policy requirement
///
/// When [`KeyValueStore`] is **used for this type**, the corresponding
/// `Routine` **must also implement** [`OffchainStorageError`] for the
/// *same specialized* `Persistent<Context, Value, Routine>` type.
///
/// This ensures that:
/// - storage-level failures are surfaced as **caller-defined errors**,
/// - errors are attributed to the routine's domain rather than the
///   storage layer,
/// - and each failure is logged exactly once.
///
/// ## Timestamp semantics
///
/// This storage backend is **explicitly bound to block numbers** as its
/// timestamp source. It does **not** accept a generic timestamp parameter.
///
/// All logging and routine behavior associated with this backend uses
/// [`BlockNumberFor`], reflecting its intended use inside
/// FRAME-based runtimes.
///
/// ## Type parameters
///
/// - `Context`: The active runtime type (i.e. a type implementing
///   [`frame_system::Config`]). This binds the storage to a specific
///   runtime configuration.
/// - `Value`: The value type stored in fork-aware offchain storage.
/// - `Routine`: A routine type implementing [`Routines`] parameterized
///   by [`BlockNumberFor`], ensuring logging, error handling,
///   and behavior are specialized to block-based execution.
/// - `Handler`: A type implementing [`ForksHandler`] using [`ForkLocalDepot`]
///   that manages the fork graph and scope tracking for this storage backend.
/// 
/// This type is a **marker only** and carries no runtime data.
#[derive(Clone, Copy, Debug, Default)]
pub struct ForkAware<Context, Value, Routine, Handler>(PhantomData<(Value, Context, Routine, Handler)>)
where
    Context: frame_system::Config,
    Value: Portable,
    Routine: Routines<BlockNumberFor<Context>>, 
    Handler: ForksHandler<Context, ForkLocalDepot>;

/// Default backend marker implementation for all valid [`ForkAware`] specializations.
///
/// This blanket implementation marks every well-formed [`ForkAware<Context, Value, Routine>`]
/// type as a supported Substrate offchain storage backend.
impl<Context, Value, Routine, Handler> SubstrateOffchainStorage for ForkAware<Context, Value, Routine, Handler>
where
    Context: frame_system::Config,
    Value: Portable,
    Routine: Routines<BlockNumberFor<Context>>,
    Handler: ForksHandler<Context, ForkLocalDepot>,
{
}

/// **Fork-Aware Offchain Storage Kind/Backend** Default [`KeyValueStore`]
/// Implementation.
///
/// Intended for Substrate FRAME-based runtimes only.
///
/// This implementation is backed by **fork-aware offchain storage**
/// ([`StorageValueRef::local`]).
///
/// The timestamp type is the runtime's block number ([`BlockNumberFor`]),
/// ensuring that all logs are tagged with the block context in which
/// the operation occurred.
impl<T, Value, Routine, Handler> KeyValueStore<Value, BlockNumberFor<T>> for ForkAware<T, Value, Routine, Handler>
where
    T: frame_system::Config,
    Value: Portable,
    Routine: OffchainStorageError<Self> + Routines<BlockNumberFor<T>>,
    Handler: ForksHandler<T, ForkLocalDepot> + Logging<BlockNumberFor<T>, Level = LogLevel, Logger = DispatchError>,
{
    /// Keys are raw byte slices; allows flexible usage for any encoded identifier.
    type Key = [u8];

    /// Value type implementing [`Encode`] and [`Decode`]
    type Value = Value;

    /// Writes a value to **fork-aware offchain storage**.
    ///
    /// For **get-check-set** use [`Self::mutate`] instead for concurrency safety.
    ///
    /// Values written using this method are scoped to the current fork and
    /// will be reverted automatically if the fork is abandoned.
    fn insert(
        key: &Self::Key,
        value: &Value,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        let key = key.to_vec();
        let scope_key = Handler::add_to_scope(key, target, fmt)?;
        let storage_ref = StorageValueRef::persistent(&scope_key);
        storage_ref.set(value);
        Ok(())
    }

    /// Reads a value from **fork-aware offchain storage**.
    ///
    /// The returned value reflects only the state of the current fork and
    /// may differ across competing forks.
    fn get(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<Option<Value>, Self::Logger> {
        let scope_key = Handler::gen_scope_item_key(&key.to_vec());
        if !Handler::scope_item_exists(&scope_key, target, fmt)? {
            return Ok(None)
        };

        let storage_ref = StorageValueRef::persistent(&scope_key);
        let block = frame_system::Pallet::<T>::block_number();

        let Ok(value) = storage_ref.get() else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                &<Routine as OffchainStorageError<Self>>::decode_failed().into(),
                block,
                target,
                fmt,
            ));
        };

        Ok(value)
    }

    /// Removes a value from **fork-aware offchain storage**.
    ///
    /// Removals are scoped to the current fork and are reverted automatically
    /// if the fork is abandoned.
    fn remove(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<Option<Value>, Self::Logger> {
        let scope_key = Handler::gen_scope_item_key(&key.to_vec());
        if !Handler::scope_item_exists(&scope_key, target, fmt)? {
            return Ok(None)
        }

        let storage_ref = StorageValueRef::persistent(&scope_key);
        let block = frame_system::Pallet::<T>::block_number();

        let Ok(existing) = storage_ref.get::<Value>() else {
            return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                &<Routine as OffchainStorageError<Self>>::decode_failed().into(),
                block,
                target,
                fmt,
            ));
        };

        let mut storage_ref = storage_ref;
        storage_ref.clear();
        Handler::remove_from_scope(&scope_key, target, fmt)?;

        Ok(existing)
    }

    /// Performs an atomic read-modify-write operation on **fork-aware offchain
    /// storage**.
    ///
    /// The closure is invoked with the current value, if any, and **must return**
    /// the new value to store. Removal is not supported by this method.
    ///
    /// Mutations performed by this method:
    /// - are visible only on the current fork,
    /// - are reverted automatically on re-orgs,
    /// - and are safe to use with speculative chain state.
    fn mutate<F>(
        key: &Self::Key,
        f: F,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger>
    where
        F: FnOnce(Result<Option<Value>, Self::Logger>) -> Result<Value, Self::Logger>,
    {

        let scope_key = Handler::gen_scope_item_key(&key.to_vec());
        if !Handler::scope_item_exists(&scope_key, target, fmt)? {
            let value = f(Ok(None))?;
            Self::insert(key, &value, target, fmt)?;
            return Ok(())
        }

        let storage_ref = StorageValueRef::persistent(&scope_key);
        let block = frame_system::Pallet::<T>::block_number();

        let res = storage_ref.mutate::<Value, Self::Logger, _>(|current| {
            // Normalize storage read into Result<Option<Value>, Logged>
            let current = match current {
                Ok(opt) => Ok(opt),
                Err(_) => Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &<Routine as OffchainStorageError<Self>>::decode_failed().into(),
                    block,
                    target,
                    fmt,
                )),
            };

            // Delegate mutation logic to caller
            f(current)
        });

        match res {
            Ok(_) => Ok(()),

            Err(MutateStorageError::ConcurrentModification(_)) => {
                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                    &<Routine as OffchainStorageError<Self>>::concurrent_mutation().into(),
                    block,
                    target,
                    fmt,
                ));
            }

            Err(MutateStorageError::ValueFunctionFailed(logged)) => {
                Err(<Self as Logging<BlockNumberFor<T>>>::error(
                    &logged, block, target, fmt,
                ))
            }
        }
    }
}

// ===============================================================================
// ````````````````````````` FINALIZED STORAGE UTILITIES `````````````````````````
// ===============================================================================

/// Defines a **finality evaluation policy** for values managed by
/// [`Finalized`] storage.
///
/// Intended for **Substrate FRAME-based runtimes only**.
///
/// This trait specifies the parameters used to derive a **confidence signal**
/// for speculative, fork-aware data based on:
/// - elapsed wall-clock time, and
/// - block-scoped repeated observations.
///
/// It only answers:
/// - *how long* a value must survive before it may be considered stable, and
/// - *how many distinct block observations* are required to strengthen confidence.
///
/// The policy provides *inputs* to confidence evaluation and does not
/// imply on-chain finality or absolute truth.
pub trait FinalizedPolicy<Context>
where
    Context: pallet_timestamp::Config,
{
    /// Wall-clock **elapsed time window** that must pass *after the first
    /// observation* before a value may begin to contribute to a stronger
    /// confidence signal.
    ///
    /// Conceptually, this represents the delay between:
    /// - an **initial observation window**, and
    /// - an **optimal finalized window** where confidence can be evaluated.
    ///
    /// The duration is expressed using the runtime's timestamp type
    /// (see [`pallet_timestamp::Config::Moment`]).
    fn finality_after() -> <Context as pallet_timestamp::Config>::Moment;

    /// Number of **distinct blocks** in which the value must be observed
    /// *after* the finality window has elapsed.
    ///
    /// Observations are block-scoped:
    /// - At most one observation per block is counted.
    /// - Repeated OCW executions within the same block do not increase this value.
    ///
    /// This parameter acts as a confidence-strengthening threshold
    /// to guard against transient forks.
    fn finality_ticks() -> BlockNumberFor<Context>;
}

/// Defines **caller-facing error signals** specific to
/// [`Finalized`] storage.
///
/// Intended for **Substrate FRAME-based runtimes only**.
///
/// This trait allows callers to control how **semantic invariant violations**
/// detected by the `Finalized` storage model are surfaced as
/// [`DispatchError`] values.
///
/// These errors:
/// - originate from finality-specific consistency checks,
/// - are logged by the storage layer,
/// - and are returned to the caller as *signals* of inconsistency,
///   not as definitive storage failures.
///
/// This trait does not affect storage behavior. It only defines
/// which error values are emitted when an invariant is violated.
pub trait FinalizedOffchainStorageError<Context, Value>
where
    Context: frame_system::Config,
{
    /// Concrete error type chosen by the caller.
    ///
    /// This error is converted into a [`DispatchError`] before being
    /// logged or returned.
    type Error: RuntimeError;

    /// Emitted when a **fork-aware value hash exists without a corresponding
    /// entry in persistent storage**.
    ///
    /// This indicates a hanging speculative value. The fork-aware entry
    /// is cleaned up automatically before this error is returned.
    fn hanging_hash() -> Self::Error;

    /// Emitted when **persistent storage contains no value at all**
    /// for the given key.
    ///
    /// This means there is **no speculative value being tracked**,
    /// and therefore the fork-aware entry has no semantic meaning.
    ///
    /// When this condition is detected, the fork-aware entry is
    /// cleaned up automatically before this error is returned.
    fn hanging_value() -> Self::Error;
}

// ===============================================================================
// `````````````````````````````` FINALIZED STORAGE ``````````````````````````````
// ===============================================================================

/// Marker type for finality-aware offchain storage, combining fork-aware and
/// persistent state to derive confidence-graded values via routine-defined
/// policies and observations.
///
/// Intended for **Substrate FRAME-based runtimes only**.
///
/// The `Finalized` storage model:
/// - records values speculatively using fork-aware storage,
/// - tracks historical observations in persistent storage,
/// - and exposes values only after evaluating time and observation-based
///   finality guarantees.
///
/// Finality is determined by:
/// - a wall-clock time window (see [`FinalizedPolicy`]),
/// - and repeated successful observations.
///
/// This marker is used to specialize [`KeyValueStore`] implementations that
/// combine [`ForkAware`] and [`Persistent`] storage to provide confidence-graded
/// values (see [`Confidence`]).
///
/// ## Behavioral contract
///
/// Any routine using `Finalized` storage **must provide error policies for the
/// exact internal storage forms used by this model** via
/// [`OffchainStorageError`]:
///
/// - [`ForkAware<Context, ValueHash, Routine>`], which stores speculative
///   fork-local identity using [`ValueHash`], and
/// - [`Persistent<Context, Ledger<Context, Moment<Context>, Value>, Routine>`],
///   which stores the persistent observation ledger using [`Ledger`] and
///   wall-clock [`Moment`].
///
/// In addition, the routine must define:
/// - a [`FinalizedPolicy`] describing when a value becomes stable, and
/// - [`FinalizedOffchainStorageError`] values for finality-specific
///   invariant violations.
///
/// Together, these requirements ensure that:
/// - fork-aware and persistent state remain consistent,
/// - semantic invariants are enforced at a single, centralized layer,
/// - and all failures are surfaced as **caller-defined error signals**
///   and logged exactly once.
///
/// ## Value-first semantics
///
/// This storage model is **value-first**: confidence is tied to the observed
/// *value*, not just the key. If the same value is inserted again for the same
/// key, its accumulated confidence is **reset**, as the insertion is treated as
/// a fresh observation sequence.
///
/// Callers are therefore responsible for deciding whether repeated insertions
/// of the same value are semantically meaningful. To avoid unintended confidence
/// resets, routines should refrain from inserting identical values multiple
/// times unless a reset is explicitly desired.
///
/// ## Timestamp semantics
///
/// All logging and routine behavior associated with this storage model is
/// **explicitly bound to block numbers** via [`BlockNumberFor<Context>`].
/// This type does **not** accept a generic timestamp parameter.
///
/// Wall-clock time, when required for finality evaluation, is obtained
/// explicitly from [`pallet_timestamp`].
///
/// ## Type parameters
///
/// - `Context`: The active runtime type (i.e. a type implementing
///   [`frame_system::Config`]). This binds the storage model to a specific
///   runtime configuration.
/// - `Value`: The value type whose finality is being tracked.
/// - `Routine`: A routine type implementing [`Routines`] parameterized
///   by [`BlockNumberFor<Context>`], allowing logging, error handling,
///   policy evaluation, and invariant enforcement to be specialized
///   at the type level.
/// - `Handler`: A type implementing [`ForksHandler`] using [`ForkLocalDepot`]
///   that manages the fork graph and scope tracking for this storage backend.
///
/// This type is a **marker only** and carries no runtime data.
#[derive(Clone, Copy, Debug)]
pub struct Finalized<Context, Value, Routine, Handler>(PhantomData<(Value, Context, Routine, Handler)>)
where
    Context: frame_system::Config,
    Value: Portable,
    Routine: Routines<BlockNumberFor<Context>>,
    Handler: ForksHandler<Context, ForkLocalDepot>;

/// Stable, fork-independent identifier for values managed by [`Finalized`] storage.
///
/// `ValueHash` is computed using the `blake2_256` hash of the
/// SCALE-encoded representation of a value.
///
/// Within the [`Finalized`] storage model, this hash is used to:
/// - identify the *actual content* associated with a fork-aware key,
/// - correlate speculative fork-aware entries with their corresponding
///   persistent ledger records,
/// - and track value observations across forks.
///
/// Fork-aware storage records only the `ValueHash`, while the full value
/// and its observation metadata are stored persistently. This ensures that
/// semantic identity is preserved across re-orgs while allowing speculative
/// state to be reverted safely.
#[derive(Encode, Decode, Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValueHash(pub [u8; 32]);

impl ValueHash {
    pub fn new(hash: [u8; 32]) -> Self {
        ValueHash(hash)
    }
}

/// Persistent observation record for a value managed by [`Finalized`] storage.
///
/// An `Observation` captures *when* a value was seen and *how many distinct
/// blocks* it has survived after entering the finality window.
///
/// This structure does not imply finality by itself; it only provides
/// the evidence required by [`FinalizedPolicy`] to derive a
/// [`Confidence`] level.
#[derive(Encode, Decode, Debug, Clone)]
pub struct Observation<Context, Value>
where
    Context: pallet_timestamp::Config,
{
    /// Wall-clock time when the value was first observed.
    pub first_seen: Moment<Context>,

    /// Wall-clock time when the value was last observed.
    pub last_seen: Moment<Context>,

    /// Number of distinct blocks in which the value was observed
    /// after the finality window elapsed.
    pub blocks_seen: BlockNumberFor<Context>,

    /// The observed value.
    pub value: Value,
}

/// Persistent observation ledger used by [`Finalized`] storage.
///
/// A `Ledger` maps a stable [`ValueHash`] to its corresponding
/// [`Observation`] record.
///
/// Within the [`Finalized`] storage model:
/// - The ledger is stored in **persistent offchain storage**.
/// - Entries are **fork-independent** and survive chain re-organizations.
/// - Each entry accumulates observation history across OCW executions.
///
/// The ledger acts as the authoritative source of truth for:
/// - value identity (via [`ValueHash`]),
/// - temporal stability,
/// - and block-scoped confirmation counts.
///
/// It is consulted to derive confidence levels (see [`Confidence`])
/// and to detect and clean up fork-aware inconsistencies.
#[derive(Encode, Decode, Debug, Clone)]
pub struct Ledger<Context, Value>(pub ConfidenceMap<Context, Value>)
where
    Context: pallet_timestamp::Config,
    Value: Encode + Decode + Clone;

// Type Alias for Persistent Ledger
type ConfidenceMap<Context, Value> = BTreeMap<ValueHash, Observation<Context, Value>>;

/// Confidence **signal** derived for a value evaluated by [`Finalized`] storage.
///
/// This enum represents the outcome of applying a [`FinalizedPolicy`] to an
/// observed value, based on elapsed time and block-scoped observations.
///
/// It expresses a **signal of stability**, not on-chain finality and not a
/// definitive statement of truth.
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub enum Confidence<Value>
where
    Value: Portable,
{
    /// A **strong confidence signal**.
    ///
    /// The value has:
    /// - survived the configured finality time window, and
    /// - been observed across enough distinct blocks.
    ///
    /// This signal suggests that the value is *likely stable* and that
    /// irreversible or non-recoverable actions may be reasonable.
    Safe(Value),

    /// A **weak confidence signal**.
    ///
    /// The value has survived the finality time window, but has **not yet**
    /// accumulated enough block-scoped observations.
    ///
    /// This signal suggests that only optimistic or recoverable actions
    /// should be considered.
    Risky(Value),

    /// A **negative confidence signal**.
    ///
    /// The value exists, but the finality time window has **not** elapsed yet.
    ///
    /// This signal suggests that no action-reversible or irreversible-
    /// should be taken at this stage.
    Unsafe(Value),
}

/// Wall-clock timestamp type used by [`Finalized`] storage.
///
/// An alias for the timestamp type provided by [`pallet_timestamp`]
/// for the given runtime.
///
/// It is used exclusively for **time-based finality evaluation**
/// (for example, measuring how long a value has survived),
/// and is **not** used for ordering, counting, or block-based logic.
///
/// Block-scoped semantics (such as observation counts) are expressed
/// separately via [`BlockNumberFor`].
pub type Moment<T> = <T as pallet_timestamp::Config>::Moment;

/// [`KeyValueStore`] implementation for [`Finalized`] storage semantics.
///
/// This implementation materializes the behavioral contract defined by
/// [`Finalized`] by combining:
/// - fork-aware storage for speculative state,
/// - persistent storage for observation history,
/// - and routine-defined policies for finality evaluation and error signaling.
///
/// The required bounds ensure that the routine:
/// - defines **when** a value becomes stable ([`FinalizedPolicy`]),
/// - provides caller-defined error signals for finality invariants
///   ([`FinalizedOffchainStorageError`]),
/// - and supplies [`OffchainStorageError`] error policies for the
/// **exact storage forms** used internally by this model for:
///   - [`ForkAware<.., ValueHash, ..>`] using [`ValueHash`], and
///   - [`Persistent<.., Ledger<...>, ..>`] using [`Ledger`].
///
/// This guarantees that all storage failures and semantic violations are
/// surfaced consistently as caller-defined errors and are logged exactly
/// once at the correct abstraction layer.
impl<T, Value, Routine, Handler> KeyValueStore<Value, BlockNumberFor<T>> for Finalized<T, Value, Routine, Handler>
where
    T: pallet_timestamp::Config,
    Value: Portable,
    Routine: FinalizedOffchainStorageError<T, Value>
        + FinalizedPolicy<T>
        + OffchainStorageError<Persistent<T, Ledger<T, Value>, Routine>>
        + OffchainStorageError<ForkAware<T, ValueHash, Routine, Handler>>
        + Routines<BlockNumberFor<T>>,
    Handler: ForksHandler<T, ForkLocalDepot> + Logging<BlockNumberFor<T>, Level = LogLevel, Logger = DispatchError>,
{
    /// Keys are raw byte slices; allows flexible usage for any encoded identifier.
    type Key = [u8];

    /// Return value type used when querying a key.
    ///
    /// The value is wrapped in [`Confidence`], representing a
    /// **confidence signal** derived from the [`Finalized`] storage
    /// model rather than a definitive truth or on-chain finality.
    type Value = Confidence<Value>;

    /// Inserts a value **speculatively** under finality-aware semantics.
    ///
    /// This operation:
    /// - computes a stable [`ValueHash`] for fork-independent identity,
    /// - records the hash in **fork-aware storage** (speculative marker),
    /// - and inserts or updates an [`Observation`] in the **persistent ledger**.
    ///
    /// No confidence is implied by insertion alone; this operation only
    /// records *existence* and initializes observation tracking.
    fn insert(
        key: &Self::Key,
        value: &Value,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger> {
        // Compute stable value hash (identity across forks)
        let hash = ValueHash(blake2_256(&value.encode()));

        // Read wall-clock time (confidence anchor)
        let now: Moment<T> = pallet_timestamp::Pallet::<T>::get();

        // Write fork-aware speculative marker
        // (this is fork-local and will be reorged)
        ForkAware::<T, ValueHash, Routine, Handler>::insert(key, &hash, target, fmt)?;

        // Persistent ledger mutation
        Persistent::<T, Ledger<T, Value>, Routine>::mutate(
            key,
            |current| {
                let mut ledger = match current {
                    Ok(Some(existing)) => existing,
                    Ok(None) => Ledger(ConfidenceMap::new()),
                    Err(logged) => return Err(logged),
                };

                ledger.0.insert(
                    hash,
                    Observation {
                        first_seen: now,
                        last_seen: now,
                        blocks_seen: Zero::zero(),
                        value: value.clone(),
                    },
                );

                Ok(ledger)
            },
            target,
            fmt,
        )?;

        Ok(())
    }

    /// Reads the value associated with the current fork and derives the
    /// value wrapped in a [`Confidence`] signal.
    ///
    /// Returned signals:
    /// - [`Confidence::Unsafe`] - finality window not elapsed.
    /// - [`Confidence::Risky`]  - time elapsed, insufficient block observations.
    /// - [`Confidence::Safe`]   - time and observation thresholds satisfied.
    ///
    /// Any detected invariant violation (for example, a fork-aware hash
    /// without a ledger entry) is logged and cleaned up automatically.
    fn get(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<Option<Confidence<Value>>, Self::Logger> {
        let now: Moment<T> = pallet_timestamp::Pallet::<T>::get();
        let block = frame_system::Pallet::<T>::block_number();

        // Read fork-aware speculative hash
        let hash = match ForkAware::<T, ValueHash, Routine, Handler>::get(key, target, fmt)? {
            Some(h) => h,
            None => return Ok(None),
        };

        // Will be produced inside mutation
        let mut result: Option<Confidence<Value>> = None;

        // Atomic persistent ledger mutation
        Persistent::<T, Ledger<T, Value>, Routine>::mutate(
            key,
            |current| {
                let mut ledger = match current {
                    Ok(Some(l)) => l,
                    Ok(None) => {
                        // Fork-aware exists but ledger missing -> clean fork-aware
                        ForkAware::<T, ValueHash, Routine, Handler>::remove(key, target, fmt)?;
                        return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                            &<Routine as FinalizedOffchainStorageError<T, Value>>::hanging_value()
                                .into(),
                            block,
                            target,
                            fmt,
                        ));
                    }
                    Err(logged) => return Err(logged),
                };

                let obs = match ledger.0.get_mut(&hash) {
                    Some(o) => o,
                    None => {
                        // Fork-aware hash has no backing ledger entry -> cleanup
                        ForkAware::<T, ValueHash, Routine, Handler>::remove(key, target, fmt)?;
                        return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                            &<Routine as FinalizedOffchainStorageError<T, Value>>::hanging_hash()
                                .into(),
                            block,
                            target,
                            fmt,
                        ));
                    }
                };

                // Snapshot for confidence computation
                let first_seen = obs.first_seen;
                let last_seen = obs.last_seen;
                let obs_count = obs.blocks_seen;
                let value = obs.value.clone();

                // Confidence evaluation
                let after = <Routine as FinalizedPolicy<T>>::finality_after();
                let ticks = <Routine as FinalizedPolicy<T>>::finality_ticks();

                // Update observation metadata
                obs.last_seen = now;

                // Evaluate confidence based on temporal finality and repeated observations.
                //
                // Time-window finality check
                // - `first_seen`: moment of the first successful observation (insertion time)
                // - `after`:      required finality window duration
                // - `last_seen`:  moment of the most recent successful observation (from storage)
                //
                // If (first_seen + after) > last_seen, the value has NOT yet survived
                // the required finality window - meaning we have not observed it
                // long enough across time. The value remains `Unsafe`.
                //
                // Otherwise, the value has lived past the temporal finality window,
                // and we can evaluate observation-based stability.
                let confidence = match first_seen.saturating_add(after) > last_seen {
                    // Still within the finality time window -> not stable yet.
                    true => Confidence::Unsafe(value),

                    // Time window satisfied; now evaluate repeated observations.
                    false => match obs_count < ticks {
                        true => {
                            // We only increment observation ticks if this observation
                            // occurred in a strictly new moment. Multiple observations
                            // within the same moment do not increase confidence.
                            if last_seen < now {
                                obs.blocks_seen += One::one();
                            }

                            // Not enough distinct-moment observations yet -> still risky.
                            Confidence::Risky(value)
                        }

                        // Required number of distinct-moment observations reached,
                        // and the time window has already elapsed -> value is finalized.
                        false => Confidence::Safe(value),
                    },
                };

                result = Some(confidence);

                Ok(ledger)
            },
            target,
            fmt,
        )?;

        Ok(result)
    }

    /// Removes the value associated with the **current fork**.
    ///
    /// Removal semantics:
    /// - The fork-aware marker is always removed first.
    /// - The corresponding persistent ledger entry is removed next.
    /// - The ledger itself is deleted if it becomes empty.
    ///
    /// This ensures no semantic or historical state is left behind once
    /// the value is no longer relevant.
    fn remove(
        key: &Self::Key,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<Option<Value>, Self::Logger> {
        let block = frame_system::Pallet::<T>::block_number();

        // Read fork-aware hash (what we are removing)
        let hash = match ForkAware::<T, ValueHash, Routine, Handler>::get(key, target, fmt)? {
            Some(h) => h,
            None => return Ok(None), // nothing to remove
        };

        // Always remove fork-aware entry first
        ForkAware::<T, ValueHash, Routine, Handler>::remove(key, target, fmt)?;

        // Remove from persistent ledger atomically
        let mut removed: Option<Value> = None;

        let mut is_empty = false;

        Persistent::<T, Ledger<T, Value>, Routine>::mutate(
            key,
            |current| {
                let mut ledger = match current {
                    Ok(Some(l)) => l,
                    Ok(None) => {
                        return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                            &<Routine as FinalizedOffchainStorageError<T, Value>>::hanging_value()
                                .into(),
                            block,
                            target,
                            fmt,
                        ));
                    }
                    Err(logged) => return Err(logged),
                };

                if let Some(obs) = ledger.0.remove(&hash) {
                    removed = Some(obs.value);
                }

                if ledger.0.is_empty() {
                    is_empty = true;
                }

                Ok(ledger)
            },
            target,
            fmt,
        )?;

        // Drop empty ledger (no semantic meaning left)
        if is_empty {
            Persistent::<T, Ledger<T, Value>, Routine>::remove(key, target, fmt)?;
        }

        Ok(removed)
    }

    /// Mutates the value associated with a key under **finality-aware semantics**.
    ///
    /// The closure `f` receives the current value, if any, and must return
    /// a new value to replace it.
    ///
    /// Replacing a value **resets all finality observations**: the new value
    /// is treated as freshly observed and must re-accumulate confidence.
    ///
    /// The update is scoped to the current fork and preserves all storage
    /// invariants. Any detected invariant violation is logged once and
    /// cleaned up automatically before the error is returned.
    fn mutate<F>(
        key: &Self::Key,
        f: F,
        target: Option<&str>,
        fmt: Option<LogFormatter<BlockNumberFor<T>, Self::Level>>,
    ) -> Result<(), Self::Logger>
    where
        F: FnOnce(Result<Option<Value>, Self::Logger>) -> Result<Value, Self::Logger>,
    {
        let now: Moment<T> = pallet_timestamp::Pallet::<T>::get();
        let block = frame_system::Pallet::<T>::block_number();

        // Fork-aware mutation is the outer authority
        let result = ForkAware::<T, ValueHash, Routine, Handler>::mutate(
            key,
            |current_hash| {
                // Resolve current value from persistent ledger
                let current_value = match current_hash {
                    Err(logged) => {
                        return Err(logged);
                    }

                    Ok(None) => None,

                    Ok(Some(hash)) => {
                        let ledger =
                            Persistent::<T, Ledger<T, Value>, Routine>::get(key, target, fmt)?;

                        let ledger = match ledger {
                            Some(l) => l,
                            None => {
                                return Err(<Self as Logging<BlockNumberFor<T>>>::warn(
                                        &<Routine as FinalizedOffchainStorageError<
                                            T,
                                            Value,
                                        >>::hanging_value()
                                            .into(),
                                        block,
                                        target,
                                        fmt,
                                    ));
                            }
                        };

                        match ledger.0.get(&hash) {
                            Some(obs) => Some(obs.value.clone()),
                            None => None,
                        }
                    }
                };

                // Delegate domain mutation
                let new_value = f(Ok(current_value))?;

                // Compute new identity
                let new_hash = ValueHash(blake2_256(&new_value.encode()));

                // Update persistent ledger
                Persistent::<T, Ledger<T, Value>, Routine>::mutate(
                    key,
                    |ledger_result| {
                        let mut ledger = match ledger_result {
                            Ok(Some(l)) => l,
                            Ok(None) => Ledger(ConfidenceMap::new()),
                            Err(logged) => return Err(logged),
                        };

                        // Remove old observation
                        if let Ok(Some(old_hash)) = current_hash {
                            ledger.0.remove(&old_hash);
                        }

                        // Insert new observation
                        ledger.0.insert(
                            new_hash,
                            Observation {
                                first_seen: now,
                                last_seen: now,
                                blocks_seen: Zero::zero(),
                                value: new_value,
                            },
                        );

                        Ok(ledger)
                    },
                    target,
                    fmt,
                )?;
                // Commit new fork-aware hash
                Ok(new_hash)
            },
            target,
            fmt,
        );

        match result {
            Ok(_) => Ok(()),

            Err(logged) => {
                if logged
                    == <Routine as FinalizedOffchainStorageError<T, Value>>::hanging_value().into()
                {
                    ForkAware::<T, ValueHash, Routine, Handler>::remove(key, target, fmt)?;
                }
                Err(logged)
            }
        }
    }
}

// ===============================================================================
// `````````````````````````````````` ROUTINES ```````````````````````````````````
// ===============================================================================

/// Fork-local storage scope for [`ForksHandler`] implementing [`ForkScopes`](crate::ForkScopes).
///
/// `ForkLocalDepot` is the branch-local scope container used by the
/// fork-aware offchain execution system to track visibility of
/// fork-scoped storage entries across branch lineage.
///
/// It does not store the actual values themselves.
///
/// Instead, it stores only deterministic 32-byte keys (`[u8; 32]`)
/// representing items written into fork-aware storage systems such as:
///
/// - [`ForkAware`]
/// - [`Finalized`]
///
/// These keys act as stable scope references that allow the fork graph
/// to answer:
///
/// ```ignore
/// "does this item exist on this branch or any reachable ancestor branch?"
/// ```
///
/// without requiring repeated traversal of historical parent branches.
///
/// ## Why this exists
///
/// In fork-aware OCW execution, each branch must maintain isolated local
/// state while still inheriting valid reachable state from its lineage.
///
/// Example:
///
/// ```text
/// A -> B -> C
///         |-- D
///         |-- D'
/// ```
///
/// Here:
///
/// - `D` and `D'` must not overwrite each other
/// - both branches must still see inherited state from `A -> B -> C`
///
/// `ForkLocalDepot` provides that visibility layer by separating:
///
/// - current-generation writes
/// - inherited historical writes
///
/// instead of repeatedly walking parent branches during every lookup.
///
/// ## Fork inheritance model
///
/// When a new sibling branch is created:
///
/// ```text
/// Parent branch:
/// A -> B -> C
///
/// New sibling:
///         |-- D'
/// ```
///
/// the child branch receives:
///
/// ```text
/// inherited_keys(child)
/// = inherited_keys(parent) + local_keys(parent)
/// ```
///
/// while starting with:
///
/// ```text
/// local_keys(child) = {}
/// ```
///
/// This ensures:
///
/// - parent state remains reachable
/// - new writes stay isolated to the new fork
/// - existence checks remain O(log n)
/// - no ancestry walking is required for normal reads
///
/// ## Example
///
/// ```text
/// Original branch:
///
/// local_keys      = {k1, k2}
/// inherited_keys  = {}
///
/// After fork:
///
/// local_keys      = {}
/// inherited_keys  = {k1, k2}
///
/// New write:
///
/// local_keys      = {k3}
/// inherited_keys  = {k1, k2}
/// ```
///
/// The child branch can see:
///
/// ```text
/// {k1, k2, k3}
/// ```
///
/// while sibling branches remain isolated from `k3`.
#[derive(Encode, Decode, Clone, Debug, Default)]
pub struct ForkLocalDepot {
    /// Keys inherited from previous generations through [`Accrete`].
    ///
    /// These represent all reachable historical entries inherited from
    /// ancestor branches.
    ///
    /// They are not created in the current branch generation, but remain
    /// visible because they were promoted forward during fork creation.
    ///
    /// This allows branch-local reads to access valid ancestor state
    /// without walking parent branches repeatedly.
    pub inherited_keys: BTreeSet<[u8; 32]>,

    /// Keys created only in the current local generation.
    ///
    /// These represent the newest writes belonging exclusively to the
    /// current branch path.
    ///
    /// They are isolated to this branch until another fork occurs,
    /// at which point they are promoted into `inherited_keys` of the
    /// child branch through [`Accrete::accrete()`].
    ///
    /// This ensures writes remain fork-local while still preserving
    /// deterministic lineage inheritance for future branches.
    pub local_keys: BTreeSet<[u8; 32]>,
}

impl Accrete for ForkLocalDepot {
    /// The original payload used to derive deterministic keys.
    ///
    /// Only the generated `[u8; 32]` key is stored internally.
    type Item = Vec<u8>;

    /// Create the next generation.
    ///
    /// All current local keys are promoted into inherited history,
    /// and the returned generation starts with a fresh empty local layer.
    fn accrete(&self) -> Self {
        let mut inherited = self.inherited_keys.clone();

        // Promote current local generation into inherited lineage
        inherited.extend(self.local_keys.iter().copied());

        Self {
            inherited_keys: inherited,
            local_keys: BTreeSet::new(),
        }
    }

    /// Returns inherited keys only.
    fn inherited(&self) -> Vec<[u8; 32]> {
        self.inherited_keys
            .iter()
            .copied()
            .collect()
    }

    /// Returns current local generation keys only.
    fn local(&self) -> Vec<[u8; 32]> {
        self.local_keys
            .iter()
            .copied()
            .collect()
    }

    /// Insert an item's deterministic key into the local generation.
    ///
    /// The payload itself is not stored here,
    /// only its stable key hash.
    ///
    /// Returns the deterministic key used for future lookups.
    fn add_to_local(
        &mut self,
        item: Self::Item,
    ) -> [u8; 32] {
        let key = Self::make_key(&item);

        self.local_keys.insert(key);

        key
    }

    /// Checks existence only in local generation.
    fn exists_in_local(
        &self,
        key: &[u8; 32],
    ) -> bool {
        self.local_keys.contains(key)
    }

    /// Checks existence only in inherited generations.
    fn exists_in_inherited(
        &self,
        key: &[u8; 32],
    ) -> bool {
        self.inherited_keys.contains(key)
    }

    /// Remove a key only from the local generation.
    fn remove_from_local(
        &mut self,
        key: &[u8; 32],
    ) {
        self.local_keys.remove(key);
    }

    /// Remove a key only from inherited generations.
    fn remove_from_inherited(
        &mut self,
        key: &[u8; 32],
    ) {
        self.inherited_keys.remove(key);
    }
}


/// **Authorization interface for a [`Routines`]**.
///
/// `RoutineOf` defines **who is allowed to execute** a routine at a given
/// point in time. It separates **authorization** from **execution**, which
/// is especially important in offchain contexts where signing keys,
/// rotation, and node-local state must be handled explicitly.
///
/// ## Why this exists
///
/// Offchain workers do not have the same execution guarantees as runtime
/// calls:
/// - there is no transactional rollback,
/// - failures do not revert state,
/// - and execution is best-effort.
///
/// Because of this, *authorization must be explicit* and *checked separately*
/// before a routine is allowed to run. `RoutineOf` provides a uniform way to:
///
/// - derive the concrete identifier (e.g. public key) authorized to run a routine,
/// - enforce key rotation and role-based access,
/// - fail early if the node is misconfigured or missing required keys.
///
/// ## Design principles
///
/// - `who()` must be **pure**: it must not mutate state.
/// - Failures are logged via [`Logging`] and treated as hard stops.
/// - The returned `Identifier` is typically used to sign payloads or
///   parameterize execution.
///
/// ## Example
///
/// ```text
/// Determine authorized signer
///        |
///        V
///   who() -> PublicKey
///        |
///        V
///  run_service(by = PublicKey)
/// ```
pub trait RoutineOf<Identifier, TimeStamp>: Logging<TimeStamp> + Routines<TimeStamp>
where
    TimeStamp: Time,
    Identifier: Portable,
{
    /// Returns the identifier authorized to execute the routine.
    ///
    /// If no valid identifier exists (e.g. missing key, inconsistent state),
    /// an error is logged and execution must not proceed.
    fn who(at: &TimeStamp) -> Result<Identifier, Self::Logger>;
}

/// **Structured execution interface for offchain routines**.
///
/// `Routines` provides a **disciplined execution model** for offchain workers,
/// replacing ad-hoc logic with explicit phases and well-defined failure
/// semantics.
///
/// ## Why structured routines are needed
///
/// Offchain workers are fundamentally different from runtime calls:
///
/// | Runtime calls                    | Offchain workers                   |
/// |----------------------------------|------------------------------------|
/// | Transactional                    | Best-effort                        |
/// | Automatic rollback on error      | No rollback                        |
/// | State changes are atomic         | Partial execution is possible      |
/// | Errors bubble naturally          | Errors must be handled manually    |
///
/// As a result, offchain logic **must be structured explicitly** to ensure:
///
/// - invariants are checked before execution,
/// - routines run to *intentional completion*,
/// - partial state does not silently corrupt future runs,
/// - failures are observable and diagnosable.
///
/// The `Routines` trait enforces this structure.
///
/// ## Execution model
///
/// ```text
/// |-------------|
/// | can_run()   |  <- check invariants, prerequisites
/// |-----|-------|
///       |
///       V
/// |-------------|
/// | run_service |  <- perform the operation
/// |-----|-------|
///       |
///       V
/// |-------------|
/// | on_ran_*    |  <- bookkeeping, metrics, logging
/// |-------------|
/// ```
///
/// Each phase has a distinct responsibility, making offchain logic easier
/// to reason about, test, and evolve.
///
/// ## Failure semantics
///
/// - Any failure is **logged** via [`Logging`] and returned as `Logger`.
/// - Callers must treat failures as *hard stops* for the current routine.
/// - Subsequent routines may or may not execute, depending on orchestration.
///
/// This explicit handling avoids implicit control flow and makes routine
/// dependencies visible.
///
/// ## Arranging multiple routines
///
/// Structured routines compose naturally into pipelines:
///
/// ```text
/// Example
/// -------
/// Init -> Declare -> Rotate -> Elect
/// ```
///
/// Each routine:
/// - validates its own prerequisites,
/// - executes independently,
/// - leaves the system in a well-defined state.
///
/// This allows offchain workers to act as **deterministic coordinators**
/// rather than monolithic scripts.
///
/// ## Logging and observability
///
/// Because routines run outside the runtime's transactional model,
/// **logging is the primary observability mechanism**.
///
/// By integrating with [`Logging`]:
/// - all errors are logged exactly once,
/// - routine boundaries are visible in logs,
/// - execution can be traced across blocks.
///
/// This makes post-mortem debugging and operational monitoring feasible.
///
/// ## Example usage
///
/// ```text
/// let routine = MyRoutine { at: block };
///
/// if routine.can_run().is_ok() {
///     routine.run_service()?;
///     routine.on_ran_service();
/// }
/// ```
pub trait Routines<TimeStamp>: Logging<TimeStamp>
where
    TimeStamp: Time,
{
    /// Checks whether the routine is allowed to run.
    ///
    /// This method must:
    /// - validate prerequisites,
    /// - check invariants,
    /// - avoid mutating state.
    ///
    /// It exists to prevent partial execution in environments without
    /// rollback guarantees.
    fn can_run(&self) -> Result<(), Self::Logger>;

    /// Executes the routine's core logic.
    ///
    /// Implementations should assume that `can_run` has already succeeded
    /// and focus solely on performing the intended operation.
    fn run_service(&self) -> Result<(), Self::Logger>;

    /// Hook invoked after successful execution.
    ///
    /// This method is intended for:
    /// - logging,
    /// - metrics,
    /// - bookkeeping,
    /// - or emitting side effects that must only occur on success.
    ///
    /// The default implementation is a no-op.
    fn on_ran_service(&self) {}
}