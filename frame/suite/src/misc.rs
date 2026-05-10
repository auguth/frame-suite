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
// ```````````````````````````````` MISCELLANEOUS ````````````````````````````````
// ===============================================================================

//! Provides small, reusable building blocks that are broadly applicable
//! across pallets and runtime components.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{Probe, virtuals::DiscriminantTag};

// --- Core ---
use core::{fmt::Debug, marker::PhantomData};

// --- SCALE & metadata ---
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

// --- Frame Support ---
use frame_support::traits::{
    tokens::{Fortitude, Precision},
    VariantCount,
};

// --- Substrate Primitives ---
use sp_runtime::{traits::Zero, Vec};
use sp_core::blake2_256;

// ===============================================================================
// ```````````````````````````````````` TRAITS ```````````````````````````````````
// ===============================================================================

/// A trait for deriving values across different extents within a bounded space.
///
/// This trait is domain-agnostic and provides a unified interface to compute
/// values at the lower bound, upper bound, and an optimal point under given conditions.
///
/// ## Methods:
/// - `minimum`: Returns the lowest permissible or derived value.
/// - `maximum`: Returns the highest permissible or derived value.
/// - `optimal`: Returns the most suitable or preferred value within bounds.
///
/// ## Usage Scenarios:
/// - **Constraint systems**: Determining valid lower and upper limits.
/// - **Optimization problems**: Selecting a balanced or ideal value.
/// - **Rate limiting**: Bounding operations within safe thresholds.
/// - **Resource allocation**: Computing safe vs optimal utilization.
/// - **Financial systems**: Sizing deposits, mints, or adjustments.
///
/// ## Design Notes:
/// - Fully abstract and reusable across domains.
/// - Each method is independent and may be computed lazily.
/// - Implementations may choose to derive values from shared logic.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default extent,
///   meaning one unique set of bounds/derivations is assumed.
pub trait Extent<Discriminant: DiscriminantTag = ()> {
    /// The scalar representing the bounded value.
    type Scalar: PartialOrd + Copy;

    /// Returns the minimum value under given conditions.
    fn minimum(&self) -> Option<Self::Scalar>;

    /// Returns the maximum value under given conditions.
    fn maximum(&self) -> Option<Self::Scalar>;

    /// Returns the optimal value under given conditions.
    fn optimal(&self) -> Option<Self::Scalar>;

    /// Returns an unbounded extent.
    ///
    /// This represents the absence of any constraints, where:
    /// - [`Self::minimum`] returns `None`
    /// - [`Self::maximum`] returns `None`
    /// - [`Self::optimal`] returns `None`
    ///
    /// Useful as a neutral/default extent when no bounds are required.
    fn none() -> Self;

    /// Checks whether the given value lies within the extent's bounds.
    ///
    /// Returns `true` if:
    /// - The value is greater than or equal to [`Self::minimum`] (if available), and
    /// - The value is less than or equal to [`Self::maximum`] (if available).
    ///
    /// Missing bounds (`None`) are treated as unbounded in that direction.
    ///
    /// This method performs a simple range check and does not enforce
    /// optimality or other domain-specific constraints.
    fn contains(&self, value: Self::Scalar) -> bool {
        if let Some(min) = self.minimum() {
            if value < min {
                return false;
            }
        }

        if let Some(max) = self.maximum() {
            if value > max {
                return false;
            }
        }

        true
    }
}

/// A trait expressing the execution directive of an operation.
///
/// `Directive` defines *how an action should be carried out*, independent of
/// the value or state it operates on. It captures two orthogonal dimensions:
///
/// - [`Precision`]: The required exactness of the operation.
///   - `Exact`: The operation must satisfy strict conditions.
///   - `BestEffort`: The operation may proceed with relaxed or partial fulfillment.
///
/// - [`Fortitude`]: The strength or authority of execution.
///   - `Polite`: The operation should respect constraints and may yield.
///   - `Force`: The operation must be enforced regardless of resistance.
///
/// ## Interpretation
///
/// | Precision     | Fortitude | Meaning                          |
/// |---------------|-----------|----------------------------------|
/// | BestEffort    | Polite    | Soft, optional attempt           |
/// | Exact         | Polite    | Strict but non-forcing           |
/// | BestEffort    | Force     | Enforced but flexible execution  |
/// | Exact         | Force     | Strict and mandatory execution   |
///
/// ## Design Notes
///
/// - Encodes **intent**, not state or value
/// - Fully domain-agnostic
/// - Separates *what to do* from *how to do it*
/// - Enables consistent execution behavior across systems
///
/// Implementations should ensure that `from_directive` produces a value whose
/// behavior reflects the specified execution semantics.
///
/// ## Default Discriminant
///
/// - `Discriminant = ()`: defines a single default directive,
///   meaning one unique set of directions is assumed.
pub trait Directive<Discriminant: DiscriminantTag = ()>: Sized {
    /// Returns the required precision of the operation.
    fn precision(&self) -> Precision;

    /// Returns the enforcement strength of the operation.
    fn fortitude(&self) -> Fortitude;

    /// Constructs a value from execution semantics.
    ///
    /// The resulting value should embody how the operation behaves
    /// in terms of strictness and enforcement.
    fn new(precision: Precision, fortitude: Fortitude) -> Self;
}

/// Maps semantic variants to zero-based position indices.
///
/// `VariantCount` is 1-based, while `PositionIndex` is 0-based. As a result,
/// the maximum valid index is `VariantCount::VARIANT_COUNT - 1`.
pub trait PositionIndex: VariantCount {
    /// Returns the zero-based index for this variant, or `None` if the
    /// variant must not participate in indexing.
    fn index(&self) -> usize;

    /// Returns the variant for the given zero-based index, or `None`
    /// if the index is out of range or non-semantic.
    fn position_of(index: usize) -> Option<Self>
    where
        Self: Sized;
}

/// Trivial implementation for marker type [`Ignore`], representing a
/// single, non-variant position.
impl PositionIndex for Ignore {
    /// [`Ignore`] has exactly one semantic variant and always maps to index `0`.
    fn index(&self) -> usize {
        0
    }

    /// Returns the variant for index `0`; any non-zero index is invalid.
    fn position_of(index: usize) -> Option<Self> {
        if index.is_zero() {
            return Some(Ignore(PhantomData));
        }
        None
    }
}

impl VariantCount for Ignore {
    /// [`Ignore`] defines a single semantic variant.
    const VARIANT_COUNT: u32 = 1;
}

/// A trait for structures that grow by preserving previous state
/// while creating fresh local generations.
///
/// `Accrete` stores and propagates deterministic item keys,
/// not the full item payload itself.
///
/// The actual payload may live elsewhere (for example:
/// `key -> value` storage), while this structure tracks only
/// membership and historical propagation of stable keys.
///
/// Calling `accrete()` creates a new generation where:
///
/// - current local item keys are promoted into inherited history
/// - the returned value begins with fresh local state
///
/// Example flow:
///
/// | Generation | Inherited        | Local    |
/// |------------|------------------|----------|
/// | A          | `[]`             | `[a, b]` |
/// | B          | `[a, b]`         | `[]`     |
/// | B          | `[a, b]`         | `[c, d]` |
/// | C          | `[a, b, c, d]`   | `[]`     |
///
/// where `a`, `b`, `c`, and `d` represent deterministic
/// item key hashes (`[u8; 32]`), not full values.
///
/// This allows persistent, layered growth where older generations
/// remain preserved while new values are added independently.
///
/// ## Note
///
/// Always prefer working with the most recent generation, since it
/// contains the complete inherited state of all previous generations.
///
/// Older generations may be retained for history, snapshots, or audit
/// purposes, but they should not be used for active logic without first
/// consulting the latest generation to confirm whether that state still
/// exists, has changed, or has been removed.
pub trait Accrete: Clone {
    /// The payload type used to derive deterministic keys.
    ///
    /// The payload itself is not stored inside the accreted structure.
    /// Only its stable deterministic key hash is tracked.
    type Item: Probe;

    /// Creates a deterministic key for an item.
    ///
    /// This key is what gets stored, inherited, and removed
    /// across generations.
    ///
    /// This should be stable across executions so the same item
    /// always resolves to the same key.
    ///
    /// Typical implementations use:
    ///
    /// - SCALE encoding + hash (`blake2_256`)
    /// - canonical identifiers
    /// - domain-specific unique references
    fn make_key(
        item: &Self::Item,
    ) -> [u8; 32] {
        blake2_256(&item.encode())
    }

    /// Creates the next generation from the current instance.
    ///
    /// The current local key set becomes inherited history,
    /// and the returned value starts a fresh local generation.
    fn accrete(&self) -> Self;

    /// Returns keys inherited from previous generations.
    fn inherited(&self) -> Vec<[u8; 32]>;

    /// Returns keys belonging only to the current generation.
    fn local(&self) -> Vec<[u8; 32]>;

    /// Inserts an item's deterministic key into the current
    /// local generation.
    ///
    /// Returns the stable key used for future reference.
    fn add_to_local(
        &mut self,
        item: Self::Item,
    ) -> [u8; 32];

    /// Returns `true` if the given key exists in the current
    /// local generation only.
    fn exists_in_local(
        &self,
        key: &[u8; 32],
    ) -> bool;

    /// Returns `true` if the given key exists in inherited
    /// generations only.
    fn exists_in_inherited(
        &self,
        key: &[u8; 32],
    ) -> bool;

    /// Removes a key from the current local generation.
    fn remove_from_local(
        &mut self,
        key: &[u8; 32],
    );

    /// Removes a key from inherited generations.
    ///
    /// This should be used carefully since inherited state
    /// affects descendant visibility.
    fn remove_from_inherited(
        &mut self,
        key: &[u8; 32],
    );
}

// ===============================================================================
// ```````````````````````````````````` ENUMS ````````````````````````````````````
// ===============================================================================

/// A tri-state enum representing a generic disposition or stance.
///
/// This enum is designed to be domain-agnostic, enabling its use in a variety of contexts
/// such as decision making, status tracking, access control, and governance logic.
///
/// #### Variants:
/// - `Affirmative`: Represents a positive/agreeing/approved state.
/// - `Contrary`: Represents a negative/disagreeing/rejected state.
/// - `Awaiting`: Represents an undecided, pending, or inactive state.
///
/// #### Usage Scenarios:
/// - **Governance/voting**: To track votes (Yes/No/Pending).
/// - **Authorization**: Representing access status (Granted/Denied/Unverified).
/// - **Workflow/state machines**: To mark stages such as completed, failed, or pending.
/// - **Consensus/validation**: To model block author responses.
/// - **General application logic**: Anywhere a compact, expressive tri-state is useful.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
    DecodeWithMemTracking,
    Default,
    Debug,
)]
pub enum Disposition {
    /// Represents an affirmative (positive/agreeing) disposition.
    #[default]
    Affirmative,
    /// Represents a contrary (negative/disagreeing) disposition.
    Contrary,
    /// Represents a disposition that is undecided or still pending.
    Awaiting,
}

impl PositionIndex for Disposition {
    fn index(&self) -> usize {
        match self {
            Disposition::Affirmative => 0,
            Disposition::Contrary => 1,
            Disposition::Awaiting => 2,
        }
    }

    fn position_of(index: usize) -> Option<Self> {
        match index {
            0 => Some(Disposition::Affirmative),
            1 => Some(Disposition::Contrary),
            2 => Some(Disposition::Awaiting),
            _ => None,
        }
    }
}

impl VariantCount for Disposition {
    const VARIANT_COUNT: u32 = 3;
}

/// A generic enum representing the polarity or nature of an entity.
///
/// This enum captures whether something is fundamentally constructive, destructive,
/// a mix of both, or undefined. It is abstract and domain-independent, making it suitable
/// for systems that involve influence, intent, or behavioral modeling.
///
/// #### Variants:
/// - `Constructive`: Positive or generative in nature.
/// - `Destructive`: Negative or harmful in nature.
/// - `Composite`: Contains both constructive and destructive traits.
/// - `Indeterminate`: Polarity is undefined, neutral, or undecided.
///
/// #### Use Cases:
/// - **Reputation systems**: To classify behavior or actions.
/// - **Governance models**: To evaluate the polarity of proposals or arguments.
/// - **Content moderation**: Tagging interactions or submissions by nature.
/// - **Feedback analysis**: To assess sentiment or system feedback.
///
/// #### Relation to [`Disposition`]:
/// Often used **similar to `Disposition`** to provide richer semantic meaning:
/// - `Disposition` indicates the **stance** (affirmative, contrary, awaiting).
/// - `Polarity` indicates the **nature or impact** (constructive, destructive, etc.).
///
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, TypeInfo, Debug, Default)]
pub enum Polarity {
    /// Represents a positive, generative, or beneficial nature.
    #[default]
    Constructive,
    /// Represents a negative, harmful, or destructive nature.
    Destructive,
    /// Represents a mix of both constructive and destructive traits.
    Composite,
    /// Represents an undefined, neutral, or undecided polarity.
    Indeterminate,
}

impl VariantCount for Polarity {
    const VARIANT_COUNT: u32 = 4;
}

impl PositionIndex for Polarity {
    fn index(&self) -> usize {
        match self {
            Polarity::Constructive => 0,
            Polarity::Destructive => 1,
            Polarity::Composite => 2,
            Polarity::Indeterminate => 3,
        }
    }

    fn position_of(index: usize) -> Option<Self> {
        match index {
            0 => Some(Polarity::Constructive),
            1 => Some(Polarity::Destructive),
            2 => Some(Polarity::Composite),
            3 => Some(Polarity::Indeterminate),
            _ => None,
        }
    }
}

// ===============================================================================
// ``````````````````````````````````` STRUCTS ```````````````````````````````````
// ===============================================================================

/// No-op generic marker type used in place of `()` when a generic
/// parameter needs to be carried.
///
/// While `()` is commonly used as a default no-op type in Rust,
/// it cannot expose or propagate generic parameters. `Ignore`
/// provides a zero-sized alternative that preserves generic context
/// without introducing behavior.
///
/// ## Purpose
///
/// - Acts as a no-op / default implementor
/// - Carries a generic type parameter `T`
/// - Enables trait implementations that require associated types
///   derived from a generic input
///
/// ## Usage
///
/// Used in generic abstractions where:
/// - a placeholder type is required,
/// - behavior is intentionally absent,
/// - but type information must still flow through the system.
///
/// ## Example
///
/// ```ignore
/// trait ProvidesType {
///     type Item;
/// }
///
/// // Cannot use `()` here because it cannot carry `T`
/// impl<T> ProvidesType for Ignore<T> {
///     type Item = T;
/// }
/// ```
///
/// This allows a no-op type to still propagate generic information.
pub struct Ignore<T = ()>(pub PhantomData<T>);
