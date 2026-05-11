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
// ```````````````````````````` PALLET CHAIN-MANAGER `````````````````````````````
// ===============================================================================

//! The **Chain Manager pallet** is the primary orchestration layer for
//! managing **session validators** as a coordinated, session-driven system.
//!
//! It governs the full validator lifecycle spanning participation,
//! election, activation, rewards, and penalties by materializing the
//! abstractions defined in [`frame_suite::blockchain`]
//! into a concrete runtime execution model.
//!
//! This pallet does not introduce new primitives. Instead, it binds together
//! traits, plugins, and adapters to drive deterministic validator selection
//! and behavior across sessions, enabling the game-theoretic guarantees
//! required for a decentralized network.
//!
//! - [`Config`] - Runtime configuration  
//! - [`Call`] - Dispatchable extrinsics (includes unsigned)  
//! - [`Pallet`] - External usage and trait implementations  
//!
//! ## Overview
//!
//! A **validator (author)** in this system is an actor who:
//!
//! - signals intent to participate in validation,
//! - submits affidavit data (backers) for election,
//! - competes in a session-based selection process,
//! - becomes an active session validator upon selection,
//! - receives rewards or penalties based on behavior.
//!
//! Validators are modeled as **session-scoped actors**, where
//! participation, selection, and compensation are tied to
//! deterministic session transitions.
//!
//! ## Architectural Role
//!
//! The pallet acts as an **orchestration boundary**, integrating:
//!
//! - role management and funding via [`Config::RoleAdapter`]  
//! - election logic via [`Config::ElectionAdapter`]  
//! - asset interactions via [`Config::Asset`]  
//! - session and authorship via [`pallet_session`] and [`pallet_authorship`]  
//! - offence handling via [`pallet_offences`]  
//!
//! It only tightly integrates with these essential core pallets, allowing
//! any Substrate runtime that composes them to adopt this pallet
//! for validator lifecycle and session management.
//!
//! ## Validator Lifecycle
//!
//! The system progresses through **session-scoped phases**:
//!
//! - **Pursuing Validation**  
//!   Authors signal intent to validate and may pause participation (`chill`).
//!
//! - **Affidavit Phase**  
//!   Active participants submit affidavit declarations representing
//!   election weights for the upcoming validator set.
//!
//! - **Election Phase**  
//!   Elections are executed for all active participants to determine
//!   the next session validators.
//!
//! - **Activation**  
//!   Elected authors transition into active session validators.
//!
//! - **Reward & Penalty**  
//!   Validators are compensated or penalized based on behavior.
//!
//! All transitions are derived relative to session progression,
//! ensuring predictable and deterministic execution.
//!
//! ## Execution Model
//!
//! The pallet operates as a **session-driven orchestration engine**,
//! primarily driven by **offchain workers and unsigned extrinsics**.
//!
//! Once validation intent is externally invoked by an author, the system
//! progresses automatically:
//!
//! - **Offchain workers**  
//!   coordinate affidavit submission, election execution, and key rotation  
//!
//! - **Unsigned extrinsics**  
//!   are submitted to finalize deterministic state transitions  
//!
//! - **Block hooks (`on_initialize`)**  
//!   process accumulated state and scheduled transitions  
//!
//! This enables:
//!
//! - automated validator lifecycle progression  
//! - continuous election participation for active candidates  
//! - minimal manual interaction beyond intent signaling  
//!
//! ## Economic Model
//!
//! Rewards and penalties are **externally defined and injected**:
//!
//! - [`Config::InflationModel`]: derives total session payout  
//! - [`Config::RewardModel`]: distributes rewards from points  
//! - [`Config::PenaltyModel`]: transforms and normalizes penalties  
//!
//! Final application is delegated via [`Config::RoleAdapter`],
//! ensuring consistent integration with role and funding systems.
//!
//! ## Design Intent
//!
//! This pallet is a **composition layer**, not a monolithic system:
//!
//! - structure is defined via traits  
//! - behavior is injected via plugins  
//! - coordination is driven by sessions and routines  
//!
//! enabling a modular, replaceable, and evolvable validator system
//! while preserving strong type safety and deterministic execution.
//!
//! ## Development Feature Gate
//!
//! This pallet includes a `dev` feature gate for development and testing.
//!
//! Core functionality is exposed via public APIs for RPC and UI usage.
//! The `dev` feature provides thin wrapper extrinsics and extended
//! event emissions for direct inspection.
//!
//! This feature must be disabled in production runtimes due to
//! additional debugging overhead.

#![cfg_attr(not(feature = "std"), no_std)]

// ===============================================================================
// `````````````````````````````````` MODULES ````````````````````````````````````
// ===============================================================================

mod blockchain;
mod offence;
mod roles;
mod routines;
mod session;
pub mod types;
pub mod crypto;
pub mod weights;

// Re-Exports for `Config` usage

pub use crate::crypto::AffidavitCryptoEd25519;
pub use crate::crypto::AffidavitCryptoSr25519;

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
    use super::*;
    use crate::{
        crypto::ValidatePayload,
        routines::*,
        types::*,
        weights::*,
    };

    // --- Core / Std ---
    use core::fmt::Debug;

    // --- FRAME Suite ---
    use frame_suite::{
        Countable, ForkLocalDepot, ForksHandler, blockchain::*, elections::*, plugin_types, roles::*, routines::*
    };

    // --- FRAME Support ---
    use frame_support::{
        dispatch::DispatchResult,
        pallet_prelude::{StorageValue, *},
        traits::{
            fungible::{Inspect, Mutate},
            EstimateNextSessionRotation,
        },
    };

    // --- FRAME System ---
    use frame_system::{
        offchain::{AppCrypto, SignedPayload},
        pallet_prelude::{BlockNumberFor, *},
    };

    // --- Substrate primitives ---
    use sp_core::Get;
    use sp_runtime::{
        RuntimeAppPublic, SaturatedConversion, WeakBoundedVec, traits::{Convert, IdentifyAccount, Saturating}
    };

    // ===============================================================================
    // `````````````````````````````` PALLET MARKER ``````````````````````````````````
    // ===============================================================================

    /// Primary Marker type for the **Chain Manager pallet**.
    ///
    /// This pallet provides implementations for traits from
    /// [`blockchain`](frame_suite::blockchain), [`roles`](frame_suite::roles),
    /// [`session`](pallet_session), [`offences`](sp_staking::offence)
    ///
    /// Implemented traits:
    ///
    /// - [`AuthorPoints`]
    /// - [`ElectionAffidavits`]
    /// - [`SessionManager`](pallet_session::SessionManager)
    /// - [`OnOffenceHandler`](sp_staking::offence::OnOffenceHandler)
    /// - [`RoleActivity`]
    /// - [`Convert<AuthorId, Option<SessionId>>`](sp_runtime::traits::Convert)
    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    // ===============================================================================
    // ```````````````````````````` INTERNAL PALLET MARKER ```````````````````````````
    // ===============================================================================

    /// Internal helper struct for implementing not-exposable
    /// [`blockchain`](frame_suite::blockchain) trait operations.
    ///
    /// `Internals` implements the blockchain low-level helper traits:
    ///
    /// - [`RewardAuthors`](frame_suite::blockchain::RewardAuthors)
    /// - [`ElectAuthors`](frame_suite::blockchain::ElectAuthors)
    /// - [`PenalizeAuthors`](frame_suite::blockchain::PenalizeAuthors)
    pub(crate) struct Internals<T: Config>(PhantomData<T>);

    // ===============================================================================
    // `````````````````````````````` CONFIG TRAIT ```````````````````````````````````
    // ===============================================================================

    /// Configuration trait for the Chain Manager pallet.
    ///
    /// This trait defines the types, constants, and dependencies
    /// that the runtime must provide for this pallet to function.
    ///
    /// It extends several other FRAME pallets' `Config` traits, ensuring tight
    /// integration with Substrate's session, authorship, time-management and
    /// offence-handling subsystems.
    ///
    /// ### Dependencies
    ///
    /// Dependencies other than [`frame_system::Config`]:
    /// - [`pallet_authorship::Config`]: Provides access to the current block author
    ///   and authorship tracking, used to assign and record block production points.
    /// - [`pallet_offences::Config`]: Enables detection and handling of offences for
    ///   misbehaving authors, required for penalty enforcement.
    /// - [`pallet_session::Config`]: Manages session rotation and validator/author sets;
    ///   supports session-aware election and reward cycles.
    /// - [`pallet_session::historical::Config`]: Provides access to historical session
    ///   data for offence handling.
    /// - [`pallet_timestamp::Config`] : Provides access to a monotonic deterministic
    ///   onchain unix timestamp.
    /// - [`frame_system::offchain::CreateSignedTransaction`]: Enables offchain workers
    ///   to sign and submit transactions. Required for unsigned extrinsics that rely
    ///   on signed payload verification ([`ValidateUnsigned`]).
    #[pallet::config]
    pub trait Config: frame_system::Config
        + pallet_authorship::Config
        + pallet_offences::Config
        + pallet_session::Config
        + pallet_session::historical::Config
        + pallet_timestamp::Config
        + frame_system::offchain::CreateSignedTransaction<<Self as frame_system::Config>::RuntimeCall>
    {
        // --- Runtime Anchors ---

        /// Extrinsic calls aggregation type for the runtime.
        type RuntimeCall: From<Call<Self>> + Into<<Self as frame_system::Config>::RuntimeCall>;

        /// Events aggregation type for the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        // --- Pallet Adapters ---

        /// Adapter type for **author role, and compensation management**.
        ///
        /// This associated type integrates multiple traits that together define how
        /// authors are **managed**, **rewarded**, and **funded**.
        ///
        /// It acts as a unified interface between this pallet and the underlying
        /// role-management, and reward systems.
        ///
        /// ## Required Traits
        /// - [`RoleManager`]: Core author management - handles registration and membership.
        /// - [`CompensateRoles`]: Defines logic for author **rewards and penalties**.
        /// - [`FundRoles`]: Enables **external funding or backing** mechanisms for authors.
        type RoleAdapter: RoleManager<AuthorOf<Self>>
            // Since `OnOffenceHandler` uses `Perbill` directly hence
            + CompensateRoles<AuthorOf<Self>, Ratio = PenaltyRatio>
            + FundRoles<AuthorOf<Self>>;

        /// Adapter type for role **author election management system**.
        ///
        /// This associated type integrates multiple traits that together define how
        /// authors are **elected**.
        ///
        /// ## Required Traits
        /// - [`ElectionManager`]: Conducts author **elections** on behalf of this pallet.
        /// - [`InspectWeight`]: Provides APIs to **query election weights** for authors.
        type ElectionAdapter: ElectionManager<AuthorOf<Self>>
            + InspectWeight<AuthorOf<Self>, ElectionVia<Self>>;

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
        /// type Asset = pallet_assets::Pallet<T>;
        /// ```
        type Asset: Inspect<
                AuthorOf<Self>,
                // Ensures that the pallet's `Asset` type aligns with the assets
                // used by role-manager system.
                //
                // Guarantees that rewards, penalties, and funding operations
                // use a **consistent asset type**.
                Balance = AssetOf<Self>,
            > + Mutate<AuthorOf<Self>>;

        /// Adapter for managing and querying **author points**.
        ///
        /// Provides the implementation for tracking points (e.g., block production
        /// or performance metrics) associated with each author.
        ///
        /// This can be set to [`Pallet<Self>`] if using the chain-manager pallet's
        /// internal implementation, or provided externally via another pallet or adapter.
        type PointsAdapter: AuthorPoints<AuthorOf<Self>, Self::Points>;

        // --- Pallets-exposed Additional Types ---

        /// Provides the logic to **estimate the length of the next session**.
        ///
        /// Used to compute
        /// - affidavit submission windows,
        /// - election start blocks, and
        /// - session timing.
        type NextSessionRotation: EstimateNextSessionRotation<BlockNumberFor<Self>>;

        /// Custom application crypto for **affidavit submission and rotation**.
        ///
        /// Defines the cryptographic scheme used by offchain workers when
        /// submitting and rotating affidavits. This associated type creates
        /// a dedicated `KeyId` namespace for affidavit-related operations.
        ///
        /// ## Supported Cryptography
        /// - [`AffidavitCryptoEd25519`]
        /// - [`AffidavitCryptoSr25519`]
        ///
        /// Either scheme may be used depending on the desired security and
        /// performance characteristics.
        type AffidavitCrypto: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>;

        // --- Scalars ---

        /// Type representing **good-behaviour points** accumulated by an author.
        ///
        /// This is an abstract point type used to measure good behaviour
        /// (e.g. block production) within a session.
        ///
        /// ## Design Notes
        /// - Points are **session-scoped** and non-transferable.
        /// - This is **not** an economic asset.
        /// - Points are interpreted later by reward and penalty logic
        ///   and may be transformed into actual asset movements.
        type Points: Countable;

        // --- Plugins ---

        // Plugin for computing **inflation-adjusted total payout**.
        plugin_types!(
            input: AssetOf<Self>,
            output: AssetOf<Self>,

            /// Inflation adjustment **plugin model**.
            ///
            /// This model defines the logic used to transform the raw total asset
            /// (`AssetOf`) into an inflation-adjusted payout before distribution
            /// to authors.
            ///
            /// Conceptually similar to `PayoutModel` in `RewardAuthors`, but focused
            /// specifically on inflation-based adjustments in a more layman-friendly
            /// abstraction.
            ///
            /// ## Input
            /// - [`AssetOf`]: The raw total asset for the current cycle.
            ///
            /// ## Output
            /// - [`AssetOf`]: The total asset after applying inflation rules.
            ///
            /// Designed to be runtime-configurable via [`Self::InflationContext`].
            ///
            /// Designed to be selectable using template plugin models in
            /// [`frame_plugins::rewards::payout`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            model: InflationModel,

            /// Runtime **context** for the inflation adjustment plugin model.
            ///
            /// Provides configurable parameters that influence how the
            /// [`Self::InflationModel`] behaves at runtime.
            ///
            /// ## Examples
            /// - Inflation rates
            /// - Upper or lower payout caps
            /// - Scaling coefficients or dampening factors
            ///
            /// This allows the inflation logic to be tuned without changing the model
            /// implementation itself.
            context: InflationContext,
        );

        // Plugin for computing **per-author rewards** from total payout and points.
        plugin_types!(
            input: (AssetOf<Self>, PayoutFor<Self>),
            output: PayeeList<Self>,

            /// Per-author **reward distribution plugin model**.
            ///
            /// Responsible for mapping each author's contribution to a concrete payout.
            /// Conceptually performs:
            ///
            /// `(Author, Points)` -> `(Author, AssetOf)`
            ///
            /// ## Input
            /// - ([`AssetOf`], [`PayoutFor`]):
            ///   The total payout for the cycle and the per-author points allocation.
            ///
            /// ## Output
            /// - [`PayeeList`]:
            ///   The finalized list of authors and their respective rewards.
            ///
            /// ## Notes
            /// - Maps each author's points to their final reward share.
            /// - Designed to be runtime-configurable via [`Self::RewardContext`].
            ///
            /// Designed to be selectable using template plugin models in
            /// [`frame_plugins::rewards::payee`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            model: RewardModel,

            /// Runtime **context** for configuring reward plugin computation.
            ///
            /// Supplies parameters that influence how rewards are calculated and
            /// distributed by the [`Self::RewardModel`].
            ///
            /// ## Examples
            /// - Multipliers or weights
            /// - Per-author or global caps
            /// - Curves, thresholds, or smoothing factors
            ///
            /// This separation allows payout logic to evolve independently from
            /// runtime tuning and governance decisions.
            context: RewardContext,
        );

        // Plugin for **transforming penalties** of authors.
        plugin_types!(
            input: PenaltyFor<Self>,
            output: PenaltyFor<Self>,

            /// **Penalty transformation plugin model**.
            ///
            /// Defines how author penalties are adjusted before being applied.
            /// Conceptually performs:
            ///
            /// `(Author, Penalty)` -> `(Author, Penalty)`
            ///
            /// ## Input
            /// - [`PenaltyFor`]:
            ///   The raw per-author penalties for the current cycle.
            ///
            /// ## Output
            /// - [`PenaltyFor`]:
            ///   The penalties after applying transformation rules.
            ///
            /// ## Notes
            /// - Supports caps, scaling, or other penalty transformation logic.
            /// - Runtime behavior is influenced by [`Self::PenaltyContext`].
            ///
            /// Designed to be selectable using template plugin models in
            /// [`frame_plugins::penalty`] or custom model defining
            /// macros via [`frame_suite::plugins`].
            model: PenaltyModel,

            /// Runtime **context** for penalty plugin transformation.
            ///
            /// Supplies parameters that configure how the [`Self::PenaltyModel`] operates
            /// at runtime.
            ///
            /// ## Examples
            /// - Multipliers or dampening factors
            /// - Upper or lower penalty caps
            /// - Thresholds or step functions
            ///
            /// This allows penalty policy changes without modifying the model
            /// implementation.
            context: PenaltyContext,
        );

        // --- Weights ---

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        // --- Constants ---

        /// **Flag for reward inflation model**.
        ///
        /// Determines how the total reward pool is computed:
        /// - `true`: total asset supply is used to calculate inflation/rewards.
        /// - `false`: total locked stake of authors is used instead.  
        ///
        /// This allows flexible reward strategies depending on economic design.
        #[pallet::constant]
        type InflateViaSupply: Get<bool> + Clone + Debug;

        /// Maximum number of **weights an author can submit in an affidavit**.
        ///
        /// Weight represents backers or collateral information essential for
        /// conducting elections.
        ///
        /// Limits storage and ensures predictable handling of submitted affidavits.
        /// If an author submits more than this number, the weights may be truncated.
        ///
        /// Each weight must remain sortable (`Ord`) for ensuring priority ordering during
        /// truncation to this upper bound.
        #[pallet::constant]
        type MaxAffidavitWeights: Get<u32> + Clone + Debug;

        const MAX_FORKS: u32;

        const MAX_FORK_RECOVERY_TRAVERSAL: u32;

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
    // ``````````````````````````````` GENESIS CONFIG ````````````````````````````````
    // ===============================================================================

    /// Genesis configuration for the **Chain Manager pallet**.
    ///
    /// Provides the **initial runtime parameters** governing session-driven
    /// validator orchestration, including affidavit flow, election timing,
    /// transaction prioritization, and finality safeguards at chain genesis.
    ///
    /// These values define the **baseline execution schedule and coordination rules**
    /// for validator lifecycle before any sessions are processed.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Whether affidavit submission is enabled.
        /// - `true`: authors may submit affidavit data during the configured window.
        /// - `false`: affidavit submission is entirely disabled.
        pub allow_affidavits: bool,

        /// Relative point within a session at which the affidavit phase begins.
        ///
        /// Expressed as a fraction of the session (e.g., `0.2` = 20% into the session).
        /// Defines when authors can start submitting affidavit.
        /// Submissions made before this point are not permitted.
        pub afdvt_begins_at: Duration,

        /// Relative point within a session at which the affidavit phase ends.
        ///
        /// Must be greater than `afdvt_begins_at` and within session bounds.
        /// After this point, no new affidavit submissions are accepted.
        pub afdvt_ends_at: Duration,

        /// Relative point within a session at which election execution begins.
        ///
        /// Typically aligned with or after the affidavit phase to ensure
        /// all candidate data is available for selection.
        pub election_begins_at: Duration,

        /// Number of points awarded to the author responsible for executing elections.
        ///
        /// Acts as an incentive for offchain workers or authors coordinating
        /// election execution in a timely manner.
        pub election_runner_points: T::Points,

        /// Transaction priority assigned to validation-related unsigned extrinsics.
        ///
        /// Higher priority ensures inclusion in blocks under contention.
        /// Should typically be the highest among lifecycle operations.
        pub validate_tx_priority: TransactionPriority,

        /// Transaction priority assigned to affidavit submission extrinsics.
        ///
        /// Balanced to allow fair participation without starving higher-priority
        /// operations such as validation.
        pub affidavit_tx_priority: TransactionPriority,

        /// Transaction priority assigned to election execution extrinsics.
        ///
        /// Ensures elections are processed in time, but typically lower than
        /// validation and affidavit priorities.
        pub election_tx_priority: TransactionPriority,

        /// Time-based delay (in milliseconds) before an operation is considered final.
        ///
        /// Helps mitigate premature execution in unstable network conditions
        /// or during short-lived forks.
        pub finality_after: u64,

        /// Block-based confirmation threshold for finality.
        ///
        /// Represents the number of distinct blocks that must pass before
        /// an operation is finalized, providing deterministic safety
        /// against reorgs.
        pub finality_ticks: BlockNumberFor<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                allow_affidavits: false,
                afdvt_begins_at: Duration::from_rational(2u32, 10u32), // 20%
                afdvt_ends_at: Duration::from_rational(8u32, 10u32),   // 80%
                election_begins_at: Duration::from_rational(5u32, 10u32), // 50%
                election_runner_points: 10u8.into(),
                validate_tx_priority: 1_000_000,
                affidavit_tx_priority: 850_000,
                election_tx_priority: 700_000,
                // 1 minute delay
                finality_after: 60_000,
                // 5 distinct blocks
                finality_ticks: 5u32.into(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // --- Affidavit window ---
            assert!(
                self.afdvt_begins_at < self.afdvt_ends_at,
                "`AffidavitBeginsAt` must be less than `AffidavitEndsAt`"
            );
            assert!(
                self.afdvt_ends_at <= Duration::one(),
                "`AffidavitEndsAt` cannot exceed 100%"
            );
            assert!(
                self.election_begins_at <= Duration::one(),
                "`ElectionBeginsAt` must be within 0-100% of affidavit window"
            );

            // --- Tx priorities ---
            assert!(self.validate_tx_priority > 0);
            assert!(self.election_tx_priority > 0);
            assert!(self.affidavit_tx_priority > 0);

            // --- Finality ---
            assert!(self.finality_after > 0);
            assert!(self.finality_ticks > Zero::zero());

            AllowAffidavits::<T>::put(self.allow_affidavits);
            AffidavitBeginsAt::<T>::put(self.afdvt_begins_at);
            AffidavitEndsAt::<T>::put(self.afdvt_ends_at);
            ElectionBeginsAt::<T>::put(self.election_begins_at);
            ElectionRunnerPoints::<T>::put(self.election_runner_points);
            ValidateTxPriority::<T>::put(self.validate_tx_priority);
            ElectionTxPriority::<T>::put(self.election_tx_priority);
            AffidavitTxPriority::<T>::put(self.affidavit_tx_priority);
            let moment: Moment<T> = self.finality_after.saturated_into();
            FinalityAfter::<T>::put(moment);
            FinalityTicks::<T>::put(self.finality_ticks);
        }
    }

    // ===============================================================================
    // ```````````````````````````````` STORAGE TYPES ````````````````````````````````
    // ===============================================================================

    /// The **current running session index**.
    ///
    /// Used to track session-aware operations such as:
    /// - Author points accumulation ([`AuthorPoints`])
    /// - Reward and penalty distribution
    /// - Affidavit submission and election preparation
    #[pallet::storage]
    pub type CurrentSession<T: Config> = StorageValue<_, SessionIndex, ValueQuery>;

    /// Storage mapping of **authors' submitted affidavits per session**.
    ///
    /// Keyed by:
    /// 1. [`SessionIndex`] - the session for which the affidavit applies for election.
    /// 2. [`AuthorOf`] - the author submitting the affidavit
    ///
    /// Value is a tuple of:
    /// - [`BlockNumberFor`]: the block when the affidavit was submitted and,
    /// - A `WeakBoundedVec` of [`ElectionWeight`]: the author's declared weights,
    /// bounded to prevent excessive storage
    ///
    /// This storage is used for:
    /// - Preparing election candidates for the next session
    #[pallet::storage]
    pub type AuthorAffidavits<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, SessionIndex>,
            NMapKey<Blake2_128Concat, AuthorOf<T>>,
        ),
        (
            BlockNumberFor<T>,
            WeakBoundedVec<ElectionWeight<T>, T::MaxAffidavitWeights>,
        ),
        OptionQuery,
    >;

    /// **Author points accumulated per session**.
    ///
    /// This storage tracks **block-level points** awarded to authors for good behavior
    /// during block production. Each increment represents a positive contribution,
    /// such as successfully producing a block or validating transactions correctly.
    ///
    /// Keyed by:
    /// 1. [`SessionIndex`]: the session in which points are earned.
    /// 2. [`AuthorOf`]: the author receiving the points.
    ///
    /// Value:
    /// - [`Config::Points`]: the total points accumulated by the author in that session.
    ///
    /// Notes:
    /// - This serves as the **high-level points store** for reward calculation.
    /// - Each block-level good behavior contributes **one point** to this tally.
    /// - Points are **session-scoped and ephemeral**; typically cleared after reward distribution.
    /// - Can be used as a reference for evaluating other contributions or behaviors of authors.
    #[pallet::storage]
    pub type BlockPointsStore<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, SessionIndex>,
            NMapKey<Blake2_128Concat, AuthorOf<T>>,
        ),
        T::Points,
        OptionQuery,
    >;

    /// **Start block of the current session**.
    ///
    /// Tracks the block number when the current session began.  
    /// Used for:
    /// - Computing session-relative timing for elections and affidavits.
    #[pallet::storage]
    pub type SessionStartAt<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// **Flag to enable or disable affidavit submissions**.
    ///
    /// Controls whether authors can submit self-reported election weights.  
    /// When `true`, affidavit-related functions are active; when `false`, submissions
    /// are blocked. Initially, this is expected to be `false` and gradually enabled once
    /// the required authors are ready.
    #[pallet::storage]
    pub type AllowAffidavits<T: Config> = StorageValue<_, bool, ValueQuery>;

    // /// **Flag for reward inflation model**.
    // ///
    // /// Determines how the total reward pool is computed:
    // /// - `true`: total asset supply is used to calculate inflation/rewards.
    // /// - `false`: total locked stake of authors is used instead.
    // ///
    // /// This allows flexible reward strategies depending on economic design.
    // #[pallet::storage]
    // pub type InflateViaSupply<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// **Start of affidavit submission window** as a percentage ([`PerThing`]) of the
    /// current session. Authors submit affidavits for the **next upcoming session**
    /// starting from this point.
    ///
    /// ## Examples
    /// - If the current session is 1000 blocks long and `AffidavitBeginsAt = 20%`,
    ///   affidavit submissions can start at block `200` of the current session.
    /// - Authors cannot submit affidavits before this block, even if they are ready.
    #[pallet::storage]
    pub type AffidavitBeginsAt<T: Config> = StorageValue<_, Duration, ValueQuery>;

    /// **End of affidavit submission window** as a percentage ([`PerThing`]) of the
    /// current session.
    ///
    /// Authors must submit affidavits **before this block**, leaving room for the
    /// election process for the next upcoming session.
    ///
    /// ## Examples
    /// - If the current session is 100 blocks long and `AffidavitEndsAt = 80%`,
    ///   affidavit submissions must end by block `800`.
    /// - The period from `AffidavitBeginsAt` to `AffidavitEndsAt` defines the **affidavit
    ///   submission window**.
    /// - Elections for the next session should be conducted before this period ends.
    #[pallet::storage]
    pub type AffidavitEndsAt<T: Config> = StorageValue<_, Duration, ValueQuery>;

    /// **Start of the election window** as a percentage ([`PerThing`]) of the **affidavit
    /// submission period** in the current session.
    ///
    /// This does **not** refer to a percentage of the full session, but rather the **relative
    /// position within the affidavit submission window**. Authors submit affidavits first, then
    /// after this point, the election process can start.
    ///
    /// ## Examples
    /// - Suppose the affidavit window spans blocks 200-800 (`AffidavitBeginsAt = 20%`,
    ///   `AffidavitEndsAt = 80%` in a 100-block session).
    /// - If `ElectionBeginsAt = 50%`, then the election starts halfway through the
    ///   affidavit window: 200 + 50% * (800 - 200) = block 500.
    /// - Timeline:
    ///     - 200-500: affidavit submission period
    ///     - 500-800: election preparation
    #[pallet::storage]
    pub type ElectionBeginsAt<T: Config> = StorageValue<_, Duration, ValueQuery>;

    /// **Block points allocated for the election runner**.
    ///
    /// Determines how many [`Config::Points`] are required or suitable for an author
    /// to act as the election runner in the upcoming session.
    ///
    /// This value is used to assess election participation eligibility or priority.
    #[pallet::storage]
    pub type ElectionRunnerPoints<T: Config> = StorageValue<_, T::Points, ValueQuery>;

    /// **Pending upgrade of election runner points**.
    ///
    /// When an upgrade occurs, this should be set to `None` after being applied.
    ///
    /// ## Notes
    /// - Upgrades should occur **after all author rewards have been distributed**
    ///   and before the next reward cycle starts.
    /// - Ensures fairness: if upgraded mid-session, authors who have not yet
    ///   submitted affidavits may be disadvantaged in election chances.
    /// - Any user of [`ElectionRunnerPoints`] should take this storage value into account
    /// for the upcoming session.
    #[pallet::storage]
    pub type ElectionRunnerPointsUpgrade<T: Config> =
        StorageValue<_, Option<T::Points>, ValueQuery>;

    /// **Offchain affidavit keys registered by an author for a given session**.
    ///
    /// Retrieves the author who submitted affidavit keys, tied to a specific session.  
    ///
    /// - Authenticate offchain submissions (affidavits) for the upcoming session.
    /// - Ensure that only valid authors can participate in elections or submit weights.
    /// - Maintain session-specific separation, so keys do not persist beyond their intended session.
    ///
    /// ## Storage Structure
    /// - [`SessionIndex`]: the session for which the keys are valid.
    /// - [`AffidavitId`]: the actual affidavit public key ID, type chosen by
    /// the runtime configuration [`Config::AffidavitCrypto`].
    /// - [`AuthorOf<T>`]: the author who owns the key-pairs for signing.
    #[pallet::storage]
    pub type AffidavitKeys<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, SessionIndex>,
            NMapKey<Blake2_128Concat, AffidavitId<T>>,
        ),
        AuthorOf<T>,
        OptionQuery,
    >;

    /// **Tracks the author who prepared the election for a given session**.
    ///
    /// Stores the identity of the election runner and the block number at which
    /// they executed the election process for the **upcoming session**.
    ///
    /// This storage is **tied to the session index** and may be **overwritten**
    /// if multiple election runs occur within the same session.  
    /// This ensures that the latest election runner and block number are always recorded.
    ///
    /// This allows the pallet to:
    /// - Identify which author acted as the election runner for auditing or reward purposes.
    /// - Record the exact block when the election was conducted, ensuring deterministic session transitions.
    /// - Keep track of the most recent election preparation for the session.
    ///
    /// ## Storage Structure
    /// - [`SessionIndex`]: the session for which the election was prepared.
    /// - ([`AuthorOf`], [`BlockNumberFor`]): tuple of the election runner and the block number of execution.
    /// - [`OptionQuery`]: returns `None` if no election has been prepared for the session or yet.
    #[pallet::storage]
    pub type ElectsPreparedBy<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SessionIndex,
        (AuthorOf<T>, BlockNumberFor<T>),
        OptionQuery,
    >;

    /// Transaction priority for the [`Pallet::validate`] extrinsic.
    ///
    /// Used because `validate` is an **unsigned extrinsic** that carries a
    /// signed payload, rather than relying on a conventional signed origin.
    ///
    /// This extrinsic **is propagated to other peers** and participates in
    /// normal transaction pool ordering, so its priority directly affects
    /// inclusion and ordering behavior.
    #[pallet::storage]
    pub type ValidateTxPriority<T: Config> = StorageValue<_, TransactionPriority, ValueQuery>;

    /// Transaction priority for the [`Pallet::elect`] extrinsic.
    ///
    /// This extrinsic is **submitted locally by an offchain worker** and
    /// **not propagated to other peers** via the transaction pool.
    ///
    /// The priority therefore only affects **local block construction**
    /// and ordering relative to other locally submitted transactions.
    #[pallet::storage]
    pub type ElectionTxPriority<T: Config> = StorageValue<_, TransactionPriority, ValueQuery>;

    /// Transaction priority for the [`Pallet::declare`] extrinsic.
    ///
    /// Controls the relative importance of affidavit submissions in the
    /// transaction pool.
    ///
    /// This extrinsic **is propagated to other peers**, so its priority
    /// influences network-wide inclusion and ordering behavior.
    #[pallet::storage]
    pub type AffidavitTxPriority<T: Config> = StorageValue<_, TransactionPriority, ValueQuery>;

    /// Wall-clock delay (in timestamp units) that must elapse after the
    /// first observation of a value before it becomes eligible to
    /// strengthen its confidence signal.
    ///
    /// Measured from the timestamp of the initial observation:
    ///
    /// `current_timestamp >= first_observation_timestamp + FinalityAfter`
    ///
    /// This acts as a stability window to prevent immediate confidence
    /// escalation for newly observed values.
    ///
    /// ### Note
    /// - Must be strictly greater than zero.
    /// - Should be large enough to tolerate short-lived forks.
    #[pallet::storage]
    pub type FinalityAfter<T: Config> = StorageValue<_, Moment<T>, ValueQuery>;

    /// Number of distinct block observations required *after* the
    /// [`FinalityAfter`] window has elapsed in order to strengthen
    /// the confidence signal of a value.
    ///
    /// Observations are block-scoped:
    /// - At most one observation per block is counted.
    /// - Multiple observations within the same block do not increase the count.
    ///
    /// A value may strengthen its confidence only when:
    /// 1. The [`FinalityAfter`] delay has elapsed, and
    /// 2. It has been observed in at least `FinalityTicks`
    ///    distinct blocks thereafter.
    ///
    /// ### Note
    /// - Must be strictly greater than zero.
    /// - Larger values increase fork tolerance but delay confidence.
    #[pallet::storage]
    pub type FinalityTicks<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    // ===============================================================================
    // ```````````````````````````````````` ERROR ````````````````````````````````````
    // ===============================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The requested affidavit-ready author was not found.
        ///
        /// This may happen if:
        /// - The author has not declared readiness for validation via  
        /// `validate` extrinsic
        /// - The author has not declared affidavit keys for the upcoming session
        /// - The provided affidavit key is invalid
        AffidavitAuthorNotFound,

        /// Block production points for the specified author were not found.
        BlockPointsNotFound,

        /// The author has exhausted the maximum allowed block production points
        /// for the current session.
        ///
        /// Consider scaling the points type or constraining accumulation
        /// according to block production frequency.
        BlockPointsExhausted,

        /// The specified author has not submitted an affidavit
        /// for the upcoming session.
        AffidavitNotFound,

        /// Affidavit submissions are not enabled by configuration.
        AffidavitsNotAllowed,

        /// The affidavit submission window for the upcoming session
        /// has not started yet in the current session.
        NotAffidavitPeriod,

        /// The affidavit submission window for the upcoming session has ended.
        ///
        /// The author must wait for the next session's affidavit period.
        AffidavitPeriodEnded,

        /// The election period for the upcoming session
        /// has not started yet in the current session.
        NotElectionPeriod,

        /// Invalid affidavit configuration.
        ///
        /// `AffidavitBeginsAt` must be less than or equal to `AffidavitEndsAt`.
        InvalidAffidavitPeriod,

        /// The election period for the upcoming session has ended.
        ///
        /// The author must wait for the next session's election period.
        ElectionPeriodEnded,

        /// The author has not registered ownership of the given affidavit key.
        ///
        /// The key itself may be valid, but it does not belong to the author.
        AuthorNotAffidavitOwner,

        /// Failed to query the session ID for the author.
        ///
        /// This usually indicates that the author does not hold
        /// an active author role.
        SessionIdQueryFailed,

        /// The author cannot chill immediately because they are
        /// an elected validator in the current session.
        ValidatorCannotChill,

        /// The author cannot chill immediately because they have submitted
        /// an affidavit and are a candidate for election.
        CandidateCannotChill,

        /// The author cannot chill immediately because they are elected
        /// for the upcoming session.
        ElectedCannotChill,

        /// An active affidavit key already exists in the node's local keystore.
        ///
        /// No new key generation is required. If the node has a valid author role,
        /// it may already be ready for `validate` extrinsic.
        ///
        /// Or if the author is ready to declare affidavit he is eligible to do so.
        AffidavitKeyExists,

        /// A next-session affidavit key already exists in the node's local keystore.
        ///
        /// An affidavit declaration is ready to be posted and the key
        /// is ready for rotation.
        NextAffidavitKeyExists,

        /// Failed to sign the affidavit submission extrinsic payload
        /// using the active affidavit key.
        CannotSignAffidavitTxPayload,

        /// Failed to construct and sign validate payload
        /// using the active affidavit key.
        CannotSignValidateTxPayload,

        /// The node is expected to hold an active affidavit key, but none was found.
        ///
        /// This indicates an inconsistency between earlier logic and current storage.
        /// The OCW will retry, but persistent failure suggests storage corruption.
        ExpectedToHoldActiveAffidavitKey,

        /// The node is expected to not hold a active affidavit key in offchain storage,
        /// but it is available.
        ///
        /// This indicates an inconsistency between earlier logic and current storage.
        /// The OCW will retry, but persistent failure suggests storage corruption.
        ExpectedToNotHoldActiveAffidavitKey,

        /// The node is expected to hold the next affidavit key during
        /// the key-rotation lifecycle, but it is missing.
        ///
        /// This indicates an inconsistency between earlier logic and current storage.
        /// The OCW will retry, but persistent failure suggests storage corruption.
        ExpectedToHoldNextAffidavitKey,

        /// The node is expected to hold a finalized next affidavit key
        /// in offchain storage, but it is missing.
        ///
        /// This indicates an inconsistency between earlier logic and current storage.
        /// The OCW will retry, but persistent failure suggests storage corruption.
        ExpectedToHoldFinalizedNextAffidavitKey,

        /// An offchain storage operation failed.
        ///
        /// The OCW halts further decisions for safety.
        /// Thus, re-runs in next block execution.
        OCWStorageDecisionHalt,

        /// The offchain worker failed to submit the affidavit extrinsic
        /// for unknown reasons.
        CannotSubmitAffidavitTx,

        /// Extrinsic submission for declaring an affidavit failed.
        FailedToDeclareAffidavit,

        /// The election period for the upcoming session
        /// has not started yet.
        YetToElectAuthors,

        /// Failed to sign the election extrinsic payload using
        /// the very recently rotated affidavit key.
        ///
        /// Subsequent OCW executions at next block shall attempt again.
        CannotSignElectionTxPayload,

        /// The offchain worker failed to submit the election extrinsic
        /// for unknown reasons.
        ///
        /// Subsequent OCW executions at next block shall attempt again.
        CannotSubmitElectionTx,

        /// Extrinsic submission for electing authors failed.
        ///
        /// Subsequent OCW executions at next block shall attempt again.
        ExtrinsicFailedToElectAuthors,

        /// A new affidavit key was generated but failed to persist locally.
        ///
        /// The OCW will attempt another affidavit submission with a new key.
        ///
        /// Persistent failure indicates storage corruption.
        SetNewAffidavitKeyFailed,

        /// A new affidavit key for rotation and promotion as active
        /// affidavit key was generated but failed to persist locally.
        ///
        /// The OCW will attempt another affidavit key-rotation with a new key.
        ///
        /// Persistent failure indicates storage corruption.
        SetNextAffidavitKeyFailed,

        /// The active affidavit key is expected in the local keystore
        /// but could not be found.
        ///
        /// Recovery requires regenerating the key under the configured
        /// `KeyTypeId` or chilling and restarting validation.
        ExpectedActiveAffidavitKeyPairNotFound,

        /// The rotated next affidavit key is expected in the local keystore
        /// but could not be found.
        ///
        /// Recovery requires regenerating the key under the configured
        /// `KeyTypeId` or chilling and restarting validation.
        ExpectedNextAffidavitKeyPairNotFound,

        /// Validation has been stopped due to affidavit key rotation failure,
        /// storage corruption, or failed submissions.
        ///
        /// Manual re-validation via `validate` extrinsic is required.
        ValidationStopped,

        /// The author is actively validating in the current session.
        ///
        /// The `chill` extrinsic is blocked until validation completes
        /// or the session ends.
        ActivelyValidating,

        /// The author has submitted an affidavit and is actively
        /// participating in the election.
        ///
        /// The author may `chill` if allowed or wait until
        /// the election concludes to check assigned duty.
        ActivelyContestingElection,

        /// The author has been elected for the upcoming session
        /// and is preparing for validation duties.
        ///
        /// The `chill` extrinsic is blocked until duties complete.
        ActivelyWarmingForValidation,

        /// The author is detected as active, but the specific duty
        /// cannot be determined due to inconsistent state.
        ///
        /// The `chill` extrinsic may be attempted to resolve the state.
        CannotDetermineAuthorActiveDuty,

        /// Offchain storage Active Affidavit Key is not yet finalized.
        ///
        /// The key exists optimistically in fork-aware storage, but the block
        /// containing the finalized value may still be subject to re-org.
        /// Until finalized, speculative forks may observe different values.
        ///
        /// Until it is finalized subsequent OCWs may result in this state.
        ActiveAfdtKeyNotYetFinalized,

        /// Failed to decode the finalized Active Affidavit Key public key value
        /// from persistent offchain storage.
        ///
        /// Persistent failure indicates either storage corruption or a type
        /// mismatch. Manual intervention may be required. Clearing both
        /// fork-aware and persistent offchain storage allows OCWs to
        /// reinitialize the key lifecycle.
        ActiveAfdtKeyFinalizedValueDecodeFail,

        /// Failed to decode the speculative hash of the Active Affidavit Key
        /// public key from fork-aware offchain storage.
        ///
        /// This hash is used to reference the actual value stored in
        /// persistent offchain storage. Persistent failure indicates
        /// corruption or decoding inconsistencies.
        /// Clearing both fork-aware and persistent storage may resolve it.
        ActiveAfdtKeySpeculativeHashDecodeFail,

        /// Concurrent mutation detected while accessing the finalized
        /// Active Affidavit Key public key value.
        ///
        /// The operation will be retried by the OCW. Persistent failure
        /// indicates a potential deadlock or storage corruption.
        /// Clearing both fork-aware and persistent offchain storage
        /// may be required.
        ActiveAfdtKeyFinalizedValueConcurrentMutation,

        /// Concurrent mutation detected while accessing the speculative
        /// hash of the Active Affidavit Key public key.
        ///
        /// The hash resides in fork-aware storage while the actual value
        /// exists in persistent storage. The operation will be retried
        /// by the OCW. Persistent failure indicates storage corruption
        /// or a deadlock condition.
        ActiveAfdtKeySpeculativeHashConcurrentMutation,

        /// A finalized Active Affidavit Key public key value exists in
        /// persistent offchain storage without a corresponding
        /// fork-aware hash.
        ///
        /// This results in a hanging value with no canonical reference.
        /// The persistent value will be cleared automatically. Any
        /// remaining fork-aware entries may also become hanging and
        /// will be cleaned on access.
        ActiveAfdtKeyFinalizedHangingValue,

        /// A speculative fork-aware hash for the Active Affidavit Key exists
        /// without a corresponding persistent public key value.
        ///
        /// Since the actual value cannot be recovered, the hanging
        /// speculative hash will be cleared automatically.
        ActiveAfdtKeySpeculativeHangingHash,

        /// Offchain storage Next Affidavit Key is not yet finalized.
        ///
        /// The key exists optimistically in fork-aware storage, but the block
        /// containing the finalized value may still be subject to re-org.
        /// Until finalized, speculative forks may observe different values.
        ///
        /// Until it is finalized subsequent OCWs may result in this state.
        NextAfdtKeyNotYetFinalized,

        /// Failed to decode the finalized Next Affidavit Key public key value
        /// from persistent offchain storage.
        ///
        /// Persistent failure indicates either storage corruption or a type
        /// mismatch. Manual intervention may be required. Clearing both
        /// fork-aware and persistent offchain storage allows OCWs to
        /// reinitialize the key lifecycle.
        NextAfdtKeyFinalizedValueDecodeFail,

        /// Failed to decode the speculative hash of the Next Affidavit Key
        /// public key from fork-aware offchain storage.
        ///
        /// This hash is used to reference the actual value stored in
        /// persistent offchain storage. Persistent failure indicates
        /// corruption or decoding inconsistencies.
        /// Clearing both fork-aware and persistent storage may resolve it.
        NextAfdtKeySpeculativeHashDecodeFail,

        /// Concurrent mutation detected while accessing the finalized
        /// Next Affidavit Key public key value.
        ///
        /// The operation will be retried by the OCW. Persistent failure
        /// indicates a potential deadlock or storage corruption.
        /// Clearing both fork-aware and persistent offchain storage
        /// may be required.
        NextAfdtKeyFinalizedValueConcurrentMutation,

        /// Concurrent mutation detected while accessing the speculative
        /// hash of the Next Affidavit Key public key.
        ///
        /// The hash resides in fork-aware storage while the actual value
        /// exists in persistent storage. The operation will be retried
        /// by the OCW. Persistent failure indicates storage corruption
        /// or a deadlock condition.
        NextAfdtKeySpeculativeHashConcurrentMutation,

        /// A finalized Next Affidavit Key public key value exists in
        /// persistent offchain storage without a corresponding
        /// fork-aware hash.
        ///
        /// This results in a hanging value with no canonical reference.
        /// The persistent value will be cleared automatically. Any
        /// remaining fork-aware entries may also become hanging and
        /// will be cleaned on access.
        NextAfdtKeyFinalizedHangingValue,

        /// A speculative fork-aware hash for the Next Affidavit Key exists
        /// without a corresponding persistent public key value.
        ///
        /// Since the actual value cannot be recovered, the hanging
        /// speculative hash will be cleared automatically.
        NextAfdtKeySpeculativeHangingHash,

        /// The affidavit submission extrinsic has been sent by the OCW
        /// and is awaiting on-chain state confirmation and key rotation.
        AffidavitTxAwaitingStatus,

        /// Not an error.
        ///
        /// Indicates that a neccessary election attempt was made early,
        /// so the OCW will proceed with affidavit submission instead.
        ProceedingToAffidavitSubmission,

        /// Affidavit submission was declined because the author
        /// has already rotated keys for the next session.
        ///
        /// The author must wait for the next affidavit window.
        DeclareDuringNextAffidavitSession,

        /// An affidavit is declared and the key is rotated before the
        /// current affidavit submission period itself.
        DeclaredBeforeAffidavitPeriod,

        /// The given affidavit key is not the recently rotated affidavit key,
        /// from the recently declared affidavit.
        ///
        /// This implies the author have declared their affidavit and have rotated
        /// a new key for the the election after the upcoming election.
        InvalidRotatedAffidavitKey,

        /// The requested block author of this very current block is not available.
        BlockAuthorNotFound,

        /// An election is attempted by a non-block author. Elections can only be
        /// processed by block-authors only in their authored blocks only.
        TriedElectingByNonBlockAuthor,

        /// The active affidavit key is utilized for affidavit-declaration and
        /// not for processing election.
        ///
        /// Election can only be processed by recently rotated affidavit key, which
        /// poses for the next affidavit-declaration session also.
        ///
        /// All these can be valid only if the author started validating, else its
        /// a dormant affidavit key
        AffidavitKeyForDeclaration,

        /// The provided value must be non-zero.
        ///
        /// Returned when a parameter or input is required to be strictly
        /// greater than zero.
        ValueCannotBeZero,

        /// Internal success signal for the affidavit key initialization routine.
        ///
        /// Used to indicate successful execution of the initialization flow.
        InitAffidavitKeyRoutineSuccess,

        /// Internal success signal for the election execution routine.
        ///
        /// Indicates that the election process completed successfully.
        TryElectionRoutineSuccess,

        /// Internal success signal for the affidavit declaration routine.
        ///
        /// Indicates that the affidavit has been successfully declared.
        DeclarAffidavitRoutineSuccess,

        /// Internal success signal for the affidavit key rotation routine.
        ///
        /// Indicates that the key rotation process completed successfully.
        RotateAffidavitKeyRoutineSuccess,

        /// The author is already in a chilled (inactive) state.
        ///
        /// Returned when attempting to chill an author that is already inactive.
        AuthorAlreadyChilling,

        /// Authors are successfully elected, but cannot be revealed for
        /// some reasons unknown.
        ElectedButCannotReveal,

        /// Author declared an affidavit, but it could not be retrieved
        /// for reasons unknown.
        DeclaredAffidavitNotFound,

        /// The caller is not the current block author.
        ///
        /// Returned when an operation requires block authorship,
        /// but the caller does not match the current block author.
        NotABlockAuthor,

        /// No public key matching the given affidavit identifier was found
        /// in the node-local keystore.
        ///
        /// The key may have been lost, never generated on this node, or
        /// the identifier does not correspond to any locally held key pair.
        AfdtPublicKeyNotFound,

        /// No affidavit key is registered for the author in any relevant
        /// future session scope (next session or next affidavit session).
        ///
        /// The author has not called [`Pallet::validate`] or has already chilled.
        AffidavitKeyPairNotFound,

        /// The election result could not be revealed.
        ///
        /// No election has been executed for the current session, or the
        /// underlying election manager returned no result.
        UnableToRevealElected,

        /// The fork graph has reached the maximum number of concurrent forks.
        ///
        /// No new sibling branch can be created until an existing fork is
        /// resolved or pruned. This indicates an unusually high degree of
        /// chain instability relative to the configured `MAX_FORKS` constant.
        MaxOCWForksAttained,

        /// Fork-aware offchain storage was accessed before the fork graph
        /// was initialized via `ForksHandler::start`.
        OCWForksNotEnabled,

        /// The fork graph is in an inconsistent state.
        ///
        /// An internal invariant was violated, such as a branch entry that
        /// cannot be decoded or a divider that references a non-existent branch.
        /// Persistent failure indicates offchain storage corruption.
        OCWForksInconsistent,
    }

    // ===============================================================================
    // ```````````````````````````````````` EVENTS ```````````````````````````````````
    // ===============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Emitted when an election attempt fails.
        ElectionAttemptFailed {
            session: SessionIndex,
            error: DispatchError,
            runner: AuthorOf<T>,
        },

        /// Emitted after a successful election instance.
        ElectedInstance {
            session: SessionIndex,
            runner: AuthorOf<T>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            elects: ElectionElects<T>,
        },

        /// Emitted after a reward is successfully delegated to be
        /// applied to an author via [`CompensateRoles`].
        RewardInitiated {
            author: AuthorOf<T>,
            value: AssetOf<T>,
        },

        /// Emitted when rewarding an author via [`CompensateRoles`] fails.
        RewardFailed {
            author: AuthorOf<T>,
            error: DispatchError,
        },

        /// Emitted after a penalty is successfully delegated to be
        /// applied to an author via [`CompensateRoles`].
        PenaltyInitiated {
            author: AuthorOf<T>,
            penalty: PenaltyRatio,
        },

        /// Emitted when applying a penalty to an author via
        /// [`CompensateRoles`] fails.
        PenaltyFailed {
            author: AuthorOf<T>,
            error: DispatchError,
        },

        /// Emitted after a successful affidavit submission for the
        /// election to be conducted for the upcoming session.
        AffidavitSubmitted {
            afdt_id: AffidavitId<T>,
            session: SessionIndex,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            author: AuthorOf<T>,
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            affidavit: ElectionVia<T>
        },

        /// Emitted when an author successfully declares their affidavit signing 
        /// key for the next session
        ValidationBegins {
            author: AuthorOf<T>,
            for_session: SessionIndex
        },

        /// Emitted when an author successfully initiates the chilling process
        /// by removing their affidavit key for a future session.
        ChillingBegins {
            author: AuthorOf<T>,
            for_session: SessionIndex
        },

        /// Emitted by [`Pallet::inspect_affidavit`] for direct inspection of a
        /// stored affidavit and its declared election weights.
        #[cfg(feature = "dev")]
        InspectAffidavit {
            author: AuthorOf<T>,
            afdt_id: AffidavitId<T>,
            session: SessionIndex,
            affidavit: ElectionVia<T>
        },

        /// Emitted by [`Pallet::inspect_elects`] for direct inspection of the
        /// currently revealed elected author set.
        #[cfg(feature = "dev")]
        InspectElects {
            elects: ElectionElects<T>
        },

        /// Emitted by [`Pallet::prepare_validation_payload`] exposing the signed
        /// payload and signature required to submit a [`Pallet::validate`] extrinsic.  
        #[cfg(feature = "dev")]
        InspectValidatePayload {
            payload: ValidatePayloadOf<T>,
            signature: T::Signature,
        },

        /// A genesis config parameter was updated forcefully.
        GenesisConfigUpdated(ForceGenesisConfig<T>),
    }

    // ===============================================================================
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ===============================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Offchain Worker entry point coordinating affidavit lifecycle and elections.
        ///
        /// This function acts as a **deterministic orchestrator** for a sequence of
        /// structured offchain routines, executed once per block in best-effort mode.
        /// It does not implement business logic itself; instead, it delegates all
        /// responsibility to well-defined [`Routines`] and [`RoutineOf`] abstractions.
        ///
        /// ## Execution Model
        /// 
        /// All routines execute inside [`ForksHandler::start`], which resolves and
        /// persists the current fork graph branch before any routine runs.
        /// Each routine calls [`Routines::run_service`] directly, followed by
        /// [`Routines::on_ran_service`] on success. The routines are executed
        /// **imperatively and sequentially** with **fail-fast semantics**:
        ///
        /// 1. Initialize the node-local active affidavit key (if required).
        /// 2. Attempt to run the election early (if the election window permits).
        /// 3. Declare an affidavit for the upcoming session (if eligible).
        /// 4. Rotate affidavit keys for the next session.
        ///
        /// If any routine fails, execution stops immediately for the current block.
        /// All failures are already logged by the routine itself; the OCW hook
        /// performs no retries, compensation, or error interpretation.
        ///
        /// ## Semantics & Guarantees
        /// - **Best-effort execution**: no transactional rollback is assumed.
        /// - **Idempotent by design**: repeated execution across blocks or forks is safe.
        /// - **Authorization-aware**: each routine explicitly determines its authorized
        ///   signer via [`RoutineOf::who`] before execution.
        /// - **Fork-safe**: correctness is preserved across re-orgs through
        ///   fork-aware ([`frame_suite::ForkAware`]) and
        ///   finalized ([`frame_suite::Finalized`]) offchain
        ///   storage semantics.
        ///
        /// ## Notes
        /// - This function intentionally returns early on failure.
        /// - All observability is provided via structured logging inside routines.
        /// - Progress is achieved through repeated OCW invocations over time.
        fn offchain_worker(block: BlockNumberFor<T>) {
            <Pallet<T> as ForksHandler<T, ForkLocalDepot>>::start(None, None, ||{

                //------ Initiate Affidavit Key ---------

                let init = InitAffidavitKey { at: block };
                let Ok(_) = 
                <InitAffidavitKey<T> as Routines<BlockNumberFor<T>>>::run_service(&init) else {
                    return;
                };
                <InitAffidavitKey<T> as Routines<BlockNumberFor<T>>>::on_ran_service(&init);

                //------ Try Electing Authors Early ---------

                let Ok(new_afdt_key) =
                <TryElection<T> as RoutineOf<T::Public, BlockNumberFor<T>>>::who(&block) else {
                    return;
                };
                let election = TryElection {
                    by: new_afdt_key,
                    at: block,
                };
                let Ok(_) = 
                <TryElection<T> as Routines<BlockNumberFor<T>>>::run_service(&election) else {
                    return;
                };
                <TryElection<T> as Routines<BlockNumberFor<T>>>::on_ran_service(&election);

                //------ Declare Affidavit  ---------

                let Ok(afdt_key) =
                <DeclareAffidavit<T> as RoutineOf<T::Public, BlockNumberFor<T>>>::who(&block) else {
                    return;
                };

                let declare = DeclareAffidavit {
                    by: afdt_key,
                    at: block,
                };
                let Ok(_) = 
                <DeclareAffidavit<T> as Routines<BlockNumberFor<T>>>::run_service(&declare) else {
                    return;
                };
                <DeclareAffidavit<T> as Routines<BlockNumberFor<T>>>::on_ran_service(&declare);

                //------ Rotate Affidavit Keys ---------

                let Ok(next_afdt_key) =
                <RotateAffidavitKey<T> as RoutineOf<T::Public, BlockNumberFor<T>>>::who(&block) else {
                    return;
                };

                let rotate = RotateAffidavitKey {
                    by: next_afdt_key,
                    at: block,
                };
                let Ok(_) =
                <RotateAffidavitKey<T> as Routines<BlockNumberFor<T>>>::run_service(&rotate) else {
                    return;
                };
                <RotateAffidavitKey<T> as Routines<BlockNumberFor<T>>>::on_ran_service(&rotate);
            });
        }

        /// Called at the beginning of each block.
        ///
        /// Awards block production points to the current block author.
        fn on_initialize(_block: BlockNumberFor<T>) -> Weight {
            // Try to get the current block author from the authorship pallet
            let Some(author) = pallet_authorship::Pallet::<T>::author() else {
                // If no author is found (weight for 1 read operation),
                return T::DbWeight::get().reads(1);
            };

            // Increment the block points for the author.
            // Ignoring errors here, as missing role checks or exhausted points are handled elsewhere.
            let _ = <Pallet<T> as AuthorPoints<AuthorOf<T>, T::Points>>::add_point(&author);
            <T as Config>::WeightInfo::on_initialize_with_author()
        }
    }

    // ===============================================================================
    // `````````````````````````````````` EXTRINSICS `````````````````````````````````
    // ===============================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ```````````````````````````````` DISPATCHABLES ````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

        /// Register an author's new **affidavit signing key** for the upcoming session's
        /// participation as a validator.
        ///
        /// This extrinsic allows an author to declare their **off-chain signing key** that
        /// will be used to sign affidavits for the next session's election.
        ///
        /// It ensures that only valid and available authors can register keys.
        ///
        /// ## Notes
        /// - Affidavit Keys are session-specific and are updated each session.
        /// - This extrinsic allows submission of affidavit for next session only.
        /// - Affidavit Declaration/Submission will rotate keys once every submission
        /// for further sessions then.
        ///
        /// ## Errors
        /// Returns a `DispatchError` if author-role authorization fails.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::validate())]
        pub fn validate(
            origin: OriginFor<T>,
            payload: ValidatePayloadOf<T>,
            // Signature verification happens via `ValidateUnsigned::validate::unsigned`
            // can avoid here
            _signature: T::Signature,
        ) -> DispatchResult {
            // Verify signed origin i.e., author
            let author = ensure_signed(origin)?;

            // Ensure the author exists and is available in the role system
            <T::RoleAdapter as RoleManager<AuthorOf<T>>>::role_exists(&author)?;
            <T::RoleAdapter as RoleManager<AuthorOf<T>>>::is_available(&author)?;

            // Compute the session for which the key is valid
            let for_session = CurrentSession::<T>::get().saturating_add(One::one());
            let public = SignedPayload::<T>::public(&payload);
            let affidavit_pub: AffidavitId<T> = public.clone().into_account().into();

            // Store the key for the upcoming session
            AffidavitKeys::<T>::insert((for_session, affidavit_pub), author.clone());

            Self::deposit_event(Event::<T>::ValidationBegins { author, for_session });
            Ok(())
        }

        /// Submit an **affidavit for the upcoming session election**.
        ///
        /// This extrinsic allows an author to declare their election weights
        /// (affidavit) for the next session. It also rotates the author's signing
        /// key for the subsequent affidavit.
        ///
        /// ## Parameters
        /// - `origin`: Must be a signed account corresponding to the author
        ///   submitting the affidavit.
        /// - `payload`: The payload that was signed off-chain representing the
        ///   affidavit data.
        /// - `signature`: The author's signature of the payload, used for verification.
        /// - `new_key`: A new affidavit signing key to replace the current one for the
        ///  next upcoming session.
        ///
        /// ## Notes
        /// - Affidavit submissions are session-specific.
        /// - Key rotation ensures authors maintain fresh signing keys for security.
        /// - Only validated and available authors can submit affidavits.
        ///
        /// ## Errors
        /// Returns a `DispatchError` if un-privileged to submit an affidavit.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::declare())]
        pub fn declare(
            origin: OriginFor<T>,
            payload: AffidavitPayloadOf<T>,
            // Signature verification happens via `ValidateUnsigned::validate::unsigned`
            // can avoid here
            _signature: T::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let public = SignedPayload::<T>::public(&payload);
            let new_affidavit_pub = payload.rotate.clone();
            let affidavit_pub: AffidavitId<T> = public.clone().into_account().into();

            let for_session = CurrentSession::<T>::get().saturating_add(One::one());
            // Ensure author has a registered key for the upcoming session
            let author = AffidavitKeys::<T>::get((for_session, &affidavit_pub))
                .ok_or(Error::<T>::AffidavitAuthorNotFound)?;

            // Process the affidavit
            <Pallet<T> as ElectionAffidavits<AffidavitId<T>, ElectionVia<T>>>::process_affidavit(
                &affidavit_pub.clone(),
            )?;
            
            // Rotate key for the next affidavit
            AffidavitKeys::<T>::insert(
                (
                    for_session.saturating_add(One::one()),
                    new_affidavit_pub.clone(),
                ),
                &author,
            );

            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                if !T::EmitEvents::get() {
                    let block = frame_system::Pallet::<T>::block_number();
                    let Ok(affidavit) = Self::get_affidavit(&affidavit_pub) else {
                        debug_assert!(
                            false,
                            "author declared affidavit for session {:?} at block {:?}, but it could not be retrieved",
                            for_session, block
                        );
                        return Err(Error::<T>::DeclaredAffidavitNotFound.into());
                    };
                    Self::deposit_event(Event::<T>::AffidavitSubmitted {
                        afdt_id: affidavit_pub,
                        session: for_session,
                        author,
                        affidavit,
                    });
                }
            }

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                if !T::EmitEvents::get() {
                    Self::deposit_event(Event::<T>::AffidavitSubmitted {
                        afdt_id: affidavit_pub,
                        session: for_session,
                    });
                }
            }
            Ok(())
        }

        /// Execute the election for the upcoming session.
        ///
        /// This extrinsic allows an author to act as the **election runner**
        /// for the election session (next-session). It verifies the author's
        /// affidavit-based signature, prepares the election using all submitted
        /// affidavits, and records the runner for audit and reward attribution.
        ///
        /// Since this is an **unsigned extrinsic**, it is constructed and
        /// submitted by validator offchain workers (OCWs).
        ///
        /// Although unsigned, it is treated as a **pseudo-inherent**:
        /// - It is authorized via affidavit signature verification
        /// - It may carry rewards for successfully running the election
        /// - It is **expected** to be submitted locally by validators rather
        ///   than propagated by external transaction authors
        ///
        /// ## Errors
        /// Returns a `DispatchError` if election execution or authorization fails.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::elect())]
        pub fn elect(
            origin: OriginFor<T>,
            payload: ElectionPayloadOf<T>,
            // Signature verification happens via `ValidateUnsigned`
            // can avoid here
            _signature: T::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            // Determine the session for which this election applies (next session)
            let for_session = CurrentSession::<T>::get().saturating_add(One::one());

            let public = SignedPayload::<T>::public(&payload);
            let affidavit_pub: AffidavitId<T> = public.clone().into_account().into();

            // Ensure the author has a valid affidavit key registered
            // Election keys are registered for session+2 because:
            // - Current session: ongoing
            // - Next session: affidavits are submitted
            // - Session after next: election is executed
            // Hence authors who submitted affidavits, submit next election key, hence
            // queriable.
            // This makes only authors who submitted affidavits to run election
            let author = AffidavitKeys::<T>::get((
                for_session.saturating_add(One::one()),
                &affidavit_pub.clone(),
            ))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;

            let block_author =
                pallet_authorship::Pallet::<T>::author().ok_or(Error::<T>::BlockAuthorNotFound)?;
            // This ensures authors who have bypassed `ValidateUnsigned` via
            // `propagate = false` for `elect` unsigned extrinsic
            // shall be captured by the runtime itself
            ensure!(
                author == block_author,
                Error::<T>::TriedElectingByNonBlockAuthor
            );

            // Run the election preparation logic
            if let Err(error) = Internals::<T>::prepare_election(&Some(author.clone())) {
                if !T::EmitEvents::get() {
                    Self::deposit_event(Event::<T>::ElectionAttemptFailed {
                        session: for_session,
                        runner: author.clone(),
                        error,
                    });
                }
                return Err(error);
            };

            // Record the election runner and the block at which the election was conducted
            let current_block = frame_system::Pallet::<T>::block_number();
            ElectsPreparedBy::<T>::insert(for_session, (&author, current_block));

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                if !T::EmitEvents::get() {
                    Self::deposit_event(Event::<T>::ElectedInstance {
                        session: for_session,
                        runner: author,
                    });
                }
            }

            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                if !T::EmitEvents::get() {
                    let Some(elects) = Internals::<T>::reveal() else {
                        debug_assert!(
                            false,
                            "authors elected for session {:?} at 
                            block {:?} by election runner {:?}, 
                            but reveal unavailable",
                            for_session, current_block, &author
                        );
                        return Err(Error::<T>::ElectedButCannotReveal.into());
                    };
                    Self::deposit_event(Event::<T>::ElectedInstance {
                        session: for_session,
                        runner: author,
                        elects,
                    });
                }
            }
            Ok(())
        }

        /// Request to **step back or "chill" immediately from election participation**
        /// by erasing affidavit keys.
        ///
        /// This safely avoids penalties by pausing an author's duties.
        ///
        /// This extrinsic allows an author to withdraw from participating in upcoming
        /// elections or prevent submitting future affidavits. The actual effect
        /// depends on the **current block** relative to the **affidavit submissions
        /// and election windows**.
        ///
        /// Note that its always advised to `chill` validation before `resign` author role
        /// to skip unnecessary invalid affidavit declarations.
        ///
        /// ## Parameters
        /// - `origin`: Must be a signed account corresponding to the author i.e.,
        ///   controller/role account.
        /// - `affidavit_pub` : Public affidavit key registered for a new session's
        ///   affidavit submission.
        ///
        /// ## Notes
        /// - Removing affidavit keys ensures authors cannot unfairly influence
        ///   future elections.
        /// - By inspecting returned errors, callers can **compute the optimal
        ///   chill window**.
        ///
        /// ## Errors
        /// Returns a `DispatchError` with diagnostic in case of irrevocable
        /// duties assigned.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::chill())]
        pub fn chill(origin: OriginFor<T>, affidavit_pub: AffidavitId<T>) -> DispatchResult {
            let author = ensure_signed(origin)?;

            Self::can_chill(author.clone(), affidavit_pub.clone())?;

            let current_session = CurrentSession::<T>::get();
            let next_session = current_session.saturating_add(One::one());
            let next_afdt_session = next_session.saturating_add(One::one());

            // If rotated key exists, remove it.
            if AffidavitKeys::<T>::contains_key((next_afdt_session, &affidavit_pub)) {
                AffidavitKeys::<T>::remove((next_afdt_session, &affidavit_pub));
                Self::deposit_event(Event::<T>::ChillingBegins { author, for_session: next_afdt_session });
                return Ok(());
            }

            // Otherwise remove next-session key if it still exists.
            if AffidavitKeys::<T>::contains_key((next_session, &affidavit_pub)) {
                AffidavitKeys::<T>::remove((next_session, &affidavit_pub));
            }
            Self::deposit_event(Event::<T>::ChillingBegins { author, for_session: next_session });
        
            Ok(())
        }

        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        // ````````````````````````````````` INSPECTORS ``````````````````````````````````
        // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        
        /// Emit the currently revealed elected author set as an event for inspection.
        ///
        /// This is a **read-only convenience extrinsic** that does not mutate any state.
        /// It retrieves the election result from the underlying election manager via
        /// [`Pallet::get_elects`] and emits it as an [`Event::InspectElects`] event.
        ///
        /// ## Notes
        /// - Gated by the `dev` feature. Must not be included in production runtimes.
        /// - Fails if no election result is available, i.e. no election has been
        ///   executed or the result cannot be revealed.
        ///
        /// ## Errors
        /// Returns [`Error::UnableToRevealElected`] if the election result is unavailable.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::inspect_elects())]
        pub fn inspect_elects(
            origin: OriginFor<T>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let elects = Self::get_elects()?;
            Self::deposit_event(Event::InspectElects { elects });
            Ok(())
        }

        /// Construct and emit a signed [`ValidatePayload`] and its signature as an event.
        ///
        /// This is a **read-only convenience extrinsic** that does not mutate any on-chain state.
        /// It is intended to support the [`Self::validate`] extrinsic workflow by producing
        /// the signed payload and signature required to call it.
        ///
        /// Since [`Self::validate`] accepts both a signed origin and an unsigned payload,
        /// callers need a way to obtain a correctly signed payload before submitting.
        /// This extrinsic bridges that gap by performing the signing on behalf of the
        /// local node and emitting the result as an [`Event::InspectValidatePayload`] event.
        ///
        /// ## Execution Model
        /// This extrinsic reads from **node-local offchain storage** to retrieve the
        /// active affidavit key. It is therefore **client-specific**: the result reflects
        /// the state of the validator node executing the call, and may differ across nodes.
        ///
        /// ## Notes
        /// - Gated by the `dev` feature. Must not be included in production runtimes.
        /// - The emitted payload and signature can be submitted directly to [`Self::validate`].
        /// - Requires that the node holds a finalized active affidavit key in offchain storage.
        ///
        /// ## Errors
        /// Returns a `DispatchError` if the active affidavit key is not finalized,
        /// not found in the local keystore, or signing fails.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::prepare_validation_payload())]
        pub fn prepare_validation_payload(
            origin: OriginFor<T>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let (payload, signature) = Self::sign_validate_payload()?;
            Self::deposit_event(Event::InspectValidatePayload { payload, signature });
            Ok(())
        }        

        /// Emit the stored affidavit for a given affidavit identifier as an event.
        ///
        /// This is a **read-only convenience extrinsic** that does not mutate any state.
        /// It retrieves the affidavit associated with the provided [`AffidavitId`] from
        /// storage and emits it as an [`Event::InspectAffidavit`] event, making the
        /// affidavit weights observable without requiring direct storage queries.
        ///
        /// The retrieved affidavit corresponds to the **upcoming session's election**
        /// (current session index + 1).
        ///
        /// ## Notes
        /// - Gated by the `dev` feature. Must not be included in production runtimes.
        /// - Useful for verifying that a previously submitted affidavit was stored correctly
        ///   before the election window opens.
        ///
        /// ## Errors
        /// Returns [`Error::AffidavitAuthorNotFound`] if no author is mapped to the given key,
        /// or [`Error::AffidavitNotFound`] if no affidavit has been submitted for that author.
        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::inspect_affidavit())]
        pub fn inspect_affidavit(
            origin: OriginFor<T>,
            afdt_id: AffidavitId<T>,
        ) -> DispatchResult {
            let author = ensure_signed(origin)?;
            let affidavit = Self::get_affidavit(&afdt_id)?;
            let for_session = CurrentSession::<T>::get().saturating_add(One::one());
            Self::deposit_event(Event::InspectAffidavit { author, session: for_session, afdt_id, affidavit });
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
        /// - `AllowAffidavits` - Enables or disables affidavit submission.
        /// - `AffidavitBeginsAt` - Updates the start of the affidavit submission window.
        /// - `AffidavitEndsAt` - Updates the end of the affidavit submission window.
        /// - `ElectionBeginsAt` - Updates when election execution begins within the session.
        /// - `ElectionRunnerPointsUpgrade` - Updates the reward points for election runners.
        /// - `ValidateTxPriority` - Updates the priority for validation-related extrinsics.
        /// - `ElectionTxPriority` - Updates the priority for election execution extrinsics.
        /// - `AffidavitTxPriority` - Updates the priority for affidavit submission extrinsics.
        /// - `FinalityAfter` - Updates the time-based delay before operations are considered final.
        /// - `FinalityTicks` - Updates the block-based confirmation threshold for finality.
        ///
        /// The call enforces consistency constraints where applicable:
        /// - Affidavit window ordering:
        ///   - `AffidavitBeginsAt < AffidavitEndsAt`
        ///   - `AffidavitEndsAt > AffidavitBeginsAt`
        /// - The following values must be non-zero:
        ///   - transaction priorities
        ///   - finality thresholds (`FinalityAfter`, `FinalityTicks`)
        ///
        /// This call directly overwrites storage and emits an event containing the
        /// updated configuration variant.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::force_allow_affidavits()
            .max(<T as Config>::WeightInfo::force_affidavit_begins_at())
            .max(<T as Config>::WeightInfo::force_affidavit_ends_at())
            .max(<T as Config>::WeightInfo::force_election_begins_at())
            .max(<T as Config>::WeightInfo::force_election_runner_points_upgrade())
            .max(<T as Config>::WeightInfo::force_validate_tx_priority())
            .max(<T as Config>::WeightInfo::force_election_tx_priority())
            .max(<T as Config>::WeightInfo::force_affidavit_tx_priority())
            .max(<T as Config>::WeightInfo::force_finality_after())
            .max(<T as Config>::WeightInfo::force_finality_ticks())
        )]
        pub fn force_genesis_config(
            origin: OriginFor<T>,
            field: ForceGenesisConfig<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            match field {
                ForceGenesisConfig::AllowAffidavits(value) => AllowAffidavits::<T>::put(value),
                ForceGenesisConfig::AffidavitBeginsAt(aff_begins) => {
                    ensure!(
                        aff_begins < AffidavitEndsAt::<T>::get(),
                        Error::<T>::InvalidAffidavitPeriod
                    );
                    AffidavitBeginsAt::<T>::put(aff_begins);
                }
                ForceGenesisConfig::AffidavitEndsAt(aff_ends) => {
                    ensure!(
                        aff_ends > AffidavitBeginsAt::<T>::get(),
                        Error::<T>::InvalidAffidavitPeriod
                    );
                    AffidavitEndsAt::<T>::put(aff_ends);
                }
                ForceGenesisConfig::ElectionBeginsAt(elect_begins) => {
                    ElectionBeginsAt::<T>::put(elect_begins);
                }
                ForceGenesisConfig::ElectionRunnerPointsUpgrade(points) => {
                    ElectionRunnerPointsUpgrade::<T>::put(points);
                }
                ForceGenesisConfig::ValidateTxPriority(priority) => {
                    ensure!(priority > Zero::zero(), Error::<T>::ValueCannotBeZero);
                    ValidateTxPriority::<T>::put(priority);
                }
                ForceGenesisConfig::ElectionTxPriority(priority) => {
                    ensure!(priority > Zero::zero(), Error::<T>::ValueCannotBeZero);
                    ElectionTxPriority::<T>::put(priority);
                }
                ForceGenesisConfig::AffidavitTxPriority(priority) => {
                    ensure!(priority > Zero::zero(), Error::<T>::ValueCannotBeZero);
                    AffidavitTxPriority::<T>::put(priority);
                }
                ForceGenesisConfig::FinalityAfter(moment) => {
                    ensure!(moment > Zero::zero(), Error::<T>::ValueCannotBeZero);
                    FinalityAfter::<T>::put(moment);
                }
                ForceGenesisConfig::FinalityTicks(block) => {
                    ensure!(block > Zero::zero(), Error::<T>::ValueCannotBeZero);
                    FinalityTicks::<T>::put(block);
                }
            }
            Self::deposit_event(Event::GenesisConfigUpdated(field));
            Ok(())
        }
    }

    // ===============================================================================
    // ````````````````````````````````` PUBLIC APIS `````````````````````````````````
    // ===============================================================================

    impl<T: Config> Pallet<T> {
        /// Returns `Ok(())` if the author is permitted to submit the [`Self::chill`] extrinsic
        /// using the given affidavit key.
        ///
        /// Performs a read-only pre-check verifying that chilling does not
        /// conflict with any active or pending duty. The check branches on which
        /// session scope the key belongs to:
        ///
        /// - **Next affidavit session (current + 2)**: author has already declared and
        ///   rotated. Chilling is allowed only after the election window closes and
        ///   the author is not in the revealed elected set.
        ///
        /// - **Next session (current + 1)**: author has registered but may not have
        ///   declared yet. Chilling is allowed before the affidavit window or after
        ///   it closes without a declaration. Rejected if a declaration exists but
        ///   the key is stale relative to the expected post-rotation key.
        ///
        /// ## Parameters
        /// - `author`: The author requesting to chill.
        /// - `affidavit_pub`: The affidavit key registered via [`Self::validate`]
        ///   or rotated during [`Self::declare`].
        ///
        /// ## Returns
        /// - `Ok(())` if chilling is permitted.
        /// - `Err(ValidatorCannotChill)` if the author is active in the current session.
        /// - `Err(CandidateCannotChill)` if the election window is still open.
        /// - `Err(ElectedCannotChill)` if the author appears in the revealed elected set.
        /// - `Err(InvalidRotatedAffidavitKey)` if a declaration exists but the key is stale.
        /// - `Err(DispatchError)` for ownership, timing, or configuration violations.
        pub fn can_chill(author: AuthorOf<T>, affidavit_pub: AffidavitId<T>) -> DispatchResult {
            let current_session = CurrentSession::<T>::get();
            let next_session = current_session.saturating_add(One::one());
            let next_afdt_session = next_session.saturating_add(One::one());

            // Convert author -> validator id for the current session
            // If conversion fails, runtime cannot determine validator status
            let Some(validator) =
                <Pallet<T> as Convert<AuthorOf<T>, Option<SessionId<T>>>>::convert(author.clone())
            else {
                return Err(Error::<T>::SessionIdQueryFailed.into());
            };

            // Active validators of the *current* session are not allowed to chill.
            // Reason: they may still be required if elections fail or re-run.
            ensure!(
                !pallet_session::Pallet::<T>::validators().contains(&validator),
                Error::<T>::ValidatorCannotChill
            );

            // Compute affidavit submission window for the upcoming election
            let aff_window = Self::compute_affidavit_window()?;
            let start_affidavit = aff_window.start;
            let end_affidavit = aff_window.end;
            // Ensure configuration is sane: submission window must be valid
            debug_assert!(
                start_affidavit < end_affidavit,
                "Affidavit submission period invalid: starts at {:?}, ends at {:?}",
                start_affidavit,
                end_affidavit
            );
            ensure!(
                start_affidavit < end_affidavit,
                Error::<T>::InvalidAffidavitPeriod
            );

            let current_block = frame_system::Pallet::<T>::block_number();

            // CASE 1: Key belongs to next-next session (already rotated)
            //
            // Optimistically we need to check.
            //
            // This means:
            // - Author already declared an affidavit for `next_session`
            // - During declaration, a new key was rotated for `next_afdt_session`
            // Hence this author may very-well be a candidate for the ongoing election.
            if let Some(id) = AffidavitKeys::<T>::get((next_afdt_session, &affidavit_pub)) {
                ensure!(id == author, Error::<T>::AuthorNotAffidavitOwner);
                // Sanity: a rotated key implies the affidavit must have been declared
                // within the valid submission window.
                debug_assert!(
                    !(current_block < start_affidavit),
                    "affidavit key is for next-next session, effectively means it
                    declared affidavit and rotated next key, but it happened before
                    affidavit submission period itself? for author {:?} and afdt key {:?},
                    rotated during elected session {:?}, and rotated for session {:?}",
                    author,
                    affidavit_pub,
                    next_session,
                    next_afdt_session
                );
                ensure!(
                    !(current_block < start_affidavit),
                    Error::<T>::DeclaredBeforeAffidavitPeriod
                );

                // If still within affidavit submission window:
                // author is an active candidate -> cannot chill.
                // It is possible that the author is not present in the elected
                // list at this stage.
                // In that case, we ideally should purge:
                //   - the declared affidavit for `next_session`,
                //   - the affidavit key for `next_session`,
                //   - and the rotated affidavit key for `next_afdt_session`.
                // However, at this point we only have access to the
                // `next_afdt_session` affidavit key i.e., rotated key.
                ensure!(
                    current_block >= end_affidavit,
                    Error::<T>::CandidateCannotChill
                );

                // After the election period:
                // Only authors who were NOT elected are allowed to chill.
                //
                // We already enforced earlier that active validators of the *current session*
                // cannot chill. This guarantees that even if the `reveal` result here is
                // slightly outdated or later ignored by the session (e.g. the same validators
                // are re-posted due to election issues), safety is preserved.
                //
                // Therefore:
                // - If the author appears in the revealed elected set -> they must not chill.
                // - If the author does NOT appear -> they are not part of the upcoming elected set.
                // - Even if the reveal is stale, it can only reflect a previous valid election
                //   outcome, and cannot incorrectly allow a current validator to chill.
                //
                // Hence, it is safe to rely on this check to prevent elected (or soon-to-be
                // elected) authors from chilling.
                let elected = <Internals<T> as ElectAuthors<AuthorOf<T>, ElectionVia<T>>>::reveal();
                if let Some(elected) = elected {
                    for elect in elected.into_iter() {
                        if author == elect {
                            return Err(Error::<T>::ElectedCannotChill.into());
                        }
                    }
                }

                // Election finished and author not elected:
                // author can chill at next-next election.
                return Ok(());
            }

            // CASE 2: Key belongs to next session
            //
            // This means the author has registered an affidavit key but may NOT yet
            // declared the affidavit for the upcoming election.
            let id = AffidavitKeys::<T>::get((next_session, &affidavit_pub))
                .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
            ensure!(id == author, Error::<T>::AuthorNotAffidavitOwner);

            // Subcase A: Before affidavit submission window begins
            //
            // Author is simply opting out before participating in election.
            if current_block < start_affidavit {
                let afdt_decl = AuthorAffidavits::<T>::contains_key((next_session, &author));

                // Sanity: affidavit should not be declared before submission window.
                debug_assert!(
                    !afdt_decl,
                    "affidavit is declared before affidavit submission period
                    by author {:?} for election conducted for session {:?} using
                    afdt-key {:?}",
                    author, next_session, affidavit_pub
                );
                ensure!(!afdt_decl, Error::<T>::DeclaredBeforeAffidavitPeriod);

                return Ok(());
            }

            // Subcase B: During or after affidavit submission window
            match AuthorAffidavits::<T>::contains_key((next_session, &author)) {
                true => {
                    // Affidavit has already been declared for `next_session`.
                    //
                    // Normally, this implies that a new (rotated) affidavit key for `next_afdt_session`
                    // should have been registered as part of the declaration flow.
                    //
                    // If we still reached here with the current key, it likely means the provided key
                    // is stale or does not correspond to the rotated key expected after declaration.
                    //
                    // However, we must not blindly assume rotation always changes the key. In some
                    // implementations (e.g., custom declaration flows outside this pallet's OCWs),
                    // the same affidavit key might be reused during rotation.
                    //
                    // Therefore, rather than mutating any state based on uncertain assumptions,
                    // we conservatively reject the call with an explicit error.
                    return Err(Error::<T>::InvalidRotatedAffidavitKey.into());
                }
                false => {
                    // Either still inside submission window but not yet declared,
                    // or window already ended and author implicitly chilled.
                    return Ok(());
                }
            }
        }

        /// Returns `Ok(())` if the author is eligible to submit a [`Self::validate`] extrinsic.
        ///
        /// Performs a pre-check for validation readiness by verifying that:
        /// - the author is not already an active validator in the current session,
        /// - the author exists in the role system via [`Config::RoleAdapter`], and
        /// - the author is currently marked as available.
        ///
        /// Intended for use by offchain workers and RPC consumers before constructing
        /// and submitting a [`Self::validate`] extrinsic.
        ///
        /// ## Parameters
        /// - `author`: The author whose validation eligibility is being checked.
        ///
        /// ## Returns
        /// - `Ok(())` if all conditions for validation readiness are satisfied.
        /// - `Err(ActivelyValidating)` if the author is already in the active validator set.
        /// - `Err(DispatchError)` if the role check or availability check fails.
        pub fn can_validate(author: AuthorOf<T>) -> DispatchResult {
            if Self::is_validating(author.clone()) {
                return Err(Error::<T>::ActivelyValidating.into());
            }
            <T::RoleAdapter as RoleManager<AuthorOf<T>>>::role_exists(&author)?;
            <T::RoleAdapter as RoleManager<AuthorOf<T>>>::is_available(&author)?;
            Ok(())
        }

        /// Returns `true` if the author is an active validator in the **current session**.
        ///
        /// Converts the author identifier into its session-specific validator id using
        /// the configured [`Convert`] implementation, then checks whether that validator
        /// appears in the current session's active validator set via [`pallet_session`].
        ///
        /// Returns `false` if the author-to-validator conversion fails, as the runtime
        /// cannot determine validator status without a valid session identity.
        ///
        /// ## Parameters
        /// - `author`: The author whose active validation status is being checked.
        pub fn is_validating(author: AuthorOf<T>) -> bool {
            // Convert author -> validator id for the current session
            let Some(validator) =
                <Pallet<T> as Convert<AuthorOf<T>, Option<SessionId<T>>>>::convert(author.clone())
            else {
                return false;
            };

            // Active validators of the *current* session.
            if pallet_session::Pallet::<T>::validators().contains(&validator) {
                return true;
            }
            false
        }

        /// Returns `true` if the author has no registered affidavit key in any
        /// relevant future session scope and is therefore in a **chilled** state.
        ///
        /// This is the logical inverse of [`Self::is_pursuing`] and delegates
        /// directly to [`Self::get_runtime_afdt_key`].
        ///
        /// ## Parameters
        /// - `author`: The author whose chilling status is being checked.
        pub fn is_chilling(author: AuthorOf<T>) -> bool {
            Self::get_runtime_afdt_key(author).is_err()
        }

        /// Retrieve the author's registered affidavit key and its associated session
        /// from the two relevant future session scopes.
        ///
        /// Searches across:
        /// - the **next affidavit session** (current + 2), checked first, and
        /// - the **next session** (current + 1), checked second.
        ///
        /// The next affidavit session is checked first because a key stored there
        /// indicates a more advanced lifecycle state: the author has already declared
        /// an affidavit for the next session and rotated to a fresh key for the
        /// session after.
        ///
        /// ## Parameters
        /// - `author`: The author whose registered affidavit key is being retrieved.
        ///
        /// ## Returns
        /// - `Ok((session, affidavit_key))` if a key owned by the author is found
        ///   in either future session scope.
        /// - `Err(AffidavitKeyPairNotFound)` if no registered key exists for the author.
        pub fn get_runtime_afdt_key(
            author: AuthorOf<T>,
        ) -> Result<(SessionIndex, AffidavitId<T>), DispatchError> {
            let current_session = CurrentSession::<T>::get();
            let next_session = current_session.saturating_add(One::one());
            let next_afdt_session = next_session.saturating_add(One::one());

            if let Some((affidavit_pub, _)) = AffidavitKeys::<T>::iter_prefix((next_afdt_session,))
                .find(|(_, owner)| *owner == author)
            {
                return Ok((next_afdt_session, affidavit_pub));
            }

            if let Some((affidavit_pub, _)) =
                AffidavitKeys::<T>::iter_prefix((next_session,)).find(|(_, owner)| *owner == author)
            {
                return Ok((next_session, affidavit_pub));
            }

            Err(Error::<T>::AffidavitKeyPairNotFound.into())
        }

        /// Resolve an affidavit account identifier to its corresponding public key
        /// in the **node-local keystore**.
        ///
        /// Iterates all locally available affidavit application keys, derives the
        /// account-form identifier for each, and returns the public key whose derived
        /// account matches the provided `afdt_key`.
        ///
        /// ## Parameters
        /// - `afdt_key`: The affidavit account identifier to resolve.
        ///
        /// ## Returns
        /// - `Ok(public_key)` if a matching key is found in the local keystore.
        /// - `Err(AfdtPublicKeyNotFound)` if no locally held key derives to the given identifier.
        ///
        /// ## Note
        /// This function reads from the node-local keystore and may return different
        /// results across nodes depending on which keys each node holds.
        pub fn get_public_key(afdt_key: AffidavitId<T>) -> Result<T::Public, DispatchError> {
            let all_keys =
                    <<T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic
                        as RuntimeAppPublic>::all();
            for key in all_keys.into_iter() {
                let generic_pub: <T::AffidavitCrypto as AppCrypto<T::Public, T::Signature>>::GenericPublic =
                    key.into();
                let public: T::Public = generic_pub.into();
                let account: AffidavitId<T> = public.clone().into_account().into();

                if account == afdt_key {
                    return Ok(public);
                }
            }
            Err(Error::<T>::AfdtPublicKeyNotFound.into())
        }

        /// Retrieve the currently finalized **active affidavit key** from node-local
        /// offchain storage.
        ///
        /// Reads the active affidavit key using finalized offchain storage semantics
        /// and returns it only when its confidence level is [`Confidence::Safe`].
        /// Keys that exist but have not yet reached safe confidence are rejected,
        /// as they may still be subject to re-org.
        ///
        /// ## Returns
        /// - `Ok(key)` if a finalized key exists and is marked as [`Confidence::Safe`].
        /// - `Err(ActiveAfdtKeyFinalizedHangingValue)` if a finalized value exists
        ///   without a corresponding fork-aware reference.
        /// - `Err(ActiveAfdtKeyNotYetFinalized)` if the key exists but has not yet
        ///   reached safe confidence.
        /// - `Err(DispatchError)` if the offchain storage read itself fails.
        ///
        /// ## Note
        /// This value is maintained in node-local offchain storage and is not part
        /// of on-chain state. Results may differ across nodes.
        pub fn get_finalized_afdt_key() -> Result<AffidavitId<T>, DispatchError> {
            let result = Finalized::<T, AffidavitId<T>, DeclareAffidavit<T>, Pallet<T>>::get(
                ACTIVE_AFDT_KEY,
                LOG_TARGET_AFDT,
                None,
            );
            match result {
                Ok(None) => Err(Error::<T>::ActiveAfdtKeyFinalizedHangingValue.into()),
                Ok(Some(Confidence::Safe(key))) => Ok(key),
                Ok(Some(_)) => Err(Error::<T>::ActiveAfdtKeyNotYetFinalized.into()),
                Err(e) => Err(e),
            }
        }

        /// Construct and sign a [`ValidatePayload`] using the currently finalized
        /// active affidavit key from node-local offchain storage.
        ///
        /// Performs the following steps in sequence:
        /// - retrieves the finalized active affidavit key via [`Self::get_finalized_afdt_key`],
        /// - resolves that key to a local public key via [`Self::get_public_key`],
        /// - constructs a [`ValidatePayload`] wrapping the resolved public key, and
        /// - signs the payload using the configured [`Config::AffidavitCrypto`] scheme.
        ///
        /// The resulting payload and signature are the exact inputs required by
        /// the [`Self::validate`] extrinsic.
        ///
        /// ## Returns
        /// - `Ok((payload, signature))` if the key is finalized, locally available,
        ///   and signing succeeds.
        /// - `Err(CannotSignValidateTxPayload)` if signing fails.
        /// - `Err(DispatchError)` if key retrieval or resolution fails.
        ///
        /// ## Note
        /// This function reads from node-local offchain storage and the local keystore.
        /// Results are node-specific and may differ across validators.
        pub fn sign_validate_payload() -> Result<(ValidatePayloadOf<T>, T::Signature), DispatchError> {
            let active_afdt_key = Self::get_finalized_afdt_key()?;
            let afdt_pub = Self::get_public_key(active_afdt_key)?;
            let payload = ValidatePayload { public: afdt_pub };
            let Some(signature) = <ValidatePayload<T::Public> as SignedPayload<T>>::sign::<
                T::AffidavitCrypto,
            >(&payload) else {
                return Err(Error::<T>::CannotSignValidateTxPayload.into());
            };
            Ok((payload, signature))
        }

        /// Retrieve the currently revealed elected author set from the election manager.
        ///
        /// Delegates to [`ElectAuthors::reveal`] and returns the result as a 
        /// concrete [`ElectionElects`] value.
        ///
        /// This function does not trigger or re-run an election. It only reads
        /// the result of the most recently completed election preparation.
        /// If no election has been executed or the result is unavailable,
        /// it returns an error.
        ///
        /// ## Returns
        /// - `Ok(ElectionElects)` containing the elected author set.
        /// - `Err(UnableToRevealElected)` if no election result is currently available.
        pub fn get_elects() -> Result<ElectionElects<T>, DispatchError> {
            let Some(elects) = <Internals<T> as ElectAuthors<AuthorOf<T>, ElectionVia<T>>>::reveal() else {
                return Err(Error::<T>::UnableToRevealElected.into())
            };
            Ok(elects)
        }

        /// Retrieve the submitted affidavit for a given affidavit identifier
        /// targeting the **upcoming session's election**.
        ///
        /// Resolves the affidavit key to its owning author, then returns the
        /// author's declared election weights for the next session (current + 1).
        ///
        /// ## Parameters
        /// - `afdt_id`: The affidavit key identifier to look up.
        ///
        /// ## Returns
        /// - `Ok(ElectionVia)` containing the author's declared election weights.
        /// - `Err(DispatchError)` otherwise.
        pub fn fetch_affidavit(afdt_id: AffidavitId<T>) -> Result<ElectionVia<T>, DispatchError> {
            Self::get_affidavit(&afdt_id)
        }

        /// Retrieve the submitted affidavit for a given affidavit identifier
        /// targeting a **specific session**.
        ///
        /// Unlike [`Self::fetch_affidavit`], which always targets the upcoming session,
        /// this function allows querying affidavits for any session by index.
        /// It resolves the affidavit key to its registered author for the given session,
        /// then returns that author's declared election weights.
        ///
        /// ## Parameters
        /// - `afdt_id`: The affidavit key identifier to look up.
        /// - `session`: The session index for which the affidavit is queried.
        ///
        /// ## Returns
        /// - `Ok(ElectionVia)` containing the author's declared election weights for the session.
        /// - `Err(AffidavitKeyPairNotFound)` if the key is not registered for the given session.
        /// - `Err(AffidavitNotFound)` if the author has not submitted an affidavit for that session.
        pub fn fetch_affidavit_for(afdt_id: AffidavitId<T>, session: SessionIndex) -> Result<ElectionVia<T>, DispatchError> {
            let Some(author) = AffidavitKeys::<T>::get((session, afdt_id)) else { return Err(Error::<T>::AffidavitKeyPairNotFound.into()) };
            let Some((_, affidavit)) = AuthorAffidavits::<T>::get((session, author)) else { return  Err(Error::<T>::AffidavitNotFound.into()) };
            Ok(affidavit.into_iter().collect())
        }

        /// Returns `true` if the author has submitted an affidavit and is
        /// actively contesting the **upcoming session's election**.
        ///
        /// An author is considered contesting when a valid affidavit entry exists
        /// in storage for the next session (current + 1). This indicates that the
        /// author has declared election weights and is a candidate for selection.
        ///
        /// ## Parameters
        /// - `author`: The author whose election candidacy is being checked.
        ///
        /// ## Returns
        /// - `true` if the author has a stored affidavit for the upcoming session.
        /// - `false` otherwise, including when the author has never submitted
        ///   an affidavit or has withdrawn.
        pub fn is_contesting(author: AuthorOf<T>) -> bool {
            let for_session = CurrentSession::<T>::get().saturating_add(One::one());
            let Some((_, _)) = AuthorAffidavits::<T>::get((for_session, author)) else { return false };
            true
        }

        /// Returns `true` if the author is actively **pursuing validation**
        /// by maintaining a registered affidavit key for a future session.
        ///
        /// An author is considered pursuing when they have a registered affidavit key
        /// in either the next session or the next affidavit session scope, meaning
        /// they have not chilled and intend to participate in upcoming elections.
        ///
        /// This is the logical inverse of [`Self::is_chilling`].
        ///
        /// ## Parameters
        /// - `author`: The author whose pursuit status is being checked.
        ///
        /// ## Returns
        /// - `true` if the author holds an affidavit key for any relevant future session.
        /// - `false` if the author has no registered keys and is effectively chilled.
        pub fn is_pursuing(author: AuthorOf<T>) -> bool {
            !Self::is_chilling(author)
        }

        /// Returns `Ok(())` if the author identified by the given affidavit key
        /// is eligible to submit an affidavit declaration for the upcoming session.
        ///
        /// This function performs a full pre-check for the [`Self::declare`] extrinsic,
        /// verifying that:
        /// - the affidavit key is registered for the upcoming session,
        /// - the global [`AllowAffidavits`] flag is enabled,
        /// - the author associated with the key is available in the role system, and
        /// - the current block falls within the configured affidavit submission window.
        ///
        /// Intended for use by offchain workers and RPC consumers to determine
        /// whether an affidavit declaration can be safely submitted at the current block.
        ///
        /// ## Parameters
        /// - `afdt_id`: The affidavit key identifier to evaluate.
        ///
        /// ## Returns
        /// - `Ok(())` if all conditions for affidavit submission are satisfied.
        /// - `Err(DispatchError)` otherwise.
        pub fn can_declare(afdt_id: AffidavitId<T>) -> DispatchResult {
            let for_session = CurrentSession::<T>::get().saturating_add(One::one());
            ensure!(
                AffidavitKeys::<T>::contains_key((for_session, &afdt_id)), 
                Error::<T>::AffidavitAuthorNotFound
            );
            Self::can_submit_affidavit(&afdt_id)
        }

        /// Returns `Ok(())` if the given author is eligible to submit an election
        /// extrinsic at the current block.
        ///
        /// This function performs a full pre-check for the [`Self::elect`] extrinsic,
        /// verifying that:
        /// - the current block has an identifiable block author,
        /// - the provided author matches the current block author, and
        /// - the current block falls within the configured election window.
        ///
        /// Only the block author may run the election for a given block, ensuring
        /// that election submissions cannot be spoofed by non-producing validators.
        ///
        /// ## Parameters
        /// - `author`: The author asserting eligibility to run the election.
        ///
        /// ## Returns
        /// - `Ok(())` if the author is the current block author and the election window is open.
        /// - `Err(DispatchError)` otherwise.
        pub fn can_elect(author: AuthorOf<T>) -> DispatchResult {
            let block_author = pallet_authorship::Pallet::<T>::author().ok_or(Error::<T>::BlockAuthorNotFound)?;
            ensure!(
                author == block_author,
                Error::<T>::NotABlockAuthor
            );
            <Internals<T> as ElectAuthors<AuthorOf<T>, ElectionVia<T>>>::can_process_election(&Some(block_author))?;
            Ok(())
        }

        /// Computes the affidavit submission window for the current session.
        ///
        /// The window is derived from the session start and average session length:
        /// ```text
        /// start = session_start + (affidavit_begins_at * avg_session_length)
        /// end   = session_start + (affidavit_ends_at   * avg_session_length)
        /// ```
        ///
        /// Note: `affidavit_begins_at` and `affidavit_ends_at` are **percentages**
        /// of the session length and are applied to compute block offsets.
        ///
        /// ## Returns
        /// - `Ok(AffidavitWindow)` containing start and end blocks
        /// - `DispatchError` otherwise
        ///
        /// ## Notes
        /// - The resulting window is session-relative and recalculated each session.
        /// - Affidavits submitted outside this window should be rejected.
        pub fn compute_affidavit_window() -> Result<AffidavitWindow<T>, DispatchError> {
            let session_start = SessionStartAt::<T>::get();
            let avg_session_len =
                <<T as crate::Config>::NextSessionRotation as EstimateNextSessionRotation<
                    BlockNumberFor<T>,
                >>::average_session_length();

            let begin_at = AffidavitBeginsAt::<T>::get();
            let begin_offset = begin_at.mul_floor(avg_session_len);
            let start_block = session_start.saturating_add(begin_offset);

            let ends_at = AffidavitEndsAt::<T>::get();
            let invariant = ends_at > begin_at;
            debug_assert!(
                invariant,
                "invalid affidavit period configured during genesis or 
                root update to storage values, affidavit begins at {:?}
                and ends at {:?}",
                begin_at, ends_at
            );
            ensure!(invariant, Error::<T>::InvalidAffidavitPeriod);
            let end_offset = ends_at.mul_floor(avg_session_len);
            let end_block = session_start.saturating_add(end_offset);
            let aff_window = AffidavitWindow::<T> {
                start: start_block,
                end: end_block,
            };
            Ok(aff_window)
        }

        /// Computes the election window for the current session.
        ///
        /// The election window is a sub-range of the affidavit window:
        /// ```text
        /// start = affidavit_start + (election_begins_at * (affidavit_end - affidavit_start))
        /// end   = affidavit_end
        /// ```
        ///
        /// Note:
        /// - `election_begins_at` is a **percentage** of the affidavit window range.
        /// - The election always **ends when the affidavit window ends**.
        ///
        /// ## Diagram
        /// ```text
        /// |--------- Affidavit Window ---------|
        /// |------|-----------------------------|
        ///        ^                             ^
        ///   election_start               election_end (= affidavit_end)
        /// ```
        ///
        /// ## Returns
        /// - `Ok(ElectionWindow)` containing start and end blocks
        /// - `DispatchError` otherwise
        pub fn compute_election_window() -> Result<ElectionWindow<T>, DispatchError> {
            let afdt_window = Self::compute_affidavit_window()?;
            let start_affidavit = afdt_window.start;
            let end_affidavit = afdt_window.end;
            let election_begin_at = ElectionBeginsAt::<T>::get();
            let affidavit_range = end_affidavit.saturating_sub(start_affidavit);
            let start_portion = election_begin_at.mul_floor(affidavit_range);
            let start_election = start_affidavit.saturating_add(start_portion);
            let elect_window = ElectionWindow::<T> {
                start: start_election,
                end: end_affidavit,
            };
            Ok(elect_window)
        }
    }

    // ===============================================================================
    // `````````````````````````````` VALIDATE UNSIGNED ``````````````````````````````
    // ===============================================================================

    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            let Ok(aff_window) = Self::compute_affidavit_window() else {
                return InvalidTransaction::BadProof.into();
            };
            match call {
                Call::declare { payload, signature } => {
                    if !SignedPayload::<T>::verify::<T::AffidavitCrypto>(payload, signature.clone())
                    {
                        return InvalidTransaction::BadProof.into();
                    }

                    let end_block = aff_window.end;

                    // Current block number (fork-relative).
                    // Safe to use for freshness/expiry checks since validity is evaluated per fork head.
                    let current_block = frame_system::Pallet::<T>::block_number();

                    // Reject if the affidavit window has expired.
                    // This is fork-safe because it is a monotonic freshness check.
                    if current_block > end_block {
                        return InvalidTransaction::Stale.into();
                    }

                    let public = SignedPayload::<T>::public(payload);
                    let affidavit_pub: AffidavitId<T> = public.clone().into_account().into();

                    let for_session = CurrentSession::<T>::get().saturating_add(One::one());

                    // Ensure the signer is registered as a valid affidavit key for that session.
                    if !AffidavitKeys::<T>::contains_key((for_session, &affidavit_pub)) {
                        return InvalidTransaction::BadSigner.into();
                    }

                    // Longevity: how long (in blocks) this transaction should remain valid in the pool.
                    // Derived from remaining time until the affidavit window closes.
                    let longetivity = end_block.saturating_sub(current_block).into();

                    return ValidTransaction::with_tag_prefix("declare")
                        .priority(AffidavitTxPriority::<T>::get())
                        .longevity(longetivity.low_u64())
                        // Provide a uniqueness tag to prevent duplicate unsigned submissions
                        // per (session, validator). The tuple is SCALE-encoded and used as the pool key.
                        .and_provides((for_session, affidavit_pub))
                        // Allow propagation to other nodes (normal gossip).
                        .propagate(true)
                        .build();
                }
                Call::validate { payload, signature } => {
                    if !SignedPayload::<T>::verify::<T::AffidavitCrypto>(payload, signature.clone())
                    {
                        return InvalidTransaction::BadProof.into();
                    }
                    let avg_session_len = <<T as crate::Config>::NextSessionRotation as EstimateNextSessionRotation<BlockNumberFor<T>>>::average_session_length();
                    let session_start = SessionStartAt::<T>::get();
                    let current_block = frame_system::Pallet::<T>::block_number();
                    // Longevity: remaining blocks until end of session.
                    // This keeps the tx in the pool only while session is still active.
                    let longetivity = session_start
                        .saturating_add(avg_session_len)
                        .saturating_sub(current_block)
                        .into();

                    return ValidTransaction::with_tag_prefix("validate")
                        .priority(ValidateTxPriority::<T>::get())
                        .longevity(longetivity.low_u64())
                        // Provide a uniqueness tag based on the SCALE-encoded payload.
                        // This prevents duplicate submissions of the exact same validation payload
                        // from coexisting in the transaction pool (replay/spam protection).
                        .and_provides(payload)
                        .propagate(true)
                        .build();
                }
                Call::elect { payload, signature } => {
                    if !SignedPayload::<T>::verify::<T::AffidavitCrypto>(payload, signature.clone())
                    {
                        return InvalidTransaction::BadProof.into();
                    }
                    let end_affidavit = aff_window.end;
                    let current_block = frame_system::Pallet::<T>::block_number();

                    // Reject if election period has expired.
                    // Fork-safe freshness check.
                    if current_block > end_affidavit {
                        return InvalidTransaction::Stale.into();
                    }

                    // Elections target the session after next (current + 2), since
                    // affidavit declaration would have resulted in newly rotated key.
                    let for_session = CurrentSession::<T>::get().saturating_add(2u32.into());

                    let public = SignedPayload::<T>::public(payload);
                    let affidavit_pub: AffidavitId<T> = public.clone().into_account().into();

                    // Ensure signer is eligible for that future session.
                    if !AffidavitKeys::<T>::contains_key((for_session, &affidavit_pub.clone())) {
                        return InvalidTransaction::BadSigner.into();
                    }

                    // Longevity tied to remaining election window.
                    let longetivity = end_affidavit.saturating_sub(current_block).into();

                    return ValidTransaction::with_tag_prefix("elect")
                        .priority(ElectionTxPriority::<T>::get())
                        .longevity(longetivity.low_u64())
                        // Provide uniqueness per (future session, validator)
                        // to avoid duplicate elections.
                        .and_provides((for_session, affidavit_pub))
                        // Do not propagate: only the block author (local node)
                        // should include this tx, As its checked in the runtime via
                        // pallet-authorship.
                        .propagate(false)
                        .build();
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}