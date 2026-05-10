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
// ``````````````````````````````` PALLET AUTHORS ````````````````````````````````
// ===============================================================================

//! The **Authors pallet** implements a economically-backed **role system**
//! for managing **block authors* (validators) as first-class on-chain actors.
//!
//! This pallet provides a concrete, implementation of the
//! generic role abstractions, enabling authors to be enrolled, funded, elected,
//! rewarded, penalized, and governed in a deterministic and auditable manner.
//!
//! - [`Config`] - Runtime configuration
//! - [`Call`] - Dispatchable extrinsics
//! - [`Pallet`] - External Usage (Trait Impls)
//! - [`FlatElection`] - External Usage (Resolved Plugin)
//! - [`FairElection`]- External Usage (Resolved Plugin)
//!
//! ## Overview
//!
//! An **Author** represents an actor who:
//! - locks **self-collateral** as a security commitment (enrollment),
//! - progresses through a **probationary lifecycle** before permanence.
//! - may receive **external backing** from third parties (funding),
//! - participates in **elections** and other duties,
//! - accrues **rewards and penalties over time**, and
//!
//! The pallet models authors as **economic and temporal subjects**, not merely
//! accounts.
//!
//! ## Architectural Role
//!
//! The pallet provides a **composable role layer** that integrates:
//! - lifecycle management (enrollment, status, resignation),
//! - economic backing (collateral and external funding),
//! - reward and penalty scheduling,
//! - probation and risk handling,
//! - activity-aware participation controls.
//!
//! This allows the Authors pallet to act as a **pluggable role provider**
//! for other runtime modules without exposing low-level storage details.
//!
//! ## Funding Models
//!
//! Authors may receive economic support through multiple funding paths:
//!
//! - **Self-collateral** provided during enrollment.
//! - **Direct backing** from individual backers.
//! - **Aggregated backing** via indexes or pools (when enabled).
//!
//! All funding is mediated through a shared commitment abstraction, ensuring
//! consistent locking, accounting, and release semantics across the runtime.
//!
//! ## Election Models
//!
//! The pallet supports **pluggable election strategies** via
//! [`elections`](frame_suite::elections) traits, selectable at runtime-composition:
//!
//! - **Flat elections** aggregate all backing (including self-collateral)
//!   into a single influence metric per author.
//! - **Fair elections** preserve individual backer contributions and
//!   explicitly including self-collateral as a self-backing.
//!
//! Election logic is intentionally externalized using plugin-based models via [`Config`],
//! allowing governance to evolve influence and election logic without modifying pallet code.
//!
//! ## Temporal Semantics
//!
//! Rewards and penalties are:
//! - **scheduled**, not immediate,
//! - applied deterministically at block boundaries,
//! - buffered to prevent manipulation and race conditions.
//!
//! The pallet processes all pending compensations at the **start of each block**,
//! guaranteeing consistent state for subsequent extrinsics and elections.
//!
//! ## Governance & Safety
//!
//! Governance or Sudo may:
//! - adjust collateral and funding thresholds,
//! - tune probation and risk parameters,
//! - control election bounds and strictness,
//! - override configuration via root-only calls.
//!
//! Storage is intentionally **append-only** for critical mappings,
//! prioritizing auditability and long-term safety over aggressive cleanup.
//!
//! ## Intended Use
//!
//! While named "Authors", this pallet is designed to be reusable for any
//! economically-backed role such as:
//! - block producers,
//! - content curators,
//! - oracle operators,
//! - council members,
//! - or DAO representatives.
//!
//! It serves both as a reusable production pallet and as a reference
//! implementation of advanced role abstractions.
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

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod roles;
mod election;
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
    use crate::{types::*, weights::*};

    // --- FRAME Suite ---
    use frame_suite::{base::*, commitment::*, plugin_types, roles::*};

    // --- Core / Std ---
    use core::{fmt::Debug, marker::PhantomData};

    // --- FRAME Support ---
    use frame_support::{
        dispatch::DispatchResult,
        ensure,
        pallet_prelude::*,
        traits::{
            fungible::{Inspect, Mutate, UnbalancedHold},
            tokens::{Balance, Fortitude, Precision},
            VariantCount,
        },
    };

    // --- FRAME System ---
    use frame_system::{
        ensure_root, ensure_signed,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };

    // --- Substrate primitives ---
    use sp_runtime::{traits::Bounded, DispatchError, Saturating, Vec};

    // ===============================================================================
    // `````````````````````````````` PALLET MARKER ``````````````````````````````````
    // ===============================================================================

    /// Primary Marker type for the **Authors pallet**.
    ///
    /// This pallet provides implementations for traits from
    /// [`roles`](frame_suite::roles) and [`elections`](frame_suite::elections)
    ///
    /// [`Pallet`] implements the core role-system traits:
    ///
    /// - [`RoleManager`](frame_suite::roles::RoleManager)
    /// - [`FundRoles`]
    /// - [`CompensateRoles`]
    /// - [`RoleProbation`]
    /// - [`RoleManager`](frame_suite::roles::RoleManager)
    ///
    /// and **pluggable** election runner traits
    ///
    /// - [`InspectWeight`](frame_suite::elections::InspectWeight)
    /// - [`ElectionManager`](frame_suite::elections::ElectionManager)
    /// - [`Influence`](frame_suite::elections::Influence)
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ===============================================================================
    // `````````````````````````````` CONFIG TRAIT ```````````````````````````````````
    // ===============================================================================

    /// Configuration trait for the Authors pallet.
    ///
    /// This trait defines the types, constants, and dependencies
    /// that the runtime must provide for this pallet to function.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        // --- Runtime Anchors ---

        /// The overarching event type for this pallet.
        ///
        /// Allows the Authors pallet to emit events into the runtime event system,
        /// e.g., when authors are rewarded, penalized, or change status.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Type representing reason for freezing assets.
        ///
        /// Converts from the pallet-level `FreezeReason` enum and is used by
        /// the `CommitmentAdapter` to distinguish between different frozen asset categories.
        type AssetFreeze: From<FreezeReason> + RuntimeEnum + Delimited + Copy + VariantCount;

        // --- Scalars ---

        /// Represents the **computed influence** of an entity (author, account, etc.).
        ///
        /// Influence is derived from a raw backing asset (stake, tokens, or other measure)
        /// and is used as the primary metric for calculating an author's influence over others.
        ///
        /// Must implement a unsigned numeric type and support conversion from the raw fungible
        /// asset.
        type Influence: Balance + From<AuthorAsset<Self>>;

        // --- Pallet Adapters ---

        /// The **asset type** used for distributing rewards and penalties.
        ///
        /// This represents a fungible unit (e.g., a token balance) over which
        /// the pallet performs **reward**, **penalty**, and **funding** operations.
        ///
        /// ## Requirements
        /// - Must implement [`Inspect`], enabling the pallet to query and verify
        ///   account balances or holdings of authors.
        ///
        /// ## Example
        /// ```ignore
        /// type Asset = pallet_balances::Pallet<T>;
        /// ```
        type Asset: Inspect<
                Author<Self>,
                // Ensures that the pallet's `Asset` type aligns with the assets
                // used by commitment system.
                //
                // Guarantees that rewards, penalties, and funding operations
                // use a **consistent asset type**.
                Balance = <Self::CommitmentAdapter as InspectAsset<Author<Self>>>::Asset,
            > + Mutate<Author<Self>>
            + UnbalancedHold<Author<Self>>;

        /// Commitment trait adapter for managing authors' external fundings,
        /// assets and digests.
        ///
        /// This type implements multiple traits:
        /// - [`Commitment`] to track asset commitments per author.
        /// - [`CommitIndex`] for index-based funders.
        /// - [`CommitPool`] for pool-backed funding.
        /// - [`InspectAsset`] to query asset balances or holds.
        ///
        /// Provides the **core mechanism for holding and tracking author funds**.
        type CommitmentAdapter: Commitment<
                Author<Self>,
                Reason = Self::AssetFreeze,
                DigestSource = Author<Self>,
                Digest: Ord,
            > + CommitIndex<Author<Self>>
            + CommitPool<Author<Self>>
            + InspectAsset<Author<Self>>;

        /// Activity provider for authors, defining hooks or queries related to
        /// an author's participation and behavior within the runtime.
        ///
        /// This type allows the **Authors pallet to expose author activity to
        /// other pallets** that rely on author role participation.
        ///
        /// In essence, it provides a standardized, pluggable interface for
        /// **cross-pallet activity tracking**.
        type ActivityProvider: RoleActivity<Author<Self>, BlockNumberFor<Self>>;

        // --- Plugins ---

        // Plugin for computing **influence values** from raw backing assets.
        //
        // The computation logic is fully **pluggable** and **runtime-configurable**,
        // allowing influence policies to evolve without modifying pallet logic.
        plugin_types!(
            // Raw backing asset used as input for influence computation.
            //
            // Typically represents stake, funds, votes, or other measurable support
            // associated with an entity.
            //
            // The interpretation of this value is entirely model-dependent.
            input: AuthorAsset<Self>,

            // Computed influence value derived from the raw backing asset.
            //
            // This singular metric represents the effective weight or power of an
            // entity in influence-based decision systems.
            //
            // Influence values are comparable and suitable for ordering, weighting,
            // or normalization, as required by downstream logic.
            output: Self::Influence,

            /// **Influence computation plugin model**.
            ///
            /// Influence is a derived, comparable value used in influence-based systems
            /// such as elections, ranking, scoring, or governance decisions.
            ///
            /// Encapsulates the logic that transforms a raw asset value into an
            /// influence metric.
            ///
            /// Conceptually performs:
            ///
            /// `AuthorAsset -> Influence`
            ///
            /// The specific transformation semantics are entirely defined by the
            /// selected model.
            ///
            /// Designed to be selectable using template plugin models in
            /// [`frame_plugins::influence`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            model: InfluenceModel,

            /// Plugin model **context** for influence computation.
            ///
            /// Supplies parameters that configure how the [`Self::InfluenceModel`]
            /// behaves at runtime.
            ///
            /// This context enables dynamic tuning of influence policies via
            /// governance or configuration, without modifying the model implementation
            /// or pallet logic.
            ///
            /// Must match the context type expected by the selected
            /// [`Self::InfluenceModel`].
            context: InfluenceContext,
        );

        // Plugin for computing **flat elections** using influence-based metrics.
        //
        // This plugin performs election computation by evaluating candidates
        // (authors) solely on their computed influence values.
        //
        // The election logic is fully **pluggable** and **runtime-configurable**,
        // allowing different influence-based election policies to be applied
        // without modifying pallet logic.
        plugin_types!(
            // Input collection for flat election computation.
            //
            // Represents a set of candidates paired with their computed
            // influence values.
            //
            // Each candidate is associated with exactly one influence metric,
            // typically derived via the influence computation plugin.
            input: ElectViaInfluence<Self>,

            // Output collection of elected candidates.
            //
            // May represent a single elected author or multiple elected authors,
            // depending on the configured election model.
            output: ElectedAuthors<Self>,

            /// **Flat election plugin model**.
            ///
            /// Encapsulates the election logic that operates on influence values
            /// to determine the final set of elected candidates.
            ///
            /// Conceptually performs:
            ///
            /// `[(Author, Influence)] -> [Author]`
            ///
            /// The model is expected to return elected candidates in **priority
            /// order**, as the runtime may truncate the result using
            /// [`MaxElected`] or [`ForceMaxElected`].
            ///
            /// Designed to be selectable using template plugin models in
            /// [`frame_plugins::elections::flat`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            model: FlatElectionModel,

            /// Plugin model **context** for flat election computation.
            ///
            /// Supplies parameters that configure how the
            /// [`Self::FlatElectionModel`] behaves at runtime.
            ///
            /// Enables dynamic tuning of election policies without modifying
            /// the model implementation or pallet logic.
            ///
            /// Must match the context type expected by the selected
            /// [`Self::FlatElectionModel`].
            context: FlatElectionContext,
        );

        // Plugin for computing **fair elections** using backing-based metrics.
        //
        // This plugin performs election computation by evaluating candidates
        // (authors) based on their individual backing contributions.
        //
        // Each backing relationship is preserved and considered independently,
        // ensuring that election outcomes reflect the structure and distribution
        // of external support.
        //
        // The election logic is fully **pluggable** and **runtime-configurable**.
        plugin_types!(
            // Input collection for fair election computation.
            //
            // Represents a set of candidates paired with their backing
            // contributions, where each candidate may be associated with
            // multiple backers and corresponding weights.
            input: ElectViaBacking<Self>,

            // Output collection of elected candidates.
            //
            // May represent a single elected author or multiple elected authors,
            // depending on the configured election model.
            output: ElectedAuthors<Self>,

            /// **Fair election plugin model**.
            ///
            /// Encapsulates the election logic that operates on individual
            /// backing contributions to determine the final set of elected
            /// candidates.
            ///
            /// Conceptually performs:
            ///
            /// `[(Author, [(Backer, Weight)])] -> [Author]`
            ///
            /// The model is expected to return elected candidates in **priority
            /// order**, as the runtime may truncate the result using
            /// [`MaxElected`] or [`ForceMaxElected`].
            ///
            /// Designed to be selectable using template plugin models in
            /// [`frame_plugins::elections::fair`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            model: FairElectionModel,

            /// Plugin model **context** for fair election computation.
            ///
            /// Supplies parameters that configure how the
            /// [`Self::FairElectionModel`] behaves at runtime.
            ///
            /// Enables dynamic tuning of election policies without modifying
            /// the model implementation or pallet logic.
            ///
            /// Must match the context type expected by the selected
            /// [`Self::FairElectionModel`].
            context: FairElectionContext,
        );

        // --- Weights ---

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        // --- Constants ---

        #[pallet::constant]
        type EmitEvents: Get<bool> + Clone + Debug;
    }

    // ===============================================================================
    // ``````````````````````````````` COMPOSITE ENUMS ```````````````````````````````
    // ===============================================================================

    /// Reasons for which an author's assets may be temporarily frozen in the runtime.
    ///
    /// The `FreezeReason` enum is used to **categorize and isolate different types of
    /// asset holds**, allowing the runtime to manage multiple constraints independently.
    #[pallet::composite_enum]
    pub enum FreezeReason {
        /// Assets reserved due to **external author funding**.
        ///
        /// These are funds provided by backers or commitment systems, held to ensure
        /// accountability, prevent double-spending, and enforce proper reward/penalty mechanics.
        AuthorFunding,

        /// Assets reserved as **author collateral**.
        ///
        /// Collateral represents the skin-in-the-game requirement for authors, ensuring
        /// they have a stake in maintaining protocol security and correct behavior.
        AuthorCollateral,
    }

    // ===============================================================================
    // ``````````````````````````````` GENESIS CONFIG ````````````````````````````````
    // ===============================================================================

    /// Genesis configuration for the **Authors pallet**.
    ///
    /// Provides the **initial runtime parameters** governing author lifecycle,
    /// funding, and reward/penalty mechanics at chain genesis.
    ///
    /// These values define the **baseline operational rules** before any on-chain
    /// authors are enrolled or any activity occurs.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Minimum collateral an author must hold to participate.
        /// Ensures skin-in-the-game and commitment to the network.
        /// Must not be set to zero, to avoid no-commitment failures.
        pub min_collateral: AuthorAsset<T>,

        /// Maximum allowed exposure of a funding/backing operation instance.
        /// Limits systemic risk from overly funded or leveraged participants.
        ///
        /// This is ambiguous in `pool` and `index` contexts as it only represents how
        /// much the backer is willing to commit at a transaction, not specific to what scheme.
        pub max_exposure: AuthorAsset<T>,

        /// Minimum funding required of a funding/backing operation instance.
        /// Guarantees sufficient community support before an author can participate.
        /// Must not be set to zero, to avoid zero-commitment failures.
        ///
        /// This is ambiguous in `pool` and `index` contexts as it only represents how
        /// much the backer is willing to commit at a transaction, not specific to what scheme.
        pub min_fund: AuthorAsset<T>,

        /// Number of blocks newly enrolled or demoted authors must spend in probation
        /// before achieving permanent status.
        /// Enforces behavioral observation and prevents immediate promotion.
        pub probation_period: BlockNumberFor<T>,

        /// Number of blocks to reduce an author's risk period when positive behavior is observed.
        /// Facilitates promotion and rewards accountability.
        pub reduce_probation_by: BlockNumberFor<T>,

        /// Number of blocks to extend an author's risk period when unsafe behavior is detected.
        /// Enforces accountability and ensures that authors under observation remain under supervision.
        pub increase_probation_by: BlockNumberFor<T>,

        /// Number of blocks to delay reward finalization.
        /// Ensures orderly processing and temporal separation of reward events.
        pub rewards_buffer: BlockNumberFor<T>,

        /// Number of blocks to delay penalty finalization.
        /// Allows authors to react or remediate before penalties are enforced.
        pub penalties_buffer: BlockNumberFor<T>,

        /// Maximum number of authors that can be elected in a single election round.
        /// If `force_max_elected` is true, the elected list will be truncated to this limit.
        pub max_elected: u32,

        /// Minimum number of authors required to complete a valid election round.
        /// Ensures that elections maintain a minimum quorum of participants.
        pub min_elected: u32,

        /// Whether to strictly enforce the `max_elected` limit.
        /// - `true`: Forcefully truncate elected authors to `max_elected`.
        /// - `false`: Attempt to store all elected candidates.
        pub force_max_elected: bool,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                min_collateral: 1u32.into(),
                max_exposure: Bounded::max_value(),
                min_fund: 1u32.into(),
                probation_period: 10u32.into(),
                reduce_probation_by: 1u32.into(),
                increase_probation_by: 1u32.into(),
                rewards_buffer: 2u32.into(),
                penalties_buffer: 4u32.into(),
                max_elected: 100u32.into(),
                min_elected: 10u32.into(),
                force_max_elected: false,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            assert!(
                !self.min_collateral.is_zero(),
                "GenesisConfig error: min_collateral must be greater than zero"
            );
            assert!(
                !self.min_fund.is_zero(),
                "GenesisConfig error: min_fund must be greater than zero"
            );
            assert!(
                !self.min_elected.is_zero(),
                "GenesisConfig error: min_elected must be greater than zero"
            );
            assert!(
                !self.max_elected.is_zero(),
                "GenesisConfig error: max_elected must be greater than zero"
            );
            assert!(
                self.min_elected <= self.max_elected,
                "GenesisConfig error: min_elected cannot be greater than max_elected"
            );
            assert!(
                self.min_fund <= self.max_exposure,
                "GenesisConfig error: min_fund cannot be greater than max_exposure"
            );
            MinCollateral::<T>::put(&self.min_collateral);
            MaxExposure::<T>::put(&self.max_exposure);
            MinFund::<T>::put(&self.min_fund.max(One::one()));
            ProbationPeriod::<T>::put(&self.probation_period);
            ReduceProbationBy::<T>::put(&self.reduce_probation_by);
            IncreaseProbationBy::<T>::put(&self.increase_probation_by);
            RewardsBuffer::<T>::put(&self.rewards_buffer);
            PenaltiesBuffer::<T>::put(&self.penalties_buffer);
            RewardsUntil::<T>::put(BlockNumberFor::<T>::zero());
            PenaltiesUntil::<T>::put(BlockNumberFor::<T>::zero());
            MaxElected::<T>::put(&self.max_elected);
            MinElected::<T>::put(&self.min_elected.max(One::one()));
            ForceMaxElected::<T>::put(&self.force_max_elected);
            RecentElectedOn::<T>::put(BlockNumberFor::<T>::zero());
        }
    }

    // ===============================================================================
    // ```````````````````````````````` STORAGE TYPES ````````````````````````````````
    // ===============================================================================

    /// Maps each [`Author`] account to its on-chain metadata ([`AuthorInfo`]).
    ///
    /// This storage is the **primary record** for all authors, tracking their:
    /// - lifecycle status (Active, Probation, Resigned),
    /// - risk period and timestamps,
    /// - funding constraints (`min_fund`, `max_fund`).
    ///
    /// Keys are **insert-mutate-only** and **MUST NOT be removed**.
    ///
    /// Indexes and pools may retain digests indefinitely, and re-enrollment of
    /// resigned authors may utilize their previous commitment-digest (existing meta).
    ///
    /// By keeping the resigned authors meta-data alive funders can draw from resigned
    /// authors.
    ///
    /// In Future, if casual resignations are penalized via assigning new commitment-
    /// digests, then this storage can be mutated again without reuse of digest, but removable
    /// clearly requires withdrawal, and usage in indexes and pools must not create inconsistent
    /// state.
    ///
    /// Used for all author-related operations.
    #[pallet::storage]
    pub type AuthorsMap<T: Config> =
        StorageMap<_, Blake2_128Concat, Author<T>, AuthorInfo<T>, OptionQuery>;

    /// Maps each [`AuthorDigest`] to its corresponding [`Author`] account.
    ///
    /// Provides **digest-to-account resolution**, allowing higher-level logic
    /// to look up authors from commitment-based identifiers.
    ///
    /// Keys are **insert-only** and **MUST NOT be removed or mutated**.
    ///
    /// Indexes and pools may retain digests indefinitely, and authors must remain
    /// discoverable for any referenced digest.
    ///
    /// Authors may resign and later re-enroll with the same digest. Hence stale
    /// digests must remain valid to allow withdrawal of funds or soft-support commitments.
    ///
    /// Safe removal would require proving that no index or pool retains the digest,
    /// which is effectively impossible to guarantee within the subsystem.
    ///
    /// In Future, regardless even if casual resignations are penalized via assigning
    /// new commitment-digests, this doesn't need to be removed, unless the gurantees withhold,
    /// which generally cannot.
    ///
    /// Useful for funding references, reward/penalty scheduling, and auditability.
    #[pallet::storage]
    pub type AuthorsDigest<T: Config> =
        StorageMap<_, Blake2_128Concat, AuthorDigest<T>, Author<T>, OptionQuery>;

    /// Stores the **funder details** for each ([`Author`], [`Backer`]) pair.
    ///
    /// Supports multi-source funding for authors:
    /// - direct account backing
    /// - index-based commitments
    /// - pooled commitments
    ///
    /// Enables the runtime to query, update, or audit all funders for an author.
    #[pallet::storage]
    pub type AuthorFunders<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, Author<T>>,
            NMapKey<Blake2_128Concat, Backer<T>>,
        ),
        Funder<T>,
        OptionQuery,
    >;

    /// Tracks the latest block number until which **author rewards** are scheduled.
    ///
    /// Enables **efficient scanning** for pending rewards by limiting iteration
    /// to blocks up to `RewardsUntil`.
    ///
    /// Must not be updated by governance manually, as its a runtime inferred value
    /// and not a genesis configurable value.
    #[pallet::storage]
    pub type RewardsUntil<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Tracks the latest block number until which **author penalties** are scheduled.
    ///
    /// Ensures efficient access to pending penalties without scanning irrelevant blocks.
    ///
    /// Must not be updated by governance manually, as its a runtime inferred value
    /// and not a genesis configurable value.
    #[pallet::storage]
    pub type PenaltiesUntil<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Buffer period in blocks before a reward can be applied.
    ///
    /// Used to **defer rewards** and ensure orderly enforcement, preventing immediate manipulation.
    #[pallet::storage]
    pub type RewardsBuffer<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Buffer period in blocks before a penalty can be applied.
    ///
    /// Provides temporal separation for enforcement and ensures deterministic scheduling of penalties.
    #[pallet::storage]
    pub type PenaltiesBuffer<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Stores **pending rewards** for authors by ([`BlockNumberFor`], [`Author`]) key.
    ///
    /// - Enables deferred reward application at the correct block.
    /// - Supports retrieval of total reward for an author or for a specific timestamp.
    /// - Value type is [`AuthorAsset`], representing the reward amount.
    #[pallet::storage]
    pub type AuthorRewards<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, BlockNumberFor<T>>,
            NMapKey<Blake2_128Concat, Author<T>>,
        ),
        AuthorAsset<T>,
        OptionQuery,
    >;

    /// Stores **pending penalties** for authors by ([`BlockNumberFor`], [`Author`]) key.
    ///
    /// - Allows deferred penalty enforcement at the correct block.
    /// - Supports retrieval of total penalty-percentage applied on an author's
    /// risk-profile for a specific timestamp.
    /// - Value type is [`Ratio`], representing the penalty factor applied on the
    /// author's total hold.
    #[pallet::storage]
    pub type AuthorPenalties<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, BlockNumberFor<T>>,
            NMapKey<Blake2_128Concat, Author<T>>,
        ),
        Ratio<T>,
        OptionQuery,
    >;

    /// Global Minimum collateral required for any author.
    ///
    /// Must not be set to zero, else collateral query functions will fail, as there
    /// will be no actual commitment.
    ///
    /// Ensures authors maintain sufficient **stake or backing** for network security.
    #[pallet::storage]
    pub type MinCollateral<T: Config> = StorageValue<_, AuthorAsset<T>, ValueQuery>;

    /// Global Maximum exposure allowed globally per funding operation.
    ///
    /// Prevents a single funding-instance from **over-concentrating funding** or
    /// creating systemic risk.
    ///
    /// This is ambiguous in `pool` and `index` contexts as it only represents how
    /// much the backer is willing to commit at a transaction, not specific to what scheme.
    #[pallet::storage]
    pub type MaxExposure<T: Config> = StorageValue<_, AuthorAsset<T>, ValueQuery>;

    /// Global Minimum funding required globally per funding operation.
    ///
    /// Ensures authors meet **base economic requirements** for participation.
    ///
    /// This is ambiguous in `pool` and `index` contexts as it only represents how
    /// much the backer is willing to commit at a transaction, not specific to what scheme.
    #[pallet::storage]
    pub type MinFund<T: Config> = StorageValue<_, AuthorAsset<T>, ValueQuery>;

    /// Number of blocks representing the **probation period** for newly enrolled or demoted authors.
    ///
    /// This period enforces a **mandatory observation window** during which an author:
    /// - Cannot be promoted to permanent/active status.
    /// - Must demonstrate acceptable behavior to secure their role.
    ///
    /// Additionally, if an author is deemed unsafe until a timestamp overlapping this period,
    /// they are **required to remain or be moved back to probation**, ensuring the network
    /// only promotes trusted participants.
    #[pallet::storage]
    pub type ProbationPeriod<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Number of blocks by which an author's risk period is **reduced** upon good behavior.
    ///
    /// Supports gradual restoration of permanence or reduced probation impact.
    #[pallet::storage]
    pub type ReduceProbationBy<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Number of blocks by which an author's risk period is **increased** upon misbehavior.
    ///
    /// Enables dynamic punishment while maintaining predictable enforcement windows.
    #[pallet::storage]
    pub type IncreaseProbationBy<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Persistent record of **elected authors per block**, representing the outcome
    /// of each finalized election round.
    ///
    /// This is a two-dimensional mapping:
    /// - The block number in which the election was conducted.
    /// - The author (candidate) elected in that block.
    /// - Unit value (only existence matters; no extra metadata stored).
    ///
    /// Because this is a `StorageNMap`, callers can **query or iterate over all
    /// authors elected in a given block** by iterating over the entries that share
    /// the same block number key.
    ///
    /// This design allows efficient historical lookups and auditability
    /// over election rounds without overwriting previous results.
    ///
    /// ### Notes
    /// - This storage is **append-only** - each election round adds entries under
    ///   a new block key.
    /// - Multiple authors may be associated with the same block if the election
    ///   yields more than one winner.
    /// - Overwriting does not occur; historical elections remain queryable
    ///   indefinitely.
    #[pallet::storage]
    pub type Elected<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, BlockNumberFor<T>>,
            NMapKey<Blake2_128Concat, Author<T>>,
        ),
        (),
        OptionQuery,
    >;

    /// Tracks the **most recent block number** in which an election was conducted
    /// and successfully stored in [`Elected`].
    ///
    /// Acts as a quick reference for identifying the **latest election round**
    /// without scanning all historical data.
    ///
    /// ### Usage
    /// - Updated every time a new election result is finalized and stored.
    /// - Useful for logic that depends on the recency or periodicity of elections.
    ///
    /// ### Notes
    /// - Always holds a valid block number (`ValueQuery`).
    /// - Can be used to compute time intervals between election rounds.
    #[pallet::storage]
    pub type RecentElectedOn<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// The **upper bound** on the number of authors that can be elected in a single election round.
    ///
    /// ## Behavior
    /// - Acts as a *hard cap* on election size.
    /// - Used by both [`Config::FlatElectionModel`] and [`Config::FairElectionModel`] to truncate or validate
    ///   the elected authors list.
    /// - When [`ForceMaxElected`] is set to `true`, any excess elected candidates beyond this value
    ///   are **automatically truncated** before being persisted.
    ///
    /// ## Example
    /// If `MaxElected = 50`, the election will never store more than 50 authors,
    /// even if the algorithm produces 80 winners.
    #[pallet::storage]
    pub type MaxElected<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The **minimum required number of authors** that must be elected for an election
    /// to be considered valid.
    ///
    /// ## Behavior
    /// - Prevents premature elections with insufficient candidates.
    /// - If the elected count falls below this threshold,  
    ///   the election process fails with an error.
    ///
    /// ## Example
    /// If `MinElected = 10` but only 6 valid authors are produced,  
    /// the election aborts rather than storing incomplete results.
    #[pallet::storage]
    pub type MinElected<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// A **strictness flag** controlling how the system enforces the [`MaxElected`] limit.
    ///
    /// ## Behavior
    /// - When `true`:  
    ///   The system **forcefully truncates** the elected list to `MaxElected`
    ///   before persisting.
    /// - When `false`:  
    ///   The system performs a **safe bounded check**; it **fails** (returns error) if
    ///   any bounds are enforced by the underlying storage instead of truncating
    ///   automatically i.e., a [`WeakBoundedVec`].
    ///
    /// ## Purpose
    /// This provides governance-level control over whether elections prioritize
    /// **strict deterministic enforcement (force)** or **safe bounded validation (fair)**.
    ///
    /// ## Example
    /// - `ForceMaxElected = true` -> Automatically keeps top N candidates.  
    /// - `ForceMaxElected = false` -> Requires manual handling of overflows before storage.
    #[pallet::storage]
    pub type ForceMaxElected<T: Config> = StorageValue<_, bool, ValueQuery>;

    // ===============================================================================
    // ```````````````````````````````````` ERROR ````````````````````````````````````
    // ===============================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The specified author is not found in the system.
        AuthorNotFound,

        /// Lesser than minimum collateral provided for author role enrollment.
        InadequateCollateral,

        /// Lesser than minimum requirement funds provided for enroll/funding operation.
        InadequateFunds,

        /// Redundant enrollment for an already existing author.
        AlreadyEnrolled,

        /// Author is actively participating in their assigned/enrolled duties.
        ///
        /// Either internally or externally by other pallets.
        AuthorIsActive,

        /// The specified author is undergoing probation period.
        AuthorInProbation,

        /// Author has resigned and must undergo a new enrollment to rejoin.
        AuthorResigned,

        /// Author has pending penalties that must finalize before the operation can proceed.
        AuthorHasPenalties,

        /// Once an author is active after probation, cannot undergo probation again.
        AuthorActivated,

        /// Digest cannot be generated for author for external funding (commitment).
        CannotGenerateCommitDigest,

        /// Author has pending rewards that must finalize before the operation can proceed.
        AuthorHasRewards,

        /// The requested external backing/fund does not exist.
        FundDoesNotExist,

        /// The author's funding digest/hash is not found in the system.
        AuthorDigestNotFound,

        /// The backer funded another author's digest, not the specified author.
        ///
        /// May correspond to an index or pool digest.
        FundedToAnotherDigest,

        /// Author's funding digest is not available in the index entries.
        AuthorNotInIndex,

        /// Author's funding digest is not available in the pool's slots.
        AuthorNotInPool,

        /// Minimum fund not attained, either locally (author-defined) or globally.
        BelowMinimumFund,

        /// Funds exceed maximum exposable amount, either locally (author-defined) or globally.
        AboveMaximumExposure,

        /// Author is attempting to fund on top of collateral, which is not allowed.
        CannotFundOnCollateral,

        /// The requested reward for author is not found in pending rewards.
        RewardNotFound,

        /// The requested penalty for author is not found in pending penalties.
        PenaltyNotFound,

        /// The total hold of an author, including all funds and collateral, overflowed.
        ///
        /// If the commitment asset is non-issuance based, update the scalar type to maximum precision.
        /// If issuance-based, an internal audit into the asset is expected.
        AuthorTotalHoldExhausted,

        /// The requested timestamp doesn't contain any pending rewards.
        ContainsNoRewards,

        /// The requested timestamp doesn't contain any pending penalties.
        ContainsNoPenalties,

        /// The requested timestamp has finalized all obligations, such as pending rewards or penalties.
        FinalizedObligations,

        /// Author is under-collateralized due to increased system requirements to continue operations.
        AuthorNeedsMoreCollateral,

        /// The given penalty factor is zero and cannot be applied for penalization of authors.
        ZeroPenaltyFound,

        /// The author is deemed risky (unsafe); operation cannot proceed.
        ///
        /// Not a permanent condition: can be resolved with positive activities.
        AuthorIsUnsafe,

        /// Already resigned author attempting resignation again.
        RedundantResignation,

        /// The provided candidate set was **too small** to begin an election.
        ///
        /// This occurs when the number of candidates passed to the election process
        /// is less than the configured minimum.
        InadequateCandidatesToElect,

        /// No elected authors were found in storage or the election produced zero winners.
        ///
        /// This may occur in two cases:
        /// - When attempting to **reveal** or **access** election results but no election
        ///   data has been recorded for the current or recent round.
        /// - When an election process completed but resulted in **zero elected candidates**,
        ///   indicating either a lack of eligible participants or a model configuration issue.
        NoElectsFound,

        /// The given author is not elected in the recent election.
        AuthorNotElected,

        /// The number of elected authors **did not reach the configured minimum**.
        ///
        /// Triggered during the election storage phase when the computed
        /// results contain fewer authors than required.
        MinElectedNotReached,

        /// The author's accumulated risk has **not crossed the revocation limit**.
        ///
        /// This error is returned when an operation requires the author to have
        /// exceeded the allowed risk window, but the author's `risk_until`
        /// is still less than or equal to:
        ///
        /// `current_block + ProbationPeriod`.
        ///
        /// In this state, the author remains valid and **cannot yet be revoked**.
        RiskWithinThreshold,

        /// The provided author does not match the expected author for the operation.
        ///
        /// This error is returned when an operation references an author that is
        /// inconsistent with the stored author context or resolved digest.
        AuthorMismatch,

        /// The caller is not the current manager of the specified funding pool.
        InvalidPoolManager,

        /// The minimum collateral value cannot be zero.
        ///
        /// This error is returned when attempting to set the minimum required
        /// collateral to `0`. The minimum collateral must be greater than zero.
        MinCollateralZero,

        /// A minimum configuration value exceeds its corresponding maximum value.
        ///
        /// This error is returned when a parameter that represents a lower bound
        /// (e.g., `MinElected`, `MinFund`) is set greater than its associated
        /// upper bound (e.g., `MaxElected`, `MaxExposure`).
        ///
        /// Ensures logical consistency between related configuration limits.
        MinGreaterThanMax,

        /// A configuration parameter that must be strictly greater than zero
        /// was provided as zero.
        ///
        /// This error is returned when attempting to set a non-zero-required
        /// global parameter (such as `MinCollateral`, `MinFund`,
        /// `MinElected`, or `MaxElected`) to `0`.
        ///
        /// Prevents invalid economic or election configuration.
        NonZeroConfigRequired,

        /// Initiating a fund (backing) has exceeded allowed funding limits either
        /// globally or locally by the receiving model (author/index/pool) or
        /// low-level balances.
        FundingOffLimits,
    }

    // ===============================================================================
    // ```````````````````````````````````` EVENTS ```````````````````````````````````
    // ===============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Emitted when an account successfully enrolls, by enlisting itself as
        /// an author by locking the required collateral for the role.
        AuthorEnlisted {
            author: Author<T>,
            collateral: AuthorAsset<T>,
        },

        /// Emitted when an author voluntarily resigns from the role,
        /// regains their collateral and exits active participation.
        AuthorResigned {
            author: Author<T>,
            released: AuthorAsset<T>,
        },

        /// Emitted when an author's total collateral is incremented i.e.,
        /// additional collateral being added.
        AuthorCollateralRaised {
            author: Author<T>,
            raised: AuthorAsset<T>,
        },

        /// Emitted when the total collateral of an author is queried.
        AuthorTotalCollateral {
            author: Author<T>,
            collateral: AuthorAsset<T>,
        },

        /// Emitted when a backer funds to an author via any funding mechanism
        /// such as direct, indexed and pooled.
        AuthorFunded {
            author: Author<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when a backer queries their fund that is commited to
        /// a author directly.
        InspectAuthorFund {
            author: Author<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when a backer queries their fund that is commited to an
        /// author through Direct, Index or Pool based funding mechanism.
        InspectFund {
            author: Author<T>,
            funder: Funder<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when a backer funds authors through an index-based
        /// funding mechanism.
        IndexFunded {
            index: IndexDigest<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when a backer queries their fund that is commited to an
        /// index-based funding mechanism.
        InspectIndexFund {
            index: IndexDigest<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when a backer funds authors through an pool-based
        /// funding mechanism.
        PoolFunded {
            pool: PoolDigest<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when a backer queries their fund that is commited to an
        /// pool-based funding mechanism.
        InspectPoolFund {
            pool: PoolDigest<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when previously committed funds are successfully released
        /// from an author back to the backer.
        AuthorDrawn {
            author: Author<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when previously committed funds are successfully released
        /// from an index back to the backer.
        IndexDrawn {
            index: IndexDigest<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when previously committed funds are successfully released
        /// from an pool back to the backer.
        PoolDrawn {
            pool: PoolDigest<T>,
            backer: Backer<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when scheduled rewards are queried for an author, including one or more
        /// future block numbers at which the rewards will be applied.
        ScheduledRewards {
            author: Author<T>,
            rewards: Vec<(BlockNumberFor<T>, AuthorAsset<T>)>,
        },

        /// Emitted when scheduled penalties are queried for an author, including one or more
        /// future block numbers at which the penalties will be enforced.
        ScheduledPenalties {
            author: Author<T>,
            penalties: Vec<(BlockNumberFor<T>, Ratio<T>)>,
        },

        /// Emitted when a reward is applied or scheduled to an author at a specific block.
        AuthorRewardScheduled {
            author: Author<T>,
            amount: AuthorAsset<T>,
            at: BlockNumberFor<T>,
        },

        /// Emitted when a penalty is applied or scheduled to an author at a specific block.
        AuthorPenaltyScheduled {
            author: Author<T>,
            factor: Ratio<T>,
            at: BlockNumberFor<T>,
        },

        /// Emitted when an author's lifecycle status changes, including transitions
        /// between probation, active, or other defined states.
        AuthorStatus {
            author: Author<T>,
            status: AuthorStatus,
        },

        /// Emitted when an author's status is at risk until a specified block,
        /// due to negative behavior, affecting either permanence or probation state.
        AuthorAtRisk {
            author: Author<T>,
            status: AuthorStatus,
            until: BlockNumberFor<T>,
        },

        /// Emitted when a previously scheduled penalty for an author
        /// is forgiven.
        AuthorPenaltyForgiven { author: Author<T>, factor: Ratio<T> },

        /// Emitted when a previously scheduled reward for an author
        /// is reclaimed.
        AuthorRewardReclaimed {
            author: Author<T>,
            amount: AuthorAsset<T>,
        },

        /// Emitted when the held (locked) balance of an author is updated or queried.
        AuthorTotalHold {
            author: Author<T>,
            value: AuthorAsset<T>,
        },

        /// Emitted when an election preparation completes successfully and elected
        /// authors are stored for runtime-usage.
        ElectionPrepared { elects: Vec<Author<T>> },

        /// Emitted when an election preparation fails to complete successfully.
        ElectionFailed { error: DispatchError },

        /// Emitted when a new funding index is created and registered.
        IndexCreated {
            index: IndexDigest<T>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            entries: Vec<(IndexDigest<T>, Shares<T>)>,
        },

        /// Emitted when a new funding pool is created with a specified commission
        /// and an assigned pool manager.
        PoolCreated {
            pool: PoolDigest<T>,
            commission: Commission<T>,
            manager: T::AccountId,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            slots: Vec<(PoolDigest<T>, Shares<T>)>,
        },

        /// Emitted when management ownership of a funding pool is transferred.
        PoolManager {
            digest: PoolDigest<T>,
            manager: T::AccountId,
        },

        /// Emitted when a slot share weight within a pool are updated.
        PoolSlotShare {
            pool: PoolDigest<T>,
            slots: (PoolDigest<T>, Shares<T>),
        },

        /// A genesis config parameter was updated forcefully.
        GenesisConfigUpdated(ForceGenesisConfig<T>),
    }

    // ===============================================================================
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ===============================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Apply scheduled author rewards and penalties at the **start of each block**.
        ///
        /// - Rewards and penalties must be processed **before any new extrinsics or elections** in the current block
        ///   to ensure that subsequent computations are accurate.
        /// - By processing at the start, we **disallow querying or relying on the current block's rewards/penalties**
        ///   until the next block, ensuring deterministic behavior and avoiding double-counting or race conditions.
        /// - Rewards/penalties are applied **once per block at the beginning**.
        /// - This ensures deterministic, predictable state transitions, and makes querying the pallet
        ///   consistent (no author sees their reward/penalty applied mid-block).
        fn on_initialize(block: BlockNumberFor<T>) -> Weight {
            // Process all scheduled author rewards for this block:
            let reward_iter: Vec<_> = AuthorRewards::<T>::iter_prefix((block,)).collect();
            let reward_count = reward_iter.len() as u32;
            // Fetch each reward, add it to the author's hold, and trigger `on_set_hold`.
            for (author, reward) in reward_iter {
                if let Ok(hold) = Self::get_hold(&author) {
                    let value = hold.saturating_add(reward);
                    if Self::set_hold(&author, value, Precision::BestEffort, Fortitude::Polite)
                        .is_ok()
                    {
                        Self::on_set_hold(&author, value);
                    }
                }
                // Remove the applied reward to prevent double processing.
                AuthorRewards::<T>::remove((block, author));
            }

            // Process all scheduled author penalties for this block:
            // Remove the applied penalty to maintain idempotency.
            let penalty_iter: Vec<_> = AuthorPenalties::<T>::iter_prefix((block,)).collect();
            let penalty_count = penalty_iter.len() as u32;
            for (author, penalty_percent) in penalty_iter {
                if let Ok(hold) = Self::get_hold(&author) {
                    // Calculate penalty as a fraction of the current hold, apply it, and trigger `on_set_hold`.
                    let penalty = penalty_percent.mul_floor(hold);
                    let value = hold.saturating_sub(penalty);
                    if Self::set_hold(&author, value, Precision::Exact, Fortitude::Force).is_ok() {
                        Self::on_set_hold(&author, value);
                    }
                }
                // Remove the applied penalty to maintain idempotency.
                AuthorPenalties::<T>::remove((block, author));
            }

            T::WeightInfo::on_initialize_rewards_penalties(reward_count, penalty_count)
        }
    }

    // ===============================================================================
    // ````````````````````````````````` EXTRINSICS ``````````````````````````````````
    // ===============================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ```````````````````````````````` DISPATCHABLES ````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Enlist the caller as an **author** by locking the required collateral.
        ///
        /// Establishes the caller as an author by placing collateral under the
        /// author role commitment.
        ///
        /// ## Requirements
        /// - Provided collateral must be at least [`MinCollateral`].
        /// - Fails if the caller is already enrolled as an author.
        ///
        /// ## Behavior
        /// - Locks the required collateral and associates it with the author role.
        /// - Ensures at least the minimum collateral is maintained.
        ///
        /// ## Execution Controls
        /// - `fortitude` defines how the collateral is sourced:
        ///   - [`FortitudeWrapper::Force`]: Uses the caller's **liquid balance** to place
        ///     the collateral.
        ///   - [`FortitudeWrapper::Polite`]: Uses funds already deposited into the
        ///     commitment reserve (if [`Config::CommitmentAdapter`] == `pallet_commitment`).
        ///
        /// **Emits:** [`Event::AuthorEnlisted`]
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::enlist())]
        pub fn enlist(
            origin: OriginFor<T>,
            collateral: AuthorAsset<T>,
            fortitude: FortitudeWrapper,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            ensure!(
                collateral >= MinCollateral::<T>::get(),
                Error::<T>::InadequateCollateral
            );

            let fortitude = fortitude.into();
            let collateral =
                <Pallet<T> as RoleManager<Author<T>>>::enroll(&caller, collateral, fortitude)?;
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::<T>::AuthorEnlisted {
                    author: caller,
                    collateral,
                });
            }
            Ok(())
        }

        /// Provide **economic backing** support to an author using a supported funding model.
        ///
        /// This extrinsic allows the caller to economically support an author by
        /// locking assets under the [`FreezeReason::AuthorFunding`] commitment domain.
        /// Funding directly affects an author's eligibility, exposure, and election weight.
        ///
        /// ## Funding Models
        /// - **Direct:** Funds are committed explicitly to a single author.
        /// - **Index:** Route funds through an index resolving to multiple author digests.
        /// - **Pool:** Commit funds to a managed pool of authors with commission-based withdrawals.
        ///
        /// ## Guarantees
        /// - Funds are **fully locked** and cannot be double-used elsewhere.
        /// - Author collateral is **never mixed** with external funding.
        /// - [`MinFund`] and [`MaxExposure`] are strictly enforced.
        /// - Local minimum and maximum limits enforced by authors and underlying
        ///   systems are considered.
        /// - Funding is rejected if the resolved author or digest is invalid.
        ///
        /// ## Execution Controls
        /// - `precision` defines how strictly the requested amount must be satisfied:
        ///   - [`Precision::Exact`]: Requires full amount commitment.
        ///   - [`Precision::BestEffort`]: Allows partial fulfillment where supported.
        /// - `force` defines how funds are sourced:
        ///   - [`Fortitude::Force`]: Uses the caller's **liquid balance**, enforcing
        ///     the commitment directly.
        ///   - [`Fortitude::Polite`]: Uses funds already deposited in the commitment
        ///     reserve (if [`Config::CommitmentAdapter`] == `pallet_commitment`).
        ///
        /// **Emits** via internal hook:
        ///     - [`Event::AuthorFunded`] if direct author backing
        ///     - [`Event::IndexFunded`] multiple authors via an index
        ///     - [`Event::PoolFunded`] multiple authors via a managed pool
        #[pallet::call_index(1)]
        #[pallet::weight(
        T::WeightInfo::direct_fund()
            .max(T::WeightInfo::index_fund())
            .max(T::WeightInfo::pool_fund())
        )]
        pub fn back(
            origin: OriginFor<T>,
            via: FundingTarget<T>,
            value: AuthorAsset<T>,
            fortitude: FortitudeWrapper,
            precision: PrecisionWrapper,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let (to, funder) = match via {
                FundingTarget::Direct(author) => {
                    let funder = Funder::Direct(caller);
                    (author, funder)
                }
                FundingTarget::Index(index_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let index_entry_shares =
                        T::CommitmentAdapter::get_entries_shares(reason, &index_digest)?;
                    let entry_digest = &index_entry_shares[0].0;
                    // A neccessary value required which is the runtime can get instead of asking
                    // the caller
                    let to = AuthorsDigest::<T>::get(&entry_digest)
                        .ok_or(Error::<T>::AuthorDigestNotFound)?;
                    let index_funder = Funder::Index {
                        digest: index_digest,
                        backer: caller,
                    };
                    (to, index_funder)
                }
                FundingTarget::Pool(pool_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let pool_entry_shares =
                        T::CommitmentAdapter::get_slots_shares(reason, &pool_digest)?;
                    let slot_digest = &pool_entry_shares[0].0;
                    let to = AuthorsDigest::<T>::get(&slot_digest)
                        .ok_or(Error::<T>::AuthorDigestNotFound)?;
                    let pool_funder = Funder::Pool {
                        digest: pool_digest,
                        backer: caller,
                    };
                    (to, pool_funder)
                }
            };
            let precision: Precision = precision.into();
            let fortitude: Fortitude = fortitude.into();
            let actual = <Pallet<T> as FundRoles<Author<T>>>::fund(
                &to, &funder, value, precision, fortitude,
            )?;
            if !T::EmitEvents::get() {
                match funder {
                    Funder::Direct(backer) => {
                        Self::deposit_event(Event::<T>::AuthorFunded {
                            author: to,
                            backer,
                            amount: actual,
                        });
                    }
                    Funder::Index { digest, backer } => {
                        Self::deposit_event(Event::<T>::IndexFunded {
                            index: digest,
                            backer,
                            amount: actual,
                        });
                    }
                    Funder::Pool { digest, backer } => {
                        Self::deposit_event(Event::<T>::PoolFunded {
                            pool: digest,
                            backer,
                            amount: actual,
                        });
                    }
                }
            }
            Ok(())
        }

        /// Increase the caller's **collateral** by locking additional assets.
        ///
        /// Adds to the existing collateral, strengthening the author's position
        /// and ensuring compliance with evolving system requirements such
        /// as [`MinCollateral`].
        ///
        /// ## Behavior
        /// - Collateral is added on top of existing locked collateral.
        ///
        /// ## Execution Controls
        /// - `fortitude` defines how the collateral is sourced:
        ///   - [`FortitudeWrapper::Force`]: Uses the caller's **liquid balance** to place
        ///     the additional collateral.
        ///   - [`FortitudeWrapper::Polite`]: Uses funds already deposited into the
        ///     commitment reserve (if [`Config::CommitmentAdapter`] == `pallet_commitment`).
        ///
        /// **Emits:** [`Event::AuthorCollateralRaised`]
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::refill())]
        pub fn refill(
            origin: OriginFor<T>,
            collateral: AuthorAsset<T>,
            fortitude: FortitudeWrapper,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let fortitude = fortitude.into();
            let raised = <Pallet<T> as RoleManager<Author<T>>>::add_collateral(
                &caller, collateral, fortitude,
            )?;
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::<T>::AuthorCollateralRaised {
                    author: caller,
                    raised,
                });
            }
            Ok(())
        }

        /// Confirm the caller as an **active author** after completing probation.
        ///
        /// Transitions the author from probation to active status once all
        /// probation conditions are satisfied.
        ///
        /// ## Requirements
        /// - Caller must be under probation.
        /// - Probation conditions must be fulfilled.
        ///
        /// ## Notes
        /// - Authors are responsible for completing their probation requirements.
        /// - Activation enables full participation in author duties.
        ///
        /// **Emits:** [`Event::AuthorStatus`] via internal hook
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::confirm())]
        pub fn confirm(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            <Pallet<T> as RoleManager<Author<T>>>::set_status(&caller, AuthorStatus::Active)?;
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::AuthorStatus {
                    author: caller,
                    status: AuthorStatus::Active,
                });
            }
            Ok(())
        }

        /// Exit an existing **backing position** towards an author.
        ///
        /// Releases the caller's committed funds (including any applicable rewards
        /// or penalties) from a direct, index, or pool-based backing, once the
        /// position is eligible for exit.
        ///
        /// ## Behavior
        /// - Exits only the caller's backing position.
        /// - Author collateral remains **unaffected**.
        /// - Other backers' commitments remain intact.
        ///
        /// ## Validation
        /// - Ensures the backing position exists and is withdrawable.
        /// - Resolves the target author from index or pool digests.
        ///
        /// **Emits:**
        /// - [`Event::AuthorDrawn`] for direct backing
        /// - [`Event::IndexDrawn`] for index-based backing
        /// - [`Event::PoolDrawn`] for pool-based backing
        #[pallet::call_index(4)]
        #[pallet::weight(
        T::WeightInfo::release_direct_fund()
            .max(T::WeightInfo::release_index_fund())
            .max(T::WeightInfo::release_pool_fund())
        )]
        pub fn exit(origin: OriginFor<T>, from: FundingTarget<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let (from, funder) = match from {
                FundingTarget::Direct(author) => {
                    let funder = Funder::Direct(caller);
                    (author, funder)
                }
                FundingTarget::Index(index_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let index_entry_shares =
                        T::CommitmentAdapter::get_entries_shares(reason, &index_digest)?;
                    let entry_digest = &index_entry_shares[0].0;
                    let from = AuthorsDigest::<T>::get(&entry_digest)
                        .ok_or(Error::<T>::AuthorDigestNotFound)?;
                    let index_funder = Funder::Index {
                        digest: index_digest,
                        backer: caller,
                    };
                    (from, index_funder)
                }
                FundingTarget::Pool(pool_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let pool_entry_shares =
                        T::CommitmentAdapter::get_slots_shares(reason, &pool_digest)?;
                    let slot_digest = &pool_entry_shares[0].0;
                    let from = AuthorsDigest::<T>::get(&slot_digest)
                        .ok_or(Error::<T>::AuthorDigestNotFound)?;
                    let pool_funder = Funder::Pool {
                        digest: pool_digest,
                        backer: caller,
                    };
                    (from, pool_funder)
                }
            };
            let draw_value = <Pallet<T> as FundRoles<Author<T>>>::draw(&from, &funder)?;
            if !T::EmitEvents::get() {
                match funder {
                    Funder::Direct(backer) => {
                        Self::deposit_event(Event::<T>::AuthorDrawn {
                            author: from,
                            backer,
                            amount: draw_value,
                        });
                    }
                    Funder::Index { digest, backer } => {
                        Self::deposit_event(Event::<T>::IndexDrawn {
                            index: digest,
                            backer,
                            amount: draw_value,
                        });
                    }
                    Funder::Pool { digest, backer } => {
                        Self::deposit_event(Event::<T>::PoolDrawn {
                            pool: digest,
                            backer,
                            amount: draw_value,
                        });
                    }
                }
            }
            Ok(())
        }

        /// Resign from the **author role** and exit active participation.
        ///
        /// Releases the caller's collateral while retaining any external backing
        /// relationships until funders explicitly exit their positions.
        ///
        /// ## Behavior
        /// - Removes the caller from active author participation.
        /// - Releases all locked collateral back to the caller.
        /// - External funders/backers must withdraw separately via [`Pallet::exit`].
        /// - The author's digest is reaped only if no active backing remains.
        ///
        /// **Emits:** [`Event::AuthorResigned`]
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::demit())]
        pub fn demit(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let released = <Pallet<T> as RoleManager<Author<T>>>::resign(&caller)?;
            if !T::EmitEvents::get() {
                Self::deposit_event(Event::<T>::AuthorResigned {
                    author: caller,
                    released,
                });
            }
            Ok(())
        }

        /// Create a new **funding index** over a set of authors.
        ///
        /// An index represents a weighted collection of author commitment digests,
        /// enabling aggregated funding and proportional exposure.
        ///
        /// - Index entries resolve exclusively to author collateral digests.
        /// - Share weights are deterministic and auditable.
        /// - The caller becomes the index owner.
        ///
        /// **Emits:** [`Event::IndexCreated`] via deposit event
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::create_index())]
        pub fn create_index(
            origin: OriginFor<T>,
            entries: Vec<(Author<T>, Shares<T>)>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let mut digest_entries = Vec::new();
            for (author, shares) in entries {
                let author_reason = FreezeReason::AuthorCollateral.into();
                let author_digest =
                    <T as Config>::CommitmentAdapter::get_commit_digest(&author, &author_reason)?;
                digest_entries.push((author_digest, shares));
            }
            let funding_reason = FreezeReason::AuthorFunding.into();
            let index_info = <T as Config>::CommitmentAdapter::prepare_index(
                &caller,
                &funding_reason,
                &digest_entries,
            )?;
            let index_digest = <T as Config>::CommitmentAdapter::gen_index_digest(
                &caller,
                &funding_reason,
                &index_info,
            )?;
            <T as Config>::CommitmentAdapter::set_index(
                &caller,
                &funding_reason,
                &index_info,
                &index_digest,
            )?;

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T>::IndexCreated {
                    index: index_digest,
                });
            }

            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let entries = <T as Config>::CommitmentAdapter::get_entries_shares(
                    &funding_reason,
                    &index_digest,
                )?;
                Self::deposit_event(Event::<T>::IndexCreated {
                    index: index_digest,
                    entries: entries,
                });
            }
            Ok(())
        }

        /// Create a new **funding pool** backed by an existing index.
        ///
        /// Pools enable managed aggregation of funds with an explicit commission
        /// applied to rewards earned via the underlying index.
        ///
        /// - Pool configuration is uniquely identified by its digest.
        /// - Commission is fixed at creation time, changing commission after
        /// creates a new pool with same slots and updated commission.
        /// - The caller becomes the initial pool manager.
        ///
        /// **Emits:** [`Event::PoolCreated`] via deposit event
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::create_pool())]
        pub fn create_pool(
            origin: OriginFor<T>,
            index: IndexDigest<T>,
            commission: Commission<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let funding_reason = FreezeReason::AuthorFunding.into();
            let pool_digest = <T as Config>::CommitmentAdapter::gen_pool_digest(
                &caller,
                &funding_reason,
                &index,
                commission,
            )?;
            <T as Config>::CommitmentAdapter::set_pool(
                &caller,
                &funding_reason,
                &pool_digest,
                &index,
                commission,
            )?;

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T>::PoolCreated {
                    pool: pool_digest,
                    commission: commission,
                    manager: caller,
                });
            }

            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let slots = <T as Config>::CommitmentAdapter::get_slots_shares(
                    &funding_reason,
                    &pool_digest,
                )?;
                Self::deposit_event(Event::<T>::PoolCreated {
                    pool: pool_digest,
                    commission: commission,
                    manager: caller,
                    slots: slots,
                });
            }
            Ok(())
        }

        /// Transfer **management ownership** of a funding pool.
        ///
        /// Updates the pool manager without affecting custody of funds.
        /// The pool remains **non-custodial**, and all underlying funds,
        /// slots, and backing relationships remain unchanged.
        ///
        /// ## Requirements
        /// - Callable only by the current pool manager.
        ///
        /// ## Behavior
        /// - Transfers only management control.
        /// - Does not transfer ownership of any pooled funds.
        ///
        /// **Emits:** [`Event::PoolManager`]
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::transfer_pool())]
        pub fn transfer_pool(
            origin: OriginFor<T>,
            pool: PoolDigest<T>,
            to: T::AccountId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let funding_reason = FreezeReason::AuthorFunding.into();
            let current_manager =
                <T as Config>::CommitmentAdapter::get_manager(&funding_reason, &pool)?;
            ensure!(current_manager == caller, Error::<T>::InvalidPoolManager);
            <T as Config>::CommitmentAdapter::set_pool_manager(&funding_reason, &pool, &to)?;
            Self::deposit_event(Event::<T>::PoolManager {
                digest: pool,
                manager: to,
            });
            Ok(())
        }

        /// Update the commission for a funding pool by creating a new pool instance.
        ///
        /// This operation does **not mutate the existing pool**. Instead, it creates
        /// a new pool derived from the given index with the updated commission,
        /// assigning the caller as the new pool manager.
        ///
        /// ## Behavior
        /// - A new pool is created with the specified commission.
        /// - The caller becomes the manager of the new pool.
        /// - The new pool starts with **no funds or backing**.
        /// - The original pool remains unchanged.
        ///
        /// ## Notes
        /// - Pools are **non-custodial**, and funds are not transferred during this operation.
        /// - This effectively creates a fresh pool configuration with updated parameters.
        ///
        /// **Emits:** [`Event::PoolCreated`]
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::update_commission())]
        pub fn update_commission(
            origin: OriginFor<T>,
            index: IndexDigest<T>,
            commission: Commission<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let funding_reason = FreezeReason::AuthorFunding.into();
            let pool_digest = <T as Config>::CommitmentAdapter::set_commission(
                &caller,
                &funding_reason,
                &index,
                commission,
            )?;

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T>::PoolCreated {
                    pool: pool_digest,
                    commission: commission,
                    manager: caller,
                });
            }

            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let slots = <T as Config>::CommitmentAdapter::get_slots_shares(
                    &funding_reason,
                    &pool_digest,
                )?;
                Self::deposit_event(Event::<T>::PoolCreated {
                    pool: pool_digest,
                    commission: commission,
                    manager: caller,
                    slots: slots,
                });
            }
            Ok(())
        }

        /// Derive a **new index** with updated share weight for a specific entry.
        ///
        /// This operation does **not mutate** the existing index. Instead, it creates
        /// a new index configuration where the specified `entry` is assigned the
        /// given `shares`, while all other entries remain unchanged.
        ///
        /// ## Behavior
        /// - Produces a new index digest reflecting the updated share distribution.
        /// - The original index remains immutable and unchanged.
        ///
        /// ## Notes
        /// - Indexes are **immutable** once created.
        /// - Any modification to entry shares results in a **new index instance**.
        ///
        /// **Emits:** [`Event::IndexCreated`]
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::update_entry_shares())]
        pub fn update_entry_shares(
            origin: OriginFor<T>,
            index: IndexDigest<T>,
            entry: IndexDigest<T>,
            shares: Shares<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let funding_reason = FreezeReason::AuthorFunding.into();
            let new_index = <T as Config>::CommitmentAdapter::set_entry_shares(
                &caller,
                &funding_reason,
                &index,
                &entry,
                shares,
            )?;

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T>::IndexCreated { index: new_index });
            }

            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let entries = <T as Config>::CommitmentAdapter::get_entries_shares(
                    &funding_reason,
                    &new_index,
                )?;
                Self::deposit_event(Event::<T>::IndexCreated {
                    index: new_index,
                    entries: entries,
                });
            }
            Ok(())
        }

        /// Update the share weight of a **slot** within an existing pool.
        ///
        /// Modifies the share allocation of the specified `slot` in the given `pool`
        /// without altering the pool's identity or existing commitments.
        ///
        /// ## Requirements
        /// - Caller must be the current pool manager.
        /// - The specified pool and slot must be valid.
        ///
        /// ## Behavior
        /// - Updates the slot's share weight **in place**.
        /// - Preserves all existing funds, commitments, and pool configuration.
        ///
        /// ## Notes
        /// - Unlike indexes, pools are **mutable** and support in-place updates.
        ///
        /// **Emits:** [`Event::PoolSlotShare`]
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::update_slot_shares())]
        pub fn update_slot_shares(
            origin: OriginFor<T>,
            pool: PoolDigest<T>,
            slot: PoolDigest<T>,
            shares: Shares<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let funding_reason = FreezeReason::AuthorFunding.into();
            let current_manager =
                <T as Config>::CommitmentAdapter::get_manager(&funding_reason, &pool)?;
            ensure!(current_manager == caller, Error::<T>::InvalidPoolManager);
            <T as Config>::CommitmentAdapter::set_slot_shares(
                &caller,
                &funding_reason,
                &pool,
                &slot,
                shares,
            )?;
            Self::deposit_event(Event::<T>::PoolSlotShare {
                pool,
                slots: (slot, shares),
            });
            Ok(())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ````````````````````````````````` INSPECTORS ``````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Inspect and emit the caller's current **total collateral**.
        ///
        /// Provides an up-to-date view of the caller's collateral, since on-chain
        /// storage may reflect only a lazy or stale balance representation.
        ///
        /// ## Behavior
        /// - Emits the latest computed collateral value.
        /// - Performs no state mutation.
        ///
        /// **Emits:** [`Event::AuthorTotalCollateral`]
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::my_collateral())]
        pub fn my_collateral(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            <Pallet<T> as RoleManager<Author<T>>>::role_exists(&caller)?;
            let collateral = <Pallet<T> as RoleManager<Author<T>>>::get_collateral(&caller)?;
            Self::deposit_event(Event::<T>::AuthorTotalCollateral {
                author: caller,
                collateral,
            });
            Ok(())
        }

        /// Inspect the caller's **total committed funds** under a specific funding target.
        ///
        /// Provides a read-only view of the caller's currently locked funds
        /// across supported funding models.
        ///
        /// ## Behavior
        /// - **Direct:** Emits funds committed to a single author.
        /// - **Index:** Emits total funds committed via an index.
        /// - **Pool:** Emits total funds committed via a pool.
        ///
        /// ## Validation
        /// - Ensures the provided index or pool digest matches the caller's
        ///   active commitment.
        /// - Fails if the digest is invalid or belongs to another commitment.
        ///
        /// **Emits:**
        /// - [`Event::InspectAuthorFund`] for direct funding
        /// - [`Event::InspectIndexFund`] for index-based funding
        /// - [`Event::InspectPoolFund`] for pool-based funding
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(13)]
        #[pallet::weight(
        T::WeightInfo::check_direct_fund()
            .max(T::WeightInfo::check_index_fund())
            .max(T::WeightInfo::check_pool_fund())
        )]
        pub fn my_fund(origin: OriginFor<T>, from: FundingTarget<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            match from {
                FundingTarget::Direct(author) => {
                    let funder = Funder::<T>::Direct(caller.clone());
                    let amount = <Pallet<T> as FundRoles<Author<T>>>::get_fund(&author, &funder)?;
                    Self::deposit_event(Event::<T>::InspectAuthorFund {
                        author,
                        backer: caller,
                        amount,
                    });
                }
                FundingTarget::Index(index_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let actual_digest = T::CommitmentAdapter::get_commit_digest(&caller, reason)?;
                    T::CommitmentAdapter::index_exists(reason, &actual_digest)?;
                    ensure!(
                        index_digest == actual_digest,
                        Error::<T>::FundedToAnotherDigest
                    );
                    let index_fund = T::CommitmentAdapter::get_commit_value(&caller, reason)?;
                    Self::deposit_event(Event::<T>::InspectIndexFund {
                        index: index_digest,
                        backer: caller,
                        amount: index_fund,
                    });
                }
                FundingTarget::Pool(pool_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let actual_digest = T::CommitmentAdapter::get_commit_digest(&caller, reason)?;
                    T::CommitmentAdapter::pool_exists(reason, &actual_digest)?;
                    ensure!(
                        pool_digest == actual_digest,
                        Error::<T>::FundedToAnotherDigest
                    );
                    let pool_fund = T::CommitmentAdapter::get_commit_value(&caller, reason)?;
                    Self::deposit_event(Event::<T>::InspectPoolFund {
                        pool: pool_digest,
                        backer: caller,
                        amount: pool_fund,
                    });
                }
            };
            Ok(())
        }

        /// Inspect the caller's committed funds **towards a specific author**.
        ///
        /// Resolves the caller's funding relationship with the given author
        /// across the specified funding target.
        ///
        /// ## Behavior
        /// - Supports **Direct**, **Index**, and **Pool** funding models.
        /// - Emits the amount currently committed by the caller to the specified
        /// author.
        ///
        /// **Emits:** [`Event::InspectFund`]
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::check_index_fund_towards()
            .max(T::WeightInfo::check_pool_fund_towards())
            .max(T::WeightInfo::check_direct_fund())
        )]
        pub fn my_author_fund(
            origin: OriginFor<T>,
            author: Author<T>,
            from: FundingTarget<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let funder = match from {
                FundingTarget::Direct(_author) => {
                    let funder = Funder::<T>::Direct(caller);
                    funder
                }
                FundingTarget::Index(index_digest) => {
                    let funder = Funder::<T>::Index {
                        digest: index_digest,
                        backer: caller,
                    };
                    funder
                }
                FundingTarget::Pool(pool_digest) => {
                    let funder = Funder::<T>::Pool {
                        digest: pool_digest,
                        backer: caller,
                    };
                    funder
                }
            };
            let amount = <Pallet<T> as FundRoles<Author<T>>>::get_fund(&author, &funder)?;
            Self::deposit_event(Event::<T>::InspectFund {
                author,
                funder,
                amount,
            });
            Ok(())
        }

        /// Shed (expose) all **scheduled rewards** for the caller i.e., author.
        ///
        /// Retrieves and emits the rewards currently scheduled for the caller
        /// without modifying state.
        ///
        /// **Emits:** [`Event::ScheduledRewards`] via internal hook
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::shed_rewards())]
        pub fn shed_rewards(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::has_reward(&caller)?;
            let shed_rewards = Self::get_rewards_of(&caller)?;
            Self::deposit_event(Event::<T>::ScheduledRewards {
                author: caller,
                rewards: shed_rewards,
            });
            Ok(())
        }

        /// Shed (expose) all **scheduled penalties** for the caller.
        ///
        /// Retrieves and emits the penalties currently scheduled for the caller
        /// without modifying state.
        ///
        /// ## Notes
        /// - Each penalty factor represents a **ratio applied to the author's
        ///   total funds**, including both collateral and external backing.
        ///
        /// **Emits:** [`Event::ScheduledPenalties`]
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::shed_penalties())]
        pub fn shed_penalties(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::has_penalty(&caller)?;
            let shed_penalties = Self::get_penalties_of(&caller)?;
            Self::deposit_event(Event::<T>::ScheduledPenalties {
                author: caller,
                penalties: shed_penalties,
            });
            Ok(())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ``````````````````````````````` ROOT PRIVILEGED ```````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Force-update a selected genesis configuration parameter.
        ///
        /// **Origin:** Root only.
        ///
        /// This extrinsic allows privileged modification of runtime parameters
        /// that were originally defined at genesis.
        ///
        /// - `ProbationPeriod` - Updates the number of blocks authors must remain in probation.
        /// - `ReduceProbationBy` - Updates how much probation is reduced on good behavior.
        /// - `IncreaseProbationBy` - Updates how much probation is increased on misbehavior.
        /// - `RewardsBuffer` - Updates the delay (in blocks) before rewards are finalized.
        /// - `PenaltiesBuffer` - Updates the delay (in blocks) before penalties are enforced.
        /// - `MaxElected` - Updates the maximum number of authors that can be elected.
        /// - `MinElected` - Updates the minimum number of authors required for a valid election.
        /// - `EnforceMaxElected` - Toggles strict enforcement of the `MaxElected` limit.
        /// - `MinFund` - Updates the minimum funding required per backing operation.
        /// - `MaxExposure` - Updates the maximum allowed exposure per funding operation.
        /// - `MinCollateral` - Updates the minimum collateral required for authors.
        ///
        /// The call enforces consistency constraints where applicable:
        /// - Values that must be non-zero will fail with [`Error::NonZeroConfigRequired`].
        /// - Fails with [`Error::MinGreaterThanMax`] if:
        ///   - `MinElected > MaxElected`, or
        ///   - `MinFund > MaxExposure`, or
        ///   - `MaxElected < MinElected`, or
        ///   - `MaxExposure < MinFund`.
        ///
        /// This call directly overwrites storage and emits an event containing the
        /// updated configuration variant.
        #[pallet::call_index(17)]
        #[pallet::weight(
        T::WeightInfo::force_probation_period()
            .max(T::WeightInfo::force_enforce_max_elected())
            .max(T::WeightInfo::force_increase_probation_by())
            .max(T::WeightInfo::force_max_elected())
            .max(T::WeightInfo::force_max_exposure())
            .max(T::WeightInfo::force_min_collateral())
            .max(T::WeightInfo::force_min_elected())
            .max(T::WeightInfo::force_min_fund())
            .max(T::WeightInfo::force_penalties_buffer())
            .max(T::WeightInfo::force_reduce_probation_by())
            .max(T::WeightInfo::force_rewards_buffer())
        )]
        pub fn force_genesis_config(
            origin: OriginFor<T>,
            field: ForceGenesisConfig<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            match field {
                ForceGenesisConfig::ProbationPeriod(block) => ProbationPeriod::<T>::put(block),
                ForceGenesisConfig::ReduceProbationBy(block) => ReduceProbationBy::<T>::put(block),
                ForceGenesisConfig::IncreaseProbationBy(block) => {
                    IncreaseProbationBy::<T>::put(block)
                }
                ForceGenesisConfig::RewardsBuffer(block) => RewardsBuffer::<T>::put(block),
                ForceGenesisConfig::PenaltiesBuffer(block) => PenaltiesBuffer::<T>::put(block),
                ForceGenesisConfig::MaxElected(max_elected) => {
                    ensure!(!max_elected.is_zero(), Error::<T>::NonZeroConfigRequired);
                    ensure!(
                        max_elected >= MinElected::<T>::get(),
                        Error::<T>::MinGreaterThanMax
                    );
                    MaxElected::<T>::put(max_elected);
                }
                ForceGenesisConfig::MinElected(min_elected) => {
                    ensure!(!min_elected.is_zero(), Error::<T>::NonZeroConfigRequired);
                    ensure!(
                        min_elected <= MaxElected::<T>::get(),
                        Error::<T>::MinGreaterThanMax
                    );
                    MinElected::<T>::put(min_elected);
                }
                ForceGenesisConfig::EnforceMaxElected(bool) => ForceMaxElected::<T>::put(bool),
                ForceGenesisConfig::MinFund(asset) => {
                    ensure!(!asset.is_zero(), Error::<T>::NonZeroConfigRequired);
                    ensure!(
                        asset <= MaxExposure::<T>::get(),
                        Error::<T>::MinGreaterThanMax
                    );
                    MinFund::<T>::put(asset);
                }
                ForceGenesisConfig::MaxExposure(asset) => {
                    ensure!(asset >= MinFund::<T>::get(), Error::<T>::MinGreaterThanMax);
                    MaxExposure::<T>::put(asset);
                }
                ForceGenesisConfig::MinCollateral(asset) => {
                    ensure!(!asset.is_zero(), Error::<T>::NonZeroConfigRequired);
                    MinCollateral::<T>::put(asset);
                }
            }
            Self::deposit_event(Event::GenesisConfigUpdated(field));
            Ok(())
        }
    }

    // ===============================================================================
    // ````````````````````````````````` PUBLIC APIS `````````````````````````````````
    // ===============================================================================

    /// Public read-only functions for inspecting author collateral and funding state.
    ///
    /// This interface exposes non-mutating functions that allow external consumers
    /// (e.g. off-chain clients, RPC layers, and other pallets) to inspect the
    /// economic state of authors and funding relationships.
    impl<T: Config> Pallet<T> {
        /// Return the total **locked collateral** of an author.
        ///  - `who` must be an enrolled author.
        pub fn fetch_collateral(who: Author<T>) -> Result<AuthorAsset<T>, DispatchError> {
            <Pallet<T> as RoleManager<Author<T>>>::role_exists(&who)?;
            let total_collateral = <Pallet<T> as RoleManager<Author<T>>>::get_collateral(&who)?;
            Ok(total_collateral)
        }

        /// Fetch the caller's **total committed funding** under a specific funding model.
        ///
        /// - The caller must have an active commitment under the specified target.
        /// - The provided digest must match the caller's active commitment.
        pub fn inspect_fund(
            caller: T::AccountId,
            from: FundingTarget<T>,
        ) -> Result<AuthorAsset<T>, DispatchError> {
            match from {
                FundingTarget::Direct(author) => {
                    let funder = Funder::<T>::Direct(caller);
                    let fund = <Pallet<T> as FundRoles<Author<T>>>::get_fund(&author, &funder)?;
                    return Ok(fund);
                }
                FundingTarget::Index(index_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let actual_digest = T::CommitmentAdapter::get_commit_digest(&caller, reason)?;
                    T::CommitmentAdapter::index_exists(reason, &actual_digest)?;
                    ensure!(
                        index_digest == actual_digest,
                        Error::<T>::FundedToAnotherDigest
                    );
                    let index_fund = T::CommitmentAdapter::get_commit_value(&caller, reason)?;
                    return Ok(index_fund);
                }
                FundingTarget::Pool(pool_digest) => {
                    let reason = &FreezeReason::AuthorFunding.into();
                    let actual_digest = T::CommitmentAdapter::get_commit_digest(&caller, reason)?;
                    T::CommitmentAdapter::pool_exists(reason, &actual_digest)?;
                    ensure!(
                        pool_digest == actual_digest,
                        Error::<T>::FundedToAnotherDigest
                    );
                    let pool_fund = T::CommitmentAdapter::get_commit_value(&caller, reason)?;
                    return Ok(pool_fund);
                }
            };
        }

        /// Fetch the caller's **committed funding towards a specific author**.
        ///
        /// - The caller must have an active funding relationship with the author
        ///   under the specified funding target.
        pub fn inspect_author_fund(
            caller: T::AccountId,
            author: Author<T>,
            from: FundingTarget<T>,
        ) -> Result<AuthorAsset<T>, DispatchError> {
            let funder = match from {
                FundingTarget::Direct(_author) => {
                    let funder = Funder::<T>::Direct(caller);
                    funder
                }
                FundingTarget::Index(index_digest) => {
                    let funder = Funder::<T>::Index {
                        digest: index_digest,
                        backer: caller,
                    };
                    funder
                }
                FundingTarget::Pool(pool_digest) => {
                    let funder = Funder::<T>::Pool {
                        digest: pool_digest,
                        backer: caller,
                    };
                    funder
                }
            };
            let fund = <Pallet<T> as FundRoles<Author<T>>>::get_fund(&author, &funder)?;
            Ok(fund)
        }
    }

    // ===============================================================================
    // `````````````````````````` REUSABLE ELECTION MODELS ```````````````````````````
    // ===============================================================================

    /// A **FlatElection** represents an election model in which an author's
    /// **entire backing position** is **flattened** into a single aggregated
    /// influence value.
    ///
    /// In this model, all sources of support associated with an author - including:
    /// - **self-collateral**, and
    /// - **external backing** (direct backers, unmanaged indexes, or managed pools)
    /// are first **combined into one total backing value**.
    ///
    /// This flattened value is then passed through the **influence computation plugin**
    /// to derive a single scalar influence metric for the author.
    ///
    /// The resulting influence values are used to compare authors **uniformly**,
    /// independent of how their backing is structured or distributed.
    ///
    /// ## Characteristics
    /// - Aggregates **all sources of support** (self + external) into one value.
    /// - Produces **exactly one influence metric per author**.
    /// - Discards backer-level structure and granularity.
    /// - Enables simple, deterministic, influence-based comparison.
    ///
    /// ## Election Flow
    /// 1. Collect all backing associated with each author (self + external).
    /// 2. Flatten the backing into a single aggregate value.
    /// 3. Compute influence using the [`Config::InfluenceModel`] plugin.
    /// 4. Rank or select authors using the [`Config::FlatElectionModel`] plugin.
    ///
    /// ## Typical Use-Case
    /// - Selecting authors or producers based purely on **total effective influence**,
    ///   where the origin or distribution of support is intentionally ignored.
    ///
    /// ## Summary
    /// `FlatElection` **flattens the backing graph**:
    /// all forms of support are reduced to a single comparable influence metric.
    ///
    /// The model is simple, deterministic, and influence-driven, but it
    /// **does not preserve fairness across individual backers**.
    pub struct FlatElection<T: Config>(PhantomData<T>);

    /// A **FairElection** represents an election model that preserves the
    /// **structure and fairness of backing** by retaining each supporter's
    /// individual contribution weight.
    ///
    /// Unlike [`FlatElection`], this model **does not flatten backing**.
    /// Instead, every supporter - including the author themselves - is treated
    /// as an individual backer with a distinct weight.
    ///
    /// Self-collateral is therefore considered **self-backing**, contributing
    /// to the author's support while still remaining structurally independent
    /// from other backers.
    ///
    /// Both:
    /// - **self-backing** (author's own collateral), and
    /// - **external backing** (direct backers, unmanaged indexes, or managed pools)
    /// are preserved as **separate weighted relationships** rather than being
    /// aggregated into a single scalar.
    ///
    /// Each backing weight is mapped **individually** to the author they support,
    /// and the election outcome is computed from these unaggregated
    /// backer-author relationships.
    ///
    /// ## Characteristics
    /// - Treats **self-collateral as self-backing** (not flattened).
    /// - Preserves **individual backer weights** without aggregation.
    /// - Maintains structural fairness across all supporters.
    /// - Reflects both commitment (self-backing) and community support.
    /// - Emphasizes proportional and decentralized representation.
    ///
    /// ## Election Flow
    /// 1. Gather all backers for each author, including self-backing.
    /// 2. Map each backer's weight individually to their chosen author.
    /// 3. Provide these mappings to the [`Config::FairElectionModel`] plugin.
    /// 4. Compute outcomes that reflect **proportional and distributed support**.
    ///
    /// ## Typical Use-Case
    /// - Selecting representatives, validators, or leaders where
    ///   **fair proportional influence** from all supporters - including the
    ///   author's own stake - should be preserved without flattening.
    ///
    /// ## Summary
    /// `FairElection` **preserves the backing structure**:
    /// every supporter (self or external) is represented individually, and each
    /// weight contributes proportionally without being merged into a single value.
    ///
    /// The model prioritizes **fair representation and structural integrity of support**.
    pub struct FairElection<T: Config>(PhantomData<T>);
}

// ===============================================================================
// `````````````````````````````````` API TESTS ``````````````````````````````````
// ===============================================================================

#[cfg(test)]
mod ext_tests {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- Local crate imports ---
    use crate::{
        mock::authors_test_ext,
        mock::*,
        types::{
            AuthorStatus, ForceGenesisConfig, FortitudeWrapper, Funder, FundingTarget,
            PrecisionWrapper,
        },
        FreezeReason,
    };

    // --- FRAME Suite ---
    use frame_suite::{
        commitment::*,
        roles::{CompensateRoles, FundRoles, RoleManager, RoleProbation},
    };

    // --- FRAME Support ---
    use frame_support::{
        assert_err, assert_ok,
        traits::{
            tokens::{Fortitude, Precision},
            Hooks,
        },
    };

    // --- Substrate primitives ---
    use sp_runtime::{DispatchError, Perbill};

    // ===============================================================================
    // ``````````````````````````````````` HELPERS ```````````````````````````````````
    // ===============================================================================

    // Finds latest IndexCreated event and returns its digest
    fn assert_index_created_and_get_digest() -> IndexDigest {
        System::events()
            .iter()
            .rev()
            .find_map(|record| {
                if let RuntimeEvent::Authors(Event::IndexCreated { index, .. }) = &record.event {
                    Some(index.clone())
                } else {
                    None
                }
            })
            .expect("IndexCreated event not emitted")
    }

    // Finds latest PoolCreated event and returns its digest
    fn assert_pool_created_and_get_digest() -> PoolDigest {
        System::events()
            .iter()
            .rev()
            .find_map(|record| {
                if let RuntimeEvent::Authors(Event::PoolCreated { pool, .. }) = &record.event {
                    Some(pool.clone())
                } else {
                    None
                }
            })
            .expect("PoolCreated event not emitted")
    }

    // ===============================================================================
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ===============================================================================

    #[test]
    fn on_initialize_succes() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                125,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(16);
            Pallet::set_permanence(&ALICE).unwrap();

            let author_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(author_collateral, 250);
            let author_backing = Pallet::get_fund(&ALICE, &Funder::Direct(CHARLIE)).unwrap();
            assert_eq!(author_backing, 125);
            let hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(hold, 375);

            System::set_block_number(20);
            let reward = 20;
            Pallet::reward(&ALICE, reward, Precision::Exact).unwrap();

            assert_ok!(Pallet::has_reward(&ALICE));
            System::set_block_number(22);
            Pallet::on_initialize(22);

            let hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(hold, 395); // reward is applied to the hold (375 + 20) -> 395

            // The reward split proportionally between the author's own collateral and the external backing.
            let author_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(author_collateral, 263);
            let author_backing = Pallet::get_fund(&ALICE, &Funder::Direct(CHARLIE)).unwrap();
            assert_eq!(author_backing, 132);

            assert_err!(Pallet::has_reward(&ALICE), Error::RewardNotFound);

            System::set_block_number(24);
            let penalty = Perbill::from_percent(10);
            Pallet::penalize(&ALICE, penalty).unwrap();

            assert_ok!(Pallet::has_penalty(&ALICE));
            System::set_block_number(28);
            Pallet::on_initialize(28);

            let hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(hold, 356); // 10% penalty is applied to the hold (395 - 39) -> 356

            // The penalty split proportionally between the author's own
            // collateral and the external backing.
            let author_collateral = Pallet::get_collateral(&ALICE).unwrap();
            // Catering to one unit rounding loss
            assert!(author_collateral == 237 || author_collateral == 236);
            let author_backing = Pallet::get_fund(&ALICE, &Funder::Direct(CHARLIE)).unwrap();
            assert!(author_backing == 119 || author_backing == 118);

            assert_err!(Pallet::has_penalty(&ALICE), Error::PenaltyNotFound);
        })
    }

    // ===============================================================================
    // ````````````````````````````````` EXTRINSICS ``````````````````````````````````
    // ===============================================================================

    #[test]
    fn enroll_author_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            assert_err!(Pallet::role_exists(&ALICE), Error::AuthorNotFound);
            System::set_block_number(10);
            assert_ok!(Pallet::enlist(
                RuntimeOrigin::signed(ALICE),
                100,
                FortitudeWrapper::Force
            ));

            assert_ok!(Pallet::role_exists(&ALICE));
            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.since, 10);
            assert_eq!(meta.status, AuthorStatus::Probation);

            System::assert_last_event(
                Event::AuthorEnlisted {
                    author: ALICE,
                    collateral: 100,
                }
                .into(),
            );
        })
    }

    #[test]
    fn enroll_author_inadequate_collateral() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            assert_err!(Pallet::role_exists(&ALICE), Error::AuthorNotFound);
            System::set_block_number(4);
            assert_err!(
                Pallet::enlist(RuntimeOrigin::signed(ALICE), 15, FortitudeWrapper::Force),
                Error::InadequateCollateral
            );
        })
    }

    #[test]
    fn enroll_author_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            assert_err!(
                Pallet::role_exists(&ALICE),
                Error::AuthorNotFound
            );
            System::set_block_number(4);
            assert_err!{Pallet::enlist(RuntimeOrigin::root(), 100, FortitudeWrapper::Force), DispatchError::BadOrigin};
        })
    }

    #[test]
    fn resign_author_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            Pallet::set_status(&ALICE, AuthorStatus::Active).unwrap();

            System::set_block_number(25);
            assert_ok!(Pallet::demit(RuntimeOrigin::signed(ALICE)));

            System::assert_last_event(
                Event::AuthorResigned {
                    author: ALICE,
                    released: 100,
                }
                .into(),
            );
        })
    }

    #[test]
    fn resign_author_err_author_in_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(15);
            assert_err!(
                Pallet::demit(RuntimeOrigin::signed(ALICE)),
                Error::AuthorInProbation
            );
        })
    }

    #[test]
    fn resign_author_err_author_has_penalties() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            Pallet::set_status(&ALICE, AuthorStatus::Active).unwrap();

            System::set_block_number(25);
            Pallet::penalize(&ALICE, Perbill::from_percent(5)).unwrap();

            System::set_block_number(26);
            assert_err!(
                Pallet::demit(RuntimeOrigin::signed(ALICE)),
                Error::AuthorHasPenalties
            );
        })
    }

    #[test]
    fn resign_author_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            Pallet::set_status(&ALICE, AuthorStatus::Active).unwrap();

            System::set_block_number(25);
            assert_err!(
                Pallet::demit(RuntimeOrigin::root()),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn raise_collateral_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            let current_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(current_collateral, 100);

            System::set_block_number(15);
            Pallet::refill(RuntimeOrigin::signed(ALICE), 50, FortitudeWrapper::Force).unwrap();

            let current_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(current_collateral, 150);

            System::assert_last_event(
                Event::AuthorCollateralRaised {
                    author: ALICE,
                    raised: 50,
                }
                .into(),
            );
        })
    }

    #[test]
    fn raise_collateral_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(15);
            assert_err!(
                Pallet::refill(RuntimeOrigin::root(), 50, FortitudeWrapper::Force),
                DispatchError::BadOrigin
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_collateral_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            assert_ok!(Pallet::my_collateral(RuntimeOrigin::signed(ALICE)));

            System::assert_last_event(
                Event::AuthorTotalCollateral {
                    author: ALICE,
                    collateral: 100,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_collateral_err_author_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::my_collateral(RuntimeOrigin::signed(BOB)),
                Error::AuthorNotFound
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_collateral_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::my_collateral(RuntimeOrigin::root()),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn fund_author_direct_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(current_hold, 200);
            let backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(backed_value, 100);
            let backers_of = Pallet::backers_of(&ALICE).unwrap();
            assert_eq!(backers_of, vec![(Funder::Direct(CHARLIE), 100)]);

            System::assert_last_event(
                Event::AuthorFunded {
                    author: ALICE,
                    backer: CHARLIE,
                    amount: 100,
                }
                .into(),
            );
        })
    }

    #[test]
    fn fund_author_direct_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_err!(
                Pallet::back(
                    RuntimeOrigin::root(),
                    FundingTarget::Direct(ALICE),
                    100,
                    FortitudeWrapper::Force,
                    PrecisionWrapper::Exact
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn fund_author_direct_err_below_minimum_fund() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_err!(
                Pallet::back(
                    RuntimeOrigin::signed(CHARLIE),
                    FundingTarget::Direct(ALICE),
                    15,
                    FortitudeWrapper::Force,
                    PrecisionWrapper::Exact
                ),
                Error::BelowMinimumFund
            );
        })
    }

    #[test]
    fn fund_author_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 100);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 150);

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(MIKE),
                FundingTarget::Index(INDEX_DIGEST),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let backers_of_alice = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers_of_alice = vec![(by.clone(), 60), (Funder::Direct(CHARLIE), 50)];
            assert_eq!(backers_of_alice, expected_backers_of_alice);

            let backers_of_bob = Pallet::backers_of(&BOB).unwrap();
            let expected_backers_of_bob = vec![(by.clone(), 40), (Funder::Direct(ALAN), 100)];
            assert_eq!(backers_of_bob, expected_backers_of_bob);

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 160);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 190);

            System::assert_last_event(
                Event::IndexFunded {
                    index: INDEX_DIGEST,
                    backer: MIKE,
                    amount: 100,
                }
                .into(),
            );
        })
    }

    #[test]
    fn fund_author_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50);
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100);

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 100);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 150);

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(MIKE),
                FundingTarget::Pool(POOL_DIGEST),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through index as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 160);
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 190);

            System::assert_last_event(
                Event::PoolFunded {
                    pool: POOL_DIGEST,
                    backer: MIKE,
                    amount: 100,
                }
                .into(),
            );
        })
    }

    #[test]
    fn release_fund_direct_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(current_hold, 200);
            let backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(backed_value, 100);
            let backers_of = Pallet::backers_of(&ALICE).unwrap();
            assert_eq!(backers_of, vec![(Funder::Direct(CHARLIE), 100)]);

            let charlie_balance = get_user_balance(&CHARLIE);
            assert_eq!(charlie_balance, 100);

            System::set_block_number(25);
            assert_ok!(Pallet::exit(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE)
            ));

            let current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(current_hold, 100);

            let backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(backed_value, 0);

            let backers_of = Pallet::backers_of(&ALICE).unwrap();
            assert_eq!(backers_of, vec![]);

            let charlie_balance = get_user_balance(&CHARLIE);
            assert_eq!(charlie_balance, 200);

            System::assert_last_event(
                Event::AuthorDrawn {
                    author: ALICE,
                    backer: CHARLIE,
                    amount: 100,
                }
                .into(),
            );
        })
    }

    #[test]
    fn release_fund_direct_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(current_hold, 200);
            let backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(backed_value, 100);
            let backers_of = Pallet::backers_of(&ALICE).unwrap();
            assert_eq!(backers_of, vec![(Funder::Direct(CHARLIE), 100)]);

            let charlie_balance = get_user_balance(&CHARLIE);
            assert_eq!(charlie_balance, 100);

            System::set_block_number(25);
            assert_err!(
                Pallet::exit(RuntimeOrigin::root(), FundingTarget::Direct(ALICE)),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn release_fund_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through index as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            assert_ok!(Pallet::exit(
                RuntimeOrigin::signed(MIKE),
                FundingTarget::Index(INDEX_DIGEST)
            ));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50); // 50 (existing) - 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100); // 100 (existing) - 40 (through index as BOB share is 40 )

            assert!(!AuthorFunders::contains_key((ALICE, MIKE)));

            let mike_balance = get_user_balance(&MIKE);
            assert_eq!(mike_balance, 200);
        })
    }

    #[test]
    fn release_fund_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(&ALICE, &by, LARGE_VALUE, Precision::Exact, Fortitude::Force).unwrap();

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 250);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 110); // 50 (existing) + 60 (through pool as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 140); // 100 (existing) + 40 (through pool as BOB share is 40 )

            let author_funders = AuthorFunders::get((ALICE, MIKE)).unwrap();
            assert_eq!(author_funders, by);
            let author_funders = AuthorFunders::get((BOB, MIKE)).unwrap();
            assert_eq!(author_funders, by);

            assert_ok!(Pallet::exit(
                RuntimeOrigin::signed(MIKE),
                FundingTarget::Pool(POOL_DIGEST)
            ));

            let total_backing = Pallet::total_backing();
            assert_eq!(total_backing, 150);
            let alice_backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backed_value, 50); // 50 (existing) - 60 (through index as ALICE share is 60 )
            let bob_backed_value = Pallet::backed_value(&BOB).unwrap();
            assert_eq!(bob_backed_value, 100); // 100 (existing) - 40 (through index as BOB share is 40 )

            assert!(!AuthorFunders::contains_key((ALICE, MIKE)));

            let mike_balance = get_user_balance(&MIKE);
            assert_eq!(mike_balance, 195); // 100 (existing) + 100 (backed) - 5 (commission)
            let alan_balance = get_user_balance(&ALAN);
            assert_eq!(alan_balance, 105); // 100 (existing) + 5 (commission)
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_fund_direct_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(current_hold, 200);
            let backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(backed_value, 100);
            let backers_of = Pallet::backers_of(&ALICE).unwrap();
            assert_eq!(backers_of, vec![(Funder::Direct(CHARLIE), 100)]);

            System::set_block_number(25);
            assert_ok!(Pallet::my_fund(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE)
            ));

            System::assert_last_event(
                Event::InspectAuthorFund {
                    author: ALICE,
                    backer: CHARLIE,
                    amount: 100,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_fund_towards_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by_mike = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Index {
                digest: INDEX_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(25);
            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(MIKE),
                ALICE,
                FundingTarget::Index(INDEX_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: ALICE,
                    funder: by_mike.clone(),
                    amount: 60,
                }
                .into(),
            );

            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(MIKE),
                BOB,
                FundingTarget::Index(INDEX_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: BOB,
                    funder: by_mike,
                    amount: 40,
                }
                .into(),
            );

            System::set_block_number(26);
            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(NIX),
                ALICE,
                FundingTarget::Index(INDEX_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: ALICE,
                    funder: by_nix.clone(),
                    amount: 30,
                }
                .into(),
            );

            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(NIX),
                BOB,
                FundingTarget::Index(INDEX_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: BOB,
                    funder: by_nix,
                    amount: 20,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_fund_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by_mike = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Index {
                digest: INDEX_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(25);
            assert_ok!(Pallet::my_fund(
                RuntimeOrigin::signed(MIKE),
                FundingTarget::Index(INDEX_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectIndexFund {
                    index: INDEX_DIGEST,
                    backer: MIKE,
                    amount: 100,
                }
                .into(),
            );

            System::set_block_number(26);
            assert_ok!(Pallet::my_fund(
                RuntimeOrigin::signed(NIX),
                FundingTarget::Index(INDEX_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectIndexFund {
                    index: INDEX_DIGEST,
                    backer: NIX,
                    amount: 50,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_fund_towards_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by_mike = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Pool {
                digest: POOL_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(25);
            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(MIKE),
                ALICE,
                FundingTarget::Pool(POOL_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: ALICE,
                    funder: by_mike.clone(),
                    amount: 60,
                }
                .into(),
            );

            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(MIKE),
                BOB,
                FundingTarget::Pool(POOL_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: BOB,
                    funder: by_mike,
                    amount: 40,
                }
                .into(),
            );

            System::set_block_number(26);
            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(NIX),
                ALICE,
                FundingTarget::Pool(POOL_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: ALICE,
                    funder: by_nix.clone(),
                    amount: 30,
                }
                .into(),
            );

            assert_ok!(Pallet::my_author_fund(
                RuntimeOrigin::signed(NIX),
                BOB,
                FundingTarget::Pool(POOL_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectFund {
                    author: BOB,
                    funder: by_nix,
                    amount: 20,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_fund_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by_mike = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Pool {
                digest: POOL_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(25);
            assert_ok!(Pallet::my_fund(
                RuntimeOrigin::signed(MIKE),
                FundingTarget::Pool(POOL_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectPoolFund {
                    pool: POOL_DIGEST,
                    backer: MIKE,
                    amount: 100,
                }
                .into(),
            );

            System::set_block_number(26);
            assert_ok!(Pallet::my_fund(
                RuntimeOrigin::signed(NIX),
                FundingTarget::Pool(POOL_DIGEST)
            ));

            System::assert_last_event(
                Event::InspectPoolFund {
                    pool: POOL_DIGEST,
                    backer: NIX,
                    amount: 50,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn upcoming_rewards_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(12);
            let reward_a = 20;
            Pallet::reward(&ALICE, reward_a, Precision::Exact).unwrap();

            System::set_block_number(13);
            let reward_b = 15;
            Pallet::reward(&ALICE, reward_b, Precision::Exact).unwrap();

            assert_ok!(Pallet::shed_rewards(RuntimeOrigin::signed(ALICE)));

            let expected_rewards = vec![(14, 20), (15, 15)];
            System::assert_last_event(
                Event::ScheduledRewards {
                    author: ALICE,
                    rewards: expected_rewards,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn upcoming_rewards_err_reward_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::shed_rewards(RuntimeOrigin::signed(ALICE)),
                Error::RewardNotFound
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn upcoming_penalties_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(12);
            let penalty_a = Perbill::from_percent(10);
            Pallet::penalize(&ALICE, penalty_a).unwrap();

            System::set_block_number(14);
            let penalty_b = Perbill::from_percent(5);
            Pallet::penalize(&ALICE, penalty_b).unwrap();

            assert_ok!(Pallet::shed_penalties(RuntimeOrigin::signed(ALICE)));

            let expected_penalties = vec![(16, penalty_a), (18, penalty_b)];
            System::assert_last_event(
                Event::ScheduledPenalties {
                    author: ALICE,
                    penalties: expected_penalties,
                }
                .into(),
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn upcoming_penalties_err_penalty_not_found() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            assert_err!(
                Pallet::shed_penalties(RuntimeOrigin::signed(ALICE)),
                Error::PenaltyNotFound
            );
        })
    }

    #[test]
    fn end_probation_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Probation);

            System::set_block_number(20);
            assert_ok!(Pallet::confirm(RuntimeOrigin::signed(ALICE)));

            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Active);

            System::assert_last_event(
                Event::AuthorStatus {
                    author: ALICE,
                    status: AuthorStatus::Active,
                }
                .into(),
            );
        })
    }

    #[test]
    fn end_probation_err_author_in_probation() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Probation);

            System::set_block_number(15);
            assert_err!(
                Pallet::confirm(RuntimeOrigin::signed(ALICE)),
                Error::AuthorInProbation
            );
        })
    }

    #[test]
    fn end_probation_err_author_is_unsafe() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Probation);

            System::set_block_number(20);

            Pallet::penalize(&ALICE, Perbill::from_percent(5)).unwrap();

            assert_err!(
                Pallet::confirm(RuntimeOrigin::signed(ALICE)),
                Error::AuthorIsUnsafe
            );
        })
    }

    #[test]
    fn end_probation_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Probation);

            System::set_block_number(20);
            assert_err!(
                Pallet::confirm(RuntimeOrigin::root()),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_probation_period_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_probation_period = ProbationPeriod::get();
            assert_eq!(before_probation_period, 10);
            let new_probation_period = 15;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::ProbationPeriod(new_probation_period)
            ));
            let after_probation_period = ProbationPeriod::get();
            assert_eq!(after_probation_period, 15);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::ProbationPeriod(
                    new_probation_period,
                ))
                .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_probation_period_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_probation_period = ProbationPeriod::get();
            assert_eq!(before_probation_period, 10);
            let new_probation_period = 15;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::ProbationPeriod(new_probation_period)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_reduce_probation_by_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_reduce_probation_by = ReduceProbationBy::get();
            assert_eq!(before_reduce_probation_by, 1);
            let new_reduce_probation_by = 2;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::ReduceProbationBy(new_reduce_probation_by)
            ));
            let after_reduce_probation_by = ReduceProbationBy::get();
            assert_eq!(after_reduce_probation_by, 2);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::ReduceProbationBy(
                    new_reduce_probation_by,
                ))
                .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_reduce_probation_by_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_reduce_probation_by = ReduceProbationBy::get();
            assert_eq!(before_reduce_probation_by, 1);
            let new_reduce_probation_by = 2;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::ReduceProbationBy(new_reduce_probation_by)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_increase_probation_by_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_increase_probation_by = IncreaseProbationBy::get();
            assert_eq!(before_increase_probation_by, 1);
            let new_increase_probation_by = 2;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::IncreaseProbationBy(new_increase_probation_by)
            ));
            let after_increase_probation_by = IncreaseProbationBy::get();
            assert_eq!(after_increase_probation_by, 2);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::IncreaseProbationBy(
                    new_increase_probation_by,
                ))
                .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_increase_probation_by_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_increase_probation_by = IncreaseProbationBy::get();
            assert_eq!(before_increase_probation_by, 1);
            let new_increase_probation_by = 2;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::IncreaseProbationBy(new_increase_probation_by)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_rewards_buffer_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_rewards_buffer = RewardsBuffer::get();
            assert_eq!(before_rewards_buffer, 2);
            let new_rewards_buffer = 4;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::RewardsBuffer(new_rewards_buffer)
            ));
            let after_rewards_buffer = RewardsBuffer::get();
            assert_eq!(after_rewards_buffer, 4);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::RewardsBuffer(new_rewards_buffer))
                    .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_rewards_buffer_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_rewards_buffer = RewardsBuffer::get();
            assert_eq!(before_rewards_buffer, 2);
            let new_rewards_buffer = 4;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::RewardsBuffer(new_rewards_buffer)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_penalties_buffer_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_penalties_buffer = PenaltiesBuffer::get();
            assert_eq!(before_penalties_buffer, 4);
            let new_penalties_buffer = 5;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::PenaltiesBuffer(new_penalties_buffer)
            ));
            let after_penalties_buffer = PenaltiesBuffer::get();
            assert_eq!(after_penalties_buffer, 5);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::PenaltiesBuffer(
                    new_penalties_buffer,
                ))
                .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_penalties_buffer_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_penalties_buffer = PenaltiesBuffer::get();
            assert_eq!(before_penalties_buffer, 4);
            let new_penalties_buffer = 5;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::PenaltiesBuffer(new_penalties_buffer)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_max_elected_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(6);
            let before_max_elected = MaxElected::get();
            assert_eq!(before_max_elected, 100);
            let new_max_elected = 75;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MaxElected(new_max_elected)
            ));
            let after_max_elected = MaxElected::get();
            assert_eq!(after_max_elected, 75);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MaxElected(new_max_elected)).into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_max_elected_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_max_elected = MaxElected::get();
            assert_eq!(before_max_elected, 100);
            let new_max_elected = 75;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::MaxElected(new_max_elected)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_min_elected_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_min_elected = MinElected::get();
            assert_eq!(before_min_elected, 6);
            let new_min_elected = 15;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MinElected(new_min_elected)
            ));
            let after_min_elected = MinElected::get();
            assert_eq!(after_min_elected, 15);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MinElected(new_min_elected)).into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_min_elected_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_min_elected = MinElected::get();
            assert_eq!(before_min_elected, 6);
            let new_min_elected = 15;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::MinElected(new_min_elected)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_enforce_max_elected_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_enforce_max_elected = ForceMaxElected::get();
            assert_eq!(before_enforce_max_elected, false);
            let new_enforce_max_elected = true;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::EnforceMaxElected(new_enforce_max_elected)
            ));
            let after_enforce_max_elected = ForceMaxElected::get();
            assert_eq!(after_enforce_max_elected, true);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::EnforceMaxElected(
                    new_enforce_max_elected,
                ))
                .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_enforce_max_elected_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_enforce_max_elected = ForceMaxElected::get();
            assert_eq!(before_enforce_max_elected, false);
            let new_enforce_max_elected = true;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::EnforceMaxElected(new_enforce_max_elected)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_min_fund_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_min_fund = MinFund::get();
            assert_eq!(before_min_fund, 25);
            let new_min_fund = 50;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MinFund(new_min_fund)
            ));
            let after_min_fund = MinFund::get();
            assert_eq!(after_min_fund, 50);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MinFund(new_min_fund)).into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_min_fund_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_min_fund = MinFund::get();
            assert_eq!(before_min_fund, 25);
            let new_min_fund = 50;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::MinFund(new_min_fund)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_max_exposure_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_max_exposure = MaxExposure::get();
            assert_eq!(before_max_exposure, 1000);
            let new_max_exposure = u64::MAX;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MaxExposure(new_max_exposure)
            ));
            let after_max_exposure = MaxExposure::get();
            assert_eq!(after_max_exposure, u64::MAX);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MaxExposure(new_max_exposure))
                    .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_max_exposure_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_max_exposure = MaxExposure::get();
            assert_eq!(before_max_exposure, 1000);
            let new_max_exposure = u64::MAX;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::MaxExposure(new_max_exposure)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_min_collateral_success() {
        authors_test_ext().execute_with(|| {
            System::set_block_number(10);
            let before_min_collateral = MinCollateral::get();
            assert_eq!(before_min_collateral, 50);
            let new_min_collateral = 100;
            assert_ok!(Pallet::force_genesis_config(
                RuntimeOrigin::root(),
                ForceGenesisConfig::MinCollateral(new_min_collateral)
            ));
            let after_min_collateral = MinCollateral::get();
            assert_eq!(after_min_collateral, 100);
            System::assert_last_event(
                Event::GenesisConfigUpdated(ForceGenesisConfig::MinCollateral(new_min_collateral))
                    .into(),
            );
        })
    }

    #[test]
    fn force_genesis_config_min_collateral_err_bad_origin() {
        authors_test_ext().execute_with(|| {
            let before_min_collateral = MinCollateral::get();
            assert_eq!(before_min_collateral, 50);
            let new_min_collateral = 100;
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::signed(ALICE),
                    ForceGenesisConfig::MinCollateral(new_min_collateral)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn force_genesis_config_enforces_invariants() {
        authors_test_ext().execute_with(|| {
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MinCollateral(0)
                ),
                Error::NonZeroConfigRequired
            );

            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MinElected(0)
                ),
                Error::NonZeroConfigRequired
            );

            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MaxElected(0)
                ),
                Error::NonZeroConfigRequired
            );

            assert_err!(
                Pallet::force_genesis_config(RuntimeOrigin::root(), ForceGenesisConfig::MinFund(0)),
                Error::NonZeroConfigRequired
            );

            let max_exposure = MaxExposure::get();
            let min_fund = MinFund::get();
            assert_eq!(max_exposure, 1000);
            assert!(min_fund <= max_exposure);
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MinFund(1001)
                ),
                Error::MinGreaterThanMax
            );
            assert_eq!(min_fund, 25);
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MaxExposure(24)
                ),
                Error::MinGreaterThanMax
            );

            let min_elected = MinElected::get();
            let max_elected = MaxElected::get();
            assert!(min_elected <= max_elected);
            assert_eq!(max_elected, 100);
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MinElected(101)
                ),
                Error::MinGreaterThanMax
            );
            assert_eq!(min_elected, 6);
            assert_err!(
                Pallet::force_genesis_config(
                    RuntimeOrigin::root(),
                    ForceGenesisConfig::MaxElected(5)
                ),
                Error::MinGreaterThanMax
            );
        })
    }

    #[test]
    fn create_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 350); // collateral + backing
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 500); // collateral + backing

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let index_reason = FreezeReason::AuthorFunding.into();
            assert_ok!(CommitAdapter::index_exists(&index_reason, &index_digest));

            let author_reason = FreezeReason::AuthorCollateral.into();
            let alice_digest = CommitAdapter::get_commit_digest(&ALICE, &author_reason).unwrap();
            let bob_digest = CommitAdapter::get_commit_digest(&BOB, &author_reason).unwrap();

            let mut actual_entries_and_shares =
                CommitAdapter::get_entries_shares(&index_reason, &index_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 100), (bob_digest.clone(), 100)];
            actual_entries_and_shares.sort();
            expected_entries_and_shares.sort();
            assert_eq!(
                actual_entries_and_shares,
                expected_entries_and_shares
            );

            let mut actual_entries_and_values =
                CommitAdapter::get_entries_value(&index_reason, &index_digest).unwrap();
            let mut expected_entries_and_values =
                vec![(alice_digest.clone(), 0), (bob_digest.clone(), 0)];
            actual_entries_and_values.sort();
            expected_entries_and_values.sort();
            assert_eq!(
                actual_entries_and_values,
                expected_entries_and_values
            );

            let index_info = CommitAdapter::get_index(&index_reason, &index_digest).unwrap();
            assert_eq!(index_info.capital(), 200);
            assert_eq!(index_info.principal(), 0);

            let fund_by = Funder::Index {
                digest: index_digest.clone(),
                backer: NIX,
            };
            Pallet::fund(&ALICE, &fund_by, 100, Precision::Exact, Fortitude::Force).unwrap();

            let mut actual_entries_and_values =
                CommitAdapter::get_entries_value(&index_reason, &index_digest).unwrap();
            let mut expected_entries_and_values = vec![(alice_digest, 50), (bob_digest, 50)];
            actual_entries_and_values.sort();
            expected_entries_and_values.sort();
            assert_eq!(
                actual_entries_and_values,
                expected_entries_and_values
            );

            let index_info = CommitAdapter::get_index(&index_reason, &index_digest).unwrap();
            assert_eq!(index_info.principal(), 100);

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 400); // collateral + backing + index fund
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 550); // collateral + backing + index fund
        })
    }

    #[test]
    fn crate_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_current_hold, 350); // collateral + backing
            let bob_current_hold = Pallet::get_hold(&BOB).unwrap();
            assert_eq!(bob_current_hold, 500); // collateral + backing

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let pool_reason = FreezeReason::AuthorFunding.into();

            System::set_block_number(15);
            let commission = Perbill::from_percent(10);
            assert_ok!(Pallet::create_pool(
                RuntimeOrigin::signed(MIKE),
                index_digest.clone(),
                commission
            ));

            let pool_digest = assert_pool_created_and_get_digest();

            assert_ok!(CommitAdapter::pool_exists(&pool_reason, &pool_digest));

            let pool_info = CommitAdapter::get_pool(&pool_reason, &pool_digest).unwrap();
            assert_eq!(pool_info.capital(), 200);

            let author_reason = FreezeReason::AuthorCollateral.into();
            let alice_digest = CommitAdapter::get_commit_digest(&ALICE, &author_reason).unwrap();
            let bob_digest = CommitAdapter::get_commit_digest(&BOB, &author_reason).unwrap();

            let mut actual_slots_and_shares =
                CommitAdapter::get_slots_shares(&pool_reason, &pool_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 100), (bob_digest.clone(), 100)];
            actual_slots_and_shares.sort();
            expected_entries_and_shares.sort();
            assert_eq!(
                actual_slots_and_shares,
                expected_entries_and_shares
            );

            let mut actual_entries_and_values =
                CommitAdapter::get_slots_value(&pool_reason, &pool_digest).unwrap();
            let mut expected_entries_and_values =
                vec![(alice_digest.clone(), 0), (bob_digest.clone(), 0)];
            actual_entries_and_values.sort();
            expected_entries_and_values.sort();
            assert_eq!(
                actual_entries_and_values,
                expected_entries_and_values
            );
        })
    }

    #[test]
    fn transfer_pool_ownership_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let pool_reason = FreezeReason::AuthorFunding.into();

            System::set_block_number(15);
            let commission = Perbill::from_percent(10);
            assert_ok!(Pallet::create_pool(
                RuntimeOrigin::signed(MIKE),
                index_digest.clone(),
                commission
            ));

            let pool_digest = assert_pool_created_and_get_digest();

            assert_ok!(CommitAdapter::pool_exists(&pool_reason, &pool_digest));

            let current_manager = CommitAdapter::get_manager(&pool_reason, &pool_digest).unwrap();
            assert_eq!(current_manager, MIKE);

            Pallet::transfer_pool(RuntimeOrigin::signed(MIKE), pool_digest.clone(), NIX).unwrap();

            System::assert_last_event(
                Event::PoolManager {
                    digest: pool_digest.clone(),
                    manager: NIX,
                }
                .into(),
            );

            let current_manager = CommitAdapter::get_manager(&pool_reason, &pool_digest).unwrap();
            assert_eq!(current_manager, NIX);
        })
    }

    #[test]
    fn transfer_pool_ownership_err_invalid_pool_manager() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let pool_reason = FreezeReason::AuthorFunding.into();

            System::set_block_number(15);
            let commission = Perbill::from_percent(10);
            assert_ok!(Pallet::create_pool(
                RuntimeOrigin::signed(MIKE),
                index_digest.clone(),
                commission
            ));

            let pool_digest = assert_pool_created_and_get_digest();

            assert_ok!(CommitAdapter::pool_exists(&pool_reason, &pool_digest));

            let current_manager = CommitAdapter::get_manager(&pool_reason, &pool_digest).unwrap();
            assert_eq!(current_manager, MIKE);

            assert_err!(
                Pallet::transfer_pool(RuntimeOrigin::signed(ALICE), pool_digest.clone(), NIX,),
                Error::InvalidPoolManager
            );
        })
    }

    #[test]
    fn update_commission_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let pool_reason = FreezeReason::AuthorFunding.into();

            System::set_block_number(15);
            let commission = Perbill::from_percent(10);
            assert_ok!(Pallet::create_pool(
                RuntimeOrigin::signed(MIKE),
                index_digest.clone(),
                commission
            ));

            let pool_digest = assert_pool_created_and_get_digest();

            assert_ok!(CommitAdapter::pool_exists(&pool_reason, &pool_digest));

            let actual_commison =
                CommitAdapter::get_commission(&pool_reason, &pool_digest).unwrap();
            assert_eq!(actual_commison, commission);

            let new_commission = Perbill::from_percent(5);
            System::set_block_number(20);
            Pallet::update_commission(RuntimeOrigin::signed(MIKE), index_digest, new_commission)
                .unwrap();

            let new_pool_digest = assert_pool_created_and_get_digest();

            #[cfg(feature = "dev")]
            {
                let slots = CommitAdapter::get_slots_shares(&pool_reason, &new_pool_digest).unwrap();
                System::assert_last_event(
                    Event::PoolCreated {
                        pool: new_pool_digest.clone(),
                        manager: MIKE,
                        commission: new_commission,
                        #[cfg(feature = "dev")]
                        slots: slots,
                    }
                    .into(),
                );
            }

            #[cfg(not(feature = "dev"))]
            {
                System::assert_last_event(
                    Event::PoolCreated {
                        pool: new_pool_digest.clone(),
                        manager: MIKE,
                        commission: new_commission,
                    }
                    .into(),
                );
            }

            let actual_commison =
                CommitAdapter::get_commission(&pool_reason, &new_pool_digest).unwrap();
            assert_eq!(actual_commison, new_commission);
        })
    }

    #[test]
    fn update_entry_shares_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let index_reason = FreezeReason::AuthorFunding.into();
            assert_ok!(CommitAdapter::index_exists(&index_reason, &index_digest));

            let author_reason = FreezeReason::AuthorCollateral.into();
            let alice_digest = CommitAdapter::get_commit_digest(&ALICE, &author_reason).unwrap();
            let bob_digest = CommitAdapter::get_commit_digest(&BOB, &author_reason).unwrap();

            let index_info = CommitAdapter::get_index(&index_reason, &index_digest).unwrap();
            assert_eq!(index_info.capital(), 200);
            let mut actual_entries_and_shares =
                CommitAdapter::get_entries_shares(&index_reason, &index_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 100), (bob_digest.clone(), 100)];
            actual_entries_and_shares.sort();
            expected_entries_and_shares.sort();
            assert_eq!(
                actual_entries_and_shares,
                expected_entries_and_shares
            );

            let new_alice_shares = 25;
            assert_ok!(Pallet::update_entry_shares(
                RuntimeOrigin::signed(MIKE),
                index_digest,
                alice_digest.clone(),
                new_alice_shares,
            ));

            let new_index_digest = assert_index_created_and_get_digest();

            let index_info = CommitAdapter::get_index(&index_reason, &new_index_digest).unwrap();
            assert_eq!(index_info.capital(), 125);
            let mut actual_entries_and_shares =
                CommitAdapter::get_entries_shares(&index_reason, &new_index_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 25), (bob_digest.clone(), 100)];
            actual_entries_and_shares.sort();
            expected_entries_and_shares.sort();
            assert_eq!(
                actual_entries_and_shares,
                expected_entries_and_shares
            );  

            let new_bob_shares = 75;
            assert_ok!(Pallet::update_entry_shares(
                RuntimeOrigin::signed(MIKE),
                new_index_digest.clone(),
                bob_digest.clone(),
                new_bob_shares,
            ));

            let new_index_digest = assert_index_created_and_get_digest();

            let index_info = CommitAdapter::get_index(&index_reason, &new_index_digest).unwrap();
            assert_eq!(index_info.capital(), 100);
            let mut actual_entries_and_shares =
                CommitAdapter::get_entries_shares(&index_reason, &new_index_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 25), (bob_digest.clone(), 75)];
            actual_entries_and_shares.sort();
            expected_entries_and_shares.sort();
            assert_eq!(
                actual_entries_and_shares,
                expected_entries_and_shares
            );
        })
    }

    #[test]
    fn update_slot_shares_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let pool_reason = FreezeReason::AuthorFunding.into();

            System::set_block_number(15);
            let commission = Perbill::from_percent(10);
            assert_ok!(Pallet::create_pool(
                RuntimeOrigin::signed(MIKE),
                index_digest.clone(),
                commission
            ));

            let pool_digest = assert_pool_created_and_get_digest();

            assert_ok!(CommitAdapter::pool_exists(&pool_reason, &pool_digest));

            let author_reason = FreezeReason::AuthorCollateral.into();
            let alice_digest = CommitAdapter::get_commit_digest(&ALICE, &author_reason).unwrap();
            let bob_digest = CommitAdapter::get_commit_digest(&BOB, &author_reason).unwrap();

            let pool_info = CommitAdapter::get_pool(&pool_reason, &pool_digest).unwrap();
            assert_eq!(pool_info.capital(), 200);
            let mut actual_slots_and_shares =
                CommitAdapter::get_slots_shares(&pool_reason, &pool_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 100), (bob_digest.clone(), 100)];
            actual_slots_and_shares.sort();
            expected_entries_and_shares.sort();
            assert_eq!(
                actual_slots_and_shares,
                expected_entries_and_shares
            );

            let new_alice_shares = 40;
            assert_ok!(Pallet::update_slot_shares(
                RuntimeOrigin::signed(MIKE),
                pool_digest.clone(),
                alice_digest.clone(),
                new_alice_shares
            ));

            let pool_info = CommitAdapter::get_pool(&pool_reason, &pool_digest).unwrap();
            assert_eq!(pool_info.capital(), 140);
            let mut actual_slots_and_shares =
                CommitAdapter::get_slots_shares(&pool_reason, &pool_digest).unwrap();
            let mut expected_slots_and_shares =
                vec![(alice_digest.clone(), 40), (bob_digest.clone(), 100)];
            actual_slots_and_shares.sort();
            expected_slots_and_shares.sort();
            assert_eq!(
                actual_slots_and_shares,
                expected_slots_and_shares
            );

            System::assert_last_event(
                Event::PoolSlotShare {
                    pool: pool_digest.clone(),
                    slots: (alice_digest.clone(), new_alice_shares),
                }
                .into(),
            );

            let new_bob_shares = 60;
            assert_ok!(Pallet::update_slot_shares(
                RuntimeOrigin::signed(MIKE),
                pool_digest.clone(),
                bob_digest.clone(),
                new_bob_shares
            ));

            let pool_info = CommitAdapter::get_pool(&pool_reason, &pool_digest).unwrap();
            assert_eq!(pool_info.capital(), 100);
            let mut actual_slots_and_shares =
                CommitAdapter::get_slots_shares(&pool_reason, &pool_digest).unwrap();
            let mut expected_slots_and_shares =
                vec![(alice_digest.clone(), 40), (bob_digest.clone(), 60)];
            actual_slots_and_shares.sort();
            expected_slots_and_shares.sort();
            assert_eq!(
                actual_slots_and_shares,
                expected_slots_and_shares
            );

            System::assert_last_event(
                Event::PoolSlotShare {
                    pool: pool_digest,
                    slots: (bob_digest, new_bob_shares),
                }
                .into(),
            );
        })
    }

    #[test]
    fn update_slot_shares_err_invalid_pool_manager() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 250, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 300, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::fund(
                &BOB,
                &Funder::Direct(CHARLIE),
                200,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let entries = vec![(ALICE, 100), (BOB, 100)];

            System::set_block_number(10);
            assert_ok!(Pallet::create_index(
                RuntimeOrigin::signed(MIKE),
                entries.clone()
            ));

            let index_digest = assert_index_created_and_get_digest();

            let pool_reason = FreezeReason::AuthorFunding.into();

            System::set_block_number(15);
            let commission = Perbill::from_percent(10);
            assert_ok!(Pallet::create_pool(
                RuntimeOrigin::signed(MIKE),
                index_digest.clone(),
                commission
            ));

            let pool_digest = assert_pool_created_and_get_digest();

            assert_ok!(CommitAdapter::pool_exists(&pool_reason, &pool_digest));

            let author_reason = FreezeReason::AuthorCollateral.into();
            let alice_digest = CommitAdapter::get_commit_digest(&ALICE, &author_reason).unwrap();
            let bob_digest = CommitAdapter::get_commit_digest(&BOB, &author_reason).unwrap();

            let pool_info = CommitAdapter::get_pool(&pool_reason, &pool_digest).unwrap();
            assert_eq!(pool_info.capital(), 200);
            let mut actual_slots_and_shares =
                CommitAdapter::get_slots_shares(&pool_reason, &pool_digest).unwrap();
            let mut expected_entries_and_shares =
                vec![(alice_digest.clone(), 100), (bob_digest.clone(), 100)];
            actual_slots_and_shares.sort();
            expected_entries_and_shares.sort();               
            assert_eq!(
                actual_slots_and_shares,
                expected_entries_and_shares
            );

            let new_alice_shares = 40;
            assert_err!(
                Pallet::update_slot_shares(
                    RuntimeOrigin::signed(CHARLIE),
                    pool_digest.clone(),
                    alice_digest.clone(),
                    new_alice_shares
                ),
                Error::InvalidPoolManager
            );
        })
    }

    #[cfg(feature = "dev")]
    #[test]
    fn check_direct_funds_towards_author_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 350, Fortitude::Force).unwrap();

            Pallet::fund(
                &ALICE,
                &Funder::Direct(BOB),
                250,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(10);
            Pallet::my_author_fund(
                RuntimeOrigin::signed(BOB),
                ALICE,
                FundingTarget::Direct(ALICE),
            )
            .unwrap();

            System::assert_last_event(
                Event::InspectFund {
                    author: ALICE,
                    funder: Funder::Direct(BOB),
                    amount: 250,
                }
                .into(),
            );
        })
    }

    // ===============================================================================
    // ````````````````````````````````` PUBLIC APIS `````````````````````````````````
    // ===============================================================================

    #[test]
    fn fetch_collateral_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, 500, 500).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, 500, 500).unwrap();

            Pallet::enroll(&ALICE, 135, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 425, Fortitude::Force).unwrap();

            let alice_collateral = Pallet::fetch_collateral(ALICE).unwrap();
            let bob_collateral = Pallet::fetch_collateral(BOB).unwrap();

            assert_eq!(alice_collateral, 135);
            assert_eq!(bob_collateral, 425);
        })
    }

    #[test]
    fn inspect_fund_direct_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            System::set_block_number(10);
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();

            System::set_block_number(20);
            assert_ok!(Pallet::back(
                RuntimeOrigin::signed(CHARLIE),
                FundingTarget::Direct(ALICE),
                100,
                FortitudeWrapper::Force,
                PrecisionWrapper::Exact
            ));

            let current_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(current_hold, 200);
            let backed_value = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(backed_value, 100);
            let backers_of = Pallet::backers_of(&ALICE).unwrap();
            assert_eq!(backers_of, vec![(Funder::Direct(CHARLIE), 100)]);

            System::set_block_number(25);
            let fund = Pallet::inspect_fund(CHARLIE, FundingTarget::Direct(ALICE)).unwrap();
            assert_eq!(fund, 100);
        })
    }

    #[test]
    fn inspect_fund_towards_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by_mike = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Index {
                digest: INDEX_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(25);
            let mike_alice_fund =
                Pallet::inspect_author_fund(MIKE, ALICE, FundingTarget::Index(INDEX_DIGEST))
                    .unwrap();
            assert_eq!(mike_alice_fund, 60);

            let mike_bob_fund =
                Pallet::inspect_author_fund(MIKE, BOB, FundingTarget::Index(INDEX_DIGEST)).unwrap();
            assert_eq!(mike_bob_fund, 40);

            let nix_alice_fund =
                Pallet::inspect_author_fund(NIX, ALICE, FundingTarget::Index(INDEX_DIGEST))
                    .unwrap();
            assert_eq!(nix_alice_fund, 30);

            let nix_bob_fund =
                Pallet::inspect_author_fund(NIX, BOB, FundingTarget::Index(INDEX_DIGEST)).unwrap();
            assert_eq!(nix_bob_fund, 20);
        })
    }

    #[test]
    fn inspect_fund_index_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            let by_mike = Funder::Index {
                digest: INDEX_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Index {
                digest: INDEX_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let mike_index_fund =
                Pallet::inspect_fund(MIKE, FundingTarget::Index(INDEX_DIGEST)).unwrap();
            assert_eq!(mike_index_fund, 100);

            let nix_index_fund =
                Pallet::inspect_fund(NIX, FundingTarget::Index(INDEX_DIGEST)).unwrap();
            assert_eq!(nix_index_fund, 50);
        })
    }

    #[test]
    fn inspect_fund_towards_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by_mike = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Pool {
                digest: POOL_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let mike_alice_fund =
                Pallet::inspect_author_fund(MIKE, ALICE, FundingTarget::Pool(POOL_DIGEST)).unwrap();
            assert_eq!(mike_alice_fund, 60);

            let mike_bob_fund =
                Pallet::inspect_author_fund(MIKE, BOB, FundingTarget::Pool(POOL_DIGEST)).unwrap();
            assert_eq!(mike_bob_fund, 40);

            let nix_alice_fund =
                Pallet::inspect_author_fund(NIX, ALICE, FundingTarget::Pool(POOL_DIGEST)).unwrap();
            assert_eq!(nix_alice_fund, 30);

            let nix_bob_fund =
                Pallet::inspect_author_fund(NIX, BOB, FundingTarget::Pool(POOL_DIGEST)).unwrap();

            assert_eq!(nix_bob_fund, 20);
        })
    }

    #[test]
    fn inspect_fund_pool_success() {
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
            initiate_key_and_set_balance_and_hold(&NIX, LARGE_VALUE, LARGE_VALUE).unwrap();

            System::set_block_number(6);
            Pallet::enroll(&ALICE, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(8);
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            System::set_block_number(12);
            Pallet::enroll(&BOB, STANDARD_VALUE, Fortitude::Force).unwrap();

            System::set_block_number(15);
            Pallet::fund(
                &BOB,
                &Funder::Direct(ALAN),
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_pool(
                ALAN,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                Perbill::from_percent(5),
            )
            .unwrap();

            let by_mike = Funder::Pool {
                digest: POOL_DIGEST,
                backer: MIKE,
            };

            Pallet::fund(
                &ALICE,
                &by_mike,
                LARGE_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let by_nix = Funder::Pool {
                digest: POOL_DIGEST,
                backer: NIX,
            };

            Pallet::fund(
                &ALICE,
                &by_nix,
                STANDARD_VALUE,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let mike_fund = Pallet::inspect_fund(MIKE, FundingTarget::Pool(POOL_DIGEST)).unwrap();
            assert_eq!(mike_fund, 100);

            let nix_fund = Pallet::inspect_fund(NIX, FundingTarget::Pool(POOL_DIGEST)).unwrap();
            assert_eq!(nix_fund, 50);
        })
    }
}
