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
// ````````````````````````````````` ASSETS SUITE ````````````````````````````````
// ===============================================================================

//! A collection of composable, type-driven balance models built on
//! [`virtual`](crate::virtuals) abstractions and
//! [`plugin`](crate::plugins)-based execution.
//!
//! ## Motivation
//!
//! Traditional balance systems tightly couple:
//! - storage layout
//! - accounting logic
//! - and update propagation
//!
//! This leads to:
//! - costly updates across all dependent states
//! - rigid data structures that are hard to evolve
//! - difficulty composing new behaviors or extending models
//!
//! This module addresses these limitations by:
//!
//! - **decoupling structure, storage, and behavior**
//! - **deferring computation until it is actually needed**
//! - **allowing logic to be injected via plugins**
//!
//! ## Lazy Balance Model
//!
//! The primary model is [`LazyBalance`], a **receipt-based,
//! lazily evaluated accounting system**.
//!
//! ```text
//! deposit -> issue receipt
//! mutate balance -> affect global state only
//! withdraw -> resolve receipt value lazily
//! ```
//!
//! In this model:
//!
//! - deposits create **receipts (claims)**
//! - balance mutations affect **global state only**
//! - receipt value is computed **at withdrawal time**
//!
//! This avoids eagerly updating all receipts while preserving correctness.
//!
//! ## Design
//!
//! ### Virtual Architecture
//!
//! All balance models are built on shared virtual primitives:
//!
//! - [`VirtualDynField`] -> defines structured components (asset, rational, time)
//! - [`VirtualDynExtension`] -> enables extensibility via addons
//! - [`VirtualDynBound`] -> provides external constraints (e.g. capacity)
//! - [`VirtualNMap`] -> externalizes heavy or dynamic storage
//!
//! These abstractions allow:
//!
//! - structure to remain **representation-agnostic**
//! - storage to be **externalized or optimized independently**
//! - components to be **composed across contexts**
//!
//! ### Plugin-Driven Execution
//!
//! Behavior is defined through plugin families via:
//!
//! - [`declare_family`]
//! - [`plugin_types`]
//! - [`plugin_output`]
//!
//! This enables:
//!
//! - **compile-time dispatch of operations**
//! - **modular and replaceable logic**
//! - **context-driven customization**
//!
//! Each operation (deposit, withdraw, etc.) is selected via a discriminant
//! and resolved through the plugin family.
//!
//! ## Summary
//!
//! This module provides a foundation for building diverse balance systems where:
//!
//! - accounting is **lazy and efficient**
//! - structure is **flexible and composable**
//! - behavior is **modular and replaceable**
//!
//! enabling multiple accounting strategies to coexist under a unified,
//! type-driven architecture.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    base::{Asset, Delimited, Fractional, RuntimeEnum, Time},
    declare_family, discriminants, impl_discriminants,
    misc::{Directive, Extent},
    mutation::MutHandle,
    plugin_output, plugin_types,
    plugins::ModelContext,
    virtuals::*,
};

// --- Substrate primitives ---
use sp_runtime::Cow;

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Resolved context type for a given [`LazyBalance`] implementation.
///
/// Extracts the concrete context provided by `BalanceContext`.
pub type Context<T> = <<T as LazyBalance>::BalanceContext as ModelContext>::Context;

/// Resolved error type for a given [`LazyBalance`] implementation.
///
/// Provided by the context via [`VirtualError`].
pub type Error<T> = <Context<T> as VirtualError<LazyBalanceError>>::Error;

// ===============================================================================
// ````````````````````````````````` LAZY BALANCE ````````````````````````````````
// ===============================================================================

/// A composable, plugin-driven balance system with **lazy
/// receipt-based accounting**.
///
/// `LazyBalance` defines a generic balance abstraction where value is managed
/// through deposits, receipts, and deferred redemption semantics.
///
/// The system integrates with the plugin framework to express all behavior
/// (validation, mutation, and queries) as pluggable operations resolved at
/// compile time.
///
/// ## Core Semantics
///
/// The defining property of `LazyBalance` is its *lazy propagation model*:
///
/// - Balance updates (e.g. `mint`, `reap`) are applied **immediately** to the
///   underlying balance state.
/// - Issued receipts are **not eagerly updated** when such changes occur.
/// - Instead, the effect of these updates is **deferred and applied at the time
///   of receipt redemption (`withdraw`)**.
///
/// In other words:
///
/// ```text
/// Deposits -> Issue Receipts
/// Balance Mutations -> Affect Global State Only
/// Withdrawals -> Resolve Final Value Lazily
/// ```
///
/// This allows the system to:
/// - avoid costly updates across all outstanding receipts
/// - maintain correctness through deferred evaluation
/// - support dynamic balance adjustments without recomputation overhead
///
/// ## Conceptual Model
///
/// The system is composed of three primary components:
///
/// - **Balance**
///   - Canonical mutable state
///   - Receives all direct updates (`deposit`, `mint`, `reap`, `drain`)
///
/// - **Receipt**
///   - Represents a claim issued at deposit time
///   - Encodes entitlement, not fixed value
///   - Final value is determined lazily at withdrawal
///
/// - **SnapShot**
///   - Time-indexed projection of balance state
///   - Enables historical queries and deferred computations
///
/// This forms a *claim-based accounting model*:
/// - deposits create claims
/// - withdrawals resolve claims
/// - intermediate balance changes influence claim outcomes
///
/// ## Storage Model
///
/// Snapshot data is externalized via [`VirtualNMap`] using
/// `SnapShotStorage`, allowing:
///
/// - efficient storage of time-indexed data
/// - separation of heavy state from encoded balance representation
/// - iteration and prefix queries over historical state
///
/// ## Plugin-Driven Behavior
///
/// All operations are expressed through a plugin family [`declare_family`]:
///
/// Each operation:
/// - is type-driven via [`Self::Input`] and [`Self::Output`]
/// - is resolved through the [`Self::BalanceFamily`] plugin
/// - may be customized by runtime configuration
///
/// ## Limits
///
/// Operations may expose dynamic limits (e.g. minimum, maximum, optimal)
/// via dedicated limit queries. These limits are derived from current
/// balance state and provide bounded inputs for safe execution.
///
/// ## Execution DispatchPolicy
///
/// Operations may be parameterized by a [`Self::Subject`], which encodes
/// qualitative behavior such as precision and fortitude,
/// influencing how operations are evaluated and executed.
///
/// ## Design Properties
///
/// - **Lazy evaluation**: receipt value is computed at redemption time
/// - **Efficient**: avoids eager updates across all outstanding claims
/// - **Composable**: behavior is modular via plugin families
/// - **Decoupled**: storage, logic, and types evolve independently
/// - **Type-safe**: all invariants enforced at compile time
///
pub trait LazyBalance:
    VirtualNMap<
    Self::Balance,
    SnapShotStorage,
    Key = (Self::Id, Self::Variant, Self::Time),
    Value = Self::SnapShot,
    Query = Option<Self::SnapShot>,
>
where
    Self: Sized,
{
    /// The underlying unit of value tracked by the system.
    ///
    /// Represents tokens, points, or any fungible asset.
    type Asset: Asset;

    /// The numeric representation used for value calculations.
    ///
    /// Typically a fixed-point or fractional type ensuring precise arithmetic.
    type Rational: Fractional;

    /// The temporal index of the system.
    ///
    /// A monotonically increasing counter (e.g. block number or timestamp)
    /// used for snapshotting and time-based computations.
    type Time: Time;

    /// Logical partition of a balance.
    ///
    /// Allows multiple independent balance states per `Id`
    /// (e.g. free, locked, reserved).
    type Variant: Delimited + RuntimeEnum + Default;

    /// Identifier of a balance owner or entity.
    ///
    /// Combined with `Variant` and `Time` to uniquely address balance state.
    type Id: Delimited;

    /// Represents the directive / intent of an operation.
    ///
    /// Encodes behavioral characteristics such as precision and fortitude,
    /// influencing how operations are evaluated and executed.
    ///
    /// Passed alongside operation inputs to allow context-aware behavior
    /// (e.g. exact vs approximate, polite vs forceful execution).
    type Subject: Delimited + Directive + Default;

    /// Primary mutable balance state.
    ///
    /// Represents the canonical stored value and is modified by all
    /// state-transition operations.
    type Balance: LazyBalanceComponent<
        Self,
        BalanceAsset,
        BalanceRational,
        BalanceTime,
        Context<Self>,
        BalanceAddon,
    >;

    /// Time-indexed projection of balance state.
    ///
    /// Used for historical queries and deferred computations.
    type SnapShot: LazyBalanceComponent<
        Self,
        SnapShotAsset,
        SnapShotRational,
        SnapShotTime,
        Context<Self>,
        SnapShotAddon,
    >;

    /// Claim over deposited value.
    ///
    /// Issued on deposit and consumed on withdrawal.
    ///
    /// A receipt represents the right to redeem value from the balance.
    /// Its redeemable value may differ from its original deposit value
    /// due to balance adjustments (e.g. minting or reaping).
    type Receipt: LazyBalanceComponent<
        Self,
        ReceiptAsset,
        ReceiptRational,
        ReceiptTime,
        Context<Self>,
        ReceiptAddon,
    >;

    /// Abstract representation of operation limits.
    ///
    /// Encapsulates derived values across different extents
    /// (e.g. minimum, maximum, optimal) for balance operations.
    ///
    /// Used by limit query operations to provide safe bounded values
    /// under current conditions.
    type Limits: LazyBalanceLimits<Self>;

    /// Input carrier for all operations.
    ///
    /// Encodes operation-specific arguments while supporting both
    /// mutable and immutable access patterns.
    type Input<'a>: LazyBalanceInput<
        'a,
        Self::Balance,
        Self::Variant,
        Self::Id,
        Self::Asset,
        Self::Receipt,
        Self,
    >;

    /// Output carrier for all operations.
    ///
    /// Encodes operation-specific results and error handling.
    type Output<'a>: LazyBalanceOutput<
        'a,
        Self::Asset,
        Self::Receipt,
        Self::SnapShot,
        Self::Time,
        Self::Limits,
        Self,
    >;

    // Plugin Family defining all balance behavior.
    //
    // The family groups all operations (validation, mutation, queries)
    // under a single type-level interface.
    //
    // Resolution flow:
    // - `root` defines the set of operation discriminants
    // - `family` binds those discriminants to concrete implementations
    // - `context` provides environment, constraints, and dependencies
    //
    // Together, they form a compile-time dispatch system where
    // behavior is selected by discriminant and resolved through context.
    plugin_types! {
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        /// [`Plugin Family`](crate::plugins) implementing all [`LazyBalance`] operations.
        ///
        /// Binds each operation discriminant (defined in [`LazyBalanceRoot`])
        /// to a concrete plugin model, determining how validation, mutation,
        /// and query logic is executed.
        ///
        /// Combined with the resolved context, this enables compile-time
        /// dispatch of behavior while keeping logic modular and replaceable.
        family: BalanceFamily,

        /// Context provider for all [`LazyBalance`] operations via [`Self::BalanceFamily`].
        ///
        /// Implements [`ModelContext`], producing a concrete execution
        /// context whose associated `Context` type must satisfy
        /// [`LazyBalanceContext`].
        ///
        /// Since [`LazyBalance`] components (e.g. Balance, SnapShot, Receipt)
        /// are modeled as *[`virtual`](crate::virtuals) structures*
        /// (see [`VirtualDynField`]) and support extensibility
        /// (see [`VirtualDynExtension`]), they do not
        /// define their bounds, schemas, or error types internally.
        ///
        /// Instead, these are supplied by the resolved context, which is
        /// required (via [`LazyBalanceContext`]) to provide:
        ///
        /// - dynamic bounds for all core dimensions through [`VirtualDynBound`]
        ///   (asset, rational, and time for balance, snapshot, and receipt)
        /// - extension schemas via [`VirtualDynExtensionSchema`] for addon
        ///   composition across components
        /// - a unified error type via [`VirtualError`] for all operations
        ///
        /// This enforces that all [`plugins`](crate::plugins) in this
        /// family execute within a fully specified environment where structure,
        /// constraints, and extensibility are externally defined yet statically guaranteed.
        context: BalanceContext,
        provides: [LazyBalanceContext],
    }

    // ---------- Capability checks ------------

    plugin_output! {
        /// Returns whether a deposit is permitted.
        fn can_deposit,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: CanDeposit,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns whether a withdrawal is permitted.
        fn can_withdraw,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: CanWithdraw,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns whether minting is permitted.
        fn can_mint,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: CanMint,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns whether reaping is permitted.
        fn can_reap,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: CanReap,
        context: Self::BalanceContext
    }

    // ---------- State transitions -------------

    plugin_output! {
        /// Deposits value into the balance and issues a receipt
        /// along with depositted amount.
        fn deposit,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: Deposit,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Withdraws value by consuming a receipt, returning withdrawed amount.
        fn withdraw,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: Withdraw,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Introduces new value into the balance, returning minted amount.
        fn mint,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: Mint,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Removes or adjusts value from the balance, returning reaped amount.
        fn reap,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: Reap,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Clears the balance state, returning drained amount.
        fn drain,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: Drain,
        context: Self::BalanceContext
    }

    // ---------- Queries -------------

    plugin_output! {
        /// Returns the total value of the balance.
        fn total_value,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: TotalValue,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns the current redeemable value of a receipt.
        fn receipt_active_value,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: ReceiptActiveValue,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns whether the balance has any deposits.
        fn has_deposits,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: HasDeposits,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns the original deposited value represented by a receipt.
        fn receipt_deposit_value,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: ReceiptDepositValue,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns limits for deposit operations if any, else permissive.
        fn deposit_limits,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: DepositLimits,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns limits for mint operations if any, else permissive.
        fn mint_limits,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: MintLimits,
        context: Self::BalanceContext
    }

    plugin_output! {
        /// Returns limits for reap operations if any, else permissive.
        fn reap_limits,
        input: Self::Input<'a>,
        output: Self::Output<'a>,
        borrow: ['a],
        root: LazyBalanceRoot,
        family: Self::BalanceFamily<'a>,
        child: ReapLimits,
        context: Self::BalanceContext
    }
}

// ===============================================================================
// ```````````````````````````````` PLUGIN FAMILY ````````````````````````````````
// ===============================================================================

declare_family!(
    /// [`Plugin Family`](crate::plugins) Root trait for [`LazyBalance`]
    ///
    /// Each child (associated-type) acts as a **discriminant** selecting
    /// a specific operation within the lazy balance model.
    root: pub LazyBalanceRoot,
    child: [
        // ----- Capability checks -----

        /// Discriminant for deposit validation.
        ///
        /// Determines whether a deposit operation is permitted.
        CanDeposit,

        /// Discriminant for withdrawal validation.
        ///
        /// Determines whether a receipt can be redeemed.
        CanWithdraw,

        /// Discriminant for reap validation.
        ///
        /// Determines whether value can be removed or adjusted.
        CanReap,

        /// Discriminant for mint validation.
        ///
        /// Determines whether new value can be introduced.
        CanMint,


        // ----- Mutations -----

        /// Discriminant for deposit execution.
        ///
        /// Deposits value and issues a receipt.
        Deposit,

        /// Discriminant for withdrawal execution.
        ///
        /// Redeems a receipt, resolving value lazily at withdrawal time.
        Withdraw,

        /// Discriminant for reap execution.
        ///
        /// Removes or adjusts balance value without mutating existing receipts.
        Reap,

        /// Discriminant for drain execution.
        ///
        /// Clears or resets the entire balance state.
        Drain,

        /// Discriminant for mint execution.
        ///
        /// Introduces new value affecting future receipt redemption.
        Mint,


        // ----- Queries -----

        /// Discriminant for total balance query.
        ///
        /// Returns the total value held in the balance.
        TotalValue,

        /// Discriminant for active receipt value query.
        ///
        /// Returns the current redeemable value, reflecting lazy adjustments.
        ReceiptActiveValue,

        /// Discriminant for receipt deposit value query.
        ///
        /// Returns the original deposited value, ignoring later adjustments.
        ReceiptDepositValue,

        /// Discriminant for deposit presence query.
        ///
        /// Indicates whether any deposits exist in the balance.
        HasDeposits,

        // ----- Limits -----

        /// Discriminant for deposit limits query.
        ///
        /// Provides limits such as the minimum and maximum deposit
        /// allowed under given conditions.
        DepositLimits,

        /// Discriminant for mint limits query.
        ///
        /// Provides limits such as the minimum and maximum mint
        /// allowed under given conditions.
        MintLimits,

        /// Discriminant for reap limits query.
        ///
        /// Provides limits such as the minimum and maximum reap
        /// allowed under given conditions.
        ReapLimits,
    ]
);

// ===============================================================================
// ````````````````````` PLUGIN FAMILY's CHILD DISCRIMINANTS `````````````````````
// ===============================================================================

impl_discriminants! {
    CanDeposit,
    CanWithdraw,
    CanReap,
    CanMint,
    Deposit,
    Withdraw,
    Reap,
    Drain,
    Mint,
    TotalValue,
    ReceiptActiveValue,
    ReceiptDepositValue,
    HasDeposits,
    DepositLimits,
    ReapLimits,
    MintLimits,
}

// ===============================================================================
// ```````````````````````````````` TRAIT ALIASES ````````````````````````````````
// ===============================================================================

/// Execution context for the [`LazyBalance`] system.
///
/// `LazyBalanceContext` aggregates all external dependencies required to
/// materialize a concrete lazy balance model. It serves as the **type-level
/// environment** that binds:
///
/// - **Bounds** for all core components:
///   - `Balance` (asset, rational, time)
///   - `SnapShot` (asset, rational, time)
///   - `Receipt` (asset, rational, time)
///
/// - **Error type** via [`VirtualError`], defining the unified failure mode
///   for all balance operations.
///
/// - **Extension schemas** for:
///   - [`BalanceAddon`]
///   - [`SnapShotAddon`]
///   - [`ReceiptAddon`]
///
/// ## Role in the System
///
/// This trait does not define behavior directly. Instead, it provides the
/// **configuration layer** required by:
///
/// - [`VirtualDynField`] -> for allocation and capacity constraints
/// - [`VirtualDynExtension`] -> for external extensibility
/// - [`Plugin`](crate::plugins) execution -> for resolving context-dependent logic
///
/// In essence, it acts as a **dependency injection boundary** at the type level,
/// allowing the same [`LazyBalance`] implementation to operate under different:
///
/// - capacity limits
/// - extension schemas
/// - error definitions
///
/// without changing its core logic.
pub trait LazyBalanceContext:
    VirtualDynBound<BalanceAsset>
    + VirtualDynBound<BalanceRational>
    + VirtualDynBound<BalanceTime>
    + VirtualDynBound<SnapShotAsset>
    + VirtualDynBound<SnapShotRational>
    + VirtualDynBound<SnapShotTime>
    + VirtualDynBound<ReceiptAsset>
    + VirtualDynBound<ReceiptRational>
    + VirtualDynBound<ReceiptTime>
    + VirtualError<LazyBalanceError>
    + VirtualDynExtensionSchema<BalanceAddon>
    + VirtualDynExtensionSchema<SnapShotAddon>
    + VirtualDynExtensionSchema<ReceiptAddon>
{
}

impl<T> LazyBalanceContext for T where
    T: VirtualDynBound<BalanceAsset>
        + VirtualDynBound<BalanceRational>
        + VirtualDynBound<BalanceTime>
        + VirtualDynBound<SnapShotAsset>
        + VirtualDynBound<SnapShotRational>
        + VirtualDynBound<SnapShotTime>
        + VirtualDynBound<ReceiptAsset>
        + VirtualDynBound<ReceiptRational>
        + VirtualDynBound<ReceiptTime>
        + VirtualError<LazyBalanceError>
        + VirtualDynExtensionSchema<BalanceAddon>
        + VirtualDynExtensionSchema<SnapShotAddon>
        + VirtualDynExtensionSchema<ReceiptAddon>
{
}

/// Input contract for all [`LazyBalance`] operations.
///
/// This trait defines how operation-specific arguments are supplied
/// through a unified input type.
///
/// Each operation (e.g. `Deposit`, `Withdraw`) declares the exact
/// shape of input it requires via [`VirtualCollector`].
///
/// Key semantics:
/// - Mutating operations receive `MutHandle<Balance>`
/// - Read-only operations receive `Cow<Balance>`
/// - Additional parameters (e.g. `Asset`, `Receipt`) are selectively required
/// - Operations may include a `Subject` to influence execution behavior
///   (e.g. precision and fortitude)
///
/// This enables a single input type to serve all operations while
/// preserving strict typing per operation.
pub trait LazyBalanceInput<'a, Balance, Variant, Id, Asset, Receipt, T: LazyBalance>:
    VirtualCollector<
        (
            MutHandle<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Asset>,
            Cow<'a, T::Subject>,
        ),
        Deposit,
    > + VirtualCollector<
        (
            MutHandle<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Asset>,
            Cow<'a, T::Subject>,
        ),
        Mint,
    > + VirtualCollector<
        (
            MutHandle<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Asset>,
            Cow<'a, T::Subject>,
        ),
        Reap,
    > + VirtualCollector<
        (
            MutHandle<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Receipt>,
        ),
        Withdraw,
    > + VirtualCollector<(MutHandle<'a, Balance>, Cow<'a, Variant>, Cow<'a, Id>), Drain>
    + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Asset>,
            Cow<'a, T::Subject>,
        ),
        CanDeposit,
    > + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Asset>,
            Cow<'a, T::Subject>,
        ),
        CanMint,
    > + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Asset>,
            Cow<'a, T::Subject>,
        ),
        CanReap,
    > + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Receipt>,
        ),
        CanWithdraw,
    > + VirtualCollector<(Cow<'a, Balance>, Cow<'a, Variant>, Cow<'a, Id>), TotalValue>
    + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, Receipt>,
        ),
        ReceiptActiveValue,
    > + VirtualCollector<(Cow<'a, Balance>, Cow<'a, Variant>, Cow<'a, Id>), HasDeposits>
    + VirtualCollector<Cow<'a, Receipt>, ReceiptDepositValue>
    + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, T::Subject>,
        ),
        DepositLimits,
    > + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, T::Subject>,
        ),
        MintLimits,
    > + VirtualCollector<
        (
            Cow<'a, Balance>,
            Cow<'a, Variant>,
            Cow<'a, Id>,
            Cow<'a, T::Subject>,
        ),
        ReapLimits,
    >
where
    Balance: 'a + Clone,
    Variant: 'a + Clone,
    Id: 'a + Clone,
    Asset: 'a + Clone,
    Receipt: 'a + Clone,
{
}

impl<'a, T, Balance, Variant, Id, Asset, Receipt, B>
    LazyBalanceInput<'a, Balance, Variant, Id, Asset, Receipt, B> for T
where
    Balance: 'a + Clone,
    Variant: 'a + Clone,
    Id: 'a + Clone,
    Asset: 'a + Clone,
    Receipt: 'a + Clone,
    B: LazyBalance,
    T: VirtualCollector<
            (
                MutHandle<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Asset>,
                Cow<'a, B::Subject>,
            ),
            Deposit,
        > + VirtualCollector<
            (
                MutHandle<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Asset>,
                Cow<'a, B::Subject>,
            ),
            Mint,
        > + VirtualCollector<
            (
                MutHandle<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Asset>,
                Cow<'a, B::Subject>,
            ),
            Reap,
        > + VirtualCollector<
            (
                MutHandle<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Receipt>,
            ),
            Withdraw,
        > + VirtualCollector<(MutHandle<'a, Balance>, Cow<'a, Variant>, Cow<'a, Id>), Drain>
        + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Asset>,
                Cow<'a, B::Subject>,
            ),
            CanDeposit,
        > + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Asset>,
                Cow<'a, B::Subject>,
            ),
            CanMint,
        > + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Asset>,
                Cow<'a, B::Subject>,
            ),
            CanReap,
        > + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Receipt>,
            ),
            CanWithdraw,
        > + VirtualCollector<(Cow<'a, Balance>, Cow<'a, Variant>, Cow<'a, Id>), TotalValue>
        + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, Receipt>,
            ),
            ReceiptActiveValue,
        > + VirtualCollector<(Cow<'a, Balance>, Cow<'a, Variant>, Cow<'a, Id>), HasDeposits>
        + VirtualCollector<Cow<'a, Receipt>, ReceiptDepositValue>
        + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, B::Subject>,
            ),
            DepositLimits,
        > + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, B::Subject>,
            ),
            MintLimits,
        > + VirtualCollector<
            (
                Cow<'a, Balance>,
                Cow<'a, Variant>,
                Cow<'a, Id>,
                Cow<'a, B::Subject>,
            ),
            ReapLimits,
        >,
{
}

/// Output contract for all [`LazyBalance`] operations.
///
/// This trait defines how results are returned for each operation.
///
/// Each operation maps to a distinct result type via [`VirtualCollector`],
/// typically wrapped in `Result<_, Error<T>>`.
///
/// Key semantics:
/// - State-changing operations return success or error
/// - Value-producing operations return assets or receipts
/// - Validation operations return `Result<(), Error<T>>`
///
/// This forms a unified, type-indexed result space across all operations.
pub trait LazyBalanceOutput<'a, Asset, Receipt, SnapShot, Time, Limits, T>:
    VirtualCollector<Result<(Cow<'a, Asset>, Cow<'a, Receipt>), Error<T>>, Deposit>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, Mint>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, Reap>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, Drain>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, Withdraw>
    + VirtualCollector<Result<(), Error<T>>, CanDeposit>
    + VirtualCollector<Result<(), Error<T>>, CanMint>
    + VirtualCollector<Result<(), Error<T>>, CanReap>
    + VirtualCollector<Result<(), Error<T>>, CanWithdraw>
    + VirtualCollector<Result<(), Error<T>>, HasDeposits>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, TotalValue>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, ReceiptActiveValue>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, ReceiptDepositValue>
    + VirtualCollector<Result<Cow<'a, Asset>, Error<T>>, ReceiptDepositValue>
    + VirtualCollector<Result<Cow<'a, Limits>, Error<T>>, DepositLimits>
    + VirtualCollector<Result<Cow<'a, Limits>, Error<T>>, MintLimits>
    + VirtualCollector<Result<Cow<'a, Limits>, Error<T>>, ReapLimits>
where
    T: LazyBalance,
    Asset: 'a + Clone,
    Receipt: 'a + Clone,
    SnapShot: 'a + Clone,
    Time: 'a + Clone,
    Limits: 'a + Clone,
{
}

impl<'a, T, Asset, Receipt, SnapShot, Time, Limits, B>
    LazyBalanceOutput<'a, Asset, Receipt, SnapShot, Time, Limits, B> for T
where
    B: LazyBalance,
    Asset: 'a + Clone,
    Receipt: 'a + Clone,
    SnapShot: 'a + Clone,
    Time: 'a + Clone,
    Limits: 'a + Clone,
    T: VirtualCollector<Result<(Cow<'a, Asset>, Cow<'a, Receipt>), Error<B>>, Deposit>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, Mint>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, Reap>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, Drain>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, Withdraw>
        + VirtualCollector<Result<(), Error<B>>, CanDeposit>
        + VirtualCollector<Result<(), Error<B>>, CanMint>
        + VirtualCollector<Result<(), Error<B>>, CanReap>
        + VirtualCollector<Result<(), Error<B>>, CanWithdraw>
        + VirtualCollector<Result<(), Error<B>>, HasDeposits>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, TotalValue>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, ReceiptActiveValue>
        + VirtualCollector<Result<Cow<'a, Asset>, Error<B>>, ReceiptDepositValue>
        + VirtualCollector<Result<Cow<'a, Limits>, Error<B>>, DepositLimits>
        + VirtualCollector<Result<Cow<'a, Limits>, Error<B>>, MintLimits>
        + VirtualCollector<Result<Cow<'a, Limits>, Error<B>>, ReapLimits>,
{
}

/// A abstract-container representation of a structured value participating in the
/// lazy balance system (e.g. Balance, SnapShot, Receipt).
///
/// Look on to [`VirtualDynField`] or [`virtuals`](crate::virtuals) for contextual info.
///
/// It does not fix its internal representation directly. Instead, it:
/// - derives its core types (asset, rational, time) from context
/// - supports allocation via [`VirtualDynField`]
/// - allows extensibility through [`VirtualDynExtension`]
///
/// This keeps components lightweight, composable, and context-driven,
/// rather than self-contained or rigid.
pub trait LazyBalanceComponent<T, Asset, Rational, Time, Context, Addon>:
    Delimited
    + VirtualDynFieldWithDelegatedBounds<T::Asset, Context, Asset>
    + VirtualDynFieldWithDelegatedBounds<T::Rational, Context, Rational>
    + VirtualDynFieldWithDelegatedBounds<T::Time, Context, Time>
    + DelegateVirtualDynExtension<Context, Addon>
where
    T: LazyBalance,
    Context: VirtualDynExtensionSchema<Addon>
        + VirtualDynBound<Asset>
        + VirtualDynBound<Rational>
        + VirtualDynBound<Time>,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
}

impl<B, T, Asset, Rational, Time, Context, Addon>
    LazyBalanceComponent<B, Asset, Rational, Time, Context, Addon> for T
where
    T: Delimited
        + VirtualDynFieldWithDelegatedBounds<B::Asset, Context, Asset>
        + VirtualDynFieldWithDelegatedBounds<B::Rational, Context, Rational>
        + VirtualDynFieldWithDelegatedBounds<B::Time, Context, Time>
        + DelegateVirtualDynExtension<Context, Addon>,
    B: LazyBalance,
    Context: VirtualDynExtensionSchema<Addon>
        + VirtualDynBound<Asset>
        + VirtualDynBound<Rational>
        + VirtualDynBound<Time>,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
}

pub trait LazyBalanceLimits<T: LazyBalance>:
    Delimited
    + VirtualDynField<LimitsAsset, Some = T::Asset>
    + VirtualDynBound<LimitsAsset>
    + Extent<LimitsAsset, Scalar = T::Asset>
{
}

impl<L, T> LazyBalanceLimits<L> for T
where
    T: Delimited
        + VirtualDynField<LimitsAsset, Some = L::Asset>
        + VirtualDynBound<LimitsAsset>
        + Extent<LimitsAsset, Scalar = L::Asset>,
    L: LazyBalance,
{
}
// ===============================================================================
// ```````````````````````````` VIRTUAL DISCRIMINANTS ````````````````````````````
// ===============================================================================

discriminants! {
    /// Discriminant for the asset type within [`LazyBalance::Balance`].
    ///
    /// Used to resolve the concrete asset binding from context.
    BalanceAsset,

    /// Discriminant for the numeric representation within [`LazyBalance::Balance`].
    ///
    /// Identifies how balance values are represented (e.g. fixed-point).
    BalanceRational,

    /// Discriminant for the time dimension within [`LazyBalance::Balance`].
    ///
    /// Used to bind the temporal component associated with balance state.
    BalanceTime,

    /// Discriminant for the asset type within [`LazyBalance::SnapShot`].
    ///
    /// Separates snapshot asset semantics from live balance state.
    SnapShotAsset,

    /// Discriminant for the numeric representation within [`LazyBalance::SnapShot`].
    ///
    /// Defines how snapshot values are expressed.
    SnapShotRational,

    /// Discriminant for the time dimension within [`LazyBalance::SnapShot`].
    ///
    /// Used to associate snapshots with specific points in time.
    SnapShotTime,

    /// Discriminant for the asset type within [`LazyBalance::Receipt`].
    ///
    /// Identifies the asset representation for receipt claims.
    ReceiptAsset,

    /// Discriminant for the numeric representation within [`LazyBalance::Receipt`].
    ///
    /// Defines how receipt values are quantified.
    ReceiptRational,

    /// Discriminant for the time dimension within [`LazyBalance::Receipt`].
    ///
    /// Used to track temporal aspects of receipt validity or evolution.
    ReceiptTime,

    /// Discriminant identifying the storage domain for snapshots within
    /// [`LazyBalance`] systems.
    ///
    /// Used to bind lazy storage behavior for snapshot data.
    SnapShotStorage,

    /// Discriminant for error types within the [`LazyBalance`] system.
    ///
    /// Allows context to provide a concrete error type for all operations.
    LazyBalanceError,

    /// Discriminant for additional balance-specific extensions.
    ///
    /// Used to inject custom logic or metadata into [`LazyBalance::Balance`].
    BalanceAddon,

    /// Discriminant for additional snapshot-specific extensions.
    ///
    /// Used to extend snapshot behavior or attach auxiliary data.
    SnapShotAddon,

    /// Discriminant for additional receipt-specific extensions.
    ///
    /// Used to extend receipt behavior or attach auxiliary data.
    ReceiptAddon,

    /// Discriminant for the asset type used in operation limits.
    ///
    /// Binds the value representation used when deriving limits
    /// (e.g. minimum, maximum, optimal) for balance operations.
    LimitsAsset,

}

// ===============================================================================
// ````````````````````````` LAZY BALANCE MODEL CHECKER ``````````````````````````
// ===============================================================================

#[cfg(feature = "std")]
pub use lazy_balance_model_checker::*;

/// Model checker module for [`LazyBalance`] implementations.
///
/// Use [`LazyBalanceModelChecker`] as the **entrypoint** for all model
/// checking operations.
///
/// This module exposes the required types:
/// - [`ManualBalanceModel`]
/// - [`BalanceOp`]
/// - [`BalanceState`]
/// - [`BalanceGuards`]
/// - [`BalanceTraps`]
/// - [`BalanceModelResults`]
///
/// These types are used to define the testing environment:
/// - reference model behavior
/// - operation space
/// - state representation
/// - validity and trap conditions
///
/// The execution engine is internal; interact with it only through
/// the trait methods.
#[cfg(feature = "std")]
mod lazy_balance_model_checker {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- Local crate imports ---
    use crate::{assets::*, base::Sortable};

    // --- Core (Rust std replacement) ---
    use core::{fmt::Debug, hash::Hash};

    // --- Substrate primitives ---
    use sp_runtime::{traits::One, Cow};

    // --- Substrate std helpers ---
    use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet, vec_deque::VecDeque};

    // --- Standard library ---
    use std::{
        fs::{create_dir_all, File},
        io::Write,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    // --- Randomness Provider ---
    use rand::{seq::SliceRandom, thread_rng};

    // ===============================================================================
    // ```````````````````` LAZY BALANCE MODEL CHECKER ENTRYPOINT ````````````````````
    // ===============================================================================

    /// Unified model checker interface over the lazy balance
    /// model checker module.
    ///
    /// This trait provides a **single entrypoint API** to access
    /// all model-checking utilities.
    ///
    /// By implementing this trait on a tester type, you bind:
    /// - the lazy model ([`Self::LazyBalance`]) under test
    /// - the manual reference model ([`Self::ManualBalance`])
    /// - guards, hashing, and execution semantics
    ///
    /// ## Underlying System
    ///
    /// - [`ManualBalanceModel`]: eager balance reference model
    /// - [`BalanceOp`]: balance operations set
    /// - [`BalanceState`]: execution state (lazy + manual + trace)
    /// - [`BalanceGuards`]: operation validity rules
    /// - [`BalanceTraps`]: controlled invalid execution
    /// - [`BalanceModelResults`]: result aggregation
    ///
    /// ## What happens during execution
    ///
    /// - sequences of [`BalanceOp`] are explored (breadth-first)
    /// - each step builds on a **trace** to form a [`BalanceState`]
    /// - operations are filtered using:
    ///   - [`BalanceGuards`] -> define normal validity
    ///   - [`BalanceTraps`] -> allow controlled invalid cases
    ///
    /// An operation is considered **valid for execution** if:
    ///   - it passes the `guard`, OR
    ///   - it is explicitly allowed by a `trap`
    ///
    /// Additionally, exploration is further controlled by `flow`:
    ///   - both `guard flow` and `trap flow` must allow the operation
    ///   - if either blocks it, the operation is not explored further
    ///
    /// - both models are executed:
    ///     - lazy: [`Self::LazyBalance`] via [`LazyBalance`]
    ///     - manual: [`Self::ManualBalance`] vi [`ManualBalanceModel`]
    ///
    /// - results are classified into:
    ///     - [`BalanceFailure`]: execution error
    ///     - [`BalanceExit`]: drift between models
    ///     - `pass`: successful sequence
    ///     - `trap`: expected failure via [`BalanceTraps`]
    ///
    /// ## Usage
    ///
    /// ```ignore
    /// struct Tester;
    ///
    /// impl LazyBalanceModelChecker for Tester {
    ///     type LazyBalance = Balance;
    ///     type ManualBalance = Manual;
    ///
    ///     type TrapFn = fn(
    ///         &BalanceState<Self::LazyBalance, Self::ManualBalance>,
    ///         &BalanceOp<Self::LazyBalance, Self::ManualBalance>,
    ///     ) -> bool;
    ///
    ///     type FlowFn = fn(
    ///         &BalanceState<Self::LazyBalance, Self::ManualBalance>,
    ///         &BalanceOp<Self::LazyBalance, Self::ManualBalance>,
    ///     ) -> bool;
    ///
    ///     type Hasher = fn(
    ///         &BalanceState<Self::LazyBalance, Self::ManualBalance>,
    ///     ) -> u64;
    /// }
    ///
    /// let mut results = Tester::initiate_results();
    ///
    /// Tester::explore(..., &mut results);
    /// // or
    /// Tester::explore_with_traps(..., Some(traps), &mut results);
    /// // or
    /// Tester::explore_custom(..., Some(traps), &mut results);
    ///
    /// Tester::write_reports(path, &results, false, false, false);
    /// ```
    pub trait LazyBalanceModelChecker: Sized {
        /// Lazy balance implementation.
        type LazyBalance: LazyBalanceMarker;

        /// Manual (reference) model used for validation.
        ///
        /// This type provides an **eager execution layer** over the lazy model:
        /// - consumes resolved outputs from [`Self::LazyBalance`] via [`LazyBalance`]
        /// - maintains a concrete, user-facing state
        ///
        /// Unlike the lazy model, this operates as a **direct global
        /// state model** via [`ManualBalanceModel`] methods:
        /// - no deferred receipts
        /// - no lazy resolution
        /// - updates are applied immediately to a single state
        ///
        /// It must also define:
        /// - [`BalanceStateHasher`]: for state deduplication during exploration
        /// - [`BalanceGuards`]: for operation validity and invariants
        ///
        /// Acts as the **reference baseline** against which the lazy model
        /// is evaluated for correctness and precision.
        type ManualBalance: ManualBalanceModel<Self::LazyBalance>
            + BalanceStateHasher<Self::LazyBalance, Self::ManualBalance>
            + BalanceGuards<Self::LazyBalance, Self::ManualBalance>
            + Clone;

        /// Trap predicate used for trap-based testing.
        ///
        /// Represents a **single trap condition** applied across all operations:
        /// - evaluated per `(state, op)`
        /// - returns `true` to override guards defined in [`BalanceGuards`]
        /// via [`Self::ManualBalance`]
        ///
        /// This trap is **injected globally** during exploration, so it will be
        /// evaluated for every operation ([`BalanceOp`]). Implementations should therefore:
        /// - precisely target the intended operation/condition
        /// - return `false` for all unrelated operations
        ///
        /// Unlike [`BalanceGuards`], which define validity for all operations at once,
        /// this is defined **per invocation** to inject a specific invalid scenario.
        ///
        /// Used to enable controlled violations:
        /// ```text
        /// guard(...) || trap(...)
        /// ```
        type TrapFn: Fn(
            &BalanceState<Self::LazyBalance, Self::ManualBalance>,
            &BalanceOp<Self::LazyBalance, Self::ManualBalance>,
        ) -> bool;

        /// Flow predicate used to control exploration.
        ///
        /// Applied globally across all operations:
        /// - evaluated per `(state, op)`
        /// - returns `true` to allow the candidate operation [`BalanceOp`] to proceed
        /// - returns `false` to block exploration of that operation
        ///
        /// Combined with the base guard flow as:
        /// ```text
        /// guard_flow(...) && trap_flow(...)
        /// ```
        ///
        /// Unlike [`Self::TrapFn`], this does **not override validity**;
        /// it only restricts whether an operation is explored.
        ///
        /// Since this is combined using `&&`, returning `false` will
        /// **prevent the operation from being explored entirely**.
        /// Therefore implementations should:
        /// - return `true` by default
        /// - only return `false` when explicitly pruning a specific case
        ///
        /// Used to shape the search space (e.g., pruning, targeting).
        type FlowFn: Fn(
            &BalanceState<Self::LazyBalance, Self::ManualBalance>,
            &BalanceOp<Self::LazyBalance, Self::ManualBalance>,
        ) -> bool;

        /// Optional additional custom state hasher.
        ///
        /// Provides an additional hashing layer over [`BalanceState`] during exploration:
        /// - evaluated alongside [`BalanceStateHasher`] from [`Self::ManualBalance`]
        /// - used for further deduplication of states
        ///
        /// Since [`Self::ManualBalance`] already defines a primary hashing strategy via
        /// [`BalanceStateHasher`], this acts as an **optional second-stage filter**.
        ///
        /// This is particularly useful when:
        /// - multiple exploration strategies or model checkers are run
        /// - additional pruning is needed beyond the primary state hash
        ///
        /// In such cases, this hasher can be applied **on top of the base hasher**
        /// to refine state uniqueness and avoid revisiting equivalent states.
        ///
        /// If provided, both hashes are used during exploration.
        type Hasher: Fn(&BalanceState<Self::LazyBalance, Self::ManualBalance>) -> u64;

        /// Creates a new [`BalanceModelResults`] container.
        fn initiate_results() -> BalanceModelResults<Self::LazyBalance, Self::ManualBalance> {
            BalanceModelResults::new()
        }

        /// Runs model exploration under valid conditions (guards only).
        ///
        /// Explores sequences of operations using [`BalanceGuards`] without any
        /// trap overrides.
        ///
        /// ## Parameters
        ///
        /// - `users`: set of users participating in operations
        /// - `deposits`: input values used for deposit operations
        /// - `adjustments`: values used for mint / reap operations
        /// - `subjects`: contextual inputs (e.g. precision, forced flags)
        /// - `max_depth`: maximum sequence length to explore
        /// - `allowed_bps`: maximum allowed drift in basis points
        /// (e.g. 10 = 0.10%, 100 = 1%)
        /// - `allowed_diff`: maximum allowed absolute value difference
        /// (not percentage-based)
        /// - `results`: mutable container collecting outcomes
        ///
        /// ## Behavior
        ///
        /// - generates operation sequences up to `max_depth`
        /// - filters using [`BalanceGuards`] (`guard(...)`)
        /// - executes both lazy and manual models
        /// - compares results and records:
        /// - `pass`: valid execution
        /// - `fail`: error
        /// - `exit`: withdrawals drifts between balance models
        ///
        /// This is the standard correctness check without testing invalid scenarios.
        fn explore(
            users: &[<Self::ManualBalance as ManualBalanceModel<Self::LazyBalance>>::User],
            deposits: &[<Self::LazyBalance as LazyBalance>::Asset],
            adjustments: &[<Self::LazyBalance as LazyBalance>::Asset],
            subjects: &[<Self::LazyBalance as LazyBalance>::Subject],
            max_depth: u32,
            allowed_bps: u32,
            allowed_diff: u32,
            results: &mut BalanceModelResults<Self::LazyBalance, Self::ManualBalance>,
        ) where
            <Self::LazyBalance as LazyBalance>::Id: Default,
            <Self::LazyBalance as LazyBalance>::Asset: From<<Self::LazyBalance as LazyBalance>::Asset>
                + Into<<Self::LazyBalance as LazyBalance>::Asset>,
        {
            explore_explore_balance_states_default::<Self::LazyBalance, Self::ManualBalance>(
                users,
                deposits,
                adjustments,
                subjects,
                max_depth,
                allowed_bps,
                allowed_diff,
                results,
            )
        }

        /// Runs exploration with trap overrides (`guard || trap`).
        ///
        /// Extends [`Self::explore`] by allowing controlled violations
        /// of guards using [`BalanceTraps`].
        ///
        /// ## Parameters
        ///
        /// - `users`: set of users participating in operations
        /// - `deposits`: input values used for deposit operations
        /// - `adjustments`: values used for mint / reap operations
        /// - `subjects`: contextual inputs (e.g. precision, forced flags)
        /// - `max_depth`: maximum sequence length to explore
        /// - `allowed_bps`: maximum allowed drift in basis points
        ///   (e.g. 10 = 0.10%, 100 = 1%)
        /// - `allowed_diff`: maximum allowed absolute value difference
        /// - `traps`: optional trap configuration (trap, flow, reason)
        /// - `results`: mutable container collecting outcomes
        ///
        /// ## Behavior
        ///
        /// - operations are allowed using:
        /// ```text
        /// guard(...) || trap(...)
        /// ```
        ///
        /// - `trap` is applied globally to every operation:
        ///   - should return `true` only for the intended operation(s)
        ///   - should return `false` by default for all unrelated operations
        ///
        /// - `flow` further filters exploration:
        /// ```text
        /// guard_flow(...) && trap_flow(...)
        /// ```
        ///   - should return `true` for the intended operation(s) or by default
        ///   - return `false` **only to explicitly block** exploration of a case
        ///
        /// - typically used to:
        ///   - force invalid transitions
        ///   - trigger expected errors
        ///   - validate edge-case behavior
        ///
        /// - executes both lazy and manual models and records:
        ///   - pass / fail / exit / trap outcomes
        ///
        /// This is used for adversarial and trap-based testing.
        fn explore_traps(
            users: &[<Self::ManualBalance as ManualBalanceModel<Self::LazyBalance>>::User],
            deposits: &[<Self::LazyBalance as LazyBalance>::Asset],
            adjustments: &[<Self::LazyBalance as LazyBalance>::Asset],
            subjects: &[<Self::LazyBalance as LazyBalance>::Subject],
            max_depth: u32,
            allowed_bps: u32,
            allowed_diff: u32,
            traps: Option<BalanceTraps<Self::TrapFn, Self::FlowFn>>,
            results: &mut BalanceModelResults<Self::LazyBalance, Self::ManualBalance>,
        ) where
            <Self::LazyBalance as LazyBalance>::Id: Default,
            <Self::LazyBalance as LazyBalance>::Asset: From<<Self::LazyBalance as LazyBalance>::Asset>
                + Into<<Self::LazyBalance as LazyBalance>::Asset>,
        {
            explore_balance_trap_states_default::<
                Self::LazyBalance,
                Self::ManualBalance,
                Self::TrapFn,
                Self::FlowFn,
            >(
                users,
                deposits,
                adjustments,
                subjects,
                max_depth,
                allowed_bps,
                allowed_diff,
                traps,
                results,
            )
        }

        /// Runs full custom exploration (custom traps + optional additional hasher).
        ///
        /// This is the most flexible entrypoint, allowing control over:
        /// - trap behavior
        /// - additional state hashing
        ///
        /// ## Parameters
        ///
        /// - `users`: set of users participating in operations
        /// - `deposits`: input values used for deposit operations
        /// - `adjustments`: values used for mint / reap operations
        /// - `subjects`: contextual inputs (e.g. precision, forced flags)
        /// - `max_depth`: maximum sequence length to explore
        /// - `allowed_bps`: maximum allowed drift in basis points
        ///   (e.g. `10` = 0.10%, `100` = 1%)
        /// - `allowed_diff`: maximum allowed absolute value difference
        /// - `traps`: optional trap configuration (`guard || trap`)
        /// - `hasher`: optional additional state hasher (second-stage pruning)
        /// - `results`: mutable container collecting outcomes
        ///
        /// ## Behavior
        ///
        /// - combines:
        ///   - [`BalanceGuards`] -> base validity
        ///   - [`BalanceTraps`] -> override (`guard || trap`)
        ///   - `flow` -> exploration control (`guard_flow && trap_flow`)
        ///
        /// - if `hasher` is provided:
        ///   - used alongside [`BalanceStateHasher`] from [`Self::ManualBalance`]
        ///   - enables additional state deduplication
        ///
        /// - executes both lazy and manual models and records:
        ///   - pass / fail / exit / trap outcomes
        ///
        /// Use this when:
        /// - testing complex trap scenarios
        /// - adding custom state pruning logic
        /// - requiring full control over exploration behavior
        fn explore_custom(
            users: &[<Self::ManualBalance as ManualBalanceModel<Self::LazyBalance>>::User],
            deposits: &[<Self::LazyBalance as LazyBalance>::Asset],
            adjustments: &[<Self::LazyBalance as LazyBalance>::Asset],
            subjects: &[<Self::LazyBalance as LazyBalance>::Subject],
            max_depth: u32,
            allowed_bps: u32,
            allowed_diff: u32,
            traps: Option<BalanceTraps<Self::TrapFn, Self::FlowFn>>,
            hasher: Option<Self::Hasher>,
            results: &mut BalanceModelResults<Self::LazyBalance, Self::ManualBalance>,
        ) where
            <Self::LazyBalance as LazyBalance>::Id: Default,
            <Self::LazyBalance as LazyBalance>::Asset: From<<Self::LazyBalance as LazyBalance>::Asset>
                + Into<<Self::LazyBalance as LazyBalance>::Asset>,
        {
            explore_balance_states::<
                Self::LazyBalance,
                Self::ManualBalance,
                Self::TrapFn,
                Self::FlowFn,
                Self::Hasher,
            >(
                users,
                deposits,
                adjustments,
                subjects,
                max_depth,
                allowed_bps,
                allowed_diff,
                traps,
                hasher,
                results,
            )
        }

        /// Writes CSV reports for analysis.
        ///
        /// Exports the contents of [`BalanceModelResults`] into CSV files
        /// under the given directory.
        ///
        /// ## Parameters
        ///
        /// - `dir`: output directory where CSV files will be created
        /// - `results`: collected model checking results
        /// - `write_pass`: whether to write successful sequences
        /// - `write_exit`: whether to write drift cases ([`BalanceExit`])
        /// - `write_trap`: whether to write trapped sequences
        ///
        /// ## Behavior
        ///
        /// - always writes **failures**
        /// - optionally writes:
        ///   - `pass`: successful executions
        ///   - `exit`: drift between lazy and manual models
        ///   - `trap`: expected trap-triggered sequences
        ///
        /// - file names are timestamped to avoid overwriting
        /// - prints a summary of results after writing
        ///
        /// Useful for:
        /// - debugging failures
        /// - analyzing drift behavior
        /// - validating trap scenarios
        fn write_reports(
            dir: std::path::PathBuf,
            results: &BalanceModelResults<Self::LazyBalance, Self::ManualBalance>,
            write_pass: bool,
            write_exit: bool,
            write_trap: bool,
        ) {
            balance_states_csv_reports(dir, results, write_pass, write_exit, write_trap)
        }
    }

    // ===============================================================================
    // ````````````````````` MODEL CHECKER ENUMS AND STRUCTURES ``````````````````````
    // ===============================================================================

    /// Operation descriptor for driving both [`LazyBalance`] and
    /// [`ManualBalanceModel`] executions in a consistent manner.
    ///
    /// Each variant represents a high-level balance action along with
    /// the required inputs. These operations are typically used in
    /// testing or simulation to:
    /// - execute the lazy balance model
    /// - pass the resolved results into a manual (eager) model
    /// - compare and validate behavior across both systems
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum BalanceOp<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// Deposit operation.
        ///
        /// `(user, amount, subject)` where:
        /// - `user` is the target account
        /// - `amount` is the requested asset value
        /// - `subject` defines execution intent (e.g., precision/behavior)
        Deposit(M::User, T::Asset, T::Subject),

        /// Withdraw operation.
        ///
        /// `(user)` where:
        /// - `user` is the account performing withdrawal
        Withdraw(M::User),

        /// Mint operation.
        ///
        /// `(amount, subject)` where:
        /// - `amount` is the asset value to introduce
        /// - `subject` defines execution intent
        Mint(T::Asset, T::Subject),

        /// Reap operation.
        ///
        /// `(amount, subject)` where:
        /// - `amount` is the asset value to remove or adjust
        /// - `subject` defines execution intent
        Reap(T::Asset, T::Subject),

        /// Drain (Full-Reap) operation.
        ///
        /// Resets the entire balance state.
        Drain,
    }

    /// Represents a failure encountered during execution of an operation sequence.
    ///
    /// This structure is used for:
    /// - validating expected failures (trap testing), where a known sequence
    ///   must produce a specific error
    /// - diagnosing mismatches between lazy and manual models
    /// - providing reproducible execution traces for debugging
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BalanceFailure<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// The error message, typically derived from the returned error
        /// (e.g. via debug formatting), describing why execution failed.
        pub reason: String,
        /// The trace of operations leading to the failure, where the
        /// final operation in the sequence is the one that triggered the error.
        pub sequence: Vec<BalanceOp<T, M>>,
    }

    /// Represents a successful withdrawal where the lazy and manual models
    /// produce different outputs.
    ///
    /// A withdrawal has completed successfully in both models, but the
    /// resulting values are not equal, indicating **drift** between the
    /// lazy (deferred) model and the manual (eager reference) model.
    ///
    /// This occurs because withdrawals are state-dependent: prior balance
    /// adjustments (`mint`, `reap`) can change the effective value of a
    /// receipt, so the redeemed amount may differ from the original deposit.
    ///
    /// This structure captures:
    /// - the outputs from both models
    /// - their absolute difference (`diff`)
    /// - the difference in basis points (`bps`)
    /// - the full operation trace leading to the withdrawal
    ///
    /// The final operation in `sequence` is always the [`BalanceOp::Withdraw`] that
    /// produced this result.
    ///
    /// [`LazyBalanceModelChecker`] methods may apply tolerance thresholds (e.g. minimum
    /// `diff` or `bps`) to ignore negligible drift. If the deviation falls within
    /// acceptable bounds, this exit may be discarded.
    ///
    /// This is primarily used to evaluate whether the lazy model maintains
    /// acceptable precision relative to the manual reference model.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BalanceExit<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// Value returned by the lazy balance model.
        pub lazy: T::Asset,

        /// Value returned by the manual (reference) model.
        pub manual: T::Asset,

        /// Absolute difference between `lazy` and `manual`.
        pub diff: T::Asset,

        /// Difference expressed in basis points.
        pub bps: T::Asset,

        /// Trace of operations leading to this withdrawal.
        /// The last operation is the `Withdraw`.
        pub sequence: Vec<BalanceOp<T, M>>,
    }

    /// Convenience container holding the owned state required to operate
    /// on a [`LazyBalance`] instance.
    ///
    /// This groups the core components needed for execution into a single
    /// structure with owned representations.
    ///
    /// Primarily used to simplify passing and managing balance state
    /// across operations and tests.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct LazyContainer<T: LazyBalanceMarker> {
        /// Underlying mutable balance state as owned.
        pub balance: T::Balance,

        /// Logical partition of the balance.
        pub variant: T::Variant,

        /// Identifier of the balance owner or context.
        pub id: T::Id,
    }

    /// Primary state container for executing and tracking a sequence of operations.
    ///
    /// Holds both lazy and manual model states along with auxiliary data
    /// required for coordinated execution and validation.
    ///
    /// This can represent:
    /// - a full state snapshot at a given step in a sequence, or
    /// - a single evolving state where operations are applied incrementally
    ///
    /// Used by model checkers to:
    /// - execute operations step-by-step
    /// - maintain consistency between lazy and manual models
    /// - track execution history for validation and debugging
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BalanceState<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// Lazy balance state container.
        pub lazy: LazyContainer<T>,

        /// Manual (eager reference) model state.
        pub manual: M,

        /// Latest receipts per user.
        pub receipts: BTreeMap<M::User, T::Receipt>,

        /// Operation trace leading to this state.
        pub trace: Vec<BalanceOp<T, M>>,
    }

    /// Defines trap behavior for state exploration.
    ///
    /// `BalanceTraps` provides mechanisms to intentionally allow and explore
    /// otherwise disallowed operations (as per [`BalanceGuards`]) for testing
    /// and validation purposes.
    ///
    /// - `trap` is a predicate over `(state, op)` that, when `true`,
    ///   overrides the guard and allows the operation:
    ///
    ///   ```text
    ///   guard(...) || trap(...)
    ///   ```
    ///
    ///   This enables controlled exploration of invalid transitions.
    ///
    /// - `flow` is an additional exploration filter combined with the
    ///   base flow using logical AND:
    ///
    ///   ```text
    ///   guard_flow && trap_flow
    ///   ```
    ///
    ///   Unlike `trap`, this does not override but restricts execution.
    ///
    /// - `reason` is the expected failure description i.e., debugged error
    ///   associated with the trap, typically used for validation or comparison
    ///   when the trapped operation results in an error.
    ///
    /// `G` and `F` are function/closure types using [`BalanceState`] and
    /// [`BalanceOp`]:
    /// - `G: Fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool`
    /// - `F: Fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool`
    pub struct BalanceTraps<G, F> {
        /// Predicate that allows an operation even if the guard rejects it.
        pub trap: G,

        /// Additional flow filter applied with logical AND.
        pub flow: F,

        /// Expected reason associated with the trapped execution.
        pub reason: String,
    }

    // ===============================================================================
    // ```````````````````````````` MODEL CHECKER TRAITS `````````````````````````````
    // ===============================================================================

    /// Convenience trait alias for a [`LazyBalance`] implementor used in
    /// model checking and testing.
    pub trait LazyBalanceMarker: LazyBalance + Clone + Debug {}

    impl<T> LazyBalanceMarker for T where T: LazyBalance + Clone + Debug {}

    /// A user-indexed **eager balance model** over a [`LazyBalance`] system,
    /// primarily intended for testing and validation.
    ///
    /// This model consumes the resolved outputs of lazy balance operations
    /// (`lazy_result`) and maintains a concrete, user-facing state without
    /// any deferred semantics (no receipts, no lazy evaluation).
    ///
    /// Unlike [`LazyBalance`], implementations are free to use the simplest
    /// possible accounting logic (e.g., a single global state machine or
    /// direct user balances), as correctness is derived from the provided
    /// results rather than internal computation.
    ///
    /// It is not a source of truth and must remain consistent with the
    /// underlying [`LazyBalance`] execution.
    ///
    /// This makes it suitable as a **reference model** for:
    /// - validating lazy execution behavior
    /// - testing invariants and edge cases
    /// - comparing expected vs actual outcomes
    pub trait ManualBalanceModel<T>: Clone + Debug
    where
        T: LazyBalanceMarker,
    {
        /// Simple user identifier type for the model.
        type User: Sortable + Hash + Copy;

        /// Error type returned by model operations.
        type Error: Debug + Clone + 'static;

        /// Creates a new instance with an empty manual balance state.
        fn new() -> Self;

        /// Applies a deposit for `user`.
        ///
        /// `amount` is the requested input. The lazy model is assumed to have been
        /// executed with the same inputs, and `lazy_result` contains its resolved output:
        /// - actual deposited value
        /// - corresponding receipt
        ///
        /// This method consumes that result to update the manual balance state.
        fn deposit(
            &mut self,
            user: Self::User,
            amount: T::Asset,
            lazy_result: &(T::Asset, T::Receipt),
        ) -> Result<(), Self::Error>;

        /// Applies a withdrawal for `user`.
        ///
        /// The lazy model is assumed to have been executed with the same inputs,
        /// and `lazy_result` contains the resolved asset produced by that withdrawal.
        ///
        /// This method consumes that result to update the manual balance state.
        ///
        /// Returns the withdrawn amount.
        fn withdraw(
            &mut self,
            user: Self::User,
            lazy_result: &T::Asset,
        ) -> Result<T::Asset, Self::Error>;

        /// Applies a mint operation.
        ///
        /// The lazy model is assumed to have been executed with the same input,
        /// and `lazy_result` contains the resolved asset produced by that mint.
        ///
        /// This method consumes that result to update the manual balance state.
        fn mint(&mut self, amount: T::Asset, lazy_result: &T::Asset) -> Result<(), Self::Error>;

        /// Applies a reap operation.
        ///
        /// The lazy model is assumed to have been executed with the same input,
        /// and `lazy_result` contains the resolved asset produced by that reap.
        ///
        /// This method consumes that result to update the manual balance state.
        fn reap(&mut self, amount: T::Asset, lazy_result: &T::Asset) -> Result<(), Self::Error>;

        /// Applies a drain (full reap), resetting the manual balance state.
        ///
        /// Brings the model into a fully drained state, consistent with a
        /// corresponding lazy balance drain operation.
        fn drain(&mut self) -> Result<(), Self::Error>;

        /// Returns the total aggregated value tracked by the model.
        ///
        /// This is a utility method for external use and is not used by
        /// the model checker during exploration.
        fn total(&self) -> T::Asset;
    }

    /// Defines a primary hashing strategy for [`BalanceState`] instances.
    ///
    /// Used to uniquely identify a state during exploration or testing,
    /// allowing model checkers to avoid revisiting previously seen states.
    ///
    /// Implementors can choose any hashing scheme appropriate for their
    /// use case, as long as it provides sufficient uniqueness for pruning
    /// duplicate states during operation sequencing.
    ///
    /// This is typically used to:
    /// - detect already explored states
    /// - prevent redundant execution paths
    /// - guide search strategies over operation sequences
    pub trait BalanceStateHasher<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// Computes a hash representing the given state.
        fn hash(state: &BalanceState<T, M>) -> u64;
    }

    /// Defines validity checks for operations during state exploration.
    ///
    /// Each method determines whether a candidate operation ([`BalanceOp`]) can be
    /// appended to the current trace, i.e., whether the next step in the sequence is
    /// valid given the current state.
    ///
    /// - A **sequence** is an ordered list of operations (`Vec<BalanceOp>`) being explored.
    /// - The **trace** is the current prefix of that sequence already applied
    ///   to reach the current [`BalanceState`].
    ///
    /// Each guard method receives the current state (derived from the trace)
    /// and the inputs of the next operation, and returns:
    /// - `true`: operation is allowed
    /// - `false`: operation is disallowed (guarded)
    ///
    /// During execution, an operation ([`BalanceOp`]) is permitted if:
    ///
    /// ```text
    /// guard(...) || trap(...)
    /// ```
    ///
    /// - `guard(...)` is the normal allow/disallow check (this trait)
    /// - [`BalanceTraps`] may override and allow a disallowed operation
    ///
    /// ## Flow
    ///
    /// `flow` is an additional filter applied after guards:
    ///
    /// ```text
    /// guard_flow(...) && trap_flow(...)
    /// ```
    ///
    /// - Both must return `true` for the operation to proceed
    /// - Unlike guards, flow is combined (`&&`), not overridden
    /// - Used only to control exploration, not correctness
    ///
    /// ## Invariant
    ///
    /// `invariant` must hold for every state reached via the
    /// trace evaluated at each step
    ///
    /// ## Example
    ///
    /// ```text
    /// trace:   [Deposit(A, 100)]
    /// next:    Withdraw(A)
    ///
    /// guard_withdraw(state, A) = true -> allowed (valid transition)
    ///
    /// --------
    ///
    /// trace:   []
    /// next:    Withdraw(A)
    ///
    /// guard_withdraw(state, A) = false -> disallowed (no receipt)
    ///
    /// trap_override(...) = true -> allowed (forced invalid case for testing)
    ///
    /// flow(...) = false -> blocked regardless of guard/trap
    /// ```
    ///
    /// BalanceGuards should only express validity conditions for extending the
    /// sequence. They should not include exploration logic or testing behavior.
    pub trait BalanceGuards<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// Returns `true` if [`BalanceOp::Deposit`] is allowed to be appended
        /// to the current trace, given the current state and inputs.
        ///
        /// `false` disallows the operation.
        fn deposit(
            _state: &BalanceState<T, M>,
            _user: &M::User,
            _amount: &T::Asset,
            _subject: &T::Subject,
        ) -> bool {
            true
        }

        /// Returns `true` if [`BalanceOp::Withdraw`] is allowed to be appended
        /// to the current trace, given the current state and inputs.
        ///
        /// `false` disallows the operation.
        fn withdraw(_state: &BalanceState<T, M>, _user: &M::User) -> bool {
            true
        }

        /// Returns `true` if [`BalanceOp::Mint`] is allowed to be appended
        /// to the current trace, given the current state and inputs.
        ///
        /// `false` disallows the operation.
        fn mint(_state: &BalanceState<T, M>, _value: &T::Asset, _subject: &T::Subject) -> bool {
            true
        }

        /// Returns `true` if [`BalanceOp::Reap`] is allowed to be appended
        /// to the current trace, given the current state and inputs.
        ///
        /// `false` disallows the operation.
        fn reap(_state: &BalanceState<T, M>, _value: &T::Asset, _subject: &T::Subject) -> bool {
            true
        }

        /// Returns `true` if [`BalanceOp::Drain`] is allowed to be appended
        /// to the current trace, given the current state and inputs.
        ///
        /// `false` disallows the operation.
        fn drain(_state: &BalanceState<T, M>) -> bool {
            true
        }

        /// Additional exploration filter applied after guards.
        ///
        /// Returns `true` if the candidate operation should be explored further.
        /// Combined with [`BalanceTraps`] `flow` using logical AND (`&&`) if traps
        /// are initiated.
        fn flow(_state: &BalanceState<T, M>, _next: &BalanceOp<T, M>) -> bool {
            true
        }

        /// Global invariant over the current state.
        ///
        /// Must hold for every state reached via the trace:
        /// - `Ok(())` -> state is valid
        /// - `Err(reason)` -> invariant violated, path terminates
        fn invariant(_state: &BalanceState<T, M>) -> Result<(), String> {
            Ok(())
        }
    }

    // ===============================================================================
    // `````````````````````````````` MODEL CHECKER API ``````````````````````````````
    // ===============================================================================

    /// Default state exploration entrypoint for [`LazyBalance`].
    ///
    /// Uses:
    /// - [`BalanceGuards`] for validity
    /// - no [`BalanceTraps`] (strict valid execution only)
    /// - Utilizes state hashing using [`BalanceStateHasher`]
    ///
    /// Explores sequences up to max_depth, applying operations that pass:
    /// - `guard(...)`
    /// - `flow(...)`
    ///
    /// Records:
    /// - successful sequences
    /// - failures
    /// - exits (drift between lazy and manual models)
    /// - asserts [`BalanceGuards::invariant`] at every state
    ///
    /// This is the standard model check under valid system behavior only.
    fn explore_explore_balance_states_default<T, M>(
        users: &[M::User],
        deposits: &[T::Asset],
        adjustments: &[T::Asset],
        subjects: &[T::Subject],
        max_depth: u32,
        allowed_bps: u32,
        allowed_diff: u32,
        results: &mut BalanceModelResults<T, M>,
    ) where
        T: LazyBalanceMarker + Clone,
        M: ManualBalanceModel<T> + BalanceStateHasher<T, M> + BalanceGuards<T, M> + Clone,
        T::Id: Default,
        T::Asset: From<T::Asset> + Into<T::Asset>,
    {
        explore_balance_states::<
            T,
            M,
            fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool,
            fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool,
            fn(&BalanceState<T, M>) -> u64,
        >(
            users,
            deposits,
            adjustments,
            subjects,
            max_depth,
            allowed_bps,
            allowed_diff,
            None,
            None,
            results,
        );
    }

    // BalanceState exploration with trap overrides for [`LazyBalance`].
    ///
    /// Extends [`explore_explore_balance_states_default`] by allowing controlled
    /// violations of [`BalanceGuards`] via [`BalanceTraps`].
    ///
    /// Uses:
    /// - [`BalanceGuards`] for baseline validity
    /// - [`BalanceTraps`] to override guards (`guard || trap`)
    /// - trap-specific flow filtering (`guard_flow && trap_flow`)
    ///
    /// This enables exploration of:
    /// - invalid transitions
    /// - edge cases
    /// - expected failure scenarios
    ///
    /// `reason` from [`BalanceTraps`] is used to validate or tag resulting errors.
    ///
    /// Suitable for trap testing and adversarial validation.
    fn explore_balance_trap_states_default<T, M, G, F>(
        users: &[M::User],
        deposits: &[T::Asset],
        adjustments: &[T::Asset],
        subjects: &[T::Subject],
        max_depth: u32,
        allowed_bps: u32,
        allowed_diff: u32,
        overrides: Option<BalanceTraps<G, F>>,
        results: &mut BalanceModelResults<T, M>,
    ) where
        T: LazyBalanceMarker + Clone,
        M: ManualBalanceModel<T> + BalanceStateHasher<T, M> + BalanceGuards<T, M> + Clone,
        T::Id: Default,
        T::Asset: From<T::Asset> + Into<T::Asset>,
        G: Fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool,
        F: Fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool,
    {
        explore_balance_states::<T, M, G, F, fn(&BalanceState<T, M>) -> u64>(
            users,
            deposits,
            adjustments,
            subjects,
            max_depth,
            allowed_bps,
            allowed_diff,
            overrides,
            None,
            results,
        );
    }

    /// Core state exploration engine for [`LazyBalance`].
    ///
    /// Performs **breadth-first** exploration over operation sequences,
    /// starting from an initial empty state.
    ///
    /// ```text
    /// -> [Deposit]   -> [Deposit, Mint] -> [Deposit, Mint, Withdraw]
    /// -> [Mint]      -> [Mint, Reap]
    /// -> [Reap]      -> ...
    /// -> [Withdraw]  -> ...
    /// -> [Drain]     -> ...
    /// ```
    ///
    /// Exploration proceeds level by level:
    /// - all sequences of length `n` are explored before `n+1`
    /// - each step extends the current **trace** by one [`BalanceOp`]
    /// - guards / traps / flow determine whether a branch continues
    ///
    /// At each step:
    /// - checks [`BalanceGuards::invariant`] on the current state
    /// - generates candidate operations (vector of [`BalanceOp`])
    /// - filters using:
    ///     - `guard_{op}(...) || trap(...)`
    ///     - `guard_flow_{op}(...) && trap_flow(...)`
    ///
    /// For each valid operation:
    /// - executes lazy model
    /// - applies manual model
    /// - updates state (receipts, trace)
    /// - evaluates withdrawal drift ([`compare_balances_values`])
    ///
    /// Uses:
    /// - visited set with [`BalanceStateHasher`] to avoid revisiting states
    /// - optional custom hasher for additional deduplication
    ///
    /// Records:
    /// - errors (failures)
    /// - exits (drift beyond tolerance)
    /// - successful paths
    ///
    /// This function defines the full execution semantics for:
    /// - guarded exploration
    /// - trap-based overrides
    /// - state-space traversal
    fn explore_balance_states<T, M, G, F, H>(
        users: &[M::User],
        deposits: &[T::Asset],
        adjustments: &[T::Asset],
        subjects: &[T::Subject],
        max_depth: u32,
        allowed_bps: u32,
        allowed_diff: u32,
        overrides: Option<BalanceTraps<G, F>>,
        hasher: Option<H>,
        results: &mut BalanceModelResults<T, M>,
    ) where
        T: LazyBalanceMarker + Clone,
        M: ManualBalanceModel<T> + BalanceStateHasher<T, M> + BalanceGuards<T, M> + Clone,
        T::Id: Default,
        T::Asset: From<T::Asset> + Into<T::Asset>,
        G: Fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool,
        F: Fn(&BalanceState<T, M>, &BalanceOp<T, M>) -> bool,
        H: Fn(&BalanceState<T, M>) -> u64,
    {
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        let mut rng = thread_rng();

        let traps = overrides.as_ref().map(|v| &v.reason);

        let flow_ok = |state: &BalanceState<T, M>, op: &BalanceOp<T, M>| {
            let base = <M as BalanceGuards<T, M>>::flow(state, op);

            if let Some(o) = &overrides {
                base && (o.flow)(state, op)
            } else {
                base
            }
        };

        let guard_ok = |state: &BalanceState<T, M>, op: &BalanceOp<T, M>| {
            let base = match op {
                BalanceOp::Deposit(u, v, l) => <M as BalanceGuards<T, M>>::deposit(state, u, v, l),
                BalanceOp::Withdraw(u) => <M as BalanceGuards<T, M>>::withdraw(state, u),
                BalanceOp::Mint(v, l) => <M as BalanceGuards<T, M>>::mint(state, v, l),
                BalanceOp::Reap(v, l) => <M as BalanceGuards<T, M>>::reap(state, v, l),
                BalanceOp::Drain => <M as BalanceGuards<T, M>>::drain(state),
            };

            let trap_override = overrides
                .as_ref()
                .map(|o| (o.trap)(state, op))
                .unwrap_or(false);

            base || trap_override
        };

        let push_trace = |trace: &[BalanceOp<T, M>], op| {
            let mut t = trace.to_vec();
            t.push(op);
            t
        };

        queue.push_back(BalanceState {
            lazy: LazyContainer::<T> {
                balance: Default::default(),
                variant: Default::default(),
                id: Default::default(),
            },
            manual: M::new(),
            receipts: BTreeMap::new(),
            trace: Vec::new(),
        });

        while let Some(state) = queue.pop_front() {
            if let Err(reason) = <M as BalanceGuards<T, M>>::invariant(&state) {
                let reason = reason.to_string();
                record_err(results, &state.trace, reason, traps);
                continue;
            }

            if state.trace.len() == max_depth as usize {
                record_pass(results, &state.trace, traps);
                continue;
            }

            if !visited.insert(<M as BalanceStateHasher<T, M>>::hash(&state)) {
                continue;
            }

            if let Some(ref h) = hasher {
                if !visited.insert(h(&state)) {
                    continue;
                }
            }

            // ---------- DEPOSIT ----------
            for &u in users {
                for _ in 0..deposits.len() {
                    if let Some(&amount) = deposits.choose(&mut rng) {
                        let sub = subjects.choose(&mut rng).cloned().unwrap_or_default();
                        if !guard_ok(&state, &BalanceOp::Deposit(u, amount, sub.clone())) {
                            continue;
                        }

                        if !flow_ok(&state, &BalanceOp::Deposit(u, amount, sub.clone())) {
                            continue;
                        }

                        let mut next = state.clone();

                        match deposit::<T>(&mut next.lazy, amount, &sub) {
                            Ok(pass) => {
                                if let Err(e) = next.manual.deposit(u, amount, &pass) {
                                    let trace = push_trace(
                                        &state.trace,
                                        BalanceOp::Deposit(u, amount, sub),
                                    );
                                    record_err(results, &trace, format!("{:?}", e), traps);
                                    continue;
                                }

                                let (actual, receipt) = pass;
                                next.receipts.insert(u, receipt);
                                next.trace.push(BalanceOp::Deposit(u, actual, sub));
                                queue.push_back(next);
                            }

                            Err(e) => {
                                let trace =
                                    push_trace(&state.trace, BalanceOp::Deposit(u, amount, sub));
                                record_err(results, &trace, format!("{:?}", e), traps);
                            }
                        }
                    }
                }
            }

            // ---------- WITHDRAW ----------
            for &u in users {
                if !guard_ok(&state, &BalanceOp::Withdraw(u)) {
                    continue;
                }

                if !flow_ok(&state, &BalanceOp::Withdraw(u)) {
                    continue;
                }

                let mut next = state.clone();

                let Some(receipt) = next.receipts.remove(&u) else {
                    let trace = push_trace(&next.trace, BalanceOp::Withdraw(u));
                    record_err(
                        results,
                        &trace,
                        "ModelChecker::WithdrawReceiptMissing".into(),
                        traps,
                    );
                    continue;
                };

                match withdraw::<T>(&mut next.lazy, receipt) {
                    Ok(lazy_value) => {
                        let manual_value = match next.manual.withdraw(u, &lazy_value) {
                            Ok(v) => v,
                            Err(e) => {
                                let trace = push_trace(&next.trace, BalanceOp::Withdraw(u));
                                record_err(results, &trace, format!("{:?}", e), traps);
                                continue;
                            }
                        };

                        let trace = push_trace(&next.trace, BalanceOp::Withdraw(u));

                        if let Err(exit) = compare_balances_values::<T, M>(
                            lazy_value,
                            manual_value,
                            &trace,
                            allowed_bps,
                            allowed_diff,
                        ) {
                            record_exit(results, exit);
                            continue;
                        }

                        next.trace = trace;
                        queue.push_back(next);
                    }

                    Err(e) => {
                        let trace = push_trace(&next.trace, BalanceOp::Withdraw(u));
                        record_err(results, &trace, format!("{:?}", e), traps);
                    }
                }
            }

            // ---------- MINT ----------
            for _ in 0..adjustments.len() {
                if let Some(&v) = adjustments.choose(&mut rng) {
                    let sub = subjects.choose(&mut rng).cloned().unwrap_or_default();
                    if !guard_ok(&state, &BalanceOp::Mint(v, sub.clone())) {
                        continue;
                    }

                    if !flow_ok(&state, &BalanceOp::Mint(v, sub.clone())) {
                        continue;
                    }

                    let mut next = state.clone();

                    match mint::<T>(&mut next.lazy, v, &sub) {
                        Ok(pass) => {
                            if let Err(e) = next.manual.mint(v, &pass) {
                                let trace = push_trace(&next.trace, BalanceOp::Mint(v, sub));
                                record_err(results, &trace, format!("{:?}", e), traps);
                                continue;
                            }

                            next.trace.push(BalanceOp::Mint(pass, sub));
                            queue.push_back(next);
                        }

                        Err(e) => {
                            let trace = push_trace(&next.trace, BalanceOp::Mint(v, sub));
                            record_err(results, &trace, format!("{:?}", e), traps);
                        }
                    }
                }
            }

            // ---------- REAP ----------
            for _ in 0..adjustments.len() {
                if let Some(&v) = adjustments.choose(&mut rng) {
                    let sub = subjects.choose(&mut rng).cloned().unwrap_or_default();
                    if !guard_ok(&state, &BalanceOp::Reap(v, sub.clone())) {
                        continue;
                    }

                    if !flow_ok(&state, &BalanceOp::Reap(v, sub.clone())) {
                        continue;
                    }

                    let mut next = state.clone();

                    match reap::<T>(&mut next.lazy, v, &sub) {
                        Ok(pass) => {
                            if let Err(e) = next.manual.reap(v, &pass) {
                                let trace = push_trace(&next.trace, BalanceOp::Reap(v, sub));
                                record_err(results, &trace, format!("{:?}", e), traps);
                                continue;
                            }

                            next.trace.push(BalanceOp::Reap(pass, sub));
                            queue.push_back(next);
                        }

                        Err(e) => {
                            let trace = push_trace(&next.trace, BalanceOp::Reap(v, sub));
                            record_err(results, &trace, format!("{:?}", e), traps);
                        }
                    }
                }
            }

            // ---------- DRAIN ----------
            if flow_ok(&state, &BalanceOp::Drain) && guard_ok(&state, &BalanceOp::Drain) {
                let mut next = state.clone();

                match drain::<T>(&mut next.lazy) {
                    Ok(_) => {
                        if let Err(e) = next.manual.drain() {
                            let trace = push_trace(&next.trace, BalanceOp::Drain);
                            record_err(results, &trace, format!("{:?}", e), traps);
                            continue;
                        }

                        next.trace.push(BalanceOp::Drain);
                        queue.push_back(next);
                    }

                    Err(e) => {
                        let trace = push_trace(&next.trace, BalanceOp::Drain);
                        record_err(results, &trace, format!("{:?}", e), traps);
                    }
                }
            }
        }
    }

    /// Writes model exploration results to CSV files.
    ///
    /// Creates a directory (if not present) and writes categorized outputs:
    ///
    /// - **fail** (always written) sequences that resulted in errors
    /// - **pass** (optional) successful sequences
    /// - **trap** (optional) sequences matching expected trap failures
    /// - **exit** (optional) sequences with drift between models
    ///
    /// File names are timestamped to avoid overwrites.
    ///
    /// Each CSV contains:
    /// - a unique ID (`sl_no`)
    /// - relevant fields (reason, values, sequence)
    ///
    /// If all result categories are empty, no files are written.
    ///
    /// Also prints a summary of counts:
    /// - trap mode includes trapped count
    /// - normal mode includes fail / pass / exit counts
    fn balance_states_csv_reports<T: LazyBalanceMarker, M: ManualBalanceModel<T>>(
        dir: PathBuf,
        results: &BalanceModelResults<T, M>,
        write_pass: bool,
        write_exit: bool,
        write_trap: bool,
    ) {
        if results.fail.is_empty()
            && results.pass.is_empty()
            && results.trap.is_empty()
            && results.exit.is_empty()
        {
            return;
        }

        let fuzz_dir = dir;

        create_dir_all(&fuzz_dir).unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // ---------- FAIL CSV (MANDATORY) ----------
        if !results.fail.is_empty() {
            let fail_path = fuzz_dir.join(format!("fail_{}.csv", now));
            let mut fail_file = File::create(&fail_path).unwrap();

            writeln!(fail_file, "sl_no,reason,step,sequence").unwrap();

            for (id, f) in &results.fail {
                writeln!(fail_file, "{},\"{}\",\"{:?}\"", id, f.reason, f.sequence).unwrap();
            }

            // println!("Fail CSV: {}", fail_path.display());
        }

        // ---------- PASS CSV (OPTIONAL) ----------
        if write_pass && !results.pass.is_empty() {
            let pass_path = fuzz_dir.join(format!("pass_{}.csv", now));
            let mut pass_file = File::create(&pass_path).unwrap();

            writeln!(pass_file, "sl_no,sequence").unwrap();

            for (id, seq) in &results.pass {
                writeln!(pass_file, "{},\"{:?}\"", id, seq).unwrap();
            }

            // println!("Pass CSV: {}", pass_path.display());
        }

        // ---------- TRAP CSV (OPTIONAL) ----------
        if write_trap && !results.trap.is_empty() {
            let trap_path = fuzz_dir.join(format!("trap_{}.csv", now));
            let mut trap_file = File::create(&trap_path).unwrap();

            writeln!(trap_file, "sl_no,reason,sequence").unwrap();

            for (id, reason, seq) in &results.trap {
                writeln!(trap_file, "{},\"{}\",\"{:?}\"", id, reason, seq).unwrap();
            }

            // println!("Trap CSV: {}", trap_path.display());
        }

        // ---------- EXIT CSV (OPTIONAL) ----------
        if write_exit && !results.exit.is_empty() {
            let exit_path = fuzz_dir.join(format!("exit_{}.csv", now));
            let mut exit_file = File::create(&exit_path).unwrap();

            writeln!(exit_file, "sl_no,lazy,manual,diff,bps,sequence").unwrap();

            for (id, exit) in &results.exit {
                writeln!(
                    exit_file,
                    "{},{:?},{:?},{:?},{:?},\"{:?}\"",
                    id, exit.lazy, exit.manual, exit.diff, exit.bps, exit.sequence
                )
                .unwrap();
            }

            // println!("BalanceExit CSV: {}", exit_path.display());
        }

        // ---------- SUMMARY ----------
        if !results.trap.is_empty() {
            println!(
                "Trap check completed: {} failed, {} passed, {} exited, {} trapped",
                results.fail.len(),
                results.pass.len(),
                results.exit.len(),
                results.trap.len(),
            );
        } else {
            println!(
                "Model check completed: {} failed, {} passed, {} exited",
                results.fail.len(),
                results.pass.len(),
                results.exit.len(),
            );
        }
    }

    // ===============================================================================
    // ```````````````````````` MODEL CHECKER INTERNAL HELPERS ```````````````````````
    // ===============================================================================

    /// Compares lazy and manual withdrawal results and evaluates drift.
    ///
    /// Computes the absolute difference (`diff`) between lazy and manual
    /// values and checks it against allowed tolerance thresholds:
    ///
    /// - Ignores negligible differences (diff <= 1) (rounding differences)
    /// - Computes basis points (bps) relative to the larger value
    /// - Accepts results within `allowed_bps` or `allowed_diff` thresholds
    ///
    /// If the deviation exceeds both thresholds, returns a [`BalanceExit`]
    /// containing:
    /// - lazy and manual values
    /// - absolute difference
    /// - basis points difference
    /// - execution trace (sequence)
    ///
    /// This is used to validate that the lazy model maintains acceptable
    /// precision relative to the manual reference model.
    fn compare_balances_values<T: LazyBalanceMarker, M: ManualBalanceModel<T>>(
        lazy: T::Asset,
        manual: T::Asset,
        seq: &[BalanceOp<T, M>],
        allowed_bps: u32,
        allowed_diff: u32,
    ) -> Result<(), BalanceExit<T, M>> {
        let lazy = lazy.into();
        let diff = if lazy > manual {
            lazy - manual
        } else {
            manual - lazy
        };

        if diff <= One::one() {
            return Ok(());
        }

        let base = lazy.max(manual).max(1u8.into());

        if diff * 10_000u32.into() > base * allowed_bps.into() {
            if diff <= allowed_diff.into() {
                return Ok(());
            }
            return Err(BalanceExit::<T, M> {
                lazy,
                manual,
                diff,
                bps: diff * 10_000u32.into() / base,
                sequence: seq.to_vec(),
            });
        }

        Ok(())
    }

    /// Records a failed execution for the given trace.
    ///
    /// Wraps the reason and trace into a [`BalanceFailure`] and stores it.
    /// If traps are active, records it as a trapped failure.
    fn record_err<T: LazyBalanceMarker, M: ManualBalanceModel<T>>(
        results: &mut BalanceModelResults<T, M>,
        trace: &[BalanceOp<T, M>],
        reason: String,
        traps: Option<&String>,
    ) {
        let failure = BalanceFailure {
            reason,
            sequence: trace.to_vec(),
        };

        match traps {
            Some(e) => results.record_trapped(trace, Err(failure), Some(e)),
            None => results.record(trace, Err(failure)),
        }
    }

    /// Records a successful execution for the given trace.
    ///
    /// Marks the sequence as passed. Behavior is identical regardless of traps.
    fn record_pass<T: LazyBalanceMarker, M: ManualBalanceModel<T>>(
        results: &mut BalanceModelResults<T, M>,
        trace: &[BalanceOp<T, M>],
        traps: Option<&String>,
    ) {
        match traps {
            Some(_) => results.record(trace, Ok(())),
            None => results.record(trace, Ok(())),
        }
    }

    /// Records an exit (drift) between lazy and manual models.
    ///
    /// Stores the [`BalanceExit`] for later analysis.
    fn record_exit<T: LazyBalanceMarker, M: ManualBalanceModel<T>>(
        results: &mut BalanceModelResults<T, M>,
        exit: BalanceExit<T, M>,
    ) {
        results.exit_record(exit)
    }

    // ===============================================================================
    // ```````````````````````````` MODEL CHECKER RESULTS ````````````````````````````
    // ===============================================================================

    /// Aggregates results produced during state exploration.
    ///
    /// Stores categorized outcomes for executed sequences:
    /// - `pass`: successful sequences
    /// - `fail`: sequences that resulted in an error ([`BalanceFailure`])
    /// - `exit`: sequences with drift between models ([`BalanceExit`])
    /// - `trap`: sequences that triggered trap conditions
    ///
    /// Each entry is paired with a unique identifier for tracking.
    ///
    /// The `next_*_id` fields maintain incremental IDs for each category.
    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct BalanceModelResults<T: LazyBalanceMarker, M: ManualBalanceModel<T>> {
        /// Successful sequences.
        pub pass: Vec<(usize, Vec<BalanceOp<T, M>>)>,

        /// Failed sequences with associated error.
        pub fail: Vec<(usize, BalanceFailure<T, M>)>,

        /// Drifted sequences between lazy and manual models.
        pub exit: Vec<(usize, BalanceExit<T, M>)>,

        /// Trap-triggered sequences with reason.
        pub trap: Vec<(usize, String, Vec<BalanceOp<T, M>>)>,

        /// Next identifier for pass entries.
        pub next_pass_id: usize,

        /// Next identifier for exit entries.
        pub next_exit_id: usize,

        /// Next identifier for fail entries.
        pub next_fail_id: usize,

        /// Next identifier for trap entries.
        pub next_trap_id: usize,
    }

    impl<T: LazyBalanceMarker, M: ManualBalanceModel<T>> BalanceModelResults<T, M> {
        /// Creates a new empty result container with initialized IDs.
        fn new() -> Self {
            Self {
                pass: Vec::new(),
                fail: Vec::new(),
                exit: Vec::new(),
                trap: Vec::new(),
                next_pass_id: 1,
                next_exit_id: 1,
                next_fail_id: 1,
                next_trap_id: 1,
            }
        }

        /// Records an exit (drift) with a unique ID.
        fn exit_record(&mut self, exit: BalanceExit<T, M>) {
            let id = self.next_exit_id;
            self.next_exit_id += 1;
            self.exit.push((id, exit));
        }

        /// Records a result without trap context.
        /// Delegates to [`Self::record_trapped`] with no trap expectation.
        fn record(&mut self, seq: &[BalanceOp<T, M>], result: Result<(), BalanceFailure<T, M>>) {
            self.record_trapped(seq, result, None);
        }

        /// Records a result with optional trap expectation.
        /// - `Ok`: stored as pass
        /// - `Err`:
        ///     - if matches expected trap reason, stored as trap
        ///     - otherwise, stored as failure
        fn record_trapped(
            &mut self,
            seq: &[BalanceOp<T, M>],
            result: Result<(), BalanceFailure<T, M>>,
            traps: Option<&String>,
        ) {
            match result {
                Ok(_) => {
                    let id = self.next_pass_id;
                    self.next_pass_id += 1;

                    self.pass.push((id, seq.to_vec()));
                }

                Err(f) => {
                    if let Some(trap) = traps {
                        if *trap == f.reason {
                            // Expected trap
                            let id = self.next_trap_id;
                            self.next_trap_id += 1;

                            self.trap.push((id, f.reason.clone(), seq.to_vec()));

                            return;
                        }
                    }

                    let id = self.next_fail_id;
                    self.next_fail_id += 1;

                    self.fail.push((id, f.clone()));
                }
            }
        }
    }

    // ===============================================================================
    // `````````````````````` LAZY BALANCE CONVENIENCE CALLERS ```````````````````````
    // ===============================================================================

    /// Executes a deposit on the lazy balance model using the provided container.
    ///
    /// Constructs the required tagged input, invokes [`LazyBalance::deposit`], and
    /// extracts the resolved (asset, receipt) output.
    ///
    /// Returns the actual deposited value and issued receipt.
    fn deposit<'a, T: LazyBalanceMarker>(
        model: &'a mut LazyContainer<T>,
        value: T::Asset,
        subject: &'a T::Subject,
    ) -> Result<(T::Asset, T::Receipt), Error<T>> {
        let input = <T::Input<'_> as FromTag<_, Deposit>>::from_tag((
            MutHandle::Borrowed(&mut model.balance),
            Cow::Borrowed(&model.variant),
            Cow::Borrowed(&model.id),
            Cow::Owned(value),
            Cow::Borrowed(subject),
        ));

        let raw = T::deposit(input);

        let Ok(result) = TryIntoTag::<_, Deposit>::try_into_tag(raw) else {
            unreachable!()
        };

        match result {
            Ok((asset, receipt)) => Ok((asset.into_owned(), receipt.into_owned())),
            Err(e) => Err(e),
        }
    }

    /// Executes a withdrawal on the lazy balance model.
    ///
    /// Consumes the provided receipt, invokes [`LazyBalance::withdraw`], and
    /// returns the resolved asset value.
    fn withdraw<'a, T: LazyBalanceMarker>(
        model: &'a mut LazyContainer<T>,
        receipt: T::Receipt,
    ) -> Result<T::Asset, Error<T>> {
        let input = <T::Input<'_> as FromTag<_, Withdraw>>::from_tag((
            MutHandle::Borrowed(&mut model.balance),
            Cow::Borrowed(&model.variant),
            Cow::Borrowed(&model.id),
            Cow::Owned(receipt),
        ));

        let raw = T::withdraw(input);

        let Ok(result) = TryIntoTag::<_, Withdraw>::try_into_tag(raw) else {
            unreachable!()
        };

        match result {
            Ok(v) => Ok(v.into_owned()),
            Err(e) => Err(e),
        }
    }

    /// Executes a mint operation on the lazy balance model.
    ///
    /// Constructs the tagged input, invokes [`LazyBalance::mint`], and returns
    /// the resolved asset value added to the balance.
    fn mint<'a, T: LazyBalanceMarker>(
        model: &'a mut LazyContainer<T>,
        value: T::Asset,
        subject: &'a T::Subject,
    ) -> Result<T::Asset, Error<T>> {
        let input = <T::Input<'_> as FromTag<_, Mint>>::from_tag((
            MutHandle::Borrowed(&mut model.balance),
            Cow::Borrowed(&model.variant),
            Cow::Borrowed(&model.id),
            Cow::Owned(value),
            Cow::Borrowed(subject),
        ));

        let raw = T::mint(input);

        let Ok(result) = TryIntoTag::<_, Mint>::try_into_tag(raw) else {
            unreachable!()
        };

        match result {
            Ok(v) => Ok(v.into_owned()),
            Err(e) => Err(e),
        }
    }

    /// Executes a reap operation on the lazy balance model.
    ///
    /// Constructs the tagged input, invokes [`LazyBalance::reap`], and returns
    /// the resolved asset value removed or adjusted from the balance.
    fn reap<'a, T: LazyBalanceMarker>(
        model: &'a mut LazyContainer<T>,
        value: T::Asset,
        subject: &'a T::Subject,
    ) -> Result<T::Asset, Error<T>> {
        let input = <T::Input<'_> as FromTag<_, Reap>>::from_tag((
            MutHandle::Borrowed(&mut model.balance),
            Cow::Borrowed(&model.variant),
            Cow::Borrowed(&model.id),
            Cow::Owned(value),
            Cow::Borrowed(subject),
        ));

        let raw = T::reap(input);

        let Ok(result) = TryIntoTag::<_, Reap>::try_into_tag(raw) else {
            unreachable!()
        };

        match result {
            Ok(v) => Ok(v.into_owned()),
            Err(e) => Err(e),
        }
    }

    /// Executes a drain operation on the lazy balance model.
    ///
    /// Constructs the tagged input, invokes [`LazyBalance::drain`].
    ///
    /// Clears the balance state and returns the resolved drained value.
    fn drain<'a, T: LazyBalanceMarker>(
        model: &'a mut LazyContainer<T>,
    ) -> Result<T::Asset, Error<T>> {
        let input = <T::Input<'_> as FromTag<_, Drain>>::from_tag((
            MutHandle::Borrowed(&mut model.balance),
            Cow::Borrowed(&model.variant),
            Cow::Borrowed(&model.id),
        ));

        let raw = <T as LazyBalance>::drain(input);

        let Ok(result) = TryIntoTag::<_, Drain>::try_into_tag(raw) else {
            unreachable!()
        };

        match result {
            Ok(v) => Ok(v.into_owned()),
            Err(e) => Err(e),
        }
    }
}
