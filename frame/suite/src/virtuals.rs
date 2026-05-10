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
// ```````````````````````````````` VIRTUALS SUITE ```````````````````````````````
// ===============================================================================

//! Virtual struct system mapping discriminants (ZSTs) to behavior via
//! trait-driven schemas.
//!
//! Traditional Rust data structures require committing upfront to:
//! - a fixed shape (`Option<T>`, `Vec<T>`, `[T; N]`, struct fields)
//! - a fixed cardinality (single vs multiple)
//! - a fixed storage layout
//!
//! This makes evolution difficult:
//! - structure and storage are tightly coupled
//! - generic abstractions often hit coherence limits
//! - external composition (plugins, schemas) is hard
//! - changing a field (`None -> Some -> Many`) requires redesign
//!
//! This module introduces a system where **structure, interpretation,
//! and storage are decoupled**, and resolved through types.
//!
//! ## Mental Model: Virtual Structs
//!
//! Think in terms of a **virtual struct**, whose fields are not stored
//! directly, but defined through traits and discriminants:
//!
//! ```ignore
//! struct <T as Trait>::Struct<K> {
//!     FieldTag: T::Field, // virtual field (identified by discriminant)
//!     ExtTag: K,          // virtual extension (identified by discriminant)
//! }
//! ```
//!
//! This is not a concrete struct. Instead:
//!
//! - fields are accessed via trait implementations
//! - each field is identified by a **discriminant (type-level key)**
//! - storage is abstracted or external
//!
//! ## Discriminants (Field Identifiers)
//!
//! ```ignore
//! struct FieldTag;
//! struct ExtTag;
//! ```
//!
//! Discriminants are zero-sized types used as **type-level field keys**.
//!
//! They act as:
//! - field identifiers in the virtual struct
//! - selectors for behavior and interpretation
//! - disambiguators for generic implementations
//!
//! This ensures:
//! - multiple fields can coexist without ambiguity
//! - overlapping generic impls remain coherence-safe
//!
//! ## Trait as Schema
//!
//! ```ignore
//! pub trait Trait {
//!     type Struct: VirtualDynField<FieldTag, Some = Self::Field>
//!                 + VirtualDynExtension<ExtTag>;
//!
//!     type Field;
//! }
//! ```
//!
//! - `Struct` is the container (virtual struct)
//! - `FieldTag` identifies a logical field
//! - `ExtTag` identifies an extension slot
//! - `Field` defines the logical type of the field
//!
//! Traits define the **schema**, not the storage.
//!
//! ## Field Behavior (Cardinality Abstraction)
//!
//! A virtual field supports:
//!
//! - `None`  - no value
//! - `Some`  - one value
//! - `Many`  - multiple values
//!
//! This replaces fixed representations like:
//!
//! ```ignore
//! Option<T> / Vec<T>
//! ```
//!
//! with a single abstraction that can evolve without redesign.
//!
//! ## Key Insight
//!
//! A virtual struct is not a fixed layout, but a **composition of
//! discriminant-keyed behaviors**, where:
//!
//! - **discriminants** -> field identifiers
//! - **traits** -> schema (what exists)
//! - **implementations** -> storage (how/where it exists)
//!
//! Structure is resolved by types, not encoded directly.
//!
//! ## Core Primitives
//!
//! The system is built from:
//!
//! ### Virtual Fields
//! - [`VirtualDynField`] - dynamic, vector-like semantics
//! - [`VirtualStaticField`] - static, array-like semantics
//!
//! ### Virtual Extensions
//! - [`VirtualDynExtension`] / [`VirtualStaticExtension`]
//! - externally defined fields via schemas
//!
//! ### Schemas & Bounds
//! - [`VirtualDynBound`] / [`VirtualStaticBound`]
//! - constraints defined independently of storage
//!
//! ### Concrete Representations
//! - [`SumDynType`] - bounded vector semantics (`None | Some | Many`)
//! - [`SumStaticType`] - fixed array semantics
//!
//! ## Design Principles
//!
//! ### Discriminant-Keyed Design
//!
//! All components are keyed by a [`DiscriminantTag`].
//!
//! This:
//! - avoids ambiguity in generic implementations
//! - enables multiple independent fields on the same container
//! - ensures coherence-safe extensibility
//!
//! ### Tagged Conversions
//!
//! Instead of `From` / `Into`, the system uses:
//!
//! - [`FromTag`], [`IntoTag`]
//! - [`TryFromTag`], [`TryIntoTag`]
//!
//! Conversions are disambiguated by discriminants:
//! - [`NoneTag`] - absence
//! - [`SomeTag`] - single value
//! - [`ManyTag`] - multiple values
//!
//! This avoids overlapping implementations in generic contexts.
//!
//! ### Layered Model
//!
//! The system separates:
//!
//! - **Type-level layer**
//!   - defines structure, schema, and behavior
//!
//! - **Value-level layer**
//!   - represents `None`, `Some`, `Many`
//!
//! This enables abstract structure with concrete representations.
//!
//! ### Dynamic vs Static
//!
//! #### Dynamic (Runtime Flexible)
//! - vector-like semantics
//! - runtime bounds (`Get<u32>`)
//! - growable/shrinkable collections
//!
//! #### Static (Compile-Time Fixed)
//! - array-like semantics
//! - compile-time bounds (`const`)
//! - zero-overhead representations
//!
//! ## Helpers
//!
//! Ergonomic helpers are provided for working with virtual components:
//!
//! - [`DynFieldHelpers`] / [`StaticFieldHelpers`]
//! - [`DynExtHelpers`] / [`StaticExtHelpers`]
//!
//! ## Summary
//!
//! This system enables:
//!
//! - evolving data shapes without redesign
//! - discriminant-keyed field composition
//! - external composition via schemas and extensions
//! - reuse of storage across multiple logical fields
//! - coherence-safe generic abstractions
//!
//! It provides a **type-level virtualization layer** where:
//!
//! > A virtual struct is a mapping from **discriminants -> behaviors**,
//! > resolved through traits and implemented via storage.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::base::{Buffer, Collection, Delimited, Growable, Indexable, RuntimeError};

// --- Core (Rust std replacement) ---
use core::fmt::{self, Debug};

// --- Scale-codec crates ---
use codec::{Decode, DecodeWithMemTracking, Encode, EncodeLike, MaxEncodedLen};
use scale_info::TypeInfo;

// --- FRAME Support ---
use frame_support::{
    storage::types::{
        EncodeLikeTuple, HasKeyPrefix, HasReversibleKeyPrefix, KeyGenerator,
        ReversibleKeyGenerator, TupleToEncodedIter,
    },
    IterableStorageMap, IterableStorageNMap, StorageMap, StorageNMap, StoragePrefixedMap,
};

// --- Substrate primitives ---
use sp_core::{ConstU32, Get};
use sp_runtime::{traits::Zero, BoundedVec, Vec};
use sp_std::vec;

// ===============================================================================
// ```````````````````````````````` DISCRIMINANTS ````````````````````````````````
// ===============================================================================

/// Marker trait for type-level discriminants.
///
/// A `Discriminant` is a zero-sized type used to uniquely identify behavior,
/// structure, or interpretation at the type level.
///
/// ## Guidelines
///
/// Implementors should:
/// - be zero-sized types (ZST)
/// - carry no runtime data
/// - act purely as type-level identifiers
///
/// ## Motivation
///
/// In Rust, trait implementations involving generic or associated types can
/// become ambiguous under the coherence rules:
///
/// - generic parameters may unify in unexpected ways
/// - multiple impls may overlap when types are not fully concrete
/// - the compiler must conservatively reject such cases
///
/// To avoid this, a **concrete type-level key** is introduced as a discriminant.
///
/// By adding a `Discriminant`:
/// - each implementation becomes uniquely identifiable
/// - ambiguity between generic impls is disposed
/// - coherence is preserved without restricting expressiveness
///
/// ## Role in the System
///
/// Discriminants are used in some cases like
/// - keys for [`VirtualDynField`]
/// - identifiers for [`plugin`](crate::plugins) operations
/// - selectors for tagged conversions ([`FromTag`], [`IntoTag`], etc.)
///
/// They allow multiple interpretations over the same underlying types
/// without conflict.
pub trait DiscriminantTag {}

/// Default Discriminant implementation if no
/// multiple interpretations are required.
impl DiscriminantTag for () {}

/// Defines one or more public zero-sized discriminant types.
///
/// A *discriminant* is a concrete type-level key used to uniquely identify
/// behavior, structure, or interpretation in generic systems.
///
/// ## Why Discriminants?
///
/// In Rust, trait implementations over generic or associated types can become
/// ambiguous under coherence rules:
/// - generic types may unify in multiple ways
/// - implementations may overlap when types are not fully concrete
/// - the compiler must reject such cases conservatively
///
/// Discriminants solve this by introducing a **concrete, unique type**
/// that disambiguates otherwise overlapping implementations.
///
/// This enables:
/// - multiple interpretations over the same types
/// - safe composition of generic abstractions
/// - coherence-safe extensibility
///
/// ## Syntax
///
/// ```ignore
/// discriminants!(
///     /// Optional docs or attributes
///     A,
///
///     B,
///
///     #[cfg(feature = "x")]
///     C,
/// );
/// ```
///
/// ## Expansion
///
/// For each identifier, this macro generates:
/// - a `pub` zero-sized struct
/// - an implementation of [`DiscriminantTag`]
///
/// ## Properties
///
/// - zero runtime cost (ZSTs)
/// - purely type-level identifiers
/// - stable and unambiguous across generic contexts
/// - reusable across virtual fields, extensions, and
/// [`plugin`](crate::plugins) systems
///
/// ## Why Always Public
///
/// Discriminants appear in type signatures and generic bounds,
/// making them part of the public type-level API.
///
/// Restricting visibility would:
/// - prevent use in external generic constraints
/// - break composability across modules or crates
/// - force duplication of identical identifiers
///
/// In practice, discriminants are **type-level contracts**, not internal details.
#[macro_export]
macro_rules! discriminants {
    (
        $(
            $(#[$meta:meta])*
            $name:ident
        ),* $(,)?
    ) => {
        $(
            $(#[$meta])*
            #[derive(Clone, Copy, Debug, Default)]
            pub struct $name;

            impl $crate::virtuals::DiscriminantTag for $name {}
        )*
    };
}

/// Implements [`DiscriminantTag`] for one or more existing types.
///
/// This macro allows pre-existing types to act as discriminants without
/// redefining them.
///
/// ## Why Discriminants?
///
/// Discriminants provide a **concrete type-level key** that avoids ambiguity
/// in generic trait resolution under Rust's coherence rules.
///
/// By associating behavior with a discriminant instead of relying solely
/// on generic types, implementations become:
/// - uniquely identifiable
/// - non-overlapping
/// - composable across abstraction boundaries
///
/// ## When to Use
///
/// Use this macro when:
/// - a type already exists and should act as a discriminant
/// - redefining it as a new ZST would be redundant or impossible
///
/// ## Syntax
///
/// ```ignore
/// struct MyTag;
/// struct OtherTag;
///
/// impl_discriminants!(MyTag, OtherTag);
/// ```
///
/// ## Behavior
///
/// - Implements [`DiscriminantTag`] for each provided type
/// - Preserves the original type definition
///
/// ## Notes
///
/// - Types are expected to behave like discriminants (typically ZSTs)
/// - No runtime guarantees are enforced; this is a semantic contract
///
/// ## Summary
///
/// This macro extends the discriminant system to existing types,
/// enabling reuse and integration without redefining identifiers.
#[macro_export]
macro_rules! impl_discriminants {
    (
        $(
            $(#[$meta:meta])*
            $ty:ty
        ),* $(,)?
    ) => {
        $(
            $(#[$meta])*
            impl $crate::virtuals::DiscriminantTag for $ty {}
        )*
    };
}

// ===============================================================================
// ````````````````````` FROM/INTO DISCRIMINANTED CONVERSIONS ````````````````````
// ===============================================================================

/// Converts a value `T` into `Self` under a given discriminant.
///
/// Unlike [`From`], this trait introduces an additional type parameter
/// (`Discriminant` implementing [`DiscriminantTag`] via
/// [`discriminants`] or [`impl_discriminants`])
/// to distinguish between conversions that would otherwise overlap under
/// Rust's coherence rules.
///
/// This is necessary because:
/// - `T` and `Self` may be generic or associated types (i.e., not fully concrete)
/// - such types may unify in the future
/// - the compiler must conservatively reject potentially overlapping impls
///
/// By adding a concrete discriminant (tag), each conversion becomes uniquely
/// identifiable at the type level.
///
/// ## Type Parameters
/// - `T`: Source type (may be generic or non-concrete).
/// - `Discriminant`: A concrete marker type used to disambiguate conversions.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: uses the unit type as a default tag,
///   meaning no additional disambiguation when a single interpretation exists.
pub trait FromTag<T, Discriminant: DiscriminantTag = ()>: Sized {
    fn from_tag(t: T) -> Self;
}

/// Fallible version of [`FromTag`].
///
/// Allows conversions that may fail, while still being disambiguated by a
/// concrete discriminant.
///
/// ## Type Parameters
/// - `T`: Source type (may be generic or non-concrete).
/// - `Discriminant`: A concrete marker type used to disambiguate conversions.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: uses the unit type as a default tag,
///   meaning no additional disambiguation when a single interpretation exists.
pub trait TryFromTag<T, Discriminant: DiscriminantTag = ()>: Sized {
    type Error;

    fn try_from_tag(t: T) -> Result<Self, Self::Error>;
}

/// Converts `self` into another representation under a given discriminant.
///
/// This is the method-based counterpart to [`FromTag`]. It exists for ergonomic
/// use, similar to how [`Into`] complements [`From`].
///
/// The additional discriminant ensures that conversions remain unambiguous even
/// when source and target types are not fully concrete.
///
/// ## Type Parameters
/// - `R`: Target type (may be generic or non-concrete).
/// - `Discriminant`: A concrete marker type used to disambiguate conversions.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: uses the unit type as a default tag,
///   meaning no additional disambiguation when a single interpretation exists.
pub trait IntoTag<R, Discriminant: DiscriminantTag = ()> {
    fn into_tag(self) -> R;
}

/// Fallible version of [`IntoTag`].
///
/// Attempts to convert `self` into another representation under a given
/// discriminant, returning an error if the conversion fails.
///
/// ## Type Parameters
/// - `R`: Target type (may be generic or non-concrete).
/// - `Discriminant`: A concrete marker type used to disambiguate conversions.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: uses the unit type as a default tag,
///   meaning no additional disambiguation when a single interpretation exists.
pub trait TryIntoTag<R, Discriminant: DiscriminantTag = ()> {
    type Error;

    fn try_into_tag(self) -> Result<R, Self::Error>;
}

/// Blanket implementation of [`IntoTag`] for any type that implements [`FromTag`].
///
/// This mirrors the relationship between [`Into`] and [`From`], while preserving
/// disambiguation via the discriminant.
///
/// The discriminant ensures that even when `T` and `U` are not fully concrete,
/// the conversion remains uniquely identifiable and does not violate coherence.
impl<T, U, Tag> IntoTag<U, Tag> for T
where
    U: FromTag<T, Tag>,
    Tag: DiscriminantTag,
{
    fn into_tag(self) -> U {
        U::from_tag(self)
    }
}

// ===============================================================================
// `````````````````````````````````` SUM TYPES ``````````````````````````````````
// ===============================================================================

discriminants! {
    /// A discriminant representing the absence of a value in tagged conversions.
    ///
    /// This is a zero-sized type used as a type-level primitive to select
    /// conversion behavior. It acts as a compile-time discriminator,
    /// allowing multiple interpretations over the same types.
    NoneTag,

    /// A discriminant representing a single value in tagged conversions.
    ///
    /// This is a zero-sized type used as a type-level primitive to select
    /// conversion behavior. It distinguishes conversions operating on
    /// a singular value.
    SomeTag,

    /// A discriminant representing multiple values in tagged conversions.
    ///
    /// This is a zero-sized type used as a type-level primitive to select
    /// conversion behavior. It distinguishes conversions operating on
    /// collections of values.
    ManyTag,
}

/// A concrete representation of field cardinality: zero, one, or many values.
///
/// `SumDynType` unifies the possible shapes of a field into a single type:
/// - absence (`None`)
/// - a single value (`Some`)
/// - multiple values (`Many`)
///
/// ## Context
///
/// In the virtual field system:
/// - [`VirtualDynField`] defines field behavior abstractly (type-level)
/// - `SumDynType` provides a concrete, value-level representation
///
/// It is commonly used as the backing representation (`Repr`) for
/// dynamically shaped fields.
///
/// ## Representation Model
///
/// The `Many` variant is backed by a [`BoundedVec`], giving it
/// **vector-like semantics**:
/// - dynamically sized (up to a bound)
/// - growable and shrinkable
/// - capacity enforced via a type-level limit
///
/// This makes `SumDynType` suitable for:
/// - flexible schemas
/// - deferred structure
/// - abstraction across boundaries where size is not fixed
///
/// ## Variants
/// - `None`: no value
/// - `Some(Type)`: exactly one value
/// - `Many(BoundedVec<Type, S>)`: multiple values with vector semantics
///
/// ## Type Parameters
/// - `Type`: element type
/// - `S`: type-level capacity bound
///
/// ## Key Property
///
/// This is a **concrete (non-virtual) representation** using
/// **bounded vector semantics**, allowing flexible cardinality
/// within a constrained capacity.
///
/// ## Default Generics
///
/// - `Type = ()`: no meaningful value (unit type)
/// - `S = ConstU32<0>`: zero capacity, `Many` cannot store elements
///
/// Together, this yields a **no-op, zero-capacity representation**,
/// useful as a placeholder in generic contexts.
#[derive(Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, Default, Debug)]
#[scale_info(skip_type_params(S))]
pub enum SumDynType<Type = (), S = ConstU32<0>>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    #[default]
    None,
    Some(Type),
    Many(BoundedVec<Type, S>),
}

impl<Type, S> PartialEq for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + fmt::Debug,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Some(a), Self::Some(b)) => a == b,
            (Self::Many(a), Self::Many(b)) => a == b,
            _ => false,
        }
    }
}

impl<Type, S> Eq for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + fmt::Debug,
{
}

impl<Type, S> FromTag<(), NoneTag> for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    fn from_tag(_t: ()) -> Self {
        SumDynType::None
    }
}

impl<Type, S> TryIntoTag<(), NoneTag> for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    type Error = ();

    fn try_into_tag(self) -> Result<(), Self::Error> {
        match self {
            SumDynType::None => Ok(()),
            _ => Err(()),
        }
    }
}

impl<Type, S> FromTag<Type, SomeTag> for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    fn from_tag(t: Type) -> Self {
        SumDynType::Some(t)
    }
}

impl<Type, S> TryIntoTag<Type, SomeTag> for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    type Error = ();

    fn try_into_tag(self) -> Result<Type, Self::Error> {
        match self {
            SumDynType::Some(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl<Type, S> TryFromTag<Vec<Type>, ManyTag> for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    type Error = ();

    fn try_from_tag(t: Vec<Type>) -> Result<Self, Self::Error> {
        Ok(SumDynType::Many(
            BoundedVec::<Type, S>::try_from(t).map_err(|_| ())?,
        ))
    }
}

impl<Type, S> IntoTag<Vec<Type>, ManyTag> for SumDynType<Type, S>
where
    Type: Delimited,
    S: Get<u32> + Clone + Debug + 'static,
{
    fn into_tag(self) -> Vec<Type> {
        match self {
            SumDynType::None => Vec::new(),
            SumDynType::Some(v) => vec![v],
            SumDynType::Many(vec) => vec.to_vec(),
        }
    }
}

/// A statically shaped representation of field cardinality.
///
/// `SumStaticType` encodes the possible shapes of a field:
/// - absence (`None`)
/// - a single value (`Some`)
/// - multiple values (`Many`)
///
/// ## Context
///
/// In the virtual field system:
/// - [`VirtualStaticField`] defines fields whose structure is
///   fully determined at compile time
/// - `SumStaticType` provides a matching concrete representation
///
/// ## Representation Model
///
/// The `Many` variant is backed by a fixed-size array (`[Type; N]`),
/// giving it **array-like semantics**:
/// - size is fixed at compile time
/// - no resizing or allocation
/// - capacity is encoded directly in the type
///
/// This makes `SumStaticType` suitable for:
/// - compile-time enforced layouts
/// - fixed schemas
/// - zero-overhead representations
///
/// ## Variants
/// - `None`: no value
/// - `Some(Type)`: exactly one value
/// - `Many([Type; N])`: fixed-size collection with array semantics
///
/// ## Type Parameters
/// - `Type`: element type
/// - `N`: compile-time capacity
///
/// ## Key Property
///
/// This is a **fully determined representation** using
/// **array semantics**, where both cardinality and capacity
/// are encoded directly in the type.
///
/// ## Default Generics
///
/// - `Type = ()`: no meaningful value (unit type)
/// - `N = 0`: zero-sized array, `Many` holds no elements
///
/// Together, this yields a **no-op, zero-sized representation**,
/// useful as a placeholder in generic contexts.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Default,
    Debug,
    PartialEq,
    Eq,
)]
pub enum SumStaticType<Type = (), const N: usize = 0>
where
    Type: Delimited,
{
    #[default]
    None,
    Some(Type),
    Many([Type; N]),
}

impl<Type, const N: usize> FromTag<(), NoneTag> for SumStaticType<Type, N>
where
    Type: Delimited,
{
    fn from_tag(_t: ()) -> Self {
        SumStaticType::None
    }
}

impl<Type, const N: usize> TryIntoTag<(), NoneTag> for SumStaticType<Type, N>
where
    Type: Delimited,
{
    type Error = ();

    fn try_into_tag(self) -> Result<(), Self::Error> {
        match self {
            SumStaticType::None => Ok(()),
            _ => Err(()),
        }
    }
}

impl<Type, const N: usize> FromTag<Type, SomeTag> for SumStaticType<Type, N>
where
    Type: Delimited,
{
    fn from_tag(t: Type) -> Self {
        SumStaticType::Some(t)
    }
}

impl<Type, const N: usize> TryIntoTag<Type, SomeTag> for SumStaticType<Type, N>
where
    Type: Delimited,
{
    type Error = ();

    fn try_into_tag(self) -> Result<Type, Self::Error> {
        match self {
            SumStaticType::Some(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl<Type, const N: usize> FromTag<[Type; N], ManyTag> for SumStaticType<Type, N>
where
    Type: Delimited,
{
    fn from_tag(t: [Type; N]) -> Self {
        SumStaticType::Many(t)
    }
}

impl<Type, const N: usize> TryIntoTag<[Type; N], ManyTag> for SumStaticType<Type, N>
where
    Type: Delimited,
{
    type Error = ();

    fn try_into_tag(self) -> Result<[Type; N], Self::Error> {
        match self {
            SumStaticType::Many(v) => Ok(v),
            _ => Err(()),
        }
    }
}

// ===============================================================================
// ```````````````````````````````` VIRTUAL FIELDS ```````````````````````````````
// ===============================================================================

/// A discriminant-keyed virtual field abstraction with flexible cardinality
/// (`None`, `Some`, or `Many`).
///
/// This trait models a field whose structure is **deferred** and resolved
/// through types rather than fixed upfront.
///
/// ## Model
///
/// A `VirtualDynField` behaves like a field in a logical record, without
/// committing to:
///
/// - a concrete container (`Option`, `Vec`, etc.)
/// - a fixed cardinality
/// - or a fixed storage layout
///
/// Instead:
/// - the **implementor** defines the backing representation (`Repr`)
/// - the **caller** selects the shape (`None`, `Some`, or `Many`)
///
/// ## Representation Semantics
///
/// The `Many` form is expected to have **vector-like semantics**:
/// - dynamically sized (within bounds)
/// - growable and shrinkable
/// - capacity enforced but not encoded in the type shape
///
/// This makes `VirtualDynField` suitable for:
/// - flexible schemas
/// - deferred structure
/// - abstraction across boundaries
///
/// ## Discriminant
///
/// The `Discriminant` acts as a type-level key, allowing multiple independent
/// fields to coexist without ambiguity.
///
/// ## Design
///
/// Responsibilities are separated:
///
/// - **Implementor (Storage Layer)**
///   - defines representation (`Repr`)
/// - **Caller (Shape Layer)**
///   - selects cardinality and interpretation
///
/// This decouples logical meaning from physical storage.
///
/// ## When to Use
///
/// Use this trait when:
/// - structure must remain flexible
/// - cardinality is not known upfront
/// - storage must be abstract and composable
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: uses the unit type as a single field key,
///   meaning only one virtual field exists (no disambiguation needed).
pub trait VirtualDynField<Discriminant: DiscriminantTag = ()>: Default {
    /// Representation of absence.
    type None: Delimited;

    /// The logical element type stored in the virtual field.
    type Some: Delimited;

    /// A collection of `Some`.
    ///
    /// Must support buffering and indexed access.
    type Many: Buffer<Self::Some>;

    /// Opaque storage backing the virtual field.
    ///
    /// Encodes the actual data and must support tagged conversions
    /// to and from `None`, `Some`, and `Many`.
    type Repr: Delimited
        // Provides an initial empty representation,
        // allowing deferred population via mutation.
        + Default
        // Construct representation from absence
        + FromTag<Self::None, NoneTag>
        // Extract absence from representation
        + TryIntoTag<Self::None, NoneTag>
        // Construct representation from a single value
        + FromTag<Self::Some, SomeTag>
        // Extract a single value from representation
        + TryIntoTag<Self::Some, SomeTag>
        // Attempt to construct representation from many values
        + TryFromTag<Self::Many, ManyTag>
        // Convert representation into many values
        + IntoTag<Self::Many, ManyTag>;

    /// Returns the current representation of the virtual field.
    ///
    /// Interaction is performed via tagged conversions.
    fn access(&self) -> Self::Repr;

    /// Replaces the current representation.
    ///
    /// The provided value must satisfy representation invariants.
    fn mutate(&mut self, v: Self::Repr);

    /// Returns the current number of elements represented.
    ///
    /// Interpreted as:
    /// - `None` -> 0
    /// - `Some` -> 1
    /// - `Many` -> n
    fn len(&self) -> usize;

    /// Returns the minimum number of elements representable.
    fn min(&self) -> usize;

    /// Returns the maximum number of elements representable.
    fn max(&self) -> usize;
}

/// Trivial `None` conversion for unit.
impl FromTag<(), NoneTag> for () {
    fn from_tag(_t: ()) -> Self {}
}

/// Trivial `Some` conversion for unit.
impl FromTag<(), SomeTag> for () {
    fn from_tag(_t: ()) -> Self {}
}

/// Trivial `Many` conversion for unit (always succeeds).
impl TryFromTag<Vec<()>, ManyTag> for () {
    type Error = core::convert::Infallible;

    fn try_from_tag(_t: Vec<()>) -> Result<Self, Self::Error> {
        Ok(())
    }
}

/// Trivial extraction of `None` from unit.
impl TryIntoTag<(), NoneTag> for () {
    type Error = core::convert::Infallible;

    fn try_into_tag(self) -> Result<Self, Self::Error> {
        Ok(())
    }
}

/// Trivial extraction of `Some` from unit.
impl TryIntoTag<(), SomeTag> for () {
    type Error = core::convert::Infallible;

    fn try_into_tag(self) -> Result<Self, Self::Error> {
        Ok(())
    }
}

/// Converts unit into an empty `Many` representation.
impl IntoTag<Vec<()>, ManyTag> for () {
    fn into_tag(self) -> Vec<()> {
        Vec::<()>::new()
    }
}

/// No-op `VirtualDynField` implementation for unit.
///
/// Represents an allocation with no storage and zero capacity.
impl VirtualDynField for () {
    type None = ();
    type Some = ();
    type Many = Vec<()>;
    type Repr = ();

    fn access(&self) -> Self::Repr {
        ()
    }

    fn mutate(&mut self, _: Self::Repr) {}

    fn len(&self) -> usize {
        Zero::zero()
    }

    fn min(&self) -> usize {
        Zero::zero()
    }

    fn max(&self) -> usize {
        Zero::zero()
    }
}

// ===============================================================================
// ```````````````````````` VIRTUAL FIELD DEFAULT-HELPERS ````````````````````````
// ===============================================================================

/// A discriminant-keyed virtual field abstraction with statically
/// determined cardinality.
///
/// This trait models a field whose structure is **fully determined
/// at compile time**, rather than deferred.
///
/// ## Model
///
/// A `VirtualStaticField` behaves like a field in a logical record,
/// where:
///
/// - cardinality is fixed or constrained at compile time
/// - storage shape is predetermined
/// - no dynamic resizing or growth is expected
///
/// Similar to [`VirtualDynField`] (but not dynamically sized):
/// - the **implementor** defines the backing representation (`Repr`)
/// - the **caller** selects the interpretation (`None`, `Some`, `Many`)
///
/// ## Representation Semantics
///
/// The `Many` form is expected to have **array-like semantics**:
/// - fixed size
/// - no resizing or allocation
/// - capacity encoded directly in the type
///
/// This makes `VirtualStaticField` suitable for:
/// - compile-time enforced layouts
/// - fixed schemas
/// - zero-overhead representations
///
/// ## Discriminant
///
/// The `Discriminant` acts as a type-level key, allowing multiple independent
/// fields to coexist without ambiguity.
///
/// ## Design
///
/// As with dynamic fields, responsibilities are separated:
///
/// - **Implementor (Storage Layer)**
///   - defines representation (`Repr`)
/// - **Caller (Shape Layer)**
///   - selects cardinality and interpretation
///
/// The key difference is that structure is **not deferred**, but
/// fully determined at compile time.
///
/// ## When to Use
///
/// Use this trait when:
/// - structure is known and fixed upfront
/// - dynamic resizing is unnecessary or undesirable
/// - compile-time guarantees are preferred over flexibility
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: uses the unit type as a single field key,
///   meaning only one virtual field exists (no disambiguation needed).
pub trait VirtualStaticField<Discriminant: DiscriminantTag = ()>: Default {
    /// Representation of absence.
    type None: Delimited;

    /// The logical element type stored in the virtual field.
    type Some: Delimited;

    /// A collection of `Some`.
    ///
    /// Must support indexed access and have a statically
    /// determined size (array-like semantics).
    type Many: Collection<Self::Some>;

    /// Opaque storage backing the virtual field.
    ///
    /// Encodes the field state and supports tagged conversions
    /// between `None`, `Some`, and `Many`.
    ///
    /// Unlike dynamic fields, this representation is expected to
    /// be fully determined at compile time and not rely on
    /// dynamic resizing or buffering.
    type Repr: Delimited
        // Provides an initial representation.
        + Default
        // Construct representation from absence
        + FromTag<Self::None, NoneTag>
        // Extract absence from representation
        + TryIntoTag<Self::None, NoneTag>
        // Construct representation from a single value
        + FromTag<Self::Some, SomeTag>
        // Extract a single value from representation
        + TryIntoTag<Self::Some, SomeTag>
        // Construct representation from many values
        + FromTag<Self::Many, ManyTag>
        // Extract many values from representation
        + TryIntoTag<Self::Many, ManyTag>;

    /// Returns the current representation of the virtual field.
    ///
    /// Interaction is performed via tagged conversions.
    fn access(&self) -> Self::Repr;

    /// Replaces the current representation.
    ///
    /// The provided value must satisfy representation invariants.
    fn mutate(&mut self, v: Self::Repr);
}

/// Trivial `Many` conversion for unit using a zero-sized array.
impl FromTag<[(); 0], ManyTag> for () {
    fn from_tag(_t: [(); 0]) -> Self {}
}

/// Trivial extraction of `Many` as a zero-sized array from unit.
impl TryIntoTag<[(); 0], ManyTag> for () {
    type Error = core::convert::Infallible;

    fn try_into_tag(self) -> Result<[(); 0], Self::Error> {
        Ok([(); 0])
    }
}

/// No-op `VirtualStaticField` implementation for unit.
///
/// Represents an allocation with no storage and zero capacity.
impl VirtualStaticField for () {
    type None = ();
    type Some = ();
    type Many = [(); 0];
    type Repr = ();

    fn access(&self) -> Self::Repr {
        ()
    }

    fn mutate(&mut self, _: Self::Repr) {}
}

/// Helper methods for accessing and mutating values in a [`VirtualDynField`].
///
/// These helpers operate on **dynamically shaped fields** with
/// **vector-like semantics**:
/// - collections may grow or shrink (within bounds)
/// - indexing and iteration are supported
/// - mutations may reallocate or fail due to bounds
///
/// All operations are performed via tagged conversions.
///
/// ## Default Discriminant
///
/// - `K = ()`: operates on a single default field,
///   meaning one virtual field is assumed.
pub trait DynFieldHelpers<K = ()>: VirtualDynField<K>
where
    K: DiscriminantTag,
{
    /// Returns the element at `index` from the field interpreted as `Many`.
    ///
    /// ## Behavior
    /// - Returns `Some(V)` if the field contains a collection and `index` is in bounds
    /// - Returns `None` if the field is `None`, `Some`, or out of bounds
    fn index_get(&self, index: usize) -> Option<Self::Some>
    where
        Self::Some: Clone,
    {
        <Self as VirtualDynField<K>>::access(self)
            .into_tag()
            .as_ref()
            .get(index)
            .cloned()
    }

    /// Returns an iterator over elements in the field interpreted as `Many`.
    ///
    /// If the field is not `Many`, the iterator will be empty.
    fn iter(&self) -> impl Iterator<Item = Self::Some>
    where
        Self::Some: Clone,
    {
        <Self as VirtualDynField<K>>::access(self)
            .into_tag()
            .into_iter()
    }

    /// Applies a mutable operation to each element in the field interpreted as `Many`.
    ///
    /// After mutation, the updated collection is written back to the container.
    ///
    /// ## Errors
    /// - Returns `Err(())` if conversion back into the representation fails
    fn iter_mut<F>(&mut self, mut f: F) -> Result<(), ()>
    where
        F: FnMut(&mut Self::Some),
    {
        let mut vec = <Self as VirtualDynField<K>>::access(self).into_tag();

        let len = vec.as_ref().len();
        for i in 0..len {
            f(&mut vec[i]);
        }

        let repr = TryFromTag::try_from_tag(vec).map_err(|_| ())?;
        <Self as VirtualDynField<K>>::mutate(self, repr);

        Ok(())
    }

    /// Sets the element at `index` in the field interpreted as `Many`.
    ///
    /// If the field is not currently a collection or is too short,
    /// it is extended with default values.
    ///
    /// ## Errors
    /// - Returns `Err(())` if conversion back fails
    fn index_set(&mut self, index: usize, value: Self::Some) -> Result<(), ()>
    where
        Self::Some: Default,
    {
        let mut v = <Self as VirtualDynField<K>>::access(self).into_tag();

        set_index(&mut v, index, value);

        let repr = TryFromTag::try_from_tag(v).map_err(|_| ())?;
        <Self as VirtualDynField<K>>::mutate(self, repr);

        Ok(())
    }

    /// Retrieves the value of the field interpreted as `Some`.
    ///
    /// ## Behavior
    /// - Returns `Some(V)` if the field contains a single value
    /// - Returns `None` otherwise
    fn get(&self) -> Option<Self::Some> {
        let repr = <Self as VirtualDynField<K>>::access(self);
        TryIntoTag::<_, SomeTag>::try_into_tag(repr).ok()
    }

    /// Sets the field to a single value (`Some`).
    ///
    /// Replaces any existing representation.
    fn set(&mut self, v: Self::Some)
    where
        Self::Some: Delimited,
    {
        <Self as VirtualDynField<K>>::mutate(self, v.into_tag());
    }
}

/// Blanket impl for all [`VirtualDynField`] types.
///
/// This trait is not intended to be implemented manually.
/// It exists as an ergonomic replacement for free helper functions.
///
/// All methods have default implementations, making this
/// forward-compatible: new helpers can be added without
/// breaking existing code.
impl<T, K> DynFieldHelpers<K> for T
where
    T: VirtualDynField<K>,
    K: DiscriminantTag,
{
}

/// Helper methods for accessing and mutating values in a [`VirtualStaticField`].
///
/// These helpers operate on **statically shaped fields** with
/// **array-like semantics**:
/// - collection size is fixed at compile time
/// - no resizing or extension is performed
/// - operations replace or read the entire structure
///
/// All operations are performed via tagged conversions.
///
/// ## Default Discriminant
///
/// - `K = ()`: operates on a single default field,
///   meaning one virtual static field is assumed.
pub trait StaticFieldHelpers<K = ()>: VirtualStaticField<K>
where
    K: DiscriminantTag,
{
    /// Retrieves the full collection from the field interpreted as `Many`.
    ///
    /// ## Behavior
    /// - Returns `Some(V)` if the field is in `Many` form
    /// - Returns `None` if the field is `None` or `Some`
    fn get_all(&self) -> Option<Self::Many> {
        let repr = <Self as VirtualStaticField<K>>::access(self);
        TryIntoTag::<_, ManyTag>::try_into_tag(repr).ok()
    }

    /// Sets the field to a collection (`Many`).
    ///
    /// ## Behavior
    /// - Replaces the entire field with the provided collection
    /// - No resizing or partial updates are performed
    fn set_all(&mut self, v: Self::Many)
    where
        Self::Many: Delimited,
    {
        <Self as VirtualStaticField<K>>::mutate(self, v.into_tag());
    }

    /// Retrieves the value of the field interpreted as `Some`.
    ///
    /// ## Behavior
    /// - Returns `Some(V)` if the field contains a single value
    /// - Returns `None` otherwise
    fn get(&self) -> Option<Self::Some> {
        let repr = <Self as VirtualStaticField<K>>::access(self);
        TryIntoTag::<_, SomeTag>::try_into_tag(repr).ok()
    }

    /// Sets the field to a single value (`Some`).
    ///
    /// Replaces any existing representation.
    fn set(&mut self, v: Self::Some)
    where
        Self::Some: Delimited,
    {
        <Self as VirtualStaticField<K>>::mutate(self, v.into_tag());
    }
}

/// Blanket impl for all [`VirtualStaticField`] types.
///
/// This trait is not intended to be implemented manually.
/// It exists as an ergonomic replacement for free helper functions.
///
/// All methods have default implementations, making this
/// forward-compatible: new helpers can be added without
/// breaking existing code.
impl<T, K> StaticFieldHelpers<K> for T
where
    T: VirtualStaticField<K>,
    K: DiscriminantTag,
{
}

// ===============================================================================
// ````````````````````````````` VIRTUAL FIELD-BOUNDS ````````````````````````````
// ===============================================================================

/// Provides the bounds associated with a [`VirtualDynField<Discriminant>`].
///
/// `VirtualDynBound` defines constraints (such as capacity limits)
/// without requiring the field itself to hardcode them.
///
/// ## Representation
///
/// The bound is provided as a type implementing [`Get<u32>`],
/// meaning the value is **resolved at runtime (or via type-level indirection)**.
///
/// This enables flexible, dynamically bounded behavior while still
/// enforcing limits.
///
/// ## Discriminant
///
/// The `Discriminant` links a field to its corresponding bound,
/// allowing multiple independent fields to coexist without ambiguity.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: provides a single default bound,
///   meaning only one dynamically bounded field is assumed.
pub trait VirtualDynBound<Discriminant: DiscriminantTag = ()> {
    type Bound: Get<u32> + Clone + Debug + 'static;
}

/// No-op bound with zero capacity.
impl VirtualDynBound for () {
    type Bound = ConstU32<0>;
}

/// Provides the bounds associated with a [`VirtualStaticField<Discriminant>`].
///
/// `VirtualStaticBound` defines constraints (such as capacity limits)
/// that are **fully determined at compile time**.
///
/// ## Representation
///
/// The bound is provided as a `const`, meaning:
/// - it is a **compile-time constant**
/// - no runtime resolution or indirection is involved
///
/// This enables fully static, zero-overhead representations.
///
/// ## Discriminant
///
/// The `Discriminant` links a field to its corresponding bound,
/// allowing multiple independent fields to coexist without ambiguity.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: provides a single default bound,
///   meaning only one dynamically bounded field is assumed.
pub trait VirtualStaticBound<Discriminant: DiscriminantTag = ()> {
    const BOUND: usize;
}
/// No-op bound provided with zero capacity.
impl VirtualStaticBound for () {
    const BOUND: usize = 0;
}

// ===============================================================================
// ```````````````````````````````` VIRTUAL ERRORS ```````````````````````````````
// ===============================================================================

/// Defines the error type associated with a virtual component
/// identified by a `Discriminant`.
///
/// `VirtualError` provides a way to associate a specific error type
/// with a [`VirtualDynField`] or related abstraction, without hardcoding
/// the error into the implementation.
///
/// ## Discriminant
///
/// The `Discriminant` acts as a key linking a virtual field (or related
/// abstraction) to its corresponding error type.
///
/// This ensures multiple independent virtual components can coexist
/// without ambiguity.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: associates a single default error type,
///   meaning only one virtual component is assumed.
pub trait VirtualError<Discriminant: DiscriminantTag = ()> {
    type Error: RuntimeError;
}

// ===============================================================================
// ``````````````````````````` DELEGATED VIRTUAL BOUNDS ``````````````````````````
// ===============================================================================

/// Delegates bound resolution to an external [`VirtualDynBound`] provider.
///
/// This trait is used when a type participates in a [`VirtualDynField`]
/// but does not define or own its bounds.
///
/// Instead, bounds are supplied externally via `Provider`,
/// allowing constraints (such as capacity) to be defined independently
/// of the field or its storage.
///
/// ## Representation
///
/// The delegated bound is a **runtime-resolved value** (via [`Get<u32>`]),
/// enabling flexible, dynamically bounded behavior.
///
/// ## Roles
///
/// - **Container (`Self`)**
///   - provides storage for the field
///
/// - **Bounds (`Provider`)**
///   - supplies constraints for that field
///
/// This separation enables:
/// - composability
/// - reuse across contexts
/// - decoupling of storage and constraints
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: delegates a single default bound,
///   meaning one dynamically bounded field is assumed.
pub trait DelegateVirtualDynBound<Provider, Discriminant = ()>
where
    Provider: VirtualDynBound<Discriminant>,
    Discriminant: DiscriminantTag,
    Self: Sized,
{
}

/// Blanket implementation enabling all types to delegate
/// dynamic bound resolution to a [`VirtualDynBound`] provider.
impl<Provider, Discriminant, T> DelegateVirtualDynBound<Provider, Discriminant> for T
where
    Provider: VirtualDynBound<Discriminant>,
    Discriminant: DiscriminantTag,
    T: Sized,
{
}

/// Delegates bound resolution to an external [`VirtualStaticBound`] provider.
///
/// This trait is used when a type participates in a [`VirtualStaticField`]
/// but does not define or own its bounds.
///
/// Instead, bounds are supplied externally via `Provider`,
/// allowing constraints (such as capacity) to be defined independently
/// of the field or its storage.
///
/// ## Representation
///
/// The delegated bound is a **compile-time constant** (`usize`),
/// enabling fully static, zero-overhead representations.
///
/// ## Roles
///
/// - **Container (`Self`)**
///   - provides storage for the field
///
/// - **Bounds (`Provider`)**
///   - supplies compile-time constraints for that field
///
/// This separation enables:
/// - composability
/// - reuse across contexts
/// - decoupling of storage and constraints
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: delegates a single default bound,
///   meaning one statically bounded field is assumed.
pub trait DelegateVirtualStaticBound<Provider, Discriminant = ()>
where
    Provider: VirtualStaticBound<Discriminant>,
    Discriminant: DiscriminantTag,
    Self: Sized,
{
}

/// Blanket implementation enabling all types to delegate
/// static bound resolution to a [`VirtualStaticBound`] provider.
impl<Provider, Discriminant, T> DelegateVirtualStaticBound<Provider, Discriminant> for T
where
    Provider: VirtualStaticBound<Discriminant>,
    Discriminant: DiscriminantTag,
    T: Sized,
{
}

// ===============================================================================
// ````````````````````` DELEGATED VIRTUAL FIELDS AND BOUNDS `````````````````````
// ===============================================================================

/// Constraint describing a virtual field whose bounds are delegated
/// to an external [`VirtualDynBound`] provider.
///
/// This composes:
/// - [`VirtualDynField`] - defines storage and representation
/// - [`DelegateVirtualDynBound`] - supplies bounds externally
///
/// ## Semantics
///
/// - **Container (`Self`)**
///   - provides storage for values of type `T`
///
/// - **Bounds (`Provider`)**
///   - supplies capacity constraints
///
/// - **Caller**
///   - selects field shape (`None`, `Some`, `Many`)
///
/// ## Representation
///
/// Bounds are resolved dynamically:
/// - provided via [`Get<u32>`]
/// - enable flexible, vector-like behavior within limits
///
/// This allows storage and constraints to remain decoupled.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: applies a single default bounded field,
///   meaning one dynamically bounded field is assumed.
pub trait VirtualDynFieldWithDelegatedBounds<T, Provider, Discriminant = ()>:
    VirtualDynField<Discriminant, Some = T> + DelegateVirtualDynBound<Provider, Discriminant>
where
    Provider: VirtualDynBound<Discriminant>,
    Discriminant: DiscriminantTag,
{
}

/// Blanket implementation for any compatible container.
impl<T, Provider, Discriminant, U> VirtualDynFieldWithDelegatedBounds<T, Provider, Discriminant>
    for U
where
    U: VirtualDynField<Discriminant, Some = T> + DelegateVirtualDynBound<Provider, Discriminant>,
    Provider: VirtualDynBound<Discriminant>,
    Discriminant: DiscriminantTag,
{
}

/// Constraint describing a virtual field whose bounds are delegated
/// to an external [`VirtualStaticBound`] provider.
///
/// This composes:
/// - [`VirtualStaticField`] - defines storage and representation
/// - [`DelegateVirtualStaticBound`] - supplies bounds externally
///
/// ## Semantics
///
/// - **Container (`Self`)**
///   - provides storage for values of type `T`
///
/// - **Bounds (`Provider`)**
///   - supplies compile-time capacity constraints
///
/// - **Caller**
///   - selects field shape (`None`, `Some`, `Many`)
///
/// ## Representation
///
/// Bounds are compile-time constants:
/// - encoded directly in the type system (`usize`)
/// - enable fixed-size, array-like behavior
///
/// This ensures fully static, zero-overhead representations.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: applies a single default bounded field,
///   meaning one statically bounded field is assumed.
pub trait VirtualStaticFieldWithDelegatedBounds<T, Provider, Discriminant = ()>:
    VirtualStaticField<Discriminant, Some = T> + DelegateVirtualStaticBound<Provider, Discriminant>
where
    Provider: VirtualStaticBound<Discriminant>,
    Discriminant: DiscriminantTag,
{
}

/// Blanket implementation for any compatible container.
impl<T, Provider, Discriminant, U> VirtualStaticFieldWithDelegatedBounds<T, Provider, Discriminant>
    for U
where
    U: VirtualStaticField<Discriminant, Some = T>
        + DelegateVirtualStaticBound<Provider, Discriminant>,
    Provider: VirtualStaticBound<Discriminant>,
    Discriminant: DiscriminantTag,
{
}

// ===============================================================================
// `````````````````````````````` VIRTUAL EXTENSIONS `````````````````````````````
// ===============================================================================

/// Defines the schema for a [`VirtualDynExtension`] identified by a `Discriminant`.
///
/// A `VirtualDynExtensionSchema` describes the **structure and representation**
/// of an extension field whose type is supplied externally.
///
/// Unlike [`VirtualDynField`], the element type and layout are not defined
/// by the container, but provided through this schema.
///
/// ## Context
///
/// In the virtual system:
/// - [`VirtualDynField`] defines fields with internally known types
/// - `VirtualDynExtensionSchema` defines fields with externally supplied types
/// - [`VirtualDynExtension`] stores values using this schema
///
/// This allows containers to support fields whose types are:
/// - not known at implementation time
/// - injected via type-level composition
///
/// ## Representation
///
/// The `Many` form is expected to have **vector-like semantics**:
/// - dynamically sized (within bounds)
/// - supports buffering and indexed access
///
/// The schema itself is purely type-level:
/// - it does not store data
/// - it defines how data is represented and interpreted
///
/// ## Discriminant
///
/// The `Discriminant` links:
/// - the extension storage
/// - to its schema
///
/// allowing multiple independent extensions to coexist safely.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default extension schema,
///   meaning only one extension is assumed.
pub trait VirtualDynExtensionSchema<Discriminant: DiscriminantTag = ()> {
    /// Representation of absence.
    type None: Delimited;

    /// The logical element type defined by the schema.
    ///
    /// This type is externally supplied.
    type Some: Delimited;

    /// A collection of `Some` with vector-like semantics.
    type Many: Buffer<Self::Some> + Indexable<Self::Some>;

    /// Opaque representation of the extension.
    ///
    /// Encodes `None`, `Some`, or `Many` and supports tagged conversions.
    type Repr: Delimited
        // Initial empty representation
        + Default
        // Construct from absence
        + FromTag<Self::None, NoneTag>
        // Extract absence
        + TryIntoTag<Self::None, NoneTag>
        // Construct from single value
        + FromTag<Self::Some, SomeTag>
        // Extract single value
        + TryIntoTag<Self::Some, SomeTag>
        // Construct from many values (may fail due to bounds)
        + TryFromTag<Self::Many, ManyTag>
        // Convert into many values
        + IntoTag<Self::Many, ManyTag>;

    /// Returns the number of elements encoded in the representation.
    fn len(v: &Self::Repr) -> usize;

    /// Returns the minimum number of elements representable.
    fn min(v: &Self::Repr) -> usize;

    /// Returns the maximum number of elements representable.
    fn max(v: &Self::Repr) -> usize;
}

/// Defines the schema for a [`VirtualStaticExtension`] identified by a `Discriminant`.
///
/// A `VirtualStaticExtensionSchema` describes the **structure and representation**
/// of an extension field whose type is supplied externally and fully
/// determined at compile time.
///
/// ## Context
///
/// In the static field model:
/// - [`VirtualStaticField`] defines fields with fixed structure
/// - `VirtualStaticExtensionSchema` defines externally supplied types and layout
/// - [`VirtualStaticExtension`] stores values using this schema
///
/// This allows containers to support externally defined fields
/// with compile-time determined structure.
///
/// ## Representation
///
/// The `Many` form is expected to have **array-like semantics**:
/// - fixed size
/// - no dynamic resizing or allocation
/// - capacity encoded in the type
///
/// The schema is purely type-level:
/// - it does not store data
/// - it defines how data is represented and interpreted
///
/// ## Discriminant
///
/// The `Discriminant` links:
/// - the extension storage
/// - to its schema
///
/// allowing multiple independent extensions to coexist safely.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default extension schema,
///   meaning only one static extension is assumed.
pub trait VirtualStaticExtensionSchema<Discriminant: DiscriminantTag = ()> {
    /// Representation of absence.
    type None: Delimited;

    /// The logical element type defined by the schema.
    ///
    /// This type is externally supplied.
    type Some: Delimited;

    /// A fixed-size collection of `Some` with array-like semantics.
    type Many: Collection<Self::Some>;

    /// Opaque representation of the extension.
    ///
    /// Encodes `None`, `Some`, or `Many` and supports tagged conversions.
    ///
    /// All conversions are expected to be total (non-fallible),
    /// as structure is fully determined at compile time.
    type Repr: Delimited
        // Initial representation
        + Default
        // Construct from absence
        + FromTag<Self::None, NoneTag>
        // Extract absence
        + TryIntoTag<Self::None, NoneTag>
        // Construct from single value
        + FromTag<Self::Some, SomeTag>
        // Extract single value
        + TryIntoTag<Self::Some, SomeTag>
        // Construct from many values (total)
        + FromTag<Self::Many, ManyTag>
        // Extract many values (total)
        + TryIntoTag<Self::Many, ManyTag>;
}

/// Allocation interface for virtual extensions whose type and schema
/// are defined externally.
///
/// This is a second-order abstraction over [`VirtualDynField`]:
/// instead of defining its own element type, the field delegates both
/// type and representation to an external schema.
///
/// ## Context
///
/// In the virtual system:
/// - [`VirtualDynField`] defines fields with internally known types
/// - [`VirtualDynExtensionSchema`] defines externally supplied types and layout
/// - `VirtualDynExtension` stores values using that schema
///
/// This allows a container (`Self`) to host fields whose types are:
/// - not known at implementation time
/// - supplied later via type-level composition
///
/// ## Representation
///
/// All operations are performed on the schema-defined representation:
/// - `TypesVia` defines `None`, `Some`, `Many`, and `Repr`
/// - `Many` is expected to have **vector-like semantics**
/// - size and bounds are resolved dynamically (within constraints)
///
/// ## Semantics
///
/// - **Container (`Self`)**
///   - owns storage
///
/// - **Schema (`TypesVia`)**
///   - defines element type and representation
///
/// This enables fields to remain fully generic over externally
/// defined and deferred types.
///
/// ## Key Property
///
/// Type and structure are **not fixed in the container**, but injected
/// externally and resolved at compile time.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default extension,
///   meaning only one dynamic extension is assumed.
pub trait VirtualDynExtension<Discriminant: DiscriminantTag = ()>: Default {
    /// External schema defining the extension.
    type TypesVia: VirtualDynExtensionSchema<Discriminant>;

    /// Returns the underlying representation.
    fn access(&self) -> <Self::TypesVia as VirtualDynExtensionSchema<Discriminant>>::Repr;

    /// Replaces the underlying representation.
    fn mutate(&mut self, v: <Self::TypesVia as VirtualDynExtensionSchema<Discriminant>>::Repr);

    /// Returns the current number of elements.
    fn len(&self) -> usize {
        <Self::TypesVia as VirtualDynExtensionSchema<Discriminant>>::len(&Self::access(&self))
    }

    /// Returns the minimum number of elements representable.
    fn min(&self) -> usize {
        <Self::TypesVia as VirtualDynExtensionSchema<Discriminant>>::min(&Self::access(&self))
    }

    /// Returns the maximum number of elements representable.
    fn max(&self) -> usize {
        <Self::TypesVia as VirtualDynExtensionSchema<Discriminant>>::max(&Self::access(&self))
    }
}

/// Allocation interface for virtual extensions whose type and schema
/// are defined externally and fully determined at compile time.
///
/// This is the static counterpart to [`VirtualDynExtension`], where
/// both type and structure are fixed via the schema.
///
/// ## Context
///
/// In the static field model:
/// - [`VirtualStaticField`] defines fields with fixed structure
/// - [`VirtualStaticExtensionSchema`] defines externally supplied types
///   and compile-time layout
/// - `VirtualStaticExtension` stores values using that schema
///
/// This allows a container (`Self`) to host externally defined fields
/// with statically determined structure.
///
/// ## Representation
///
/// All operations are performed on the schema-defined representation:
/// - `TypesVia` defines `None`, `Some`, `Many`, and `Repr`
/// - `Many` is expected to have **array-like semantics**
/// - size and capacity are encoded at compile time
///
/// ## Semantics
///
/// - **Container (`Self`)**
///   - owns storage
///
/// - **Schema (`TypesVia`)**
///   - defines element type and representation
///
/// ## Key Property
///
/// Both type and structure are **fully determined at compile time**,
/// enabling zero-overhead representations without dynamic checks.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default extension,
///   meaning only one static extension is assumed.
pub trait VirtualStaticExtension<Discriminant: DiscriminantTag = ()>: Default {
    /// External schema defining the extension.
    type TypesVia: VirtualStaticExtensionSchema<Discriminant>;

    /// Returns the underlying representation.
    fn access(&self) -> <Self::TypesVia as VirtualStaticExtensionSchema<Discriminant>>::Repr;

    /// Replaces the underlying representation.
    fn mutate(&mut self, v: <Self::TypesVia as VirtualStaticExtensionSchema<Discriminant>>::Repr);
}

// ===============================================================================
// `````````````````````` VIRTUAL EXTENSION DEFAULT-HELPERS ``````````````````````
// ===============================================================================

/// Helper methods for accessing and mutating values in a [`VirtualDynExtension`].
///
/// These helpers operate on **dynamically shaped extensions** with
/// **vector-like semantics**:
/// - collections may grow or shrink (within bounds)
/// - indexing and iteration are supported
/// - mutations may fail due to constraints
///
/// Structure and types are defined externally via [`VirtualDynExtensionSchema`],
/// while storage is handled by the container.
///
/// All operations are performed via tagged conversions.
///
/// ## Default Discriminant
///
/// - `K = ()`: operates on a single default extension,
///   meaning one dynamic extension is assumed.
pub trait DynExtHelpers<K = ()>: VirtualDynExtension<K>
where
    K: DiscriminantTag,
{
    /// Returns the element at `index` from the extension interpreted as `Many`.
    fn index_get(
        &self,
        index: usize,
    ) -> Option<<Self::TypesVia as VirtualDynExtensionSchema<K>>::Some>
    where
        <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some: Clone,
    {
        <Self as VirtualDynExtension<K>>::access(self)
            .into_tag()
            .as_ref()
            .get(index)
            .cloned()
    }

    /// Returns an iterator over elements.
    fn iter(&self) -> impl Iterator<Item = <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some>
    where
        <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some: Clone,
    {
        <Self as VirtualDynExtension<K>>::access(self)
            .into_tag()
            .into_iter()
    }

    /// Applies mutation over all elements.
    fn iter_mut<F>(&mut self, mut f: F) -> Result<(), ()>
    where
        F: FnMut(&mut <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some),
    {
        let mut repr = <Self as VirtualDynExtension<K>>::access(self).into_tag();

        let len = repr.as_ref().len();
        for i in 0..len {
            f(&mut repr[i]);
        }

        let repr = TryFromTag::try_from_tag(repr).map_err(|_| ())?;
        <Self as VirtualDynExtension<K>>::mutate(self, repr);

        Ok(())
    }

    /// Sets element at index.
    fn index_set(
        &mut self,
        index: usize,
        v: <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some,
    ) -> Result<(), ()>
    where
        <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some: Default,
    {
        let mut repr = <Self as VirtualDynExtension<K>>::access(self).into_tag();

        set_index(&mut repr, index, v);

        let repr = TryFromTag::try_from_tag(repr).map_err(|_| ())?;
        <Self as VirtualDynExtension<K>>::mutate(self, repr);

        Ok(())
    }

    /// Retrieves `Some`.
    fn get(&self) -> Option<<Self::TypesVia as VirtualDynExtensionSchema<K>>::Some> {
        let repr = <Self as VirtualDynExtension<K>>::access(self);
        TryIntoTag::<_, SomeTag>::try_into_tag(repr).ok()
    }

    /// Sets `Some`.
    fn set(&mut self, v: <Self::TypesVia as VirtualDynExtensionSchema<K>>::Some) {
        <Self as VirtualDynExtension<K>>::mutate(self, v.into_tag());
    }
}

/// Blanket impl for all [`VirtualDynExtension`] types.
///
/// This trait is not intended to be implemented manually.
/// It exists as an ergonomic replacement for free helper functions.
///
/// All methods have default implementations, making this
/// forward-compatible: new helpers can be added without
/// breaking existing code.
impl<T, K> DynExtHelpers<K> for T
where
    T: VirtualDynExtension<K>,
    K: DiscriminantTag,
{
}

/// Helper methods for accessing and mutating values in a [`VirtualStaticExtension`].
///
/// These helpers operate on **statically shaped extensions** with
/// **array-like semantics**:
/// - collection size is fixed at compile time
/// - no resizing or extension is performed
/// - operations act on the entire structure
///
/// Structure and types are defined externally via [`VirtualStaticExtensionSchema`],
/// while storage is handled by the container.
///
/// All operations are performed via tagged conversions.
///
/// ## Default Discriminant
///
/// - `K = ()`: operates on a single default extension,
///   meaning one static extension is assumed.
pub trait StaticExtHelpers<K = ()>: VirtualStaticExtension<K>
where
    K: DiscriminantTag,
{
    /// Retrieves the full collection (`Many`).
    fn get_all(&self) -> Option<<Self::TypesVia as VirtualStaticExtensionSchema<K>>::Many> {
        let repr = <Self as VirtualStaticExtension<K>>::access(self);
        TryIntoTag::<_, ManyTag>::try_into_tag(repr).ok()
    }

    /// Sets full collection.
    fn set_all(&mut self, v: <Self::TypesVia as VirtualStaticExtensionSchema<K>>::Many)
    where
        <Self::TypesVia as VirtualStaticExtensionSchema<K>>::Many: Delimited,
    {
        <Self as VirtualStaticExtension<K>>::mutate(self, v.into_tag());
    }

    /// Retrieves `Some`.
    fn get(&self) -> Option<<Self::TypesVia as VirtualStaticExtensionSchema<K>>::Some> {
        let repr = <Self as VirtualStaticExtension<K>>::access(self);
        TryIntoTag::<_, SomeTag>::try_into_tag(repr).ok()
    }

    /// Sets `Some`.
    fn set(&mut self, v: <Self::TypesVia as VirtualStaticExtensionSchema<K>>::Some) {
        <Self as VirtualStaticExtension<K>>::mutate(self, v.into_tag());
    }
}

/// Blanket impl for all [`VirtualStaticExtension`] types.
///
/// This trait is not intended to be implemented manually.
/// It exists as an ergonomic replacement for free helper functions.
///
/// All methods have default implementations, making this
/// forward-compatible: new helpers can be added without
/// breaking existing code.
impl<T, K> StaticExtHelpers<K> for T
where
    T: VirtualStaticExtension<K>,
    K: DiscriminantTag,
{
}

/// Sets the value at a given index in a collection.
///
/// If the index is out of bounds, the collection is automatically extended
/// with default values (`T::default()`) up to the required index.
///
/// ## Behavior
/// - If `index < current length`, the value is simply updated.
/// - If `index >= current length`, the collection is resized by filling
///   missing positions with default values, then the value is set.
///
/// ## Type Parameters
/// - `C`: A collection that supports indexing and extension.
/// - `T`: The element type, which must implement `Default`.
///
/// ## Example
/// If the collection has length 3 and you set index 5:
/// - Elements at index 3 and 4 will be filled with `T::default()`.
/// - Index 5 will be assigned the given `value`.
fn set_index<C, T>(v: &mut C, index: usize, value: T)
where
    C: Indexable<T> + Growable<T>,
    T: Default,
{
    let len = v.as_ref().len();
    if index >= len {
        v.extend(core::iter::repeat_with(T::default).take(index + 1 - len));
    }
    v[index] = value;
}

/// Implements an empty virtual extension schema for a given extension.
///
/// This macro defines a no-op schema where the extension has **no storage**
/// and behaves as absent.
///
/// It supports two modes:
///
/// - **Dynamic (default)** -> implements [`VirtualDynExtensionSchema`]
///   - uses vector-like semantics (`Vec<()>`)
///   - size-related operations (`len`, `min`, `max`) return `0`
///
/// - **Static (`static` keyword)** -> implements [`VirtualStaticExtensionSchema`]
///   - uses array-like semantics (`[(); 0]`)
///   - fully determined at compile time
///   - no size-related operations are required
///
/// ## Semantics
///
/// In both modes:
/// - `None`, `Some`, and `Repr` are represented as `()`
/// - no data is stored
/// - the extension is effectively non-existent
///
/// This is useful when:
/// - a container participates in the extension system
/// - but a particular extension is unsupported or intentionally omitted
///
/// ## Syntax
///
/// ### Dynamic (default)
/// ```ignore
/// empty_virtual_extension!(
///     target: MyContainer,
///     tag: MyExtension,
///     schema: MySchema,
///     generics: [T, U],
///     bounds: [T: Clone, U: Default],
/// );
/// ```
///
/// ### Static
/// ```ignore
/// empty_virtual_extension!(
///     target: MyContainer,
///     tag: MyExtension,
///     schema: static MySchema,
/// );
/// ```
///
/// ## Parameters
///
/// - `target`: container type (conceptual owner of the extension)
/// - `tag`: discriminant identifying the extension
/// - `schema`: schema type to implement
/// - `generics` *(optional)*: generics for the impl
/// - `bounds` *(optional)*: additional `where` constraints
///
/// ## Behavior
///
/// - The extension is treated as non-existent
/// - Dynamic mode:
///   - uses `Vec<()>` (always empty)
///   - returns `0` for all size queries
/// - Static mode:
///   - uses `[(); 0]` (zero-length array)
///   - fully resolved at compile time
#[macro_export]
macro_rules! empty_virtual_extension {
    // STATIC VERSION
    (
        target: $target:ty,
        tag: $extension:ty,
        schema: static $schema:ty
        $(, generics: [$($gen:tt),* $(,)? ])?
        $(, bounds: [$($extra_bounds:tt)*])?
        $(,)?
    ) => {
        impl< $($($gen,)*)? >
            $crate::virtuals::VirtualStaticExtensionSchema<$extension>
            for $schema
        where
            $($($extra_bounds)*)?
        {
            type None = ();
            type Some = ();
            type Many = [(); 0];
            type Repr = ();
        }
    };

    // DYNAMIC VERSION (default)
    (
        target: $target:ty,
        tag: $extension:ty,
        schema: $schema:ty
        $(, generics: [$($gen:tt),* $(,)? ])?
        $(, bounds: [$($extra_bounds:tt)*])?
        $(,)?
    ) => {
        impl< $($($gen,)*)? >
            $crate::virtuals::VirtualDynExtensionSchema<$extension>
            for $schema
        where
            $($($extra_bounds)*)?
        {
            type None = ();
            type Some = ();
            type Many = Vec<()>;
            type Repr = ();

            fn len(_: &Self::Repr) -> usize {
                Zero::zero()
            }

            fn min(_: &Self::Repr) -> usize {
                Zero::zero()
            }

            fn max(_: &Self::Repr) -> usize {
                Zero::zero()
            }
        }
    };
}

// ===============================================================================
// ````````````````````````` DELEGATED VIRTUAL EXTENSIONS ````````````````````````
// ===============================================================================

/// Constraint for delegating a virtual extension to an external schema.
///
/// This trait ties a container to an externally provided
/// [`VirtualDynExtensionSchema`], without requiring the container
/// to define the extension's type or representation itself.
///
/// ## Roles
///
/// - **Container (`Self`)**
///   - stores the extension representation
///
/// - **Schema (`Provider`)**
///   - defines the element type and representation
///
/// - **Caller**
///   - selects the extension via the `Discriminant`
///
/// ## Representation
///
/// The delegated schema is dynamic:
/// - `Many` has vector-like semantics
/// - size and bounds are resolved at runtime (within limits)
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: delegates a single default extension,
///   meaning one dynamic extension is assumed.
pub trait DelegateVirtualDynExtension<Provider, Discriminant = ()>:
    VirtualDynExtension<Discriminant>
where
    Provider: VirtualDynExtensionSchema<Discriminant>,
    Discriminant: DiscriminantTag,
    Self: Sized,
{
}

/// Blanket implementation enabling all types to delegate
/// extension schema resolution.
impl<Provider, Discriminant, T> DelegateVirtualDynExtension<Provider, Discriminant> for T
where
    T: VirtualDynExtension<Discriminant> + Sized,
    Discriminant: DiscriminantTag,
    Provider: VirtualDynExtensionSchema<Discriminant>,
{
}

/// Constraint for delegating a virtual extension to an external
/// [`VirtualStaticExtensionSchema`].
///
/// This is the static counterpart to [`DelegateVirtualDynExtension`],
/// where the schema defines a fully determined, compile-time structure.
///
/// ## Roles
///
/// - **Container (`Self`)**
///   - stores the extension representation
///
/// - **Schema (`Provider`)**
///   - defines the element type and representation
///
/// - **Caller**
///   - selects the extension via the `Discriminant`
///
/// ## Representation
///
/// The delegated schema is static:
/// - `Many` has array-like semantics
/// - size and capacity are fixed at compile time
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: delegates a single default extension,
///   meaning one static extension is assumed.
pub trait DelegateVirtualStaticExtension<Provider, Discriminant = ()>:
    VirtualStaticExtension<Discriminant>
where
    Provider: VirtualStaticExtensionSchema<Discriminant>,
    Discriminant: DiscriminantTag,
    Self: Sized,
{
}

/// Blanket implementation enabling all types to delegate
/// static extension schema resolution.
impl<Provider, Discriminant, T> DelegateVirtualStaticExtension<Provider, Discriminant> for T
where
    T: VirtualStaticExtension<Discriminant> + Sized,
    Discriminant: DiscriminantTag,
    Provider: VirtualStaticExtensionSchema<Discriminant>,
{
}

// ===============================================================================
// `````````````````````````````` VIRTUAL COLLECTOR ``````````````````````````````
// ===============================================================================

/// A virtual collector for values of type `T` under a discriminant.
///
/// A `VirtualCollector` represents a type that can:
/// - collect a value `T` into itself (via [`FromTag`])
/// - attempt to extract a value `T` back (via [`TryIntoTag`])
///
/// ## Virtual Field Context
///
/// In the virtual system:
/// - values may be interpreted differently depending on their role
/// - tagged conversions (`FromTag`, `TryIntoTag`) define those interpretations
/// - this trait groups types that support bidirectional interaction with `T`
///
/// ## Semantics
///
/// A `VirtualCollector` acts as a *tagged carrier* of `T`:
///
/// - `T -> Self`
///   - always succeeds (collection)
///
/// - `Self -> T`
///   - may fail depending on structure (extraction)
///
/// This is commonly implemented by enums where a specific variant
/// represents `T`.
///
/// ## Example Pattern
///
/// ```ignore
/// enum Value {
///     Number(u32),
///     Text(String),
/// }
///
/// // `Value` can act as a VirtualCollector<u32>
/// ```
///
/// ## Discriminant
///
/// The `Discriminant` ensures conversions remain unambiguous,
/// even when `T` or `Self` are generic or not fully concrete.
///
/// ## When to Use
///
/// Use this trait when:
/// - a type can *collect* values of `T`
/// - extraction may depend on internal structure
/// - tagged semantics are required for disambiguation
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default interpretation,
///   meaning one collection/extraction behavior is assumed.
pub trait VirtualCollector<T, Discriminant: DiscriminantTag = ()>:
    FromTag<T, Discriminant> + TryIntoTag<T, Discriminant>
{
}

/// Blanket implementation for all types supporting bidirectional tagged
/// conversion with `T`.
///
/// Any type that:
/// - can be constructed from `T` via [`FromTag`]
/// - can attempt to extract `T` via [`TryIntoTag`]
///
/// automatically implements [`VirtualCollector`].
///
/// This allows enums and similar container types to act as collectors
/// without requiring explicit implementations.
impl<T, Discriminant, U> VirtualCollector<T, Discriminant> for U
where
    U: FromTag<T, Discriminant> + TryIntoTag<T, Discriminant>,
    Discriminant: DiscriminantTag,
{
}

// ===============================================================================
// ````````````````````````````` VIRTUAL STORAGE MAPS ````````````````````````````
// ===============================================================================

/// A storage-backed virtual n-map owned by a container in the virtual structure system.
///
/// This trait defines a **map-like virtual component** that is logically owned
/// by a container (`For`), while its storage is delegated to an external
/// implementation (e.g. [`StorageNMap`]).
///
/// ## Virtual Structure Context
///
/// In the virtual system, a container (virtual struct) composes behavior through
/// independent, type-driven components:
///
/// - [`VirtualDynField`] / [`VirtualStaticField`] - field-level abstraction
/// over values and cardinality
/// - [`VirtualDynExtension`] / [`VirtualStaticExtension`] - externally defined
/// field schemas
/// - `VirtualNMap` - container-level map storage
///
/// These components are:
/// - **logically part of the container**
/// - but **not required to share a single physical representation**
///
/// ## Ownership and Delegation
///
/// The container (`For`) acts as the **owner** of the map:
/// - it defines the type context (e.g. key/value via associated types)
/// - it determines how the map is used
/// - but it does not store the map directly
///
/// Instead, storage is delegated to a native map implementation.
///
/// This separation allows:
/// - lightweight container representations
/// - efficient handling of large or frequently mutated data
/// - independent evolution of storage and structure
///
/// ## Type-Level Association
///
/// - `For`: the owning container (virtual struct)
/// - `Discriminant`: a type-level key identifying this map
///
/// This enables:
/// - multiple independent maps per container
/// - map definitions derived from container-level abstractions
/// - coherence-safe composition via distinct discriminants
///
/// ## Storage Model
///
/// - storage is provided by the implementor via [`StorageNMap`]
/// - keys are encoded using [`KeyGenerator`]
/// - iteration and prefix-based access are supported
///
/// The map is external in storage, but internal in ownership and usage.
///
/// ## When to Use
///
/// Use this trait when:
/// - a container logically owns map-like data
/// - key/value types depend on container-level abstractions
/// - data is large, dynamic, or frequently mutated
/// - embedding the map in a virtual field or representation is inefficient
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default map,
///   meaning one virtual map is assumed per container.
pub trait VirtualNMap<For: Delimited, Discriminant: DiscriminantTag = ()> {
    /// Key used to address entries in the map.
    ///
    /// Must:
    /// - match the input shape expected by `KeyGen::KArg`
    /// - support tuple-style encoding for multi-key storage
    /// - allow iteration over encoded components
    type Key: Delimited + EncodeLikeTuple<<Self::KeyGen as KeyGenerator>::KArg> + TupleToEncodedIter;

    /// Value stored in the map.
    ///
    /// Typically represents externally stored data associated with the container.
    type Value: Delimited;

    /// Defines how `Key` is transformed into storage keys.
    ///
    /// Supports:
    /// - forward encoding into storage
    /// - reverse decoding for iteration and prefix traversal
    type KeyGen: KeyGenerator + ReversibleKeyGenerator;

    /// Underlying storage map backing this abstraction.
    ///
    /// Must support:
    /// - basic CRUD operations
    /// - full iteration
    /// - prefix-based queries and draining
    type Map: StorageNMap<Self::KeyGen, Self::Value, Query = Self::Query>
        + IterableStorageNMap<Self::KeyGen, Self::Value, Query = Self::Query>
        + StoragePrefixedMap<Self::Value>;

    /// Return type for read operations.
    ///
    /// Encodes presence/absence semantics (e.g. `Option<Value>`).
    type Query;

    /// Fetch value associated with `key`.
    #[inline]
    fn get(key: Self::Key) -> Self::Query {
        Self::Map::get(key)
    }

    /// Insert or overwrite value at `key`.
    #[inline]
    fn insert(key: Self::Key, value: Self::Value) {
        Self::Map::insert(key, value)
    }

    /// Remove value at `key`.
    #[inline]
    fn remove(key: Self::Key) {
        Self::Map::remove(key)
    }

    /// Remove and return value at `key`.
    #[inline]
    fn take(key: Self::Key) -> Self::Query {
        Self::Map::take(key)
    }

    /// Check if `key` exists.
    #[inline]
    fn contains_key(key: Self::Key) -> bool {
        Self::Map::contains_key(key)
    }

    /// Mutate value at `key` in-place.
    ///
    /// Provides `Option<Value>` to handle both insert/update/remove cases.
    #[inline]
    fn mutate<R>(key: Self::Key, f: impl FnOnce(&mut Option<Self::Value>) -> R) -> R {
        Self::Map::mutate_exists(key, f)
    }

    /// Iterate over all `(full_key, value)` pairs.
    #[inline]
    fn iter() -> impl Iterator<Item = (<Self::KeyGen as KeyGenerator>::Key, Self::Value)> {
        Self::Map::iter()
    }

    /// Iterate over all full keys.
    #[inline]
    fn iter_keys() -> impl Iterator<Item = <Self::KeyGen as KeyGenerator>::Key> {
        Self::Map::iter_keys()
    }

    /// Iterate over all values.
    #[inline]
    fn iter_values() -> impl Iterator<Item = Self::Value> {
        Self::Map::iter_values()
    }

    /// Drain entire map, yielding all `(key, value)` pairs.
    #[inline]
    fn drain() -> impl Iterator<Item = (<Self::KeyGen as KeyGenerator>::Key, Self::Value)> {
        Self::Map::drain()
    }

    /// Count total number of entries.
    #[inline]
    fn count() -> usize {
        Self::Map::iter_keys().count()
    }

    /// Iterate over values matching a prefix.
    ///
    /// Prefix corresponds to a partial key (leading components).
    #[inline]
    fn iter_prefix_values<P>(prefix: P) -> impl Iterator<Item = Self::Value>
    where
        Self::KeyGen: HasKeyPrefix<P>,
    {
        Self::Map::iter_prefix_values(prefix)
    }

    /// Iterate over `(suffix, value)` under a prefix.
    ///
    /// `suffix` = remaining key components after the prefix.
    #[inline]
    fn iter_prefix<P>(
        prefix: P,
    ) -> impl Iterator<Item = (<Self::KeyGen as HasKeyPrefix<P>>::Suffix, Self::Value)>
    where
        Self::KeyGen: HasReversibleKeyPrefix<P>,
    {
        Self::Map::iter_prefix(prefix)
    }

    /// Iterate over suffix keys under a prefix.
    #[inline]
    fn iter_key_prefix<P>(
        prefix: P,
    ) -> impl Iterator<Item = <Self::KeyGen as HasKeyPrefix<P>>::Suffix>
    where
        Self::KeyGen: HasReversibleKeyPrefix<P>,
    {
        Self::Map::iter_key_prefix(prefix)
    }

    /// Drain entries under a prefix, yielding `(suffix, value)`.
    #[inline]
    fn drain_prefix<P>(
        prefix: P,
    ) -> impl Iterator<Item = (<Self::KeyGen as HasKeyPrefix<P>>::Suffix, Self::Value)>
    where
        Self::KeyGen: HasReversibleKeyPrefix<P>,
    {
        Self::Map::drain_prefix(prefix)
    }
}

/// A storage-backed virtual map owned by a container in the virtual structure system.
///
/// This trait defines a **map-like virtual component** that is logically owned
/// by a container (`For`), while its storage is delegated to an external
/// implementation (e.g. [`StorageMap`]).
///
/// ## Virtual Structure Context
///
/// In the virtual system, a container (virtual struct) composes behavior through
/// independent, type-driven components:
///
/// - [`VirtualDynField`] / [`VirtualStaticField`] - field-level abstraction
/// over values and cardinality
/// - [`VirtualDynExtension`] / [`VirtualStaticExtension`] - externally defined
/// field schemas
/// - `VirtualMap` - container-level map storage
///
/// These components are logically part of the container, but are not required
/// to share a single physical representation.
///
/// ## Ownership and Delegation
///
/// The container (`For`) acts as the **owner** of the map:
/// - it provides the type context for the map (key/value via associated types)
/// - it determines how the map is used
/// - but it does not store the map directly
///
/// Instead, storage is delegated to a native map implementation.
///
/// This allows:
/// - avoiding encode/decode overhead from embedding maps in representations
/// - efficient handling of large or frequently mutated data
/// - separation of structure (types) from storage (runtime)
///
/// ## Type-Level Association
///
/// - `For`: the owning container (virtual struct)
/// - `Discriminant`: a type-level key identifying this map
///
/// This enables:
/// - multiple independent maps per container
/// - map definitions derived from container-level abstractions
/// - coherence-safe composition via distinct discriminants
///
/// ## Storage Model
///
/// - storage is provided by the implementor via [`StorageMap`]
/// - keys are encoded via [`KeyGenerator`]
/// - iteration and full traversal are supported
///
/// The map is external in storage, but internal in ownership and usage.
///
/// ## When to Use
///
/// Use this trait when:
/// - a container logically owns map-like data
/// - key/value types depend on container-level abstractions
/// - data is large, dynamic, or frequently mutated
/// - embedding the map in a virtual field or representation is inefficient
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default map,
///   meaning one virtual map is assumed per container.
pub trait VirtualMap<For: Delimited, Discriminant: DiscriminantTag = ()> {
    /// Key used to address entries in the map.
    ///
    /// Must:
    /// - be encodable in the same form as `KeyGen`
    /// - support iteration over encoded components (for uniform handling)
    type Key: Delimited + EncodeLike<Self::KeyGen> + TupleToEncodedIter;

    /// Value stored in the map.
    ///
    /// Represents externally stored data associated with the container.
    type Value: Delimited;

    /// Defines how keys are encoded into storage and decoded back.
    ///
    /// Supports:
    /// - forward encoding into storage keys
    /// - reverse decoding during iteration
    type KeyGen: KeyGenerator + ReversibleKeyGenerator + EncodeLike;

    /// Underlying storage map backing this abstraction.
    ///
    /// Must support:
    /// - basic CRUD operations
    /// - full iteration over keys and values
    /// - prefixed storage layout
    type Map: StorageMap<Self::KeyGen, Self::Value, Query = Self::Query>
        + IterableStorageMap<Self::KeyGen, Self::Value, Query = Self::Query>
        + StoragePrefixedMap<Self::Value>;

    /// Return type for read operations.
    ///
    /// Typically `Option<Value>` or a query wrapper.
    type Query;

    /// Fetch value associated with `key`.
    #[inline]
    fn get(key: Self::Key) -> Self::Query {
        Self::Map::get(key)
    }

    /// Insert or overwrite value at `key`.
    #[inline]
    fn insert(key: Self::Key, value: Self::Value) {
        Self::Map::insert(key, value)
    }

    /// Remove value at `key`.
    #[inline]
    fn remove(key: Self::Key) {
        Self::Map::remove(key)
    }

    /// Remove and return value at `key`.
    #[inline]
    fn take(key: Self::Key) -> Self::Query {
        Self::Map::take(key)
    }

    /// Check if `key` exists in the map.
    #[inline]
    fn contains_key(key: Self::Key) -> bool {
        Self::Map::contains_key(key)
    }

    /// Mutate value at `key` in-place.
    ///
    /// Provides `Option<Value>` to handle insert/update/remove semantics.
    #[inline]
    fn mutate<R>(key: Self::Key, f: impl FnOnce(&mut Option<Self::Value>) -> R) -> R {
        Self::Map::mutate_exists(key, f)
    }

    /// Iterate over all `(key, value)` pairs.
    #[inline]
    fn iter() -> impl Iterator<Item = (Self::KeyGen, Self::Value)> {
        Self::Map::iter()
    }

    /// Iterate over all keys.
    #[inline]
    fn iter_keys() -> impl Iterator<Item = Self::KeyGen> {
        Self::Map::iter_keys()
    }

    /// Iterate over all values.
    #[inline]
    fn iter_values() -> impl Iterator<Item = Self::Value> {
        Self::Map::iter_values()
    }

    /// Drain entire map, yielding all `(key, value)` pairs.
    #[inline]
    fn drain() -> impl Iterator<Item = (Self::KeyGen, Self::Value)> {
        Self::Map::drain()
    }

    /// Count total number of entries.
    #[inline]
    fn count() -> usize {
        Self::Map::iter_keys().count()
    }
}
