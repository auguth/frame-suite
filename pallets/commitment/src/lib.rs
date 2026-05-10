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
// ````````````````````````````` PALLET COMMITMENT ```````````````````````````````
// ===============================================================================

//! Implementation crate for the [`Commitment`](frame_suite::commitment)
//! family of traits.
//!
//! A semantic bonding layer that binds assets to caller-defined purpose and context,
//! with collective management of bonded value and lazy adjustments.
//!
//! Used instead of direct fungible locks to enable richer control semantics and
//! automated balance management. These are called commitments as they inherently
//! carry a semantic reason defined by the consuming pallet.
//!
//! ## Overview
//!
//! - [`Config`] - Runtime configuration
//! - [`Call`] - Dispatchable extrinsics
//! - [`Pallet`] - Trait implementation for external modules
//!
//! This pallet provides a **generalized bonding (locking) mechanism** for
//! fungible (quantitative) assets, enabling other pallets to express
//! **structured financial intent**.
//!
//! Instead of treating balances as passive values, this system allows assets to be:
//!
//! - **Bonded under a reason** (e.g., `LockReason`)
//! - **Bound to a digest** (a context-specific identifier defined by the caller pallet)
//!
//! Together, this forms a **Commitment**:
//!
//! ```text
//! Commitment = bond(asset) -> (reason, digest)
//! ```
//!
//! ## Key Responsibilities
//!
//! This pallet acts as a **shared infrastructure layer** that:
//!
//! - Locks assets on behalf of any fungible account
//! - Groups commitments under **runtime-defined reasons**
//! - Tracks and manages value at the **digest level**
//! - Allows controlled updates to aggregated digest values
//!
//! The **meaning and context of each digest** are defined by the
//! caller pallet (e.g., staking, escrow, trading, governance).
//!
//! ## Core Features
//!
//! - **Commitment** - lock assets under `(reason, digest)`, enforcing one commitment per
//!   (proprietor, reason). Value can only increase per commitment, while aggregate
//!   digest values may be adjusted. Supports lazy resolution.
//!
//! - **Digest Management** - track and update aggregate value at the digest level,
//!   propagating changes to all commitments.
//!
//! - **Digest Mint / Reap** - increase or decrease the aggregate value of a digest,
//!   automatically adjusting all associated commitments proportionally.
//!
//! - **Indexes** - group multiple digests with share-based ownership
//!   (see [`CommitIndex`](frame_suite::commitment::CommitIndex)).
//!
//! - **Pools** - manager-controlled allocation with dynamic rebalancing and fixed
//!   commissions (see [`CommitPool`](frame_suite::commitment::CommitPool)).
//!
//! - **Variants** - semantic differentiation (e.g., long/short, positive/negative)
//!   (see [`CommitVariant`](frame_suite::commitment::CommitVariant)).
//!
//! - **Lazy Evaluation** - values reflect live digest state and are realized on query
//!   or resolution.
//!
//! - **Asset Agnostic** - works with any fungible asset type implementing required traits.
//!
//! Each commitment is scoped by a **Reason** and anchored to a **Digest**.
//!
//! ### Design Scope
//!
//! The pallet is a **generic implementation** of
//! [`Commitment`](frame_suite::commitment::Commitment),
//! designed to be **loosely coupled** with consumer pallets via their `Config` traits.
//!
//! ```ignore
//! // Consumer pallet configuration
//! pub trait Config {
//!     /// Commitment adapter providing bonding logic
//!     type CommitmentAdapter: Commitment<Self::AccountId>;
//! }
//! ```
//!
//! It handles commitment accounting and lifecycle management, while consumer
//! pallets define domain-specific logic and user interactions.
//!
//! This provides a **bonding abstraction over basic fungible locks**, enabling
//! rich economic behaviors without duplicating core logic.
//!
//! ## Extrinsics Scope
//!
//! This pallet exposes **no user-facing extrinsics for commitments**.
//!
//! Commitment operations are accessed by consumer pallets through the traits,
//! ensuring controlled and domain-specific usage.
//!
//! Only minimal extrinsics for **basic deposit and withdrawal** for a
//! default `PrepareForCommit` hold/reserve are provided and some read-only
//! APIs (see [`Call`]).
//!
//! ## Native Commitment Reserve
//!
//! This pallet exposes only minimal extrinsics to deposit and withdraw
//! funds into a native reserve ([`HoldReason::PrepareForCommit`]), which acts
//! as the default funding source for all commitment operations.
//!
//! Commitments consume assets from this reserve under
//! [`Fortitude::Polite`](frame_support::traits::tokens::Fortitude),
//! allowing users to pre-fund commitments once and reuse the
//! same reserve across all consumer pallets, ensuring efficient,
//! directive-driven execution
//!
//! When [`Fortitude::Force`](frame_support::traits::tokens::Fortitude)
//! is used, operations may fallback to the liquid (free) balance if
//! reserve funds are insufficient.
//!
//! Consumer pallets utilizing commitments should account for this behavior:
//! - use polite semantics to operate strictly on reserved funds
//! - use force semantics to allow fallback to liquid balance when required
//!
//! ## Instance and Model
//!
//! The pallet supports **multiple instances**, each capable of handling
//! commitments across diverse scenarios.
//!
//! - Each instance may define its own configuration via [`Config`],
//!   allowing independent behavior and specialization.
//!
//! - Each instance supports **multiple reasons**, enabling different pallets
//!   or domains to operate within the same instance.
//!
//! - A single instance can be shared across multiple consumer pallets,
//!   with separation maintained through reasons and digests.
//!
//! - Instances may be separated when isolation is required.
//!
//! This enables the pallet to act as a **shared economic layer** across use cases.
//!
//! ## Terminology
//!
//! - **Proprietor** - the account or entity that owns and manages commitments.
//! - **Reason** - the categorical purpose of a commitment.
//! - **Digest** - a unique identifier representing commitment context.
//! - **Direct Commitment** - a commitment to a single digest.
//! - **Index** - a collection of digests with shares
//!   (see [`CommitIndex`](frame_suite::commitment::CommitIndex)).
//! - **Pool** - a managed structure with allocation and commission
//!   (see [`CommitPool`](frame_suite::commitment::CommitPool)).
//! - **Entry** - a digest component within an index.
//! - **Slot** - a digest component within a pool.
//! - **Commission** - manager's share collected on resolution.
//! - **Variant** - semantic position (see
//!   [`CommitVariant`](frame_suite::commitment::CommitVariant)).
//!
//! ## Development Feature Gate
//!
//! This pallet includes a `dev` feature gate for development and testing.
//!
//! Core functionality is exposed via public APIs for RPC and UI usage.
//! The `dev` feature provides thin wrapper extrinsics and extended
//! events for direct inspection.
//!
//! This feature must be disabled in production runtimes due to additional debugging overhead.

#![cfg_attr(not(feature = "std"), no_std)]

// ===============================================================================
// `````````````````````````````````` MODULES ````````````````````````````````````
// ===============================================================================

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod balance;
mod commitment;
mod helpers;
pub mod traits;
pub mod types;
pub mod weights;

// ===============================================================================
// `````````````````````````````` PALLET MODULE ``````````````````````````````````
// ===============================================================================

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    // ===============================================================================
    // ````````````````````````````````` IMPORTS `````````````````````````````````````
    // ===============================================================================

    // --- Local crate imports ---
    use crate::{balance::*, types::*, weights::WeightInfo};

    // --- FRAME Support ---
    use frame_support::{
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::{
            fungible::InspectHold,
            tokens::{fungible::*, Fortitude, Precision, Preservation},
            BuildGenesisConfig, VariantCount,
        },
    };

    // --- FRAME System ---
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};

    // --- FRAME Suite ---
    use frame_suite::{
        assets::*,
        base::{Countable, Delimited, Fractional, Percentage, RuntimeEnum, Time},
        commitment::*,
        misc::PositionIndex,
        plugin_types,
    };

    // --- Substrate primitives ---
    use sp_runtime::{
        traits::{Debug, MaybeDisplay},
        DispatchError, FixedPointNumber, PerThing,
    };
    use sp_std::vec::Vec;

    // ===============================================================================
    // `````````````````````````````` PALLET MARKER ``````````````````````````````````
    // ===============================================================================

    /// Primary Marker type for the **Commitment pallet**.
    ///
    /// This pallet provides implementations for traits from
    /// [`commitment`](frame_suite::xp)
    ///
    /// [`Pallet`] implements the core commitment system traits:
    ///
    /// - [`InspectAsset`]
    /// - [`DigestModel`]
    /// - [`Commitment`]
    /// - [`CommitIndex`]
    /// - [`CommitPool`]
    /// - [`CommitVariant`]
    /// - [`IndexVariant`]
    /// - [`PoolVariant`]
    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    // ===============================================================================
    // ```````````````````````````` INTERNAL PALLET MARKER ```````````````````````````
    // ===============================================================================

    /// Internal helper struct for implementing low-level commitment trait operations.
    ///
    /// This marker type serves as a namespace for trait implementations defined in
    /// [`crate::traits`], providing internal access to commitment system low-level primitives
    /// without exposing unchecked-functions as part of the public API via [`Pallet`].
    ///
    /// `CommitHelpers` implements the commitment low-level helper traits:
    ///
    /// - [`CommitBalance`](crate::traits::CommitBalance)
    /// - [`CommitDeposit`](crate::traits::CommitDeposit)
    /// - [`CommitWithdraw`](crate::traits::CommitWithdraw)
    /// - [`CommitOps`](crate::traits::CommitOps)
    /// - [`CommitInspect`](crate::traits::CommitInspect)
    /// - [`PoolOps`](crate::traits::PoolOps)
    /// - [`IndexOps`](crate::traits::IndexOps)
    pub(crate) struct CommitHelpers<T: Config<I>, I: 'static = ()>(PhantomData<(T, I)>);

    // ===============================================================================
    // `````````````````````````````` CONFIG TRAIT ```````````````````````````````````
    // ===============================================================================

    /// Configuration trait for the Commitment pallet.
    ///
    /// This trait defines the types, constants, and dependencies
    /// that the runtime must provide for this pallet to function.
    ///
    /// The generic parameter `I` allows the same pallet to be instantiated
    /// multiple times within a runtime. Each instance can have its own
    /// independent storage and configuration.
    ///
    /// Example:
    /// - `I = ()` -> default (single instance)
    /// - `I = Staking`, `Governance`, etc. -> multiple independent instances
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        // --- Runtime Anchors ---

        /// The overarching event type for the runtime.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Hold reason type for locking assets.
        ///
        /// Utilized to provide a native hold reason for all commitments.
        type AssetHold: From<HoldReason> + RuntimeEnum + Delimited + Copy + VariantCount;

        /// Freeze reason type for commitment-specific freezes.
        type AssetFreeze: From<FreezeReason> + RuntimeEnum + Delimited + Copy + VariantCount;

        // --- Scalars ---

        /// Type for representing shares in indexes or pools.
        ///
        /// Must implement an unsigned integer and be convertible into the pallet's
        /// asset type (safe conversion).
        type Shares: Countable + Into<AssetOf<Self, I>> + MaybeDisplay;

        /// Bias factor used for fixed-point arithmetic for
        /// direct, index, or pool commitments.
        ///
        /// Must be a fixed-point number whose precision is sufficient to
        /// safely handle division and percentage-based arithmetic operations.
        type Bias: Fractional;

        /// Time counter used for timestamps.
        ///
        /// May be block number, Unix epoch, or an internal counter.
        type Time: Time;

        /// Commission type used for pool fee calculations.
        ///
        /// Represents a percentage or ratio value applied during pool
        /// resolution to extract commission from committed value.
        type Commission: Percentage;

        // --- Pallet Adapters ---

        /// The fungible asset type for this pallet instance.
        ///
        /// Must support inspection and unbalanced mutation, freezing, and holding operations.
        type Asset: Inspect<
                Proprietor<Self>,
                Balance: MaybeDisplay
                             + From<<Self::Bias as FixedPointNumber>::Inner>
                             + From<<Self::Commission as PerThing>::Inner>,
            > + InspectFreeze<Proprietor<Self>, Id = Self::AssetFreeze>
            + InspectHold<Proprietor<Self>, Reason = Self::AssetHold>
            + Mutate<Proprietor<Self>>
            + UnbalancedHold<Proprietor<Self>>
            + Unbalanced<Proprietor<Self>>
            + MutateHold<Proprietor<Self>>
            + MutateFreeze<Proprietor<Self>>;

        // --- Contexual Enums ---

        /// The set of commitment dispositions supported by this pallet.
        ///
        /// A disposition acts as a **meta-identifier** defining the semantic position
        /// of a commitment (e.g. affirmative, contrary). Commitments are scoped by
        /// this value.
        ///
        /// For plain commitments where no variant is explicitly specified, the
        /// [`Default`] value of this type is used.
        ///
        /// Implementations must ensure that only **semantic variants** participate
        /// in indexing; marker or non-semantic variants should be excluded in
        /// [`PositionIndex`].
        ///
        /// The [`Ignore`](frame_suite::misc::Ignore) type may be used to represent a
        /// single, non-variant position. In this configuration, all commitments map
        /// to index `0`, effectively disabling variant-based semantics.
        ///
        /// For optimal storage usage, it is recommended that the default variant
        /// maps to index `0`, allowing non-varianted commitments to occupy the
        /// first slot without requiring initialization of higher variant slots.
        type Position: PositionIndex + RuntimeEnum + Delimited + Default;

        // --- Plugins ---

        plugin_types! {
            // Input carrier for lazy balance operations.
            //
            // Encodes operation-specific arguments used by
            // `Self::BalanceFamily` when resolving behavior via
            // `LazyBalanceRoot`.
            //
            // Supports both mutable and immutable borrow access patterns,
            // allowing operations to express validation, mutation,
            // and query semantics over commitments.
            input: LazyInput<'a, Self, I>,

            // Output carrier for lazy balance operations.
            //
            // Represents the result of executing a plugin model,
            // including computed values, state transitions,
            // receipts, and error outcomes.
            //
            // Acts as the result boundary for all operations
            // resolved through `Self::BalanceFamily`.
            output: LazyOutput<'a, Self, I>,


            // Lifetime binding for plugin execution.
            //
            // Represents mutable and immutable borrows used by `LazyInput`
            // and `LazyOutput` during operation execution.
            //
            // Ensures safe propagation of references across plugin models.
            borrow: ['a],

            // Root discriminant for lazy balance operations.
            //
            // Defines the complete set of operation identifiers
            // (e.g. deposit, withdraw, resolve, query), each of
            // which maps to a concrete plugin model in
            // `Self::BalanceFamily`.
            //
            // Acts as the entry point for compile-time dispatch
            // of behavior across all lazy balance operations.
            root: LazyBalanceRoot,

            /// The [`Lazy balance`](frame_suite::assets::LazyBalance)
            /// [`plugin family`](frame_suite::plugins) anchor type.
            ///
            /// Encapsulates all lazy balance operations including validation,
            /// mutation, resolution, and query semantics over commitments,
            /// indexes, and pools.
            ///
            /// Each operation is selected via a discriminant defined in
            /// [`LazyBalanceRoot`] and executed using [`LazyInput`] -> [`LazyOutput`]
            /// transformation.
            ///
            /// Conceptually performs:
            ///
            /// `Operation(LazyInput) -> LazyOutput`
            ///
            /// where the specific behavior is determined by the plugin model
            /// associated with each operation discriminant.
            ///
            /// Designed to be selectable using template plugin family models in
            /// [`frame_plugins::balances`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            ///
            /// ## Pool Constraints
            ///
            /// Pool operations via [`CommitPool`] represent **higher-order balance
            /// compositions** (balance over balance), and therefore impose stricter
            /// requirements.
            ///
            /// These operations rely on [`Directive`] semantics, where:
            ///
            /// - [`Precision::Exact`] must be honored for correct value distribution
            /// - [`Fortitude::Force`] must be enforced for deterministic execution
            ///
            /// These guarantees are essential for correctness of pool allocation,
            /// redistribution, and resolution.
            ///
            /// If [`Config::MaxIndexEntries`] is set to `0`, index and pool
            /// commitments are disabled, and these requirements do not apply.
            ///
            /// When pools are enabled, plugin models must correctly support these
            /// semantics, otherwise operations may fail internally.
            family: BalanceFamily,

            /// Plugin family **context** for lazy balance execution.
            ///
            /// Supplies the execution environment required by
            /// [`Self::BalanceFamily`] for all operations over
            /// [`LazyInput`] -> [`LazyOutput`].
            ///
            /// Defines bounds, extension schemas, and unified error handling,
            /// ensuring consistent and type-safe execution across all operations.
            ///
            /// Must produce a context satisfying [`LazyBalanceContext`].
            context: BalanceContext,

            // Ensures the resolved context
            // `<BalanceContext as frame_suite::plugins::ModelContext>::Context`
            // satisfies the required contract for lazy balance execution.
            //
            // Guarantees that all plugin models operate within a fully
            // specified lazy-balance environment.
            provides: [LazyBalanceContext],
        }

        // --- Weights ---

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        // --- Constants ---

        /// Maximum number of entries allowed in an index.
        ///
        /// Setting this value to **zero** disables index and pool commitments for this
        /// pallet instance. A non-zero value enables hosting index and pool commitments;
        ///
        /// An index represents an **unmanaged pool** of digests with associated shares.
        /// A single committed value to the index is proportionally distributed across
        /// its entries as individual commitments.
        ///
        /// Since pools are constructed from indexes, this limit also bounds the maximum
        /// number of slots a pool may contain.
        #[pallet::constant]
        type MaxIndexEntries: Get<u32> + Clone + Debug;

        /// Maximum number of commitments allowed per digest.
        ///
        /// **Should be a Non-Zero Value**, else unstable behaviours should be
        /// expected.
        ///
        /// Each commitment represents an individual lock against a digest.
        /// A digest may therefore have at most this many active commitments.
        ///
        /// This limit applies uniformly, regardless of whether commitments
        /// originate directly, from an index, or from a pool. Commitments
        /// distributed from an index or pool still count as individual
        /// commitments to the underlying digest.
        #[pallet::constant]
        type MaxCommits: Get<u32> + Clone + Debug;

        /// Controls emission of [`Event`] via `deposit_event`.
        ///
        /// Recommended:
        /// - `false` for production runtimes (to reduce overhead)
        /// - `true` for development and mock runtimes (for testing and
        /// observability)
        #[pallet::constant]
        type EmitEvents: Get<bool> + Clone + Debug;
    }

    // ===============================================================================
    // ``````````````````````````````` COMPOSITE ENUMS ```````````````````````````````
    // ===============================================================================

    #[pallet::composite_enum]
    /// Commitment pallet `HoldReason`, merged into the runtime composite enum.
    pub enum HoldReason {
        /// Native hold reason used by the Commitment pallet.
        ///
        /// Assets held under this reason act as a **pre-reserved balance** from which
        /// commitments can be created efficiently. This allows frequent bonding users
        /// to pre-hold assets once and later use them across any consumer pallet that
        /// shares this Commitment pallet instance.
        ///
        /// Using this hold reason reduces repeated balance checks and locking overhead
        /// when creating commitments.
        PrepareForCommit,
    }

    #[pallet::composite_enum]
    /// Commitment pallet `FreezeReason`, merged into the runtime composite enum.
    pub enum FreezeReason {
        /// Placeholder freeze reason used exclusively for benchmarking.
        ///
        /// Consumer pallets typically define their own bounded freeze-reason enums
        /// and may not be able to reuse this type. This pallet itself does not freeze
        /// any assets using this reason, making it suitable as a no-op placeholder
        /// for benchmarking purposes.
        BenchTestReason,
    }

    // ===============================================================================
    // ``````````````````````````` GENESIS CONFIG (EMPTY) ````````````````````````````
    // ===============================================================================

    /// No-BalanceOp Genesis configuration for the pallet.
    ///
    /// This pallet does not currently expose any runtime-configurable parameters
    /// at genesis. Some internal values are initialized automatically during
    /// genesis execution.
    ///
    /// The genesis configuration is retained to allow future or downstream
    /// initialization extensions without breaking compatibility.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        /// Phantom data to satisfy generic parameters.
        /// No user-configurable data is stored.
        _phantom: PhantomData<(T, I)>,
    }

    impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
        /// Provides a default instance of the genesis config.
        ///
        /// Since there are no configurable parameters, this simply
        /// initializes the `_phantom` field.
        fn default() -> Self {
            Self {
                _phantom: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
        /// Builds the initial pallet state at genesis.
        ///
        /// Currently, the only action is to initialize asset tracking
        /// storages to zero:
        /// - [`AssetToIssue`]: Tracks total asset units to be issued.
        /// - [`AssetToReap`]: Tracks total asset units to be reaped (burned).
        ///
        /// No other state (indexes, digests, pools, slots, commits) is
        /// initialized at genesis.
        fn build(&self) {
            let zero = AssetOf::<T, I>::zero();
            AssetToIssue::<T, I>::put(zero); // Initialize issued assets to zero
            AssetToReap::<T, I>::put(zero); // Initialize reaped assets to zero
        }
    }

    // ===============================================================================
    // ```````````````````````````````` STORAGE TYPES ````````````````````````````````
    // ===============================================================================

    /// Tracks the **total committed asset value** for a specific `CommitReason`.
    ///
    /// This storage sums up all committed amounts (total asset-circulation) across
    /// **all digests and variants** for a given reason.
    ///
    /// Unlike digest-level or variant-level tracking, `ReasonValue` provides an **aggregated
    /// view** of total assets committed under a specific reason, which simplifies monitoring,
    /// accounting, and reporting of the total exposure.
    ///
    /// This includes assets scheduled to be issued ([`AssetToIssue`]), as newly minted
    /// value is reflected within commitments. However, it excludes assets scheduled
    /// to be reaped ([`AssetToReap`]), as those are pending removal and not considered
    /// part of the effective committed value.
    #[pallet::storage]
    pub type ReasonValue<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, CommitReason<T, I>, AssetOf<T, I>, OptionQuery>;

    /// Maps each [`CommitReason`] and its associated [`Digest`] to the digest's information.
    ///
    /// ### Key Structure:
    /// `(CommitReason, Digest) -> DigestInfo`
    ///
    /// This storage ensures that **every digest is always tied to a reason**. A digest cannot exist
    /// independently without a reason, enforcing a strict relationship between commitments and their
    /// context.
    #[pallet::storage]
    pub type DigestMap<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CommitReason<T, I>>,
            NMapKey<Blake2_128Concat, Digest<T>>,
        ),
        DigestInfo<T, I>,
        OptionQuery,
    >;

    /// Maps each [`Proprietor`] and the committed reason ([`CommitReason`]) to
    /// their commitment information.
    ///
    /// ### Key Structure:
    /// `(Proprietor, CommitReason) -> CommitInfo`
    ///
    /// This storage enforces the invariant: **one proprietor can have at most one
    /// commitment per reason**. Each commitment is tied to a single digest, and all
    /// variant-specific details for that commitment are stored within [`CommitInfo`].
    ///
    /// Unlike [`DigestMap`], which allows multiple digests per reason, `CommitMap`
    /// enforces at most one [`CommitInfo`] per [`CommitReason`] for each proprietor
    /// through its storage structure.
    #[pallet::storage]
    pub type CommitMap<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, Proprietor<T>>,
            NMapKey<Blake2_128Concat, CommitReason<T, I>>,
        ),
        CommitInfo<T, I>,
        OptionQuery,
    >;

    /// Stores **index information** for a given [`CommitReason`] additionally keyed
    /// by [`IndexDigest`].
    ///
    /// Each index represents a collection of digest entries, each holding a share and variant,  
    /// functioning as an aggregation layer between individual digests and higher-level pools.  
    ///
    /// ### Key Structure:
    /// `(CommitReason, IndexDigest) -> IndexInfo`
    ///
    /// ### Purpose:
    /// - Provides a way to group multiple digest entries under a single reason.
    /// - Simplifies calculations related to composite positions and share-based aggregation.
    /// - Allows reusability and modularization of commitment logic (e.g., an index can feed
    /// into a pool).
    ///
    /// ### Notes:
    /// - An index **cannot exist without an associated reason**, enforcing a clear
    /// ownership hierarchy.
    /// - Each [`IndexInfo`] tracks its own [`Entries`], total `capital`, and
    /// current `balance_of`.
    /// - [`IndexInfo`] is used as a foundational structure for generating higher-order
    /// constructs like [`PoolInfo`].
    #[pallet::storage]
    pub type IndexMap<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CommitReason<T, I>>,
            NMapKey<Blake2_128Concat, IndexDigest<T>>,
        ),
        IndexInfo<T, I>,
        OptionQuery,
    >;

    /// Stores **commit information for individual entries within an index**, scoped by both
    /// a [`CommitReason`] and the corresponding [`Proprietor`].
    ///
    /// ### Key Structure:
    /// `(CommitReason, IndexDigest, EntryDigest, Proprietor) -> Commits`
    ///
    /// ### Purpose:
    /// - Tracks how much each proprietor ([`Proprietor`]) has committed to a specific entry
    ///   under a particular index and reason.
    /// - Enforces the hierarchical structure:
    ///   - **Reason -> Index -> Entry -> Proprietor -> Commits**
    /// - Each [`Commits`] encompasses one or more commitment instances made by the same proprietor.
    ///
    /// ### Notes:
    /// - Each proprietor can have **only one active commit per (reason, index, entry)** combination.
    /// - This mapping is essential for resolving commitment provenance during digest updates
    ///   or rebalancing operations.
    /// - Commitments stored here are "lazy" - meaning changes may not be immediately reflected
    ///   in the underlying asset accounting until resolution occurs.
    #[pallet::storage]
    pub type EntryMap<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CommitReason<T, I>>,
            NMapKey<Blake2_128Concat, IndexDigest<T>>,
            NMapKey<Blake2_128Concat, EntryDigest<T>>,
            NMapKey<Blake2_128Concat, Proprietor<T>>,
        ),
        Commits<T, I>,
        OptionQuery,
    >;

    /// Stores **pool-level information** associated with a specific [`CommitReason`].
    ///
    /// ### Key Structure:
    /// `(CommitReason, PoolDigest) -> PoolInfo`
    ///
    /// ### Purpose:
    /// - Represents a **composite pool** of multiple slots, each corresponding to one or more
    ///   underlying commitments, entries, or digests.
    /// - Provides aggregation of collective commitment state, total capital, and operational
    ///   parameters such as commission.
    ///
    /// ### Notes:
    /// - Used together with [`PoolManager`] for management and operational control.
    #[pallet::storage]
    pub type PoolMap<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CommitReason<T, I>>,
            NMapKey<Blake2_128Concat, PoolDigest<T>>,
        ),
        PoolInfo<T, I>,
        OptionQuery,
    >;

    /// Tracks the manager of each pool for a given reason.
    ///
    /// Each pool has a designated manager (proprietor) responsible for slot management.
    /// Pools are mutable, but their manager can be transfered/updated.
    ///
    /// - Keys:
    ///   1. [`CommitReason`] - the reason/context under which the pool exists.
    ///   2. [`PoolDigest`] - the unique identifier of the pool.
    /// - Value: [`Proprietor`] - account managing the pool.
    /// - Query behavior: `OptionQuery` returns `None` if the pool has no assigned manager,
    /// effectively stale.
    #[pallet::storage]
    pub type PoolManager<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CommitReason<T, I>>,
            NMapKey<Blake2_128Concat, PoolDigest<T>>,
        ),
        Proprietor<T>,
        OptionQuery,
    >;

    /// Tracks the total amount of assets that are scheduled to be issued/minted.
    ///
    /// This value is updated whenever commitments increase a digest's balance (reward/inflation).  
    ///
    /// Since this pallet uses **lazy on-demand operations**, the underlying base asset pallet may not  
    /// immediately reflect these changes. `AssetToIssue` provides an **accounting view** of the total  
    /// assets that will soon be minted and added to the system eventually.
    ///
    /// This helps in auditing, ensuring the system knows **how much asset value is pending issuance**.
    #[pallet::storage]
    pub type AssetToIssue<T: Config<I>, I: 'static = ()> =
        StorageValue<_, AssetOf<T, I>, ValueQuery>;

    /// Tracks the total amount of assets that are scheduled to be reaped/burned.
    ///
    /// This value is updated whenever commitments decrease a digest's balance (penalty/deflation).  
    ///
    /// Similar to `AssetToIssue`, these operations are lazy and may not immediately affect the underlying  
    /// base asset pallet. `AssetToReap` provides an **accounting view** of the total assets that will  
    /// soon be removed eventually from circulation, ensuring proper bookkeeping and equilibrium.
    #[pallet::storage]
    pub type AssetToReap<T: Config<I>, I: 'static = ()> =
        StorageValue<_, AssetOf<T, I>, ValueQuery>;

    /// Snapshot storage for [`LazyBalance`] state.
    ///
    /// When a balance needs to capture and store its current state for 
    /// later queries, a snapshot is recorded here.
    ///
    /// Used by plugin families implementing [`LazyBalanceRoot`] via
    /// [`Config::BalanceFamily`] to support snapshot-based, lazy resolution.
    ///
    /// This is the concrete storage backing the
    /// [`VirtualNMap`](frame_suite::virtuals::VirtualNMap) used by [`LazyBalance`].
    ///
    /// Each snapshot is indexed by:
    /// - `Digest`: the balance identifier (linked to a commitment reason)
    /// - `Position`: the balance position (see [`Config::Position`])
    /// - `Time`: the snapshot time (typically a counter)
    #[pallet::storage]
    pub type BalanceSnapShots<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, Digest<T>>,
            NMapKey<Blake2_128Concat, T::Position>,
            NMapKey<Blake2_128Concat, T::Time>,
        ),
        VirtualSnapShot<T, I>,
        OptionQuery,
    >;

    // ===============================================================================
    // ```````````````````````````````````` ERROR ````````````````````````````````````
    // ===============================================================================

    #[pallet::error]
    /// Commitment Pallet Errors
    pub enum Error<T, I = ()> {
        /// Digest not found in the system.
        /// Cannot determine whether it is a direct digest, index, or pool.
        DigestNotFoundToDetermine,

        /// Insufficient funds for the requested operation.
        ///
        /// Consider forcing the operation or reducing the given
        /// asset value.
        InsufficientFunds,

        /// The proprietor already holds a commitment for the reason.
        ///
        /// Only a single commit should exist for a reason. Utilize
        /// indexes and pools in case of distributing multiple commitments.
        CommitAlreadyExists,

        /// Failed to generate a digest for a given source or
        /// index/pool structure.
        CannotGenerateDigest,

        /// A commitment with zero value is invalid.
        ///
        /// Commitments are economic in nature and must not be used
        /// as zero-value markers.
        MarkerCommitNotAllowed,

        /// The proprietor's commit was not found in the system.
        CommitNotFound,

        /// The specified direct-digest was not found in the system.
        DigestNotFound,

        /// The specified index digest was not found in the system.
        IndexNotFound,

        /// The specified entry digest is not found in the index.
        EntryOfIndexNotFound,

        /// The specified index digest already exists for the given reason.
        IndexDigestTaken,

        /// Attempted to remove/reap a direct-digest that still holds funds.
        DigestHasFunds,

        /// Attempted to remove/reap an index that still holds funds.
        IndexHasFunds,

        /// The specified pool digest was not found in the system.
        PoolNotFound,

        /// The specified slot digest is not found in the pool.
        SlotOfPoolNotFound,

        /// The Manager for the specified Pool is not found.
        /// A Pool is expected to have a manager at all times.
        PoolManagerNotFound,

        /// The specified pool digest already exists for the given reason.
        PoolDigestTaken,

        /// Attempted to remove/reap a pool that still holds funds.
        PoolHasFunds,

        /// The direct-digest balance specialized for the given commit-variant (position)
        /// is not initialized or found to be.
        ///
        /// It can only be initialized during deposit operations. Not while being queried.
        DigestVariantBalanceNotFound,

        /// Cannot include more asset value for issuing as the total issue balance exhausted.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        MaxAssetIssued,

        /// Cannot include more asset value for reaping as the total reapable balance exhausted.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        MaxAssetReaped,

        /// Shares given for an index's entry or a pool's slot cannot be zero.
        /// Marker entries or slots are invalid in the system.
        ShareCannotBeZero,

        /// Capital cannot be zero when creating an index or pool.  
        ///
        /// Correct behavior:
        /// - Index/Pool must have `sum(shares) == capital`.
        /// - Entries/Slots must not have empty shares.
        CapitalCannotBeZero,

        /// Share value exceeded capital when creating an index or pool.  
        ///
        /// Correct behavior: every entry/slot must satisfy `share <= capital`.
        ShareGreaterThanCapital,

        /// Asset Issued and Minting to the underlying fungible system
        /// detected inconsistency.
        MintingMoreThanIssued,

        /// Asset To Reap and Burning to the underlying fungible system
        /// detected inconsistency.
        BurningMoreThanReapable,

        /// Indicates that the operation expects the proprietor's existing reserves
        /// (held funds) to be released in order to proceed, typically to be
        /// re-deposited under the commitment hold reason (`PrepareForCommit`).
        ///
        /// If this is not possible, the operation may attempt to proceed by forcing
        /// withdrawal from liquid funds, potentially risking account closure if
        /// enforced by the asset provider or best-effort methods.
        ExpectsHoldWithdrawal,

        /// Asset units overflown when commit-reserve balance is added with liquidly held funds.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        ReserveLiquidOverflow,

        /// Indicates that the operation expects the proprietor's existing reserves
        /// (held funds) and freezes (locked funds) to be released in order to proceed,
        /// typically to be re-deposited under the commitment hold reason (`PrepareForCommit`).
        ///
        /// Since, the operation cannot attempt to proceed by forcing withdrawal from just
        /// liquid funds, as its insufficient.
        ///
        /// Try to reduce provided asset amount or do operation based on best-effort possible
        /// only.
        ExpectsFreezeAndHoldWithdrawal,

        /// The Model of Digest constructed is invalid, since its possibly a
        /// compile time marker.
        InvalidDigestModel,

        /// Digest commit-variant (positional) balances are exhausted
        /// or at maximum capacity.
        ///
        /// This reveals that the [`Config::Position`]'s trait `PositionIndex`
        /// is implemented not as-per its defined invariants.
        VariantsExhausted,

        /// Share value is too small (underflow) to produce a valid
        /// factor when calculating `share/capital`.
        ///
        /// Possible resolutions:
        /// - Increase the fixed-point precision of `Bias` in future upgrades.
        /// - Increase the index entry or pool slot's share value and retry.
        TooSmallShareValue,

        /// Deposit derivation overflowed during index or pool deposit operation.  
        ///
        /// Occurs when an excessively high `share/capital` ratio, multiplied by
        /// a balance value, overflows the scalar.
        DepositDeriveOverflowed,

        /// `share/capital` ratio produced a factor greater than 1.
        ///
        /// This results in errors which may indicate invariants are broken.
        ///  - `sum(shares) <= capital` or,
        ///  - `current_share > capital`
        FactorGreaterThanOne,

        /// Index total balance (deposits-only) has reached its maximum top-level capacity.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        MaxIndexCapacityReached,

        /// Proprietor doesn't hold commits for the specified entry of an index.
        ///
        /// Indicates that the prorprietor haven't committed to the index at all.
        CommitNotFoundForEntry,

        /// Proprietor doesn't hold commits for the specified slot of a pool.
        ///
        /// Indicates that the prorprietor haven't committed to the pool at all.
        CommitNotFoundForSlot,

        /// Proprietor doesn't hold commits for the specified pool.
        CommitNotFoundForPool,

        /// Accumulating total deposit for direct and indirect digests (index/pools)
        /// has overflowed the provided asset type.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        DepositAccumulationExhausted,

        /// The digest for the specified entry of index was not found in the list of digests.
        ///
        /// The digest list represents the underlying direct digests for which
        /// commitments have been made.
        EntryDigestNotFound,

        /// Accumulating total withdrawal for direct or indirect digests (pool/index)
        /// has overflowed the provided asset type.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        WithdrawAccumulationExhausted,

        /// Accumulating total real-time values of all commit instances has overflowed the
        /// provided asset type.
        ///
        /// - For non-issuance assets: migrate to a larger scalar type immediately.
        /// - For issuance assets: conduct an internal audit on the asset (unexpected behavior).
        CommitsAccumulationExhausted,

        /// Indicates that a pool was recovered without being released i.e., empty.
        ReleasePoolToRecover,

        /// The specified pool requires atleast a single slot, with valid shares to carry
        /// the operation.
        EmptySlotsNotAllowed,

        /// Capital shares underflowed during index or pool creation/modification.
        CapitalUnderflowed,

        /// Capital shares overflowed during index or pool creation/modification.
        CapitalOverflowed,

        /// Maximum number of slots in a pool is reached.
        MaxSlotsReached,

        /// The digest for the specified slot of pool was not found in the list of digests.
        ///
        /// The digest list represents the underlying direct digests for which
        /// commitments have been made.
        SlotDigestNotFound,

        /// Max Commits per reason (as per commitment invariant for a single digest model) is exhausted.
        ///
        /// Try resolving and committing a new value instead to the same digest model.
        MaxCommitsReached,

        /// Maximum number of entries in an index is reached.
        MaxEntriesReached,

        /// Reason exists (as its compile-time proved) but contains no commitments.
        CommitsNotFoundForReason,

        /// Index logic is broken, since withdrawing index balance is only for higher
        /// level queries and deposits (principal) is only withdrawn.
        IndexBalanceUnderflow,

        /// There exists an invalid commit-variant [`Config::Position`] via invalid
        /// trait implementation of [`PositionIndex`].
        ///
        /// Indicates the position cannot be derived from the positional index or
        /// that the index doesn't pertain to the actual position.
        InvalidCommitVariantIndex,

        /// This pallet instance's maximum commitment per proprietor configuration is
        /// set at zero, effectively restricts the commitment-pallet's operations.
        ///
        /// Require [`Config::MaxCommits`] to be set to more than zero to operate.
        ZeroMaxCommits,

        /// This pallet instance's index and pool support is halted via setting
        /// [`Config::MaxIndexEntries`] to zero and tried attempting to create an
        /// index (and in future pools via indexes).
        ///
        /// If indexes and pools are required [`Config::MaxIndexEntries`] should be set
        /// to more than zero.
        TriedCreatingHaltedIndexes,

        /// Attempted to create index via empty entries. Indexes require
        /// valid entries, with non-zero share values.
        ///
        /// It is to note that pools are created via existing
        /// indexes and pool slots are mutated via valid individual entries.
        EmptyEntriesNotAllowed,

        /// A proprietor's commit is found to be empty without any commit-instance
        /// which is an invalid state.
        EmptyCommitsNotAllowed,

        /// Attempted to insert a duplicate entry into an index.
        DuplicateEntry,

        /// Attempted to insert a duplicate slot into a pool.
        DuplicateSlot,

        /// For reasons unknown, the commit-instance construction has failed.
        CommitConstructionFailed,

        /// The required entry's commit for proprietor is not found while raising
        /// commit.
        EntryCommitNotFound,

        /// The balance plugin was corrupted
        CorruptedPlugin,

        /// During Division Scaling the value underflowed, which is fallible only
        /// if the accuracy (denominator) is invalid.
        DerivedLessThanZeroValue,

        /// Withdraw derivation overflowed during index or pool deposit operation.  
        ///
        /// Occurs when an excessively high `share/capital` ratio, multiplied by
        /// a balance value, overflows the scalar.
        WithdrawalOverflow,

        /// Derived Commission Amount Overflowed the Scalar Asset Type.
        CommissionOverflow,

        /// Pools are unsupported when the underlying balance plugin cannot
        /// guarantee precision-exact and forceful (unbounded) operations.
        ///
        /// Pools maintain their own top-level balance and rely on exact value
        /// propagation and forced execution to remain consistent. If the plugin
        /// enforces limits even under forced execution, these requirements cannot
        /// be satisfied, making pool semantics invalid.
        PoolUnsupported,

        /// Direct Digest Minting exceeded allowed limits by the underlying
        /// Lazy Balance Plugin Family for the current operation.
        MintingOffLimits,

        /// Direct Digest Reaping (Burning) exceeded allowed limits by the
        /// underlying Lazy Balance Plugin Family for the current operation.
        ReapingOffLimits,

        /// Placing a new commitment has exceeded allowed deposit limits by the
        /// underlying Lazy Balance Plugin Family for the current operation.
        PlacingOffLimits,

        /// Increasing (raising) an existing commitment has exceeded allowed
        /// raising (deposit) limits by the underlying Lazy Balance Plugin Family
        /// for the current operation.
        RaisingOffLimits,
    }

    // ===============================================================================
    // ```````````````````````````````````` EVENTS ```````````````````````````````````
    // ===============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// Emitted when a proprietor places a new commit on a
        /// digest with a specific variant.
        CommitPlaced {
            who: Proprietor<T>,
            reason: CommitReason<T, I>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            model: DigestVariant<T, I>,
            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            digest: Digest<T>,
            value: AssetOf<T, I>,
            variant: T::Position,
        },

        /// Emitted when an existing commit for a digest is
        /// increased or raised.
        CommitRaised {
            who: Proprietor<T>,
            reason: CommitReason<T, I>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            model: DigestVariant<T, I>,
            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            digest: Digest<T>,
            value: AssetOf<T, I>,
        },

        /// Emitted when a commit is resolved (finalized/withdrawn)
        /// for a digest.
        CommitResolved {
            who: Proprietor<T>,
            reason: CommitReason<T, I>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            model: DigestVariant<T, I>,
            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            digest: Digest<T>,
            value: AssetOf<T, I>,
        },

        /// Emitted when querying the current committed value
        /// for a specific digest and reason.
        CommitValue {
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            model: DigestVariant<T, I>,
            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            digest: Digest<T>,
            reason: CommitReason<T, I>,
            value: AssetOf<T, I>,
        },

        /// Emitted when the effective value of the digest
        /// variant is updated.
        DigestInfo {
            digest: Digest<T>,
            reason: CommitReason<T, I>,
            value: AssetOf<T, I>,
            variant: T::Position,
        },

        /// Emitted when a direct-digest is reaped (removed)
        /// after all commitments are cleared from it.
        ///
        /// `dust` represents unclaimable dead asset value.
        DigestReaped {
            digest: Digest<T>,
            reason: CommitReason<T, I>,
            dust: AssetOf<T, I>,
        },

        /// Emitted when a new index of variants is initialized.
        /// Contains all entries (digests, sharesa and variant) of the index.
        IndexInitialized {
            index_of: IndexDigest<T>,
            reason: CommitReason<T, I>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            entries: Vec<(EntryDigest<T>, T::Shares, T::Position)>,
        },

        /// Emitted when querying the total value of an index
        /// for a specific proprietor.
        IndexValue {
            index_of: IndexDigest<T>,
            reason: CommitReason<T, I>,
            value: AssetOf<T, I>,
        },

        /// Emitted when querying the value of a specific entry
        /// within an index.
        IndexEntryValue {
            index_of: IndexDigest<T>,
            reason: CommitReason<T, I>,
            entry_of: Digest<T>,
            value: AssetOf<T, I>,
        },

        /// Emitted when querying the values of all entries
        /// within an index.
        IndexEntriesValue {
            index_of: IndexDigest<T>,
            reason: CommitReason<T, I>,
            entries: Vec<(EntryDigest<T>, AssetOf<T, I>)>,
        },

        /// Emitted when a index is reaped (removed)
        /// after all entry commitments are cleared from it.
        IndexReaped {
            index_of: IndexDigest<T>,
            reason: CommitReason<T, I>,
        },

        /// Emitted when a pool's manager is set or updated.
        /// The manager is responsible for managing slots
        /// and internal pool operations.
        PoolManager {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            manager: Proprietor<T>,
        },

        /// Emitted when a new pool is initialized from an index.
        /// Includes the commission rate and initial slots with their
        /// associated shares and variants.
        PoolInitialized {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            commission: T::Commission,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            slots: Vec<(SlotDigest<T>, T::Shares, T::Position)>,
        },

        /// Emitted when a slot within a pool has its variant updated or a
        /// new slot is added.
        PoolSlot {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            slot_of: SlotDigest<T>,
            variant: T::Position,
            shares: T::Shares,
        },

        /// Emitted when querying the total value of a pool
        /// for a specific proprietor.
        PoolValue {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            value: AssetOf<T, I>,
        },

        /// Emitted when querying the value of a specific slot
        /// within a pool.
        PoolSlotValue {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            slot_of: SlotDigest<T>,
            value: AssetOf<T, I>,
        },

        /// Emitted when querying the values of all slots
        /// within a pool.
        PoolSlotsValue {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            slots: Vec<(SlotDigest<T>, AssetOf<T, I>)>,
        },

        /// Emitted when querying or updating the commission
        /// rate of a pool.
        PoolCommission {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            commission: T::Commission,
        },

        /// Emitted when funds are deposited into reserve (held balance)
        /// under the prepare-for-commit reason.
        ReserveDeposited {
            amount: AssetOf<T, I>,
            total_on_hold: AssetOf<T, I>,
        },

        /// Emitted when reserved funds are withdrawn back to free balance.
        ReserveWithdrawn {
            amount: AssetOf<T, I>,
            total_on_hold: AssetOf<T, I>,
        },

        /// Emitted when a pool is reaped (removed)
        /// after all slot commitments are cleared from it.
        PoolReaped {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
        },

        /// Emitted when a pool slot is removed due to its shares being zero.
        PoolSlotRemoved {
            pool_of: PoolDigest<T>,
            reason: CommitReason<T, I>,
            slot_of: SlotDigest<T>,
            variant: T::Position,
        },

        /// Emitted when determining the digest model
        /// (Direct, Index, or Pool) for a given digest.
        DigestModel { digest: DigestVariant<T, I> },

        /// Emitted when the total assets pending issuance are queried.
        AssetIssuable { asset: AssetOf<T, I> },

        /// Emitted when the total assets pending reaping are queried.
        AssetReapable { asset: AssetOf<T, I> },

        /// Emitted when the total committed value for a reason is queried.
        ReasonValuation {
            reason: CommitReason<T, I>,
            value: AssetOf<T, I>,
        },
    }

    // ===============================================================================
    // `````````````````````````````````` EXTRINSICS `````````````````````````````````
    // ===============================================================================

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ```````````````````````````````` DISPATCHABLES ````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Deposits funds from free balance into reserve for future commitments.
        ///
        /// Locks the specified amount under the [`HoldReason::PrepareForCommit`] hold reason.
        ///
        /// These funds remain available for placing or raising commitments until explicitly
        /// withdrawn via [`Pallet::withdraw_reserve`].
        ///
        /// ### Behavior
        /// - If `precision` is `BestEffort`, deposits the maximum available balance when insufficient
        /// - If `precision` is `Exact`, requires exact amount or fails with [`Error::InsufficientFunds`]
        ///
        /// ### Emits
        /// [`Event::ReserveDeposited`]: Contains the amount deposited and the total balance on hold.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::deposit_reserve())]
        pub fn deposit_reserve(
            origin: OriginFor<T>,
            amount: AssetOf<T, I>,
            precision: PrecisionWrapper,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let hold_reason: T::AssetHold = HoldReason::PrepareForCommit.into();
            let reducible_balance = <T as Config<I>>::Asset::reducible_balance(
                &caller,
                Preservation::Preserve,
                Fortitude::Polite,
            );
            if reducible_balance < amount {
                if precision == PrecisionWrapper::Exact {
                    return Err(Error::<T, I>::InsufficientFunds.into());
                }
                <T as Config<I>>::Asset::decrease_balance(
                    &caller,
                    reducible_balance,
                    Precision::Exact,
                    Preservation::Preserve,
                    Fortitude::Polite,
                )?;
                <T as Config<I>>::Asset::increase_balance_on_hold(
                    &hold_reason,
                    &caller,
                    reducible_balance,
                    Precision::Exact,
                )?;
                let total_on_hold = <T as Config<I>>::Asset::balance_on_hold(&hold_reason, &caller);
                Self::deposit_event(Event::<T, I>::ReserveDeposited {
                    amount: reducible_balance,
                    total_on_hold: total_on_hold,
                });
                return Ok(());
            }
            <T as Config<I>>::Asset::decrease_balance(
                &caller,
                amount,
                Precision::Exact,
                Preservation::Preserve,
                Fortitude::Force,
            )?;
            <T as Config<I>>::Asset::increase_balance_on_hold(
                &hold_reason,
                &caller,
                amount,
                Precision::Exact,
            )?;
            let total_on_hold = <T as Config<I>>::Asset::balance_on_hold(&hold_reason, &caller);
            Self::deposit_event(Event::<T, I>::ReserveDeposited {
                amount: amount,
                total_on_hold: total_on_hold,
            });
            Ok(())
        }

        /// Withdraws reserved funds back to the caller's free balance.
        ///
        /// Releases funds held under the [`HoldReason::PrepareForCommit`] reason and
        /// returns them to the caller's free balance.
        ///
        /// ### Behavior
        /// - If `amount` is `None`, all reserved funds under the hold reason are released.
        /// - If `amount` is `Some(value)`, only the specified amount is released,
        ///   leaving any remaining reserved funds intact.
        ///
        /// This call decreases the balance on hold with `Precision::Exact` and
        /// increases the caller's free balance by the same amount.
        ///
        /// Returns [`Error::InsufficientFunds`] if a specific `amount` is provided
        /// and the held balance is less than the requested amount.
        ///
        /// ### Emits
        /// [`Event::ReserveWithdrawn`]: Contains the amount withdrawn and the amount balance on hold.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::withdraw_reserve()
            .max(T::WeightInfo::withdraw_reserve_partial())
        )]
        pub fn withdraw_reserve(
            origin: OriginFor<T>,
            amount: Option<AssetOf<T, I>>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let hold_reason: T::AssetHold = HoldReason::PrepareForCommit.into();
            let hold_balance = <T as Config<I>>::Asset::balance_on_hold(&hold_reason, &caller);
            match amount {
                None => {
                    <T as Config<I>>::Asset::decrease_balance_on_hold(
                        &hold_reason,
                        &caller,
                        hold_balance,
                        Precision::Exact,
                    )?;
                    <T as Config<I>>::Asset::increase_balance(
                        &caller,
                        hold_balance,
                        Precision::Exact,
                    )?;
                    let total_on_hold =
                        <T as Config<I>>::Asset::balance_on_hold(&hold_reason, &caller);
                    Self::deposit_event(Event::<T, I>::ReserveWithdrawn {
                        amount: hold_balance,
                        total_on_hold: total_on_hold,
                    });
                }
                Some(amount) => {
                    if hold_balance < amount {
                        return Err(Error::<T, I>::InsufficientFunds.into());
                    }
                    <T as Config<I>>::Asset::decrease_balance_on_hold(
                        &hold_reason,
                        &caller,
                        amount,
                        Precision::Exact,
                    )?;
                    <T as Config<I>>::Asset::increase_balance(&caller, amount, Precision::Exact)?;
                    let total_on_hold =
                        <T as Config<I>>::Asset::balance_on_hold(&hold_reason, &caller);
                    Self::deposit_event(Event::<T, I>::ReserveWithdrawn {
                        amount: amount,
                        total_on_hold: total_on_hold,
                    });
                }
            }
            Ok(())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ````````````````````````````````` INSPECTORS ``````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Queries the current value of a proprietor's commitment.
        ///
        /// Returns the real-time committed amount for the caller's active commitment under
        /// the specified reason. This value reflects any changes to the underlying digest
        /// value since the commitment was placed, as digest values can be updated.
        ///
        /// ### Emits
        /// [`Event::CommitValue`]: Contains the current commitment value
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::inspect_commit_value())]
        pub fn inspect_commit_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let commit_value = Self::query_commit_value(caller.clone(), reason)?;
            let digest_model = Self::resolve_digest_model_for(caller.clone(), reason)?;
            Self::deposit_event(Event::<T, I>::CommitValue {
                model: digest_model,
                reason: reason,
                value: commit_value,
            });
            Ok(())
        }

        /// Determines the digest model classification for a given digest.
        ///
        /// Queries whether the specified digest exists as a direct digest, index, or pool
        /// under the given reason. The result is wrapped in a [`DigestVariant`] and emitted
        /// via the [`Event::DigestModel`] event.
        ///
        /// ### Emits
        /// [`Event::DigestModel`]: Contains the resolved digest variant
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::inspect_digest_model())]
        pub fn inspect_digest_model(
            origin: OriginFor<T>,
            digest: Digest<T>,
            reason: CommitReason<T, I>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let digest_variant = Self::resolve_digest_model(digest, reason)?;
            Self::deposit_event(Event::<T, I>::DigestModel {
                digest: digest_variant,
            });
            Ok(())
        }

        /// Queries the total value of a proprietor's commitment to an index.
        ///
        /// Aggregates all entry values within the index, weighted by their respective shares,
        /// to compute the proprietor's total exposure. Each entry's value is computed in
        /// real-time, reflecting any changes since commitment.
        ///
        /// ### Emits
        /// [`Event::IndexValue`]: Contains the total index commitment value
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::inspect_index_value())]
        pub fn inspect_index_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            index_of: IndexDigest<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let index_value =
                Self::query_index_value_for(caller, reason, index_of.clone())?;
            Self::deposit_event(Event::<T, I>::IndexValue {
                index_of: index_of,
                reason: reason,
                value: index_value,
            });
            Ok(())
        }

        /// Queries the values of all entries within an index.
        ///
        /// Returns a vector of (entry_digest, value) pairs showing how the proprietor's
        /// total index commitment is distributed across all entries. Each value is weighted
        /// by its entry's share and computed in real-time.
        ///
        /// ### Emits
        /// [`Event::IndexEntriesValue`]: Contains the vector of entry-value pairs
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::inspect_entries_value())]
        pub fn inspect_entries_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            index_of: IndexDigest<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let entries_value =
                Self::query_entries_value_for(caller, reason, index_of.clone())?;
            Self::deposit_event(Event::<T, I>::IndexEntriesValue {
                index_of: index_of,
                reason: reason,
                entries: entries_value,
            });
            Ok(())
        }

        /// Queries the value of a specific entry within an index.
        ///
        /// Returns the portion of the proprietor's index commitment allocated to this
        /// particular entry, weighted by its share within the index. The value is computed
        /// in real-time, reflecting any changes to the underlying entry digest since commitment.
        ///
        /// ### Emits
        /// [`Event::IndexEntryValue`]: Contains the entry's commitment value
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::inspect_entry_value())]
        pub fn inspect_entry_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            index_of: IndexDigest<T>,
            entry_of: Digest<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let entry_value = Self::query_entry_value_for(
                caller,
                reason,
                index_of.clone(),
                entry_of.clone(),
            )?;
            Self::deposit_event(Event::<T, I>::IndexEntryValue {
                index_of: index_of,
                reason: reason,
                entry_of: entry_of,
                value: entry_value,
            });
            Ok(())
        }

        /// Queries the total value of a proprietor's commitment to a pool.
        ///
        /// Aggregates all slot values within the pool, weighted by their respective shares
        /// and accounting for the pool's commission rate.
        ///
        /// ### Emits
        /// [`Event::PoolValue`]: Contains the total pool commitment value
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::inspect_pool_value())]
        pub fn inspect_pool_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            pool_of: PoolDigest<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let pool_value = Self::query_pool_value_for(caller, reason, pool_of.clone())?;
            Self::deposit_event(Event::<T, I>::PoolValue {
                pool_of: pool_of,
                reason: reason,
                value: pool_value,
            });
            Ok(())
        }

        /// Query the values of all slots within a pool.
        ///
        /// Returns a vector of (slot_digest, value) pairs showing how the proprietor's
        /// total pool commitment is distributed across all pool slots.
        ///
        /// ### Emits
        /// [`Event::PoolSlotsValue`]: Contains the vector of slot-value pairs
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::inspect_slots_value())]
        pub fn inspect_slots_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            pool_of: PoolDigest<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let slots_value = Self::query_slots_value_for(caller, reason, pool_of.clone())?;
            Self::deposit_event(Event::<T, I>::PoolSlotsValue {
                pool_of: pool_of,
                reason: reason,
                slots: slots_value,
            });
            Ok(())
        }

        /// Queries the value of a specific slot within a pool.
        ///
        /// Returns the portion of the proprietor's pool commitment allocated to this
        /// particular slot, weighted by its share within the pool.
        ///
        /// ### Emits
        /// [`Event::PoolSlotValue`]: Contains the slot's commitment value
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::inspect_slot_value())]
        pub fn inspect_slot_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            pool_of: PoolDigest<T>,
            slot_of: Digest<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let slot_value = Self::query_slot_value_for(
                caller,
                reason,
                pool_of.clone(),
                slot_of.clone(),
            )?;
            Self::deposit_event(Event::<T, I>::PoolSlotValue {
                pool_of: pool_of,
                reason: reason,
                slot_of: slot_of,
                value: slot_value,
            });
            Ok(())
        }

        /// Queries a pool's commission rate.
        ///
        /// Returns the percentage of withdrawals that the pool manager
        /// collects as commission. Commission rates are immutable after pool creation to
        /// protect depositors' economic expectations.
        ///
        /// ### Emits
        /// [`Event::PoolCommission`]: Contains the commission rate.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::inspect_pool_commission())]
        pub fn inspect_pool_commission(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            pool_of: PoolDigest<T>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let commission = Self::query_pool_commission(reason, pool_of.clone())?;
            Self::deposit_event(Event::<T, I>::PoolCommission {
                pool_of: pool_of,
                reason: reason,
                commission: commission,
            });
            Ok(())
        }

        /// Queries a pool's manager account.
        ///
        /// Returns the account responsible for managing the pool's operations, including
        /// slot configuration, share adjustments, and commission collection.
        ///
        /// ### Emits
        /// - [`Event::PoolManager`]: Contains the manager's account identifier
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::inspect_pool_manager())]
        pub fn inspect_pool_manager(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
            pool_of: PoolDigest<T>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let manager = Self::query_pool_manager(reason, pool_of.clone())?;
            Self::deposit_event(Event::<T, I>::PoolManager {
                pool_of: pool_of,
                reason: reason,
                manager: manager,
            });
            Ok(())
        }

        /// Queries the total amount of assets currently recorded as pending issuance.
        ///
        /// This value reflects the amount tracked in [`AssetToIssue`], representing
        /// assets that have been accounted for as "to be minted" but may not yet be
        /// reflected in the underlying asset system due to lazy execution.
        ///
        /// The returned value is purely an **accounting snapshot** and does not
        /// guarantee that minting has already occurred.
        ///
        /// ### Emits
        /// [`Event::AssetIssuable`]: Contains the total pending issuance amount.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::inspect_asset_to_issue())]
        pub fn inspect_asset_to_issue(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed(origin)?;
            let asset = AssetToIssue::<T, I>::get();
            Self::deposit_event(Event::<T, I>::AssetIssuable { asset });
            Ok(())
        }

        /// Queries the total amount of assets currently recorded as pending reaping.
        ///
        /// This value reflects the amount tracked in [`AssetToReap`], representing
        /// assets that have been accounted for as "to be removed" but may not yet be
        /// reflected in the underlying asset system due to lazy execution.
        ///
        /// The returned value is purely an **accounting snapshot** and does not
        /// guarantee that reaping (burn/removal) has already occurred.
        ///
        /// ### Emits
        /// [`Event::AssetReapable`]: Contains the total pending reaping amount.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::inspect_asset_to_reap())]
        pub fn inspect_asset_to_reap(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed(origin)?;
            let asset = AssetToReap::<T, I>::get();
            Self::deposit_event(Event::<T, I>::AssetReapable { asset });
            Ok(())
        }

        /// Queries the total committed asset value for the specified [`CommitReason`].
        ///
        /// This value is read directly from [`ReasonValue`] and represents the
        /// aggregated committed amount across all digests and variants associated
        /// with the given reason.
        ///
        /// The returned value:
        /// - **Includes** assets that are accounted for in commitments (including those pending issuance)
        /// - **Excludes** assets pending reaping, as they are not considered part of active committed value
        ///
        /// If no value exists for the given reason, the storage returns `Zero`,
        /// which is emitted as-is in the event.
        ///
        /// ### Emits
        /// [`Event::ReasonValuation`]: Contains the queried committed value for the reason.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::inspect_reason_value())]
        pub fn inspect_reason_value(
            origin: OriginFor<T>,
            reason: CommitReason<T, I>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let value = ReasonValue::<T, I>::get(reason).unwrap_or(Zero::zero());
            Self::deposit_event(Event::<T, I>::ReasonValuation { reason, value });
            Ok(())
        }
    }

    // ===============================================================================
    // ````````````````````````````````` PUBLIC APIS `````````````````````````````````
    // ===============================================================================

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ``````````````````````````````````` GENERAL ```````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Resolves the [`DigestVariant`] for a given digest and commit reason.
        ///
        /// The returned variant defines how the digest is interpreted
        /// within the commitment system (Direct, Index or Pool).
        pub fn resolve_digest_model(
            digest: Digest<T>,
            reason: CommitReason<T, I>,
        ) -> Result<DigestVariant<T, I>, DispatchError> {
            let digest_variant =
                <Pallet<T, I> as DigestModel<Proprietor<T>>>::determine_digest(&digest, &reason)?;
            Ok(digest_variant)
        }

        /// Resolves the digest variant of caller's active commitment under `reason`.
        ///
        /// Retrieves the commitment digest currently associated with the `caller`
        /// for the specified `reason`, then determines its classification within
        /// the commitment system.
        ///
        /// The returned [`DigestVariant`] indicates whether the commitment
        /// is a Direct, Index, or Pool type.
        pub fn resolve_digest_model_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
        ) -> Result<DigestVariant<T, I>, DispatchError> {
            let digest = Self::get_commit_digest(&caller, &reason)?;
            let digest_variant =
                <Pallet<T, I> as DigestModel<Proprietor<T>>>::determine_digest(&digest, &reason)?;
            Ok(digest_variant)
        }

        /// Returns the total value committed by `caller` under
        /// a `reason`.
        ///
        /// This represents the caller's full active commitment
        /// for the specified reason.
        pub fn query_commit_value(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let commit_value =
                <Pallet<T, I> as Commitment<Proprietor<T>>>::get_commit_value(&caller, &reason)?;
            Ok(commit_value)
        }

        /// Returns the total amount of assets currently recorded as pending issuance.
        ///
        /// The returned value is an accounting value only. It does not guarantee
        /// that the underlying asset system has already minted the assets.
        pub fn query_asset_to_issue() -> AssetOf<T, I> {
            AssetToIssue::<T, I>::get()
        }

        /// Returns the total amount of assets currently recorded as pending reaping.
        ///
        /// The returned value is an accounting value only. It does not guarantee
        /// that the underlying asset system has already reaped, burned, or removed
        /// the assets.
        pub fn query_asset_to_reap() -> AssetOf<T, I> {
            AssetToReap::<T, I>::get()
        }

        /// Returns the total committed asset value recorded for the given `reason`.
        ///
        /// This value is read directly from [`ReasonValue`] and represents the
        /// aggregated committed value across all digests and variants associated
        /// with the specified reason.
        ///
        /// Returns `Zero` if no committed value is currently stored for the reason.
        pub fn query_reason_value(reason: CommitReason<T, I>) -> AssetOf<T, I> {
            ReasonValue::<T, I>::get(reason).unwrap_or(Zero::zero())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ```````````````````````````````` INDEX (GLOBAL) ```````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Returns the total value committed to an `index`
        /// under a `reason`.
        ///
        /// The returned value is the sum of all entry
        /// commitments from all accounts within the index.
        pub fn query_index_value(
            reason: CommitReason<T, I>,
            index: IndexDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let index_value =
                <Pallet<T, I> as CommitIndex<Proprietor<T>>>::get_index_value(&reason, &index)?;
            Ok(index_value)
        }

        /// Returns the total committed value of every `entry`
        /// within an `index` under a `reason`.
        ///
        /// Each tuple contains:
        /// - Entry digest
        /// - Aggregated value committed to that entry
        pub fn query_entries_value(
            reason: CommitReason<T, I>,
            index: IndexDigest<T>,
        ) -> Result<Vec<(Digest<T>, AssetOf<T, I>)>, DispatchError> {
            let entries_value =
                <Pallet<T, I> as CommitIndex<Proprietor<T>>>::get_entries_value(&reason, &index)?;
            Ok(entries_value)
        }

        /// Returns the total value committed to an `entry` digest
        /// within an `index` under a `reason`.
        ///
        /// The returned value is aggregated across all accounts
        /// that have committed to the entry.
        pub fn query_entry_value(
            reason: CommitReason<T, I>,
            index: IndexDigest<T>,
            entry: EntryDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let entry_value = <Pallet<T, I> as CommitIndex<Proprietor<T>>>::get_entry_value(
                &reason, &index, &entry,
            )?;
            Ok(entry_value)
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // `````````````````````````````` INDEX (PER CALLER) `````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Returns the total value committed by `caller`
        /// to an `index` under a `reason`.
        pub fn query_index_value_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
            index: IndexDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let index_value = <Pallet<T, I> as CommitIndex<Proprietor<T>>>::get_index_value_for(
                &caller, &reason, &index,
            )?;
            Ok(index_value)
        }

        /// Returns the caller's committed value for each `entry`
        /// within an `index` under `reason`.
        ///
        /// Each tuple contains:
        /// - Entry digest
        /// - Value committed by the caller to that entry
        pub fn query_entries_value_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
            index: IndexDigest<T>,
        ) -> Result<Vec<(Digest<T>, AssetOf<T, I>)>, DispatchError> {
            let entries_value =
                <Pallet<T, I> as CommitIndex<Proprietor<T>>>::get_entries_value_for(
                    &caller, &reason, &index,
                )?;
            Ok(entries_value)
        }

        /// Returns the value committed by `caller`
        /// to an `entry` within an `index` under a `reason`.
        pub fn query_entry_value_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
            index: IndexDigest<T>,
            entry: EntryDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let entry_value = <Pallet<T, I> as CommitIndex<Proprietor<T>>>::get_entry_value_for(
                &caller, &reason, &index, &entry,
            )?;
            Ok(entry_value)
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ```````````````````````````````` POOL (GLOBAL) ````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Returns the total value committed to a `pool` under a `reason`.
        ///
        /// The returned value is the sum of all slot commitments
        /// from all accounts within the pool.
        pub fn query_pool_value(
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let pool_of =
                <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_pool_value(&reason, &pool)?;
            Ok(pool_of)
        }

        /// Returns the total committed value of every `slot`
        /// within a `pool` under a `reason`.
        ///
        /// Each tuple contains:
        /// - Slot digest
        /// - Aggregated value committed to that slot
        pub fn query_slots_value(
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
        ) -> Result<Vec<(Digest<T>, AssetOf<T, I>)>, DispatchError> {
            let slots_value =
                <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_slots_value(&reason, &pool)?;
            Ok(slots_value)
        }

        /// Returns the total value committed to a `slot`
        /// within a `pool` under a `reason`.
        ///
        /// The returned value is aggregated across all accounts
        /// that have committed to the slot.
        pub fn query_slot_value(
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
            slot: SlotDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let slot_value =
                <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_slot_value(&reason, &pool, &slot)?;
            Ok(slot_value)
        }

        /// Returns the commission rate configured for a `pool` under a `reason`.
        pub fn query_pool_commission(
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
        ) -> Result<T::Commission, DispatchError> {
            let commission =
                <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_commission(&reason, &pool)?;
            Ok(commission)
        }

        /// Returns the current manager account for a `pool` under a  `reason`.
        ///
        /// The returned account is the authority responsible for
        /// managing the pool's configuration and slot definitions.
        pub fn query_pool_manager(
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
        ) -> Result<T::AccountId, DispatchError> {
            let manager = <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_manager(&reason, &pool)?;
            Ok(manager)
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // `````````````````````````````` INDEX (PER CALLER) `````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Returns the total value committed by `caller`
        /// to a `pool` under a `reason`.
        pub fn query_pool_value_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let pool_value = <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_pool_value_for(
                &caller, &reason, &pool,
            )?;
            Ok(pool_value)
        }

        /// Returns the caller's committed value for each `slot`
        /// within a `pool` under a `reason`.
        ///
        /// Each tuple contains:
        /// - Slot digest
        /// - Value committed by the caller to that slot
        pub fn query_slots_value_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
            pool_of: PoolDigest<T>,
        ) -> Result<Vec<(Digest<T>, AssetOf<T, I>)>, DispatchError> {
            let slots_value = <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_slots_value_for(
                &caller, &reason, &pool_of,
            )?;
            Ok(slots_value)
        }

        /// Returns the value committed by a `caller`
        /// to  a `slot` within a `pool` under a `reason`.
        pub fn query_slot_value_for(
            caller: T::AccountId,
            reason: CommitReason<T, I>,
            pool: PoolDigest<T>,
            slot: SlotDigest<T>,
        ) -> Result<AssetOf<T, I>, DispatchError> {
            let slot_value = <Pallet<T, I> as CommitPool<Proprietor<T>>>::get_slot_value_for(
                &caller, &reason, &pool, &slot,
            )?;
            Ok(slot_value)
        }
    }
}

// ===============================================================================
// `````````````````````````````````` API TESTS ``````````````````````````````````
// ===============================================================================

#[cfg(test)]
/// Unit tests for Extrinsics and Public APIs of [`pallet_commitment`](crate).
mod ext_tests {
        
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::{mock::*, types::PrecisionWrapper};

    // --- FRAME Suite ---
    use frame_suite::{commitment::*, misc::Directive};

    // --- FRAME Support ---
    use frame_support::{
        assert_err, assert_ok,
        pallet_prelude::DispatchError,
        traits::{
            fungible::{Inspect, InspectHold},
            tokens::{Fortitude, Precision},
        },
    };

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````` EXTRINSIC TESTS ```````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn deposit_reserve_success_exact() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(2);
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            Pallet::deposit_reserve(RuntimeOrigin::signed(ALICE), 10, PrecisionWrapper::Exact)
                .unwrap();
            // balance check
            let actual_balance = AssetOf::balance(&ALICE);
            let actual_hold_balance = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance = 10;
            let expected_hold_balance = 30;
            assert_eq!(actual_balance, expected_balance);
            assert_eq!(actual_hold_balance, expected_hold_balance);
            System::assert_last_event(
                Event::ReserveDeposited {
                    amount: 10,
                    total_on_hold: actual_hold_balance,
                }
                .into(),
            );
        })
    }

    #[test]
    fn deposit_reserve_success_best_efforts() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(2);
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            Pallet::deposit_reserve(
                RuntimeOrigin::signed(ALICE),
                25,
                PrecisionWrapper::BestEffort,
            )
            .unwrap();
            // balance check
            let actual_balance = AssetOf::balance(&ALICE);
            let actual_hold_balance = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance = 0;
            let expected_hold_balance = 40;
            assert_eq!(actual_balance, expected_balance);
            assert_eq!(actual_hold_balance, expected_hold_balance);
            System::assert_last_event(
                Event::ReserveDeposited {
                    amount: 20,
                    total_on_hold: actual_hold_balance,
                }
                .into(),
            );
        })
    }

    #[test]
    fn deposit_reserve_err_insufficient_funds() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            assert_err!(
                Pallet::deposit_reserve(
                    RuntimeOrigin::signed(ALICE),
                    25,
                    PrecisionWrapper::Exact
                ),
                Error::InsufficientFunds
            );
        })
    }

    #[test]
    fn withdraw_reserve_success() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(2);
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            Pallet::withdraw_reserve(RuntimeOrigin::signed(ALICE), None).unwrap();
            // balance check
            let actual_balance = AssetOf::balance(&ALICE);
            let actual_hold_balance = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance = 40;
            let expected_hold_balance = 0;
            assert_eq!(actual_balance, expected_balance);
            assert_eq!(actual_hold_balance, expected_hold_balance);
            System::assert_last_event(
                Event::ReserveWithdrawn {
                    amount: 20,
                    total_on_hold: 0,
                }
                .into(),
            );
        })
    }

    #[test]
    fn withdraw_reserve_err_insufficient_funds() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            assert_err!(
                Pallet::withdraw_reserve(RuntimeOrigin::signed(ALICE), Some(25)),
                Error::InsufficientFunds
            );
        })
    }

    #[test]
    fn withdraw_reserve_partial_success() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(2);
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            Pallet::withdraw_reserve(RuntimeOrigin::signed(ALICE), Some(15)).unwrap();
            // balance check
            let actual_balance = AssetOf::balance(&ALICE);
            let actual_hold_balance = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance = 35;
            let expected_hold_balance = 5;
            assert_eq!(actual_balance, expected_balance);
            assert_eq!(actual_hold_balance, expected_hold_balance);
            System::assert_last_event(
                Event::ReserveWithdrawn {
                    amount: 15,
                    total_on_hold: 5,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_digest_model_direct_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_DIGEST,
                15,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::inspect_digest_model(RuntimeOrigin::signed(ALICE), ALPHA_DIGEST, STAKING)
                .unwrap();
            System::assert_last_event(
                Event::DigestModel {
                    digest: DigestVariant::Direct(ALPHA_DIGEST),
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_digest_model_index_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            initiate_digest_with_default_balance(STAKING, ALPHA_ENTRY_DIGEST).unwrap();
            System::set_block_number(4);
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[(ALPHA_ENTRY_DIGEST, 40)],
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &ALPHA_INDEX_DIGEST));
            System::set_block_number(6);
            // Place commit to an index
            let commit_amount = 20;
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_INDEX_DIGEST,
                commit_amount,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            Pallet::inspect_digest_model(
                RuntimeOrigin::signed(ALICE),
                ALPHA_INDEX_DIGEST,
                STAKING,
            )
            .unwrap();
            System::assert_last_event(
                Event::DigestModel {
                    digest: DigestVariant::Index(ALPHA_INDEX_DIGEST),
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_digest_model_pool_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, 30).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let entries = vec![(ALPHA_ENTRY_DIGEST, 40)];
            prepare_and_initiate_pool(
                BOB,
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                COMMISSION_ZERO,
            )
            .unwrap();
            let commit_amount = 25;
            System::set_block_number(6);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                commit_amount,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            Pallet::inspect_digest_model(
                RuntimeOrigin::signed(ALICE),
                ALPHA_POOL_DIGEST,
                STAKING,
            )
            .unwrap();
            System::assert_last_event(
                Event::DigestModel {
                    digest: DigestVariant::Pool(ALPHA_POOL_DIGEST),
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_digest_model_err_bad_origin() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_DIGEST,
                15,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::inspect_digest_model(RuntimeOrigin::root(), ALPHA_DIGEST, STAKING,),
                DispatchError::BadOrigin
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_commit_value_for_direct_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, STANDARD_VALUE).unwrap();
            System::set_block_number(2);
            let commit_amount = 10;
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_DIGEST,
                commit_amount,
                &Directive::new(Precision::BestEffort, Fortitude::Force),
            )
            .unwrap();
            // fetch the commit value
            Pallet::inspect_commit_value(RuntimeOrigin::signed(ALICE), STAKING).unwrap();
            // verify if the data in the event emmission is correct
            System::assert_last_event(
                Event::CommitValue {
                    model: DigestVariant::Direct(ALPHA_DIGEST),
                    reason: STAKING,
                    value: commit_amount,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_commit_value_for_index_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            initiate_digest_with_default_balance(STAKING, ALPHA_ENTRY_DIGEST).unwrap();
            System::set_block_number(4);
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[(ALPHA_ENTRY_DIGEST, 40)],
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &ALPHA_INDEX_DIGEST));
            System::set_block_number(6);
            // Place commit to an index
            let commit_amount = 20;
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_INDEX_DIGEST,
                commit_amount,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            // fetch the commit value
            Pallet::inspect_commit_value(RuntimeOrigin::signed(ALICE), STAKING).unwrap();
            // verify if the data in the event emmission is correct
            System::assert_last_event(
                Event::CommitValue {
                    model: DigestVariant::Index(ALPHA_INDEX_DIGEST),
                    reason: STAKING,
                    value: commit_amount,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_commit_value_for_pool_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, 30).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let entries = vec![(ALPHA_ENTRY_DIGEST, 40)];
            prepare_and_initiate_pool(
                BOB,
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                COMMISSION_ZERO,
            )
            .unwrap();
            let commit_amount = 25;
            System::set_block_number(6);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                commit_amount,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            // fetch the commit value
            Pallet::inspect_commit_value(RuntimeOrigin::signed(BOB), STAKING).unwrap();
            // verify if the data in the event emmission is correct
            System::assert_last_event(
                Event::CommitValue {
                    model: DigestVariant::Pool(ALPHA_POOL_DIGEST),
                    reason: STAKING,
                    value: commit_amount,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_commit_value_err_bad_origin() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_DIGEST,
                15,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::inspect_commit_value(RuntimeOrigin::root(), STAKING),
                DispatchError::BadOrigin
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_index_value_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, 40).unwrap();
            initiate_digest_with_default_balance(STAKING, ALPHA_ENTRY_DIGEST).unwrap();
            initiate_digest_with_default_balance(STAKING, BETA_ENTRY_DIGEST).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[(ALPHA_ENTRY_DIGEST, 40), (BETA_ENTRY_DIGEST, 60)],
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_INDEX_DIGEST,
                35,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_index_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::IndexValue {
                    index_of: ALPHA_INDEX_DIGEST,
                    reason: STAKING,
                    value: 35,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_entry_value_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, 40).unwrap();
            initiate_digest_with_default_balance(STAKING, ALPHA_ENTRY_DIGEST).unwrap();
            initiate_digest_with_default_balance(STAKING, BETA_ENTRY_DIGEST).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[(ALPHA_ENTRY_DIGEST, 40), (BETA_ENTRY_DIGEST, 60)],
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_INDEX_DIGEST,
                35,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_entry_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_INDEX_DIGEST,
                ALPHA_ENTRY_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::IndexEntryValue {
                    index_of: ALPHA_INDEX_DIGEST,
                    reason: STAKING,
                    entry_of: ALPHA_ENTRY_DIGEST,
                    value: 14,
                }
                .into(),
            );

            Pallet::inspect_entry_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_INDEX_DIGEST,
                BETA_ENTRY_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::IndexEntryValue {
                    index_of: ALPHA_INDEX_DIGEST,
                    reason: STAKING,
                    entry_of: BETA_ENTRY_DIGEST,
                    value: 21,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_entries_value_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, 40).unwrap();
            initiate_digest_with_default_balance(STAKING, ALPHA_ENTRY_DIGEST).unwrap();
            initiate_digest_with_default_balance(STAKING, BETA_ENTRY_DIGEST).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[(ALPHA_ENTRY_DIGEST, 40), (BETA_ENTRY_DIGEST, 60)],
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_INDEX_DIGEST,
                35,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_entries_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_INDEX_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::IndexEntriesValue {
                    index_of: ALPHA_INDEX_DIGEST,
                    reason: STAKING,
                    entries: vec![(ALPHA_ENTRY_DIGEST, 14), (BETA_ENTRY_DIGEST, 21)],
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_pool_value_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            System::set_block_number(6);
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &BETA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            let entries = vec![(ALPHA_ENTRY_DIGEST, 60), (BETA_ENTRY_DIGEST, 40)];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                COMMISSION_ZERO,
            )
            .unwrap();

            System::set_block_number(10);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                LARGE_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_pool_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_POOL_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::PoolValue {
                    pool_of: ALPHA_POOL_DIGEST,
                    reason: STAKING,
                    value: 20,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_slot_value_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            System::set_block_number(6);
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &BETA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            let entries = vec![(ALPHA_ENTRY_DIGEST, 60), (BETA_ENTRY_DIGEST, 40)];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                COMMISSION_ZERO,
            )
            .unwrap();

            System::set_block_number(10);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                LARGE_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_slot_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_POOL_DIGEST,
                ALPHA_ENTRY_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::PoolSlotValue {
                    pool_of: ALPHA_POOL_DIGEST,
                    reason: STAKING,
                    slot_of: ALPHA_ENTRY_DIGEST,
                    value: 12,
                }
                .into(),
            );

            Pallet::inspect_slot_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_POOL_DIGEST,
                BETA_ENTRY_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::PoolSlotValue {
                    pool_of: ALPHA_POOL_DIGEST,
                    reason: STAKING,
                    slot_of: BETA_ENTRY_DIGEST,
                    value: 8,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_slots_value_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            System::set_block_number(6);
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &BETA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            let entries = vec![(ALPHA_ENTRY_DIGEST, 60), (BETA_ENTRY_DIGEST, 40)];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                COMMISSION_ZERO,
            )
            .unwrap();

            System::set_block_number(10);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                LARGE_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_slots_value(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_POOL_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::PoolSlotsValue {
                    pool_of: ALPHA_POOL_DIGEST,
                    reason: STAKING,
                    slots: vec![(ALPHA_ENTRY_DIGEST, 12), (BETA_ENTRY_DIGEST, 8)],
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_pool_commission_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            System::set_block_number(6);
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &BETA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            let entries = vec![(ALPHA_ENTRY_DIGEST, 60), (BETA_ENTRY_DIGEST, 40)];
            let init_commission = COMMISSION_HIGH;
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                init_commission,
            )
            .unwrap();

            System::set_block_number(10);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                LARGE_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_pool_commission(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_POOL_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::PoolCommission {
                    pool_of: ALPHA_POOL_DIGEST,
                    reason: STAKING,
                    commission: init_commission,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_pool_manager_success() {
        commit_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &ALPHA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            System::set_block_number(6);
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &BETA_ENTRY_DIGEST,
                STANDARD_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            let entries = vec![(ALPHA_ENTRY_DIGEST, 60), (BETA_ENTRY_DIGEST, 40)];
            let manager = ALICE;
            prepare_and_initiate_pool(
                manager.clone(),
                STAKING,
                &entries,
                ALPHA_INDEX_DIGEST,
                ALPHA_POOL_DIGEST,
                COMMISSION_ZERO,
            )
            .unwrap();

            System::set_block_number(10);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &ALPHA_POOL_DIGEST,
                LARGE_VALUE,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_pool_manager(
                RuntimeOrigin::signed(ALICE),
                STAKING,
                ALPHA_POOL_DIGEST,
            )
            .unwrap();

            System::assert_last_event(
                Event::PoolManager {
                    pool_of: ALPHA_POOL_DIGEST,
                    reason: STAKING,
                    manager: manager,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_asset_to_mint() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(ALICE, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let new_digest_val = 325; // 250 -> 325
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_digest_val,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let expected_issuable = new_digest_val.saturating_sub(STANDARD_COMMIT); // 75
            Pallet::inspect_asset_to_issue(RuntimeOrigin::signed(ALICE)).unwrap();
            System::assert_last_event(
                Event::AssetIssuable {
                    asset: expected_issuable,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_asset_to_reap() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(ALICE, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let new_digest_val = 215; // 250 -> 215
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_digest_val,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let expected_reapable = STANDARD_COMMIT.saturating_sub(new_digest_val); // 35
            Pallet::inspect_asset_to_reap(RuntimeOrigin::signed(ALICE)).unwrap();
            System::assert_last_event(
                Event::AssetReapable {
                    asset: expected_reapable,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn inspect_reason_value() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(ALICE, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();
            initiate_key_and_set_balance_and_hold(BOB, STANDARD_COMMIT, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(ALAN, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                150,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();

            Pallet::inspect_reason_value(RuntimeOrigin::signed(ALICE), STAKING).unwrap();
            System::assert_last_event(
                Event::ReasonValuation {
                    reason: STAKING,
                    value: 150,
                }
                .into(),
            );

            Pallet::place_commit(
                &BOB,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            Pallet::inspect_reason_value(RuntimeOrigin::signed(ALICE), ESCROW).unwrap();
            System::assert_last_event(
                Event::ReasonValuation {
                    reason: ESCROW,
                    value: 250,
                }
                .into(),
            );

            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            Pallet::inspect_reason_value(RuntimeOrigin::signed(ALICE), STAKING).unwrap();
            System::assert_last_event(
                Event::ReasonValuation {
                    reason: STAKING,
                    value: 400,
                }
                .into(),
            );

            Pallet::inspect_reason_value(RuntimeOrigin::signed(ALICE), GOVERNANCE).unwrap();
            System::assert_last_event(
                Event::ReasonValuation {
                    reason: GOVERNANCE,
                    value: 0,
                }
                .into(),
            );
        })
    }

    #[test]
    fn query_asset_to_mint() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(ALICE, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let new_digest_val = 325; // 250 -> 325
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_digest_val,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let expected_issuable = new_digest_val.saturating_sub(STANDARD_COMMIT); // 75
            let actual_issuable = Pallet::query_asset_to_issue();
            assert_eq!(expected_issuable, actual_issuable);
        })
    }

    #[test]
    fn query_asset_to_reap() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(ALICE, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let new_digest_val = 215; // 250 -> 215
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_digest_val,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let expected_reapable = STANDARD_COMMIT.saturating_sub(new_digest_val); // 35
            let actual_reapable = Pallet::query_asset_to_reap();
            assert_eq!(expected_reapable, actual_reapable);
        })
    }

    #[test]
    fn query_reason_value() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_key_and_set_balance_and_hold(ALICE, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();
            initiate_key_and_set_balance_and_hold(BOB, STANDARD_COMMIT, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(ALAN, STANDARD_COMMIT, STANDARD_HOLD)
                .unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                150,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let staking_value = Pallet::query_reason_value(STAKING);
            assert_eq!(staking_value, 150);

            Pallet::place_commit(
                &BOB,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let escrow_value = Pallet::query_reason_value(ESCROW);
            assert_eq!(escrow_value, 250);

            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let staking_value = Pallet::query_reason_value(STAKING);
            assert_eq!(staking_value, 400);

            let governance_value = Pallet::query_reason_value(GOVERNANCE);
            assert_eq!(governance_value, 0);
        })
    }
}