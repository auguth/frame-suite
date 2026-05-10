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
// ```````````````````````````` ROLE MANAGEMENT SUITE ````````````````````````````
// ===============================================================================

//! Defines a **unified abstraction layer** for managing *role-based logic*
//! within a runtime system.  
//!
//! It provides **trait-based contracts** that describe the lifecycle, funding,
//! and economic behaviors of entities that assume specific roles, such as:
//!
//! - Validators and collators
//! - Governance council members
//! - Oracle operators or data feeders
//! - Curators, auditors, or relayers
//!
//! ## Overview
//!
//! Each trait in this module represents a **composable building block** for defining
//! how a role behaves in a decentralized system. These abstractions are designed to be
//! generic, interoperable, and extensible across multiple pallets or runtime modules.
//!
//! ### Currently Included Traits
//!
//! - [`RoleManager`] - Core lifecycle and collateral management for role-bearing entities.  
//! - [`FundRoles`] - Extends `RoleManager` with *backing* and *funding* capabilities.  
//! - [`CompensateRoles`] - Extends `RoleManager` with *reward* and *penalty* mechanics.  
//! - [`RoleProbation`] - Extends `RoleManager` with **probation** and **permanence** privileges.  
//!
//! ### Future Extensions
//!
//! These traits will compose together to form a **role framework** enabling complex
//! behaviors (like staking, governance, delegation, conflict resolutions, etc)
//! to be implemented consistently across different runtime modules.
//!
//! ## Design Philosophy
//!
//! - **Composability:** Each role trait defines minimal, orthogonal functionality that
//!   can be combined with others to form rich behavior.  
//! - **Abstraction over implementation:** These traits are *interfaces*, not storage-bound
//!   logic. Concrete modules implement them.  
//! - **Interoperability:** Enables higher-level systems (e.g. governance, incentives,
//!   or auditing) to operate generically across role types.  
//! - **Auditability:** Built-in temporal tracking (timestamps, holds, etc.) ensures
//!   transparent lifecycle and accounting for each role.  
//!
//! This approach allows shared governance, staking, and incentive systems to operate on
//! **generic roles** without being tightly coupled to any single pallet or implementation.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{Asset, Delimited, Percentage, RuntimeEnum, Time};

// --- FRAME Support ---
use frame_support::traits::{
    tokens::{Fortitude, Precision},
    VariantCount,
};

// --- Substrate primitives ---
use sp_runtime::{DispatchError, DispatchResult, Vec};

// ===============================================================================
// ```````````````````````````````` ROLE MANAGER `````````````````````````````````
// ===============================================================================

/// A **universal abstraction** for managing *roles* within a runtime context.
///
/// This trait defines the full lifecycle of a role-bearing entity - from enrollment
/// and collateral management to status transitions and resignation.  
///
/// It is intended to be the foundational interface for any module that wishes to
/// assign and track *roles* such as:
///
/// - **Validators** - entities providing consensus participation and staking collateral.
/// - **Council Members / Governance Actors** - accounts participating in decision-making.
/// - **Oracle Operators** - off-chain data feeders bonded with collateral.
/// - **Bounty Curators / Auditors / Relayers** - specialized economic actors.
///
/// By providing a unified API for checking role existence, managing enrollment conditions,
/// and handling collateral and lifecycle events, this trait enables **role composability**
/// and modular runtime design.
///
/// ## Type Parameters
///
/// - `Candidate`: The identifier type representing an account or entity that can assume a role.
///   Typically this is an `AccountId`, but it can also be a multi-entity struct or hash ID.
///
/// ## Invariants
///
/// - Collateral associated with a `Candidate` **must remain locked** while the
/// role is active.
/// - Status transitions must be **atomic and consistent** - i.e.,  
///   once [`Self::set_status`] succeeds, [`Self::get_status`] should immediately
/// reflect the new state.
///
/// ## Example Implementations
///
/// ### Example 1: Validator Role
///
/// A validator role might involve participants who are responsible for block production.
/// In this context:
/// - `Status` could include `Pending`, `Active`, `Slashed`, or `Resigned`.
/// - Candidates are required to provide a minimum collateral to enroll.
/// - The system checks whether a candidate is already enrolled before allowing enrollment.
/// - The trait implementation manages enrollment, resignation, collateral tracking, and
/// status updates.
///
/// ### Example 2: Governance Council Member
///
/// A council member role represents participants in a governance body:
/// - `Status` could include `Candidate`, `Active`, `Expelled`, or `Retired`.
/// - Enrollment may not require collateral but must still enforce eligibility rules.
/// - Resignation or removal is validated against membership in the council.
/// - The trait implementation allows tracking of active members, status changes, and
/// role lifecycle events.
///
/// These examples illustrate how the `RoleManager` trait can be applied to **different
/// kinds of roles**, demonstrating the flexibility of the trait without tying it to a
/// specific runtime storage or pallet.
///
/// ## Usage
///
/// - This trait is **not tied to a specific module/pallet**. It can be implemented by multiple
///   role-managing pallets (e.g. `pallet_validators`, `pallet_council`, etc.).
/// - Generic logic (e.g. in governance or reputation systems) can rely on `RoleManager`
///   to interact with roles abstractly without hardcoding pallet internals.
pub trait RoleManager<Candidate> {
    /// Represents the discrete state (status) of the role (e.g. Active, Pending, Resigned, etc).  
    /// Must implement [`VariantCount`] for easy introspection of status variants.
    type Status: RuntimeEnum + Delimited + VariantCount;

    /// Represents the metadata of the candidate.
    ///
    /// Could include valuable information pertaining to the role-activity
    /// of the candidate.
    type Meta: Delimited;

    /// Represents the collateral or balance type associated with the role.  
    type Asset: Asset;

    /// Represents a timestamp or block number used to track temporal data
    /// like enrollment or status changes.
    type TimeStamp: Time;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether a given `Candidate` currently has a registered role
    /// in the system.
    ///
    /// Returns `Ok(())` if the role exists, otherwise `DispatchError`.
    fn role_exists(who: &Candidate) -> DispatchResult;

    /// Validates whether the `Candidate` is eligible to enroll for a role,
    /// given a specific collateral amount.
    ///
    /// This function only performs validation and **not mutates state**.
    ///
    /// Returns `Ok(())` if the candidate can enroll, or a `DispatchError` if not.
    fn can_enroll(who: &Candidate, collateral: Self::Asset) -> DispatchResult;

    /// Verifies whether the `Candidate` can safely resign their role.
    ///
    /// This typically ensures there are no pending obligations or locked collateral.
    fn can_resign(who: &Candidate) -> DispatchResult;

    /// Returns `Ok(())` if the `Candidate` is considered not *defaulted* i.e., available.
    ///
    /// A **defaulted** state represents a breach of expected behavior such as
    /// under-collateralization, missed performance targets, or protocol violations.
    ///
    /// Depending on implementation, a defaulted candidate may be:
    /// - **Temporarily suspended** - can later recover or be reinstated once obligations are met.  
    /// - **Permanently defaulted** - considered resigned, fired, or barred from reactivation.
    fn is_available(who: &Candidate) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the meta-data of the `Candidate`.
    ///
    /// DispatchError otherwise.
    fn get_meta(who: &Candidate) -> Result<Self::Meta, DispatchError>;

    /// Retrieves the amount of collateral currently locked by the `Candidate`.
    ///
    /// Returns a [`Result`] containing the collateral value or a [`DispatchError`].
    fn get_collateral(who: &Candidate) -> Result<Self::Asset, DispatchError>;

    /// Retrieves the real-time amount of collateral currently locked by all
    /// the `Candidate`s.
    fn total_collateral() -> Self::Asset;

    /// Returns the timestamp (or block number) when the `Candidate` enrolled
    /// in the role.
    fn enroll_since(who: &Candidate) -> Result<Self::TimeStamp, DispatchError>;

    /// Returns the current `Status` of the `Candidate`.
    fn get_status(who: &Candidate) -> Result<Self::Status, DispatchError>;

    /// Returns the timestamp of the last status change for the `Candidate`.
    fn status_since(who: &Candidate) -> Result<Self::TimeStamp, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Updates the role status of the `Candidate` to a new `Status` value.
    fn set_status(who: &Candidate, status: Self::Status) -> DispatchResult;

    /// Enrolls a new `Candidate` with the specified collateral amount.
    ///
    /// Should handle collateral reservation and emit necessary side effects.
    ///
    /// The `force` parameter determines the privilege with which the operation is conducted.
    ///
    /// Returns the actual amount of collateral accepted or a `DispatchError`.
    fn enroll(
        who: &Candidate,
        collateral: Self::Asset,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError>;

    /// Resigns the `Candidate` from their role and releases any associated collateral.
    ///
    /// Should reverse the effects of `enroll`.
    ///
    /// Returns the released collateral amount.
    fn resign(who: &Candidate) -> Result<Self::Asset, DispatchError>;

    /// Increases a `Candidate`'s collateral. Required if the implementation enforces
    /// variable collateral.
    ///
    /// The `force` parameter determines the privilege with which the operation is conducted.
    ///
    /// Returns the actual amount of collateral added or a `DispatchError`.
    fn add_collateral(
        who: &Candidate,
        collateral: Self::Asset,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook triggered after a successful enrollment
    /// along with its collateral.
    ///
    /// Used for post-processing actions such as emitting events or
    /// initializing role data.
    ///
    /// Default is no-op.
    fn on_enroll(_who: &Candidate, _collateral: Self::Asset) {}

    /// Hook triggered after a role resignation along with
    /// regained (unlocked) collateral.
    ///
    /// Typically used to perform cleanup or emit resignation events.
    ///
    /// Default is no-op.
    fn on_resign(_who: &Candidate, _released: Self::Asset) {}

    /// Hook triggered when a `Candidate`'s status is mutated.
    ///
    /// Allows implementing pallets to react to state transitions (e.g. Active -> Inactive).
    ///
    /// Default is no-op.
    fn on_status_update(_who: &Candidate, _status: &Self::Status) {}

    /// Hook triggered when the collateral value of a role is incremented.
    ///
    /// Can be used to adjust dependent metrics or notify external systems.
    ///
    /// Default is no-op.
    fn on_add_collateral(_who: &Candidate, _raised: Self::Asset) {}
}

// ===============================================================================
// ````````````````````````````````` FUND ROLES ``````````````````````````````````
// ===============================================================================

/// Extends [`RoleManager`] to introduce **funding and backing mechanics**
/// for role-based systems.
///
/// This trait defines a common interface for *roles that require financial
/// support or collateralization from external entities* (called **backers**).
/// It models relationships  where one or more backers provide funds to a candidate
/// in exchange for shared exposure, yield, or delegated influence.
///
/// Typical applications include:
///
/// - **Validator nomination systems** - nominators/delegators (backers) stake
/// funds behind validators (candidates).  
/// - **DAO or governance roles** - community members back representatives or
/// proposal leaders.  
/// - **DeFi-style credit systems** - where backers fund loan candidates, or
/// insurance underwriters provide coverage.  
/// - **Service networks** - e.g., oracle or relayer pools where reputation and
/// collateral are pooled.
///
/// ## Type Parameters
/// - `Candidate`: The entity or account being backed.
///
/// ## Invariants
///
/// - Each backer must fund with at least [`Self::min_fund`] units of asset.  
/// - When a `Candidate` is **not available** (See [`RoleManager::is_available`]),
/// backers may withdraw all or some of their stake (context-basis).  
/// - [`Self::fund`] and [`Self::draw`] operations must maintain **consistent
/// accounting symmetry** between [`Self::backers_of`] and [`Self::backed_for`].  
///
/// ## Example Implementations
///
/// ### Example 1: Nominated Validator System
///
/// In a nominated staking system:
/// - Candidates are validators who receive backing from multiple nominators (backers).
/// - Each backer must meet a minimum funding requirement when supporting a candidate.
/// - The total funding for a candidate cannot exceed a maximum exposure limit.
/// - Funding increases the candidate's active collateral, while drawing allows backers
/// to withdraw.
/// - This example shows how the trait enforces eligibility, updates backing relationships,
///   and maintains real-time funded values.
///
/// ### Example 2: Governance Role Backing
///
/// In a governance system where community members can financially support representatives:
/// - Candidates are council members or proposal leads.
/// - Backers provide funds to indicate confidence and to incentivize participation.
/// - The trait ensures backers cannot overexpose themselves or fund below minimum thresholds.
/// - Candidates may become defaulted if they violate rules or fail to perform, triggering
/// potential fund loss.
/// - Rewards, penalties, or collateral adjustments can be layered on top, creating a
/// dynamic funding ecosystem.
///
/// These examples illustrate how `FundRoles` can handle **different backing scenarios**,
/// from staking and validator support to governance funding, while maintaining **real-time
/// tracking** of funds and enforcing role-related invariants.
///
/// ## Usage
///
/// Implement this trait for any role that allows external capital participation or support.
/// Higher-level logic such as slashing, reward distribution, or governance power weighting
/// can be layered on top by combining [`CompensateRoles`] or custom traits.
pub trait FundRoles<Candidate>: RoleManager<Candidate> {
    /// Type representing the entity providing funding or backing support.
    type Backer: Delimited;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether the `Candidate` currently holds any funds or active backing relationships.
    ///
    /// Returns `Ok(())` if funds exist, otherwise a `DispatchError`.
    fn has_funds(who: &Candidate) -> DispatchResult;

    /// Validates whether a `Backer` can fund a `Candidate` with the specified amount.
    ///
    /// This performs **only validation**, not mutation.
    ///
    /// The `precision` parameter defines proportional allocation or best behavior.
    /// The `force` parameter defines enforcement principle of this funding operation.
    ///
    /// Should verify minimum and maximum exposure constraints possibly
    /// via [`Self::min_fund`] and [`Self::max_exposure`].
    fn can_fund(
        by: &Self::Backer,
        to: &Candidate,
        value: Self::Asset,
        precision: Precision,
        force: Fortitude,
    ) -> DispatchResult;

    /// Validates whether a `Backer` can draw its funds backed to a `Candidate`.
    ///
    /// This performs **only validation**, not mutation.
    fn can_draw(by: &Self::Backer, from: &Candidate) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Returns the **maximum exposure** (total backable amount) allowed for this `Candidate`.
    ///
    /// This acts as an upper cap on all active fundings combined.
    ///
    /// The `precision` and `force` parameters simulate a funding attempt under
    /// the given directive, determining the effective exposure limits.
    fn max_exposure(
        from: &Self::Backer,
        towards: &Candidate,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError>;

    /// Returns the **minimum funding amount** required for a `Backer` to participate.
    ///
    /// The `precision` and `force` parameters simulate a funding attempt under
    /// the given directive, determining the effective minimum requirement.
    fn min_fund(
        from: &Self::Backer,
        towards: &Candidate,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError>;

    /// Returns the real-time total **value of funds currently backing** the given `Candidate`.
    ///
    /// This reflects the **latest on-chain state**, meaning the value may vary over time
    /// due to dynamic adjustments such as:
    /// - Collateral revaluation or slashing
    /// - Partial withdrawals or new backings
    /// - Protocol-driven rewards or penalties
    ///
    /// Implementations should compute or retrieve this value *at the moment of call*,
    /// ensuring it reflects the candidate's **live backing exposure**.
    fn backed_value(who: &Candidate) -> Result<Self::Asset, DispatchError>;

    /// Returns all `Backer`s currently funding the specified `Candidate`,
    /// along with each backer's **real-time effective contribution**.
    ///
    /// Note that returned contribution values are **not static** - they represent the
    /// candidate's current funding snapshot and may change due to slashing,
    /// rebalancing, or collateral mutation.
    fn backers_of(who: &Candidate) -> Result<Vec<(Self::Backer, Self::Asset)>, DispatchError>;

    /// Returns all `Candidate`s that the given `Backer` is funding,
    /// along with each **real-time funded amount**.
    ///
    /// The returned data represents the backer's **current live exposure**, not a
    /// historical record. Implementations should resolve these dynamically from
    /// the latest runtime state.
    fn backed_for(by: &Self::Backer) -> Result<Vec<(Candidate, Self::Asset)>, DispatchError>;

    /// Retrieves the real-time amount of funds currently funded for all the `Candidate`s.
    fn total_backing() -> Self::Asset;

    /// Retrieves the **current effective funded amount** that `by` has provided to `who`.
    ///
    /// This is a *real-time query* that reflects the live value of the fund relationship,
    /// factoring in any protocol-driven adjustments (e.g., penalties, rewards,
    /// partial draws, or slashes).
    fn get_fund(who: &Candidate, by: &Self::Backer) -> Result<Self::Asset, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Performs the **funding operation**, locking the given value of asset
    /// from the `Backer` in support of the `Candidate`.
    ///
    /// The `precision` parameter defines proportional allocation or best behavior.
    /// The `force` parameter defines enforcement principle of this funding operation.
    ///
    /// Returns the actual funded amount.
    fn fund(
        to: &Candidate,
        by: &Self::Backer,
        value: Self::Asset,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Asset, DispatchError>;

    /// Allows the `Backer` to **draw back** (withdraw) their previously funded assets
    /// from a `Candidate`.
    ///
    /// Returns the amount successfully withdrawn.
    fn draw(from: &Candidate, by: &Self::Backer) -> Result<Self::Asset, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook triggered when a funding action is completed.
    ///
    /// Can be used to emit events, update metrics, or notify off-chain logic.
    ///
    /// Default is no-op.
    fn on_funded(_who: &Candidate, _by: &Self::Backer, _value: Self::Asset) {}

    /// Hook triggered when funds are drawn or withdrawn.
    ///
    /// Use this for cleanup or balance reallocation tasks.
    ///
    /// Default is no-op.
    fn on_drawn(_who: &Candidate, _by: &Self::Backer, _value: Self::Asset) {}
}

// ===============================================================================
// `````````````````````````````` COMPENSATE ROLES ```````````````````````````````
// ===============================================================================

/// Extends [`RoleManager`] to introduce **reward and penalty mechanics**
/// for role-based systems.
///
/// This trait manages economic incentives and behavioral enforcement for candidates,
/// tracking rewards, penalties, and temporary holdings. Typical applications include
/// (but not constrained to):
///
/// - **Validators** - rewards for block production, penalties for missed duties.
/// - **Council members** - incentives for active participation, slashing for misconduct.
/// - **Oracle operators** - rewards for accurate reporting, penalties for stale or
/// incorrect data.
///
/// ## Concepts
///
/// - **Hold**: The total reservation of a candidate's assets, includes rewards and other
/// backings (collaterals, fundings, etc).
/// - **Reward**: An addition to a candidate's assets for performing expected duties.
/// - **Penalty**: A fractional deduction (ratio) applied to assets for misbehavior or
/// non-performance.
/// - **Forgive**: Reverses penalties, partially or fully, for rehabilitation or
/// governance action.
/// - **Reclaim**: Withdraw previously rewarded assets for redistribution or correction.
///
/// All returned values (assets, rewards, penalties) are **real-time**, reflecting the
/// candidate's current effective state.
///
/// ## Example Implementations
///
/// ### Example 1: Validator Rewards and Penalties
///
/// In a blockchain validator system:
/// - Validators earn rewards for producing blocks, finalizing chains, or performing
/// other expected duties.
/// - Misbehavior, missed blocks, or downtime results in penalties, which reduce the
/// validator's hold.
/// - Rewards and penalties are recorded with timestamps, allowing auditability and
/// historical tracking.
/// - The system may forgive temporary penalties or reclaim rewards in case of protocol
/// corrections.
/// - Real-time queries (get_rewards_of, get_penalties_of) reflect the current effective
/// state of the validator.
///
/// ### Example 2: Council Member Performance Incentives
///
/// In a governance council:
/// - Council members receive rewards for attending sessions, voting, or completing
/// assigned tasks.
/// - Failure to participate or violating rules triggers penalties, possibly reducing
/// influence or rewards.
/// - Holds represent assets reserved for potential penalties or pending rewards.
/// - Forgiveness mechanisms allow partial reversal of penalties for rehabilitation or
/// governance decisions.
/// - Reclaiming rewards may occur if a proposal is invalidated or performance is reversed.
/// - Real-time computations ensure all incentives and penalties are up-to-date when queried.
///
/// These examples illustrate how `CompensateRoles` can manage **dynamic reward and penalty
/// systems** across different roles, ensuring fairness, accountability, and flexibility in
/// role-based operations.
///
/// ## Invariants
///
/// - Total penalties cannot exceed the candidate's current hold or collateral.
/// - Implementations should track timestamps accurately to maintain auditability.
pub trait CompensateRoles<Candidate>: RoleManager<Candidate> {
    /// The ratio type used to represent **penalty factors** applied to a candidate.
    ///
    /// This type defines how much of a candidate's assets should be deducted
    /// during a penalty event, expressed as a fractional percentage.
    ///
    /// - Values should be within `[0, 1]` (0% to 100%)
    /// - Example: `0.05` -> 5% penalty, `0.25` -> 25% penalty
    type Ratio: Percentage;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether the candidate has any pending rewards.
    fn has_reward(who: &Candidate) -> DispatchResult;

    /// Checks whether the candidate has any pending penalties.
    fn has_penalty(who: &Candidate) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Returns the current **hold amount** of a candidate.
    ///
    /// Holds are total reserved assets (may include rewards) that may be used
    /// for penalties or temporary constraints.
    fn get_hold(who: &Candidate) -> Result<Self::Asset, DispatchError>;

    /// Returns all pending rewards for a candidate along with their timestamp of enforcement.
    ///
    /// The list reflects **real-time pending rewards**, not historical logs.
    fn get_rewards_of(
        who: &Candidate,
    ) -> Result<Vec<(Self::TimeStamp, Self::Asset)>, DispatchError>;

    /// Returns all rewards of candidates issued at a specific timestamp.
    ///
    /// - This is a **snapshot query** for the given timestamp; it does **not mutate state**.
    /// - Returned values may reflect the **rewards actually enforced or pending** based on
    ///   the given timestamp, which may be used for auditing, reporting, or batch processing.
    fn get_rewards_on(
        time_stamp: Self::TimeStamp,
    ) -> Result<Vec<(Candidate, Self::Asset)>, DispatchError>;

    /// Returns all pending penalties for a candidate along with their timestamp of enforcement.
    ///
    /// The list reflects **real-time pending penalties**, not historical logs.
    fn get_penalties_of(
        who: &Candidate,
    ) -> Result<Vec<(Self::TimeStamp, Self::Ratio)>, DispatchError>;

    /// Returns all penalties of candidates issued at a specific timestamp.
    ///
    /// - This is a **snapshot query** for the given timestamp; it does **not mutate state**.
    /// - Returned values may reflect the **penalties actually enforced or pending** based on
    ///   the given timestamp, which may be used for auditing, reporting, or batch processing.
    fn get_penalties_on(
        time_stamp: Self::TimeStamp,
    ) -> Result<Vec<(Candidate, Self::Ratio)>, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Updates the hold of a candidate to the specified amount.
    ///
    /// This method is utilized to reward or penalize a `Candidate`'s hold which
    /// reflects on its individual backings.
    fn set_hold(
        who: &Candidate,
        value: Self::Asset,
        precision: Precision,
        force: Fortitude,
    ) -> DispatchResult;

    /// Issues a reward to a candidate, marks it as pending and returning the timestamp.
    ///
    /// Once the reward is enforced during the timestamp, it is applied to the
    /// `Candidate`'s hold.
    ///
    /// This is to ensure rewards reversal (regaining) if applied wrongly.
    fn reward(
        who: &Candidate,
        value: Self::Asset,
        precision: Precision,
    ) -> Result<Self::TimeStamp, DispatchError>;

    /// Applies a penalty (fractional [`Ratio`](Self::Ratio)) to a candidate, marks it as pending
    /// and returning the timestamp.
    ///
    /// Once the penalty is enforced during the timestamp, it is applied to the
    /// `Candidate`'s hold.
    ///
    /// This is to ensure penalty reversal (forgiving) if applied wrongly.
    fn penalize(who: &Candidate, factor: Self::Ratio) -> Result<Self::TimeStamp, DispatchError>;

    /// Forgives a pending penalty, returning its factor.
    ///
    /// Cannot be utilized for enforced penalties, only pending ones.
    ///
    /// `from` specifies the timestamp of the penalty to forgive.
    fn forgive(who: &Candidate, from: Self::TimeStamp) -> Result<Self::Ratio, DispatchError>;

    /// Reclaims a pending reward from a candidate, returning its reward value.
    ///
    /// Cannot be utilized for enforced rewards, only pending ones.
    ///
    /// `from` specifies the timestamp of the reward to reclaim.
    fn reclaim(who: &Candidate, from: Self::TimeStamp) -> Result<Self::Asset, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook triggered when a reward is issued.
    ///
    /// Default is no-op.
    fn on_reward(_who: &Candidate, _amount: Self::Asset, _at: Self::TimeStamp) {}

    /// Hook triggered when a penalty is applied.
    ///
    /// Default is no-op.
    fn on_penalize(_who: &Candidate, _factor: Self::Ratio, _at: Self::TimeStamp) {}

    /// Hook triggered when a penalty is forgiven.
    ///
    /// Default is no-op.
    fn on_forgive(_who: &Candidate, _factor: Self::Ratio) {}

    /// Hook triggered when a reward is reclaimed.
    ///
    /// Default is no-op.
    fn on_reclaim(_who: &Candidate, _amount: Self::Asset) {}

    /// Hook triggered when an author's total hold is updated.
    ///
    /// Signals a mutation maybe due to rewards/penalties or internal
    /// changes enforced (finalized)
    ///
    /// Default is no-op.
    fn on_set_hold(_who: &Candidate, _value: Self::Asset) {}
}

// ===============================================================================
// ``````````````````````````````` ROLE PROBATION ````````````````````````````````
// ===============================================================================

/// Extends [`RoleManager`] to introduce **probation and permanent status mechanics**
/// for role-based systems.
///
/// This trait manages the probation lifecycle of candidates, tracking their risk,
/// confirmation, and eligibility for permanent status. Typical applications include:
/// - **Employees** - probation periods before permanent employment.
/// - **Validators or council members** - temporary risk periods before confirmed full status.
/// - **Accounts or participants** - temporary monitoring before achieving full privileges.
///
/// ## Concepts
///
/// - **Probation / Risk**: Candidate is under evaluation; actions or failures
/// may have consequences.
/// - **Permanent / Confirmation**: Candidate has successfully passed evaluation;
/// fully confirmed.
/// - **Secure**: Temporarily avoids risk without confirming permanent status.
///
/// ## Probation Flow
/// - A new enrolled candidate starts in **probation** status.
/// - Upon negative performance, they risk losing permanence.
/// - Upon meeting required criteria, they are **confirmed permanent**.
/// - If a permanent candidate violates policies, permanent status may be revoked.
///
/// ## Invariants
/// - Candidates should not simultaneously be in probation and permanent status.
pub trait RoleProbation<Candidate>: RoleManager<Candidate> {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks if the candidate is currently under probation.
    ///
    /// Returns `DispatchError` if the query fails.
    fn is_on_probation(who: &Candidate) -> DispatchResult;

    /// Checks if the candidate has secured permanent / confirmed status.
    ///
    /// Returns `DispatchError` if the query fails.
    fn is_permanent(who: &Candidate) -> DispatchResult;

    /// Checks if the candidate is eligible to become permanent.
    ///
    /// Returns `DispatchError` if the candidate is not eligible.
    fn can_be_permanent(who: &Candidate) -> DispatchResult;

    /// Checks if the candidate's permanent status can be revoked,
    /// returning them to probation.
    ///
    /// Returns `DispatchError` if the candidate is not eligible.
    fn can_revoke_permanence(who: &Candidate) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Places the candidate's probation at risk (risking permanence).
    ///
    /// Returns `DispatchError` if the candidate is not in probation.
    fn risk_probation(who: &Candidate) -> DispatchResult;

    /// Places the candidate's permanence at risk (risking probation).
    ///
    /// Returns `DispatchError` if the candidate is not in permanence.
    fn risk_permanence(who: &Candidate) -> DispatchResult;

    /// Marks the candidate as positively progressing toward permanent status.
    ///
    /// This indicates that the candidate has demonstrated sufficient performance
    /// or compliance during probation, making them eligible to become permanent
    /// once probation concludes.
    ///
    /// Returns `DispatchError` if the candidate is not in probation.
    fn secure_permanence(who: &Candidate) -> DispatchResult;

    /// Marks the candidate as secured / confirmed / permanent.
    ///
    /// Returns the new status on success.
    /// Returns `DispatchError` if the operation fails.
    fn set_permanence(who: &Candidate) -> Result<Self::Status, DispatchError>;

    /// Revokes permanent status and places the candidate back under probation.
    ///
    /// Returns the new status on success.
    /// Returns `DispatchError` if the operation fails.
    fn revoke_permanence(who: &Candidate) -> Result<Self::Status, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook triggered after the candidate under probation
    /// risks their permanence.
    ///
    /// Default implementation is a no-op.
    fn on_risk_probation(_who: &Candidate) {}

    /// Hook triggered after the permanent candidate
    /// risks their permanence.
    ///
    /// Default implementation is a no-op.
    fn on_risk_permanence(_who: &Candidate) {}

    /// Hook triggered after a candidate secures being
    /// permanently promoted soon without negative impacts.
    ///
    /// Default implementation is a no-op.
    fn on_secure_permanence(_who: &Candidate) {}

    /// Hook triggered after a candidate gets permanent status.
    ///
    /// Default implementation is a no-op.
    fn on_set_permance(_who: &Candidate) {}

    /// Hook triggered after a candidate's permanence is
    /// revoked, placing them back under probation.
    ///
    /// Default implementation is a no-op.
    fn on_revoke_permanence(_who: &Candidate) {}
}

// ===============================================================================
// ```````````````````````````````` ROLE ACTIVITY ````````````````````````````````
// ===============================================================================

/// A lightweight abstraction for determining whether a role-bearing `Candidate` is
/// currently **idle** or **actively performing duties**.
///
/// This trait models **real-time operational activity**, independent of role
/// lifecycle, status, or eligibility. It is intended to compose with role
/// management, funding, and compensation logic.
///
/// ## Semantics
///
/// The core API uses an **inverted pattern which allows activity to be represented
/// with structured context rather than a lossy boolean.
///
/// The associated `Activity` type must represent **non-fatal engagement**
/// and be convertible into [`DispatchError`].
pub trait RoleActivity<Candidate, TimeStamp> {
    /// Describes the duty currently being performed by the candidate.
    ///
    /// Returned when the candidate is active. When converted into
    /// [`DispatchError`], it must clearly indicate **why the operation is blocked**
    /// and **what must occur to withdraw from or complete the ongoing activity**.
    ///
    /// This type represents non-fatal operational engagement only.
    type Activity: RuntimeEnum + Delimited + Into<DispatchError>;

    /// Returns `Ok(())` if the candidate is currently **idle** (not performing duties).
    ///
    /// Returns `Err(Activity)` if the candidate is **active**, where `Activity`
    /// describes the duty being performed.
    ///
    /// This method must not mutate state and should reflect real-time activity.
    fn is_idle(who: &Candidate) -> Result<(), Self::Activity>;

    /// Returns `Ok(Activity)` if the candidate is **active**, where `Activity`
    /// describes the duty being performed.
    ///
    /// Returns `Err(())` if the candidate is **idle**.
    fn is_active(who: &Candidate) -> Result<Self::Activity, ()> {
        match Self::is_idle(who) {
            Ok(_) => Err(()),
            Err(a) => Ok(a),
        }
    }
}
