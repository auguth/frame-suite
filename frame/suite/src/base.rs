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
// `````````````````````````````` BASE TRAITS SUITE ``````````````````````````````
// ===============================================================================

//! Defines the **foundational trait system** for deterministic,
//! codec-safe, and metadata-aware types used across the runtime.
//!
//! Each trait represents a specific capability (e.g. encoding,
//! ordering, bounded size), and higher-level traits build on top of them.
//!
//! Instead of directly depending on low-level traits (e.g. `Encode`, `TypeInfo`),
//! downstream code should prefer these abstractions to ensure consistency,
//! safety, and clarity across the codebase.

// ===============================================================================
// ````````````````````````````````` IMPORTS `````````````````````````````````````
// ===============================================================================

// --- Core ---
use core::fmt::Debug;

// --- SCALE & metadata ---
use codec::{
    Decode, DecodeWithMemTracking, Encode, EncodeLike, FullCodec, HasCompact, MaxEncodedLen,
};
use scale_info::TypeInfo;

// --- Substrate crates ---
use sp_arithmetic::traits::AtLeast8BitUnsigned;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, MaybeDisplay, MaybeSerializeDeserialize},
    DispatchError, FixedPointNumber, PerThing,
};

// ===============================================================================
// ````````````````````````````````` ESSENTIALS ``````````````````````````````````
// ===============================================================================

/// Deterministic, encode-only probe value for canonical byte-level matching.
///
/// Represents values that are not intended to be decoded or compared by their
/// raw Rust representation. Instead, they are used by encoding them into their
/// canonical SCALE form and matching that representation externally.
///
/// This includes dynamically sized types such as slices, allowing borrowed
/// views (e.g. `&[u8]`, `&[T]`) to act as probes without requiring ownership.
///
/// - [`Encode`]: Produces the canonical deterministic representation.
/// - [`Debug`]: Supports diagnostics in both `no_std` and native builds.
pub trait Probe: Encode + Debug {}

impl<T> Probe for T where T: Encode + Debug + ?Sized {}

/// Codec-safe and cloneable type portable across runtime boundaries.
///
/// `Portable` captures the minimal guarantees required for values that can be
/// encoded, decoded, and transported deterministically within a `no_std`
/// runtime. These bounds ensure values can be reconstructed from SCALE,
/// inspected during diagnostics, and safely duplicated when passed across
/// runtime logic.
///
/// - [`Encode`] + [`Decode`]: Required for deterministic SCALE serialization.
/// - [`Clone`]: Allows owned duplication across execution boundaries.
pub trait Portable: Encode + Decode + Clone {}

impl<T> Portable for T where T: Encode + Decode + Clone {}

/// Metadata-aware type that can be described in runtime metadata.
///
/// `MetaAware` ensures external tools (RPC, UI, and off-chain components) can
/// interpret the type layout without native Rust reflection while still
/// supporting lightweight diagnostics.
///
/// - [`TypeInfo`]: Enables SCALE metadata generation for external consumers.
/// - [`Debug`]: Allows readable diagnostics during runtime and testing.
pub trait MetaAware: TypeInfo + Debug {}

impl<T> MetaAware for T where T: TypeInfo + Debug {}

// ===============================================================================
// ```````````````````````````````` RUNTIME-TYPES ````````````````````````````````
// ===============================================================================

/// Deterministic, portable, and metadata-aware domain type for `no_std` runtimes.
///
/// Combines [`Portable`] and [`MetaAware`] with [`Eq`] to capture the core
/// guarantees required by most runtime domain types. These bounds ensure values
/// can be encoded, reconstructed, compared, and described in metadata
/// deterministically inside a WASM runtime.
///
/// - [`Portable`]: Provides codec safety, cloneability, and debug support.
/// - [`MetaAware`]: Enables metadata introspection by external tools.
/// - [`Eq`]: Ensures deterministic equality for consistent state comparisons.
pub trait RuntimeType: Portable + MetaAware + Eq {}

impl<T> RuntimeType for T where T: Portable + MetaAware + Eq {}

/// Codec-safe, metadata-aware enum-like runtime type representing named states.
///
/// Unlike [`RuntimeType`], this does not require [`Clone`] or [`Eq`] since such
/// enums are primarily used to express distinct named variants (states) through
/// their discriminants, not value semantics that rely on cloning or structural
/// equality comparisons. The focus is on their role as state markers rather than
/// data-bearing domain values.
///
/// When composed within other trait bounds (e.g. structs or domain aggregates),
/// the selected variant may still govern or contextualize associated values,
/// influencing behavior without itself being treated as a value-bearing domain
/// object.
///
/// [`FullCodec`] and [`TypeInfo`] ensure the enum can be SCALE encoded/decoded
/// and described in runtime metadata, while [`Debug`] supports diagnostics in
/// both `no_std` and native builds.
pub trait RuntimeEnum: FullCodec + TypeInfo + Debug {}

impl<T> RuntimeEnum for T where T: FullCodec + TypeInfo + Debug {}

/// Persistable, codec-safe runtime error convertible into [`DispatchError`].
///
/// Extends [`RuntimeEnum`] and `'static` to ensure errors are represented
/// as discrete named variants, fully owned, and safe to encode, decode, and
/// store across deterministic execution boundaries.
///
/// The [`Into<DispatchError>`] bound enables seamless integration with FRAME
/// dispatch logic while still allowing domain-specific error enumerations.
pub trait RuntimeError: RuntimeEnum + 'static + Into<DispatchError> {}

impl<T> RuntimeError for T where T: RuntimeEnum + 'static + Into<DispatchError> {}

/// Lifetime-independent, fully owned runtime type safe for on-chain storage.
///
/// Extends [`RuntimeType`] with a `'static` guarantee, ensuring the type
/// contains no non-static borrows (e.g. `&'a T`) and can be reliably encoded,
/// decoded, and persisted across deterministic runtime execution boundaries.
///
/// Since runtime storage reconstructs values purely from their encoded form,
/// any type containing temporary lifetimes cannot be safely restored. Requiring
/// `'static` guarantees the value is fully owned and stable across calls,
/// blocks, and forks.
///
/// The [`EncodeLike`] bound further ensures that equivalent representations
/// (e.g. owned vs borrowed views or transparent wrappers) produce the same
/// canonical SCALE encoding, enabling consistent storage access and matching
/// across interchangeable forms.
///
/// Use this for storage items, events, and any value that must survive beyond
/// the current execution context.
pub trait Storable: 'static + RuntimeType + EncodeLike {}

impl<T> Storable for T where T: RuntimeType + 'static + EncodeLike {}

/// Upper-bounded encoded-size runtime type with flexible internal structure.
///
/// Extends [`RuntimeType`] with [`MaxEncodedLen`], guaranteeing a predictable
/// upper bound on encoded size while allowing structural flexibility
/// (e.g. weak-bounded collections).
///
/// Unlike [`Delimited`], this does *not* require `'static`, so it may be used in
/// temporary or lifetime-bound contexts. Add [`Storable`] when persistence is
/// required.
pub trait Elastic: RuntimeType + MaxEncodedLen {}

impl<T> Elastic for T where T: RuntimeType + MaxEncodedLen {}

/// Strictly size-bounded and lifetime-independent runtime type for persistence.
///
/// Combines [`Storable`] and [`MaxEncodedLen`] to ensure values are fully
/// owned and have a predictable maximum encoded footprint, enabling safe
/// storage, decoding, and metadata generation.
///
/// Prefer this only when **all** fields are strictly bounded; otherwise use
/// [`Elastic`].
pub trait Delimited: Storable + MaxEncodedLen + DecodeWithMemTracking {}

impl<T> Delimited for T where T: Storable + MaxEncodedLen + DecodeWithMemTracking {}

/// Persistable, orderable, strictly bounded type for multi-instance comparison.
///
/// Builds on [`Storable`] and [`MaxEncodedLen`] while adding [`Ord`] to enable
/// deterministic total ordering when multiple values coexist or are compared
/// across execution paths.
///
/// Suitable for keys, sorted collections, and deduplication logic where stable
/// ordering across nodes is required.
pub trait Sortable: Storable + Ord + MaxEncodedLen + DecodeWithMemTracking {}

impl<T> Sortable for T where T: Storable + Ord + MaxEncodedLen + DecodeWithMemTracking {}

/// Persistable runtime value representing a stable identity/key.
///
/// Extends [`Sortable`] and [`MaybeDisplay`] to indicate the value is not only
/// deterministically orderable but also suitable for human-readable identity
/// representation (e.g. hashes, IDs, or addresses) in logs, events, and UIs.
///
/// This trait conveys role semantics rather than additional structural bounds:
/// the type behaves as a uniquely distinguishable identity that can be
/// consistently compared across deterministic execution and meaningfully
/// displayed when surfaced externally.
pub trait Keyed: Sortable + MaybeDisplay {}

impl<T> Keyed for T where T: Sortable + MaybeDisplay {}

/// Cross-environment portable runtime type usable in both `std` and `no_std`.
///
/// Extends [`RuntimeType`] with thread-safety and optional serialization so the
/// same domain type can be safely shared between:
/// - deterministic WASM runtime execution, and
/// - native node/RPC/off-chain contexts.
///
/// This avoids semantic divergence between runtime and host representations.
pub trait CrossEnvType: RuntimeType + Send + Sync + MaybeSerializeDeserialize {}

impl<T> CrossEnvType for T where T: RuntimeType + Send + Sync + MaybeSerializeDeserialize {}

// ===============================================================================
// ``````````````````````````````` RUNTIME-NUMBERS ```````````````````````````````
// ===============================================================================

/// Deterministic, compact, persistable numeric type for runtime quantities.
///
/// Extends [`Sortable`] with numeric ergonomics:
/// - [`Copy`] for primitive-like bitwise copy semantics
/// - [`HasCompact`] for efficient numeric SCALE encoding by truncating unused bits
/// - [`Default`] for a canonical zero-like value for instantiation
/// - [`CrossEnvType`] to ensure safe portability across `no_std` runtime and
///   native (`std`) environments, enabling optional serialization (e.g. `serde`)
///   and thread-safe usage in off-chain, RPC, or testing contexts.
///
/// These guarantees make the type safe for repeated arithmetic, deterministic
/// ordering, compact persistence, and cross-environment interoperability within
/// runtime logic.
pub trait RuntimeNum:
    Sortable + Copy + HasCompact<Type: DecodeWithMemTracking> + Default + CrossEnvType
{
}

impl<T> RuntimeNum for T where
    T: Sortable + Copy + HasCompact<Type: DecodeWithMemTracking> + Default + CrossEnvType
{
}

/// Unsigned arithmetic-capable quantity used for balances and financial logic.
///
/// Focuses on numeric semantics while delegating persistence, ordering, and
/// codec guarantees to [`RuntimeNum`].
pub trait Asset: AtLeast32BitUnsigned + RuntimeNum {}

impl<T> Asset for T where T: AtLeast32BitUnsigned + RuntimeNum {}

/// Unsigned temporal quantity for block numbers, timestamps, and durations.
///
/// Represents non-negative time-related values (e.g. block heights, Unix
/// timestamps, or high-precision durations) while inheriting deterministic
/// ordering, compact encoding, bounded persistence, and cross-environment
/// portability from [`RuntimeNum`].
///
/// The [`MaybeDisplay`] bound allows such values to be meaningfully presented in
/// logs, events, and UIs when available, without affecting `no_std` compatibility.
pub trait Time: AtLeast32BitUnsigned + RuntimeNum + MaybeDisplay {}

impl<T> Time for T where T: AtLeast32BitUnsigned + RuntimeNum + MaybeDisplay {}

/// Unsigned, low-precision quantity for user-scale counts and limits.
///
/// Represents small non-negative values (indices, limits, pagination, etc.)
/// while inheriting determinism, compact encoding, and bounded persistence
/// guarantees from [`RuntimeNum`].
pub trait Countable: AtLeast8BitUnsigned + RuntimeNum {}

impl<T> Countable for T where T: AtLeast8BitUnsigned + RuntimeNum {}

/// Deterministic fixed-point numeric type for fractional runtime arithmetic.
///
/// Encapsulates precise fractional math via [`FixedPointNumber`] while relying
/// on [`RuntimeNum`] for ordering, compact encoding, and bounded persistence.
pub trait Fractional: FixedPointNumber + RuntimeNum {}

impl<T> Fractional for T where T: FixedPointNumber + RuntimeNum {}

/// Deterministic fixed-precision percentage / ratio for runtime arithmetic.
///
/// Extends [`RuntimeNum`] for ordering, compact encoding, and bounded
/// persistence.
///
/// The [`PerThing`] bound provides fixed-point fractional behavior with a
/// compile-time accuracy factor, enabling precise percentage-style operations
/// (fees, weights, slashing ratios, interest rates) without floating-point
/// semantics.
pub trait Percentage: RuntimeNum + PerThing {}

impl<T> Percentage for T where T: RuntimeNum + PerThing {}

// ===============================================================================
// ```````````````````````````````` RUNTIME-LISTS ````````````````````````````````
// ===============================================================================

/// Slice-like container providing direct indexed and range-based access.
///
/// `Indexable` represents types whose contents can be accessed using the full
/// family of Rust slice indexing operations (single index and ranges). This
/// enables algorithms that rely on positional access rather than iteration.
///
/// However, these operations follow the standard Rust indexing semantics and
/// **may panic if the index or range is out of bounds**. For this reason,
/// `Indexable` should only be used when the caller can guarantee that bounds
/// are valid or when the structure's invariants ensure safe indexing.
///
/// In most runtime logic, **iteration-based abstractions** such as [`Buffer`]
/// or [`Collection`] are preferred since they avoid panic-prone positional
/// access and better support deterministic runtime execution.
///
/// `Indexable` is therefore best suited for:
///
/// - fixed-size or strictly bounded containers
/// - structures where indices are validated by prior logic
/// - algorithms that require deterministic positional access
///
/// When combined with other runtime traits (e.g. [`RuntimeType`], [`Elastic`],
/// [`Delimited`]), the container can participate safely in deterministic runtime
/// computation while still exposing slice-style access semantics.
///
/// Typical implementors include:
///
/// - [`Vec<T>`]
/// - fixed-size arrays (`[T; N]`)
/// - bounded containers such as `BoundedVec<T, _>`
pub trait Indexable<T>:
    AsRef<[T]>
    + AsMut<[T]>
    + core::ops::Index<usize, Output = T>
    + core::ops::IndexMut<usize>
    + core::ops::Index<core::ops::Range<usize>, Output = [T]>
    + core::ops::IndexMut<core::ops::Range<usize>>
    + core::ops::Index<core::ops::RangeFrom<usize>, Output = [T]>
    + core::ops::IndexMut<core::ops::RangeFrom<usize>>
    + core::ops::Index<core::ops::RangeTo<usize>, Output = [T]>
    + core::ops::IndexMut<core::ops::RangeTo<usize>>
    + core::ops::Index<core::ops::RangeInclusive<usize>, Output = [T]>
    + core::ops::IndexMut<core::ops::RangeInclusive<usize>>
    + core::ops::Index<core::ops::RangeToInclusive<usize>, Output = [T]>
    + core::ops::IndexMut<core::ops::RangeToInclusive<usize>>
    + core::ops::Index<core::ops::RangeFull, Output = [T]>
    + core::ops::IndexMut<core::ops::RangeFull>
{
}

impl<C, T> Indexable<T> for C where
    C: AsRef<[T]>
        + AsMut<[T]>
        + core::ops::Index<usize, Output = T>
        + core::ops::IndexMut<usize>
        + core::ops::Index<core::ops::Range<usize>, Output = [T]>
        + core::ops::IndexMut<core::ops::Range<usize>>
        + core::ops::Index<core::ops::RangeFrom<usize>, Output = [T]>
        + core::ops::IndexMut<core::ops::RangeFrom<usize>>
        + core::ops::Index<core::ops::RangeTo<usize>, Output = [T]>
        + core::ops::IndexMut<core::ops::RangeTo<usize>>
        + core::ops::Index<core::ops::RangeInclusive<usize>, Output = [T]>
        + core::ops::IndexMut<core::ops::RangeInclusive<usize>>
        + core::ops::Index<core::ops::RangeToInclusive<usize>, Output = [T]>
        + core::ops::IndexMut<core::ops::RangeToInclusive<usize>>
        + core::ops::Index<core::ops::RangeFull, Output = [T]>
        + core::ops::IndexMut<core::ops::RangeFull>
{
}

/// A generic iterable, indexable sequence supporting deterministic traversal.
///
/// This trait captures the **minimal capabilities required for iteration and
/// positional access** without assuming construction or mutation.
///
/// ## Capabilities:
/// - [`IntoIterator`]: Enables deterministic iteration over contained values.
/// - [`Indexable`]: Provides direct indexed and range-based access to elements.
///
/// ## Design Notes:
/// - Represents **read-only traversal + positional access semantics**.
/// - Does not assume the ability to construct or mutate the sequence.
/// - Compatible with both fixed-size and dynamic containers.
/// - Enables algorithms that require both iteration and indexing.
///
/// ## Supported Types:
/// - Fixed-size arrays (e.g. `[T; N]`)
/// - Dynamic collections (e.g. [`Vec<T>`], `VecDeque`, `BoundedVec`)
///
/// ## Usage:
/// Use this trait when:
/// - iteration and indexed access are required,
/// - no assumptions about construction or mutation should be made.
///
/// For construction and mutation, see [`Growable`].
pub trait Collection<Item>: IntoIterator<Item = Item> + Indexable<Item> {}

impl<T, Item> Collection<Item> for T where T: IntoIterator<Item = Item> + Indexable<Item> {}

/// A growable collection supporting construction, incremental accumulation,
/// and empty initialization.
///
/// This trait extends [`Collection`] with **construction and mutation capabilities**,
/// enabling values to be built and appended over time.
///
/// ## Capabilities:
/// - [`FromIterator`]: Allows constructing the collection from iterator output.
/// - [`Extend`]: Supports incremental addition of elements from an iterator.
/// - [`Default`]: Provides a canonical empty instance for initialization.
///
/// ## Design Notes:
/// - Represents **dynamic, resizable collections**.
/// - Excludes fixed-size containers that cannot grow or be constructed generically.
/// - Separates construction and mutation semantics from traversal and indexing.
///
/// ## Supported Types:
/// - [`Vec<T>`]
/// - `VecDeque<T>`
/// - `BTreeSet<T>`
///
/// ## Usage:
/// Use this trait when:
/// - collections must be constructed from iterators,
/// - elements are appended dynamically,
/// - mutation is part of the execution logic.
///
/// For iteration and indexing without mutation, see [`Collection`].
pub trait Growable<Item>: Collection<Item> + FromIterator<Item> + Extend<Item> + Default {}

impl<T, Item> Growable<Item> for T where
    T: Collection<Item> + FromIterator<Item> + Extend<Item> + Default
{
}

/// Deterministic, codec-safe growable buffer for transient runtime aggregation.
///
/// This trait extends [`Growable`] with runtime guarantees so that collections of
/// runtime domain types can be safely constructed, extended, and iterated
/// during deterministic execution.
///
/// ## Element Requirements:
/// The contained `Item` must implement [`RuntimeType`], ensuring each element is:
/// - codec-safe via SCALE encoding/decoding,
/// - metadata-aware for external tooling,
/// - deterministic and comparable within runtime logic.
///
/// ## Design Notes:
/// - Represents **transient, in-memory aggregation structures**.
/// - Does not imply persistence or storage semantics.
/// - Does not require strict encoded-size bounds.
/// - Focuses on safe, deterministic computation within runtime execution.
///
/// ## Usage Scenarios:
/// - Batching operations
/// - Collecting intermediate results
/// - Building temporary working sets
///
/// ## Supported Types:
/// - [`Vec<T>`]
/// - Other runtime-safe growable collections whose elements satisfy [`RuntimeType`]
pub trait Buffer<Item: RuntimeType>: RuntimeType + Growable<Item> {}

impl<T, Item> Buffer<Item> for T
where
    Item: RuntimeType,
    T: RuntimeType + Growable<Item>,
{
}
