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
// ````````````````````````````` LAZY BALANCE PLUGINS ````````````````````````````
// ===============================================================================

//! Lazy balance plugin families built on top of
//! [`LazyBalanceRoot`](frame_suite::assets::LazyBalanceRoot).
//!
//! This module defines reusable `plugin families` that implement different
//! lazy balance models using the [`LazyBalance`](frame_suite::assets::LazyBalance)
//! interface.
//!
//! Each family provides:
//! - execution logic (deposit, withdraw, mint, reap, drain)
//! - validation (`Can*` plugins)
//! - read-only queries
//! - [`virtual balance`](frame_suite::virtuals)
//! structure accessors.
//!
//! Use a specific family (e.g. [`ShareBalanceFamily`]) together with its
//! context to integrate a concrete lazy balance model.

// ===============================================================================
// ```````````````````````````````` SHARE-BALANCE ````````````````````````````````
// ===============================================================================
pub use share_balance::*;

/// Share-based lazy balance model implementation.
///
/// Provides [`ShareBalanceFamily`] and [`ShareBalanceContext`] for
/// a proportional ownership (shares) based
/// [`LazyBalance`](frame_suite::assets::LazyBalance) model.
///
/// Use:
/// - [`ShareBalanceFamily`]: plugin family (execution + validation)
/// - [`ShareBalanceContext`]: context binding for the implementation
///
/// This model tracks ownership via shares and resolves value lazily
/// at withdrawal time.
mod share_balance {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- Core (Rust std replacement) ---
    use core::marker::PhantomData;

    // --- Scale-codec crates ---
    use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
    use scale_info::TypeInfo;

    // --- FRAME Suite ---
    use frame_suite::{
        assets::*, define_family, empty_virtual_extension, misc::Directive, mutation::MutHandle, plugin_model,
        virtuals::*,
    };

    // --- FRAME Support ---
    use frame_support::traits::tokens::{Fortitude, Precision};

    // --- Substrate primitives ---
    use sp_core::ConstU32;
    use sp_runtime::{
        traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Zero},
        Cow, DispatchError, FixedPointNumber, Saturating, Vec,
    };

    // ===============================================================================
    // ````````````````````````` SHARE-BALANCE PLUGIN FAMILY `````````````````````````
    // ===============================================================================

    define_family! {
        // Root trait defining the LazyBalance execution surface
        root: LazyBalanceRoot,

        /// Plugin family implementing a **share-based lazy balance model**.
        ///
        /// ## Model
        ///
        /// Value is tracked via **shares (proportional ownership)**:
        ///
        /// ```text
        /// deposit  -> mint shares
        /// mint/reap -> mutate balance state
        /// withdraw -> redeem shares at current share-price
        /// ```
        ///
        /// - Receipts encode **shares**, not fixed value
        /// - Balance mutations affect only the given **balance state**
        /// - Final value is resolved **lazily at withdrawal**
        ///
        /// ## Complexity
        ///
        /// All operations are **O(1)**:
        /// - no iteration or global recomputation
        ///
        /// ## Constraints
        ///
        /// Operations are intentionally **unbounded**:
        /// - `deposit`, `mint`, `reap` has no intrinsic limits
        ///
        /// Minimal invariants:
        ///
        /// - No `deposit` after a full drain (complete reap)
        ///   - requires a `mint` to reinitialize the balance
        /// - No `mint` or `reap` before any deposit exists
        ///
        /// ## Edge Conditions
        ///
        /// Arbitrary or unstructured use of `mint`/`reap`
        /// (e.g. without a consistent economic model) can lead to
        /// **skewed redemption outcomes** at withdrawal which reflects
        /// **misuse**, not a violation of system invariants.
        ///
        /// The design permits unrestricted operations, but assumes
        /// coherent, policy-driven execution in production
        ///
        /// ## Dust Handling
        ///
        /// Any residual balance ("dust") caused by rounding or share
        /// precision is not redistributed.
        ///
        /// The **last withdrawer of the final receipt** receives the
        /// entire remaining dust in the balance.
        ///
        /// ## Lifetime
        ///
        /// The lifetime `'a` ties plugin execution to the caller's borrow scope,
        /// allowing a mutable reference to the `balance` to be passed into the plugin
        /// and safely used across operation boundaries.
        family: pub ShareBalanceFamily,

        // Lifetimes for mutable borrow of (balance) carried by the family marker
        // (execution-time borrowing)
        // Propagated through LazyBalance::Input/Output into all models
        borrow: ['a],

        // Input / Output carriers (discriminanted-dispatched across plugins)
        input: In,
        output: Out,

        // Context type providing layout, environment, and error mapping
        context: ShareBalanceContext<T>,

        // Generics applied on the impl (context specialization)
        // T binds the LazyBalance implementation into the context
        marker: [T],

        bounds: [
            // Core contract: provides associated types + execution interface
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,

            // Error translation layer across all plugin executions
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,

            // Output carrier must support all operation result shapes
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,

            // Input carrier must support all operation parameter shapes
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,

            // Required for fixed-point -> asset conversion (withdraw path)
            T::Asset: From<<T::Rational as FixedPointNumber>::Inner>,
        ],

        child: [
            // --- State mutations (modify balance state) ---
            Deposit => ModelDeposit,     // issues shares for value
            Mint => ModelMint,           // increases price per share (bias +)
            Reap => ModelReap,           // decreases price per share (bias -)
            Withdraw => ModelWithdraw,   // resolves receipt -> burns shares
            Drain => ModelDrain,         // full depletion (Reap(total_value))

            // --- Validation (pure pre-checks, no mutation) ---
            CanDeposit => ModelCanDeposit,
            CanMint => ModelCanMint,
            CanReap => ModelCanReap,
            CanWithdraw => ModelCanWithdraw,

            // --- Read-only queries (state inspection) ---
            TotalValue => ModelTotalValue,                   // total balance value
            ReceiptActiveValue => ModelReceiptActiveValue,   // simulated withdraw value
            HasDeposits => ModelHasDeposits,                 // issued > 0
            ReceiptDepositValue => ModelReceiptDepositValue, // original deposit value

            // --- Limits (policy layer: unbounded/permissive in this model) ---
            DepositLimits => ModelDepositLimits,
            MintLimits => ModelMintLimits,
            ReapLimits => ModelReapLimits,
        ]
    }

    // ===============================================================================
    // ```````````````````````` SHARE-BALANCE INTERNAL HELPERS ```````````````````````
    // ===============================================================================

    /// Advances the checkpoint on balance updates (mint/reap).
    ///
    /// The checkpoint is a monotonically increasing counter that represents
    /// logical time. It is incremented whenever the balance changes, i.e.,
    /// when the share price (bias) is updated which helps track when deposits
    /// were made relative to balance state changes.
    ///
    /// This is mainly useful for handling drain scenarios. If a full reap
    /// (drain) occurs after a deposit, earlier receipts may no longer
    /// be meaningful. A drain resets the share price (bias) to zero, so
    /// during withdrawal the receipt's original pricing context becomes
    /// outdated, even though the shares still exist.
    ///
    /// The checkpoint and drain point (stored in both balance and receipt)
    /// allow withdrawals to efficiently detect and handle such cases.
    ///
    /// This keeps withdrawal logic simple and `O(1)` while maintaining
    /// correctness across balance resets.
    fn balance_checkpoint<T: LazyBalance>(
        balance: &mut T::Balance,
    ) -> Result<(), ShareBalanceError> {
        let checkpoint = balance::checkpoint::<T>(balance)
            .ok_or(ShareBalanceError::BalanceNotInitiatedViaDeposit)?;

        let bias =
            balance::bias::<T>(balance).ok_or(ShareBalanceError::BalanceNotInitiatedViaDeposit)?;

        if bias.is_zero() {
            balance::set_drainpoint::<T>(balance, checkpoint.saturating_add(One::one()))?;
        }

        balance::set_checkpoint::<T>(balance, checkpoint.saturating_add(One::one()))?;

        Ok(())
    }

    // ===============================================================================
    // ```````````````````````````````` PLUGIN MODELS ````````````````````````````````
    // ===============================================================================

    /// Plugin execution context for [`ShareBalanceFamily`].
    ///
    /// The generic `T` is expected to be a concrete implementation of
    /// [`LazyBalance`], defining the core types this context operates on.
    ///
    /// Implements [`LazyBalanceContext`], providing bounds, extension schemas,
    /// and error typing required by the balance model.
    pub struct ShareBalanceContext<T>(pub PhantomData<T>);

    plugin_model!(

        /// [`Deposit`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::Deposit`].
        ///
        /// ## Overview
        ///
        /// Accepts an asset deposit and issues a [`LazyBalance::Receipt`]
        /// representing a proportional claim over the balance along with
        /// the total deposited amount.
        ///
        /// Effects:
        /// - increases `effective`
        /// - increases `issued` (shares)
        /// - returns a receipt encoding the depositor's stake
        ///
        /// ## Share Derivation
        ///
        /// Shares are computed relative to current state:
        ///
        /// - `issued == 0` -> shares = asset (bootstrap)
        /// - otherwise     -> shares = asset / bias (share-price)
        ///
        /// where `bias = effective / issued`.
        ///
        /// Rational results are floored when converting to integer shares,
        /// preventing implicit value creation and allowing fractional
        /// dust to accumulate.
        ///
        /// ## Constraints
        ///
        /// - zero-value deposits are rejected
        /// - fresh balances are initialized on first deposit
        /// - deposits are disallowed if `bias == 0 && issued != 0`
        ///   (fully drained state; requires mint to recover the bias)
        ///
        /// ## Receipt
        ///
        /// The issued [`LazyBalance::Receipt`] contains:
        /// - `principal` : deposited value
        /// - `shares`    : issued shares
        /// - `bias`      : balance bias
        /// - `checkpoint`: balance checkpoint
        ///
        /// Receipts encode relative ownership, not fixed value.
        name: pub ModelDeposit,
        input: In,
        output: Out,
        others: ['a, T],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
            T::Asset: From<<T::Rational as FixedPointNumber>::Inner>,
        ],
        compute: |input, _context| {
            let Ok((mut balance, variant, id, asset, subject)) = TryIntoTag::<_, Deposit>::try_into_tag(input) else {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            let balance = &mut balance;

            // Deposit amount must be non-zero
            if asset.is_zero() {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::ZeroDepositNotAllowed))
            }

            // Initialize balance if this is the first interaction
            if let Err(e) = balance::is_fresh_balance::<T>(balance) {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(e))
            }

            let Some(bias) = balance::bias::<T>(balance) else {
                // unreachable unless balance virtual dyn field is corrupted
                debug_assert!(
                    false,
                    "corrupted virtual dyn field balance::bias during \
                    ShareBalanceFamily::Deposit for id {:?}, variant {:?} \
                    amount {:?} subject {:?}",
                    variant, id, asset, subject
                );
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::CorruptedVirtualField));
            };

            let Some(effective) = balance::effective::<T>(balance) else {
                // unreachable unless balance virtual dyn field is corrupted
                debug_assert!(
                    false,
                    "corrupted virtual dyn field balance::effective during \
                    ShareBalanceFamily::Deposit for id {:?}, variant {:?} \
                    amount {:?} subject {:?}",
                    variant, id, asset, subject
                );
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::CorruptedVirtualField));
            };

            let Some(issued) = balance::issued::<T>(balance) else {
                // unreachable unless balance virtual dyn field is corrupted
                debug_assert!(
                    false,
                    "corrupted virtual dyn field balance::issued during \
                    ShareBalanceFamily::Deposit for id {:?}, variant {:?} \
                    amount {:?} subject {:?}",
                    variant, id, asset, subject
                );
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::CorruptedVirtualField));
            };

            // Disallow deposits on drained balance (bias == 0)
            // Requires mint to re-establish share pricing
            if bias.is_zero() {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::BalanceDrainedCannotDeposit))
            }

            // Derive shares:
            // - bootstrap: 1:1 mapping
            // - otherwise: asset / bias (price per share)
            let shares = match issued.is_zero() {
                true => *asset,
                false => {
                    let Some(div) = T::Rational::saturating_from_integer(*asset).checked_div(&bias) else {
                        // invalid bias state (overflow / underflow domain)
                        return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::InadequatePrecision))
                    };
                    let Some(actual) = div.into_inner().checked_div(&T::Rational::DIV) else {
                        // unreachable unless DIV is zero
                        debug_assert!(
                            false,
                            "divide by zero during fixed-point scaling during \
                            ShareBalanceFamily::Deposit for id {:?}, variant {:?} \
                            amount {:?} subject {:?}",
                            variant, id, asset, subject
                        );
                        return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::FixedPointScalingFailed))
                    };
                    // implicit flooring -> prevents value creation, accumulates fractional dust
                    actual.into()
                }
            };

            // Reject deposits that resolve to zero shares (too small may be under one due to flooring)
            if shares.is_zero() {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::LessThanOneShareDerived))
            }

            // Update effective balance (real value)
            let Some(new_effective) = effective.checked_add(&asset) else {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::AssetOverflow))
            };

            // Update total issued shares
            let Some(new_issued) = issued.checked_add(&shares) else {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::SharesOverflow))
            };

            if let Err(e) = balance::set_effective::<T>(balance, new_effective) {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(e));
            };

            if let Err(e) = balance::set_issued::<T>(balance, new_issued) {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(e));
            };

            // Create new receipt (initially empty virtual struct)
            let mut receipt = T::Receipt::default();

            // Capture checkpoint -> anchors future withdrawal derivation
            let Some(checkpoint) = balance::checkpoint::<T>(balance) else {
                // unreachable unless balance virtual dyn field is corrupted
                debug_assert!(
                    false,
                    "corrupted virtual dyn field balance::checkpoint during \
                    ShareBalanceFamily::Deposit for id {:?}, variant {:?} \
                    amount {:?} subject {:?}",
                    variant, id, asset, subject
                );
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(ShareBalanceError::CorruptedVirtualField));
            };

            // Store original deposit value
            if let Err(e) = receipt::set_principal::<T>(&mut receipt, *asset) {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(e));
            };

            // Store issued shares at deposit time
            if let Err(e) = receipt::set_shares::<T>(&mut receipt, shares) {
                return <Out as FromTag::<_, Deposit>>::from_tag(Err(e));
            };

            // Store pricing context
            receipt::set_bias::<T>(&mut receipt, bias);

            // Store time anchor
            receipt::set_checkpoint::<T>(&mut receipt, checkpoint);

            // Return (deposited asset, receipt)
            <Out as FromTag::<_, Deposit>>::from_tag(Ok((asset, Cow::Owned(receipt))))
        }
    );

    plugin_model!(
        /// [`CanDeposit`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::CanDeposit`].
        ///
        /// ## Overview
        ///
        /// Performs pre-checks to determine whether a deposit is allowed,
        /// without mutating balance state.
        ///
        /// ## Validation Rules
        ///
        /// - deposit amount must be non-zero
        /// - addition to balance's `effective` must not overflow
        /// - deposits are disallowed if `bias == 0 && issued != 0`
        ///   (balance is fully drained and requires minting)
        ///
        /// ## Fresh Balance
        ///
        /// If the balance is uninitialized (default `effective` missing),
        /// the deposit is considered valid and initialization is deferred
        /// to the deposit operation itself.
        ///
        /// ## Semantics
        ///
        /// This check mirrors [`ModelDeposit`] model constraints without applying
        /// state transitions, ensuring that execution will succeed if
        /// validation passes.
        name: pub ModelCanDeposit,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {

            let Ok((balance, variant, id, asset, subject)) = TryIntoTag::<_, CanDeposit>::try_into_tag(input) else {
                return <Out as FromTag::<_, CanDeposit>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Fresh balance -> allow deposit (initialization deferred to execution)
            let Some(effective) = balance::effective::<T>(&balance) else {
                return <Out as FromTag::<_, CanDeposit>>::from_tag(Ok(()))
            };

            let Some(issued) = balance::issued::<T>(&balance) else {
                // unreachable unless balance virtual dyn field is corrupted
                debug_assert!(
                    false,
                    "corrupted virtual dyn field balance::issued during \
                    ShareBalanceFamily::CanDeposit for id {:?}, variant {:?} \
                    amount {:?} subject {:?}",
                    variant, id, asset, subject
                );
                return <Out as FromTag::<_, CanDeposit>>::from_tag(Err(ShareBalanceError::CorruptedVirtualField))
            };

            let Some(bias) = balance::bias::<T>(&balance) else {
                // unreachable unless balance virtual dyn field is corrupted
                debug_assert!(
                    false,
                    "corrupted virtual dyn field balance::bias during \
                    ShareBalanceFamily::CanDeposit for id {:?}, variant {:?} \
                    amount {:?} subject {:?}",
                    variant, id, asset, subject
                );
                return <Out as FromTag::<_, CanDeposit>>::from_tag(Err(ShareBalanceError::CorruptedVirtualField))
            };

            // Disallow deposits on bankrupt i.e., drained balance (bias == 0 && issued != 0)
            if bias.is_zero() && !issued.is_zero() {
                return <Out as FromTag::<_, CanDeposit>>::from_tag(
                    Err(ShareBalanceError::BalanceDrainedCannotDeposit)
                )
            }

            // Ensure deposit does not overflow effective balance
            if effective.checked_add(&asset).is_none() {
                return <Out as FromTag::<_, CanDeposit>>::from_tag(
                    Err(ShareBalanceError::AssetOverflow)
                )
            }

            <Out as FromTag::<_, CanDeposit>>::from_tag(Ok(()))
        }
    );

    plugin_model!(
        /// [`Mint`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::Mint`].
        ///
        /// ## Overview
        ///
        /// Introduces new value into the balance without issuing new shares.
        ///
        /// This increases balance's `effective` while keeping `issued` untouched,
        /// thereby increasing `bias` (value per share).
        ///
        /// ## Effect
        ///
        /// - `effective += asset`
        /// - `issued` remains unchanged
        /// - `bias = effective / issued` is recomputed
        ///
        /// This distributes value proportionally across all existing shares
        /// implicitly and lazily acquired during withdrawal.
        ///
        /// ## Constraints
        ///
        /// - zero-value mint is a no-op
        /// - balance must not be fresh (must be initialized)
        /// - balance must have existing deposits (`issued > 0`)
        /// - addition to `effective` must not overflow
        ///
        /// ## Semantics
        ///
        /// Minting represents an external value injection:
        /// - no new ownership is created
        /// - all existing receipts gain value proportionally lazily
        ///
        /// This is the only way to revive a fully drained balance
        /// (where `bias == 0 && issued != 0`).
        name: pub ModelMint,
        input: In,
        output: Out,
        others: ['a, T,],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {

            let Ok((mut balance, _variant, _id, asset, _subject)) = TryIntoTag::<_, Mint>::try_into_tag(input) else {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Zero mint -> no-op
            if asset.is_zero() {
                return <Out as FromTag::<_, Mint>>::from_tag(Ok(Cow::Owned(Zero::zero())));
            }

            let balance = &mut balance;

            // Uninitialized balance
            let Some(effective) = balance::effective::<T>(balance) else {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            // Uninitialized balance
            let Some(issued) = balance::issued::<T>(balance) else {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            // Mint requires existing shares (cannot bootstrap)
            if issued.is_zero() {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(ShareBalanceError::RequiresExistingDeposits))
            };

            // Increase effective balance
            let Some(new_effective) = effective.checked_add(&asset) else {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(ShareBalanceError::AssetOverflow))
            };

            // Recompute price per share (bias)
            let Some(new_bias) = T::Rational::checked_from_rational(new_effective, issued) else {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(ShareBalanceError::InadequatePrecision))
            };

            // Apply state updates
            if let Err(e) = balance::set_effective::<T>(balance, new_effective) {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(e))
            };

            balance::set_bias::<T>(balance, new_bias);

            // Price change boundary -> update checkpoint (Mint/Reap only)
            if let Err(e) = balance_checkpoint::<T>(balance) {
                return <Out as FromTag::<_, Mint>>::from_tag(Err(e))
            }

            <Out as FromTag::<_, Mint>>::from_tag(Ok(asset))
        }
    );

    plugin_model!(
        /// [`CanMint`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::CanMint`].
        ///
        /// ## Overview
        ///
        /// Performs pre-checks to determine whether minting is allowed,
        /// without mutating balance state.
        ///
        /// ## Validation Rules
        ///
        /// - zero-value mint is disallowed (although execution treats it as no-op)
        /// - balance must not be fresh (must be initialized)
        /// - balance must have existing deposits (`issued > 0`)
        /// - addition to `effective` must not overflow
        ///
        /// ## Semantics
        ///
        /// This mirrors [`ModelMint`] constraints without applying state changes,
        /// ensuring mint execution will succeed if validation passes.
        name: pub ModelCanMint,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok((balance, _variant, _id, asset, _subject)) = TryIntoTag::<_, CanMint>::try_into_tag(input) else {
                return <Out as FromTag::<_, CanMint>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Zero mint - invalid (execution treats as no-op, validation rejects)
            if asset.is_zero() {
                return <Out as FromTag::<_, CanMint>>::from_tag(Err(ShareBalanceError::ZeroAdjustmentNotAllowed))
            }

            let Some(issued) = balance::issued::<T>(&balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, CanMint>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit))
            };

            let Some(effective) = balance::effective::<T>(&balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, CanMint>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit))
            };

            // Mint requires existing shares (cannot bootstrap)
            if issued.is_zero() {
                return <Out as FromTag::<_, CanMint>>::from_tag(Err(ShareBalanceError::RequiresExistingDeposits))
            };

            // Ensure addition does not overflow effective balance
            if effective.checked_add(&asset).is_none() {
                return <Out as FromTag::<_, CanMint>>::from_tag(
                    Err(ShareBalanceError::AssetOverflow)
                )
            }

            <Out as FromTag::<_, CanMint>>::from_tag(Ok(()))
        }
    );

    plugin_model!(
        /// [`Reap`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::Reap`].
        ///
        /// ## Overview
        ///
        /// Removes value from the balance without modifying issued shares.
        ///
        /// This decreases `effective` while keeping `issued` constant,
        /// thereby decreasing `bias` (value per share).
        ///
        /// ## Effect
        ///
        /// - `effective -= asset`
        /// - `issued` remains unchanged
        /// - `bias = effective / issued` is recomputed
        ///
        /// This proportionally reduces the value of all existing shares
        /// implicitly, which is lazily reflected during withdrawal.
        ///
        /// ## Full Drain
        ///
        /// If `effective` becomes zero:
        /// - `bias` is set to zero
        /// - `drainpoint` (time-reference) is recorded for optimized withdrawals
        ///
        /// This marks the balance as fully drained. Deposits are disallowed
        /// in this state until new value is introduced via minting. In this
        /// state withdrawals shall be simply zero valued until minted further.
        ///
        /// ## Constraints
        ///
        /// - zero-value reap is a no-op
        /// - balance must not be fresh (must be initialized)
        /// - balance must have existing deposits (`issued > 0`)
        /// - subtraction from `effective` must not underflow
        ///
        /// ## Semantics
        ///
        /// Reaping represents value removal:
        /// - no shares are burned
        /// - all existing receipts lose value proportionally lazily.
        ///
        /// Withdrawals ensures correct lazy resolution for receipts across drained
        /// states.
        name: pub ModelReap,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok((mut balance, _variant, _id, asset, _subject)) = TryIntoTag::<_, Reap>::try_into_tag(input) else {
                return <Out as FromTag::<_, Reap>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Zero reap -> no-op
            if asset.is_zero() {
                return <Out as FromTag::<_, Reap>>::from_tag(Ok(Cow::Owned(Zero::zero())));
            }

            let balance = &mut balance;

            let Some(effective) = balance::effective::<T>(balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, Reap>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            let Some(issued) = balance::issued::<T>(balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, Reap>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            // Reap requires existing shares (cannot operate on empty balance)
            if issued.is_zero() {
                return <Out as FromTag::<_, Reap>>::from_tag(Err(ShareBalanceError::RequiresExistingDeposits))
            };

            // Decrease effective balance
            let Some(new_effective) = effective.checked_sub(&asset) else {
                return <Out as FromTag::<_, Reap>>::from_tag(Err(ShareBalanceError::AssetUnderflow))
            };

            // Apply new effective value
            if let Err(e) = balance::set_effective::<T>(balance, new_effective) {
                return <Out as FromTag::<_, Reap>>::from_tag(Err(e))
            };

            match new_effective.is_zero() {
                true => {
                    // Fully drained -> invalidate pricing (bias = 0)
                    let zero_bias = Zero::zero();
                    balance::set_bias::<T>(balance, zero_bias);
                },
                false => {
                    // Recompute price per share (bias)
                    let Some(new_bias) = T::Rational::checked_from_rational(new_effective, issued) else {
                        return <Out as FromTag::<_, Reap>>::from_tag(Err(ShareBalanceError::InadequatePrecision))
                    };

                    balance::set_bias::<T>(balance, new_bias);
                }
            };

            // Handle lifecycle transition (drain boundary if reached)
            if let Err(e) = balance_checkpoint::<T>(balance){
                return <Out as FromTag::<_, Reap>>::from_tag(Err(e))
            }

            <Out as FromTag::<_, Reap>>::from_tag(Ok(asset))
        }
    );

    plugin_model!(
        /// [`CanReap`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::CanReap`].
        ///
        /// ## Overview
        ///
        /// Performs pre-checks to determine whether value can be removed
        /// from the balance, without mutating state.
        ///
        /// ## Validation Rules
        ///
        /// - zero-value reap is dis-allowed although
        /// actual operation treats it as no-op
        /// - balance must not be fresh (must be initialized)
        /// - balance must have existing deposits (`issued > 0`)
        /// - subtraction from `effective` must not underflow
        ///
        /// ## Semantics
        ///
        /// This mirrors [`ModelReap`] constraints without applying state changes,
        /// ensuring reap execution will succeed if validation passes.
        name: pub ModelCanReap,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok((balance, _variant, _id, asset, _subject)) = TryIntoTag::<_, CanReap>::try_into_tag(input) else {
                return <Out as FromTag::<_, CanReap>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Zero reap -> invalid (execution treats as no-op, validation rejects)
            if asset.is_zero() {
                return <Out as FromTag::<_, CanReap>>::from_tag(Err(ShareBalanceError::ZeroAdjustmentNotAllowed))
            }

            let Some(issued) = balance::issued::<T>(&balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, CanReap>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit))
            };

            // Reap requires existing shares (cannot bootstrap)
            if issued.is_zero() {
                return <Out as FromTag::<_, CanReap>>::from_tag(Err(ShareBalanceError::RequiresExistingDeposits))
            };

            let Some(effective) = balance::effective::<T>(&balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, CanReap>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit))
            };

            // Ensure subtraction does not underflow effective balance
            if effective.checked_sub(&asset).is_none() {
                return <Out as FromTag::<_, CanReap>>::from_tag(
                    Err(ShareBalanceError::AssetUnderflow)
                )
            }
            <Out as FromTag::<_, CanReap>>::from_tag(Ok(()))
        }
    );

    plugin_model!(
        /// [`Withdraw`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::Withdraw`].
        ///
        /// ## Overview
        ///
        /// Resolves a [`LazyBalance::Receipt`] into a concrete asset value
        /// and updates the balance state accordingly.
        ///
        /// This burns shares (`issued`) and reduces `effective`, returning
        /// the derived value to the caller.
        ///
        /// ## Derivation
        ///
        /// Withdrawal value is computed relative to:
        ///
        /// - receipt's `shares`
        /// - receipt's `bias` (at deposit time)
        /// - current balance `bias` (price per share)
        ///
        /// ```text
        /// value = shares * receipt_bias * (current_bias / receipt_bias) // or
        ///       = shares * current_bias // if balance drained after deposit hence receipt is outdated
        /// ```
        ///
        /// Although the formula simplifies to `shares * current_bias` if
        /// the receipt gets outdated due to a recent drain, the stored
        /// `receipt_bias` is required to correctly handle:
        /// - drain scenarios
        /// - checkpoint-based invalidation
        /// - historical pricing context
        ///
        /// Special handling:
        ///
        /// - if balance was drained after receipt checkpoint:
        ///   - receipt bias is reset (treated as 1:1)
        ///   - shares map directly to value
        ///
        /// ## Effect
        ///
        /// - `effective -= withdraw`
        /// - `issued -= shares`
        ///
        /// ## Edge Cases
        ///
        /// - withdrawal is capped at `effective`
        /// - full withdrawal resets or reinitializes balance
        ///
        /// ## Semantics
        ///
        /// Withdrawal represents **lazy resolution of ownership**:
        ///
        /// - receipts encode relative claim
        /// - value is derived at execution time
        /// - drained states are handled via checkpoint logic
        name: pub ModelWithdraw,
        input: In,
        output: Out,
        others: ['a, T],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
            T::Asset: From<<T::Rational as FixedPointNumber>::Inner>,
        ],
        compute: |input, _context| {
            let Ok((mut balance, variant, id, receipt)) = TryIntoTag::<_, Withdraw>::try_into_tag(input) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            let balance = &mut balance;

            let Some(effective) = balance::effective::<T>(balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            let Some(issued) = balance::issued::<T>(balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            let Some(shares) = receipt::shares::<T>(&receipt) else {
                // invalid receipt structure
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            };

            // Invalid receipt obvious cases
            if shares > issued || shares.is_zero() || issued.is_zero() {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            }

            let Some(mut receipt_bias) = receipt::bias::<T>(&receipt) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            };

            // Base value at deposit time: shares * receipt_bias
            let Some(mut value_fixed) = T::Rational::saturating_from_integer(shares).checked_mul(&receipt_bias) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InadequatePrecision))
            };

            let Some(checkpoint) = receipt::checkpoint::<T>(&receipt) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            };

            let Some(drainpoint) = balance::drainpoint::<T>(balance) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            // If balance was drained after receipt creation:
            // reset derivation to 1:1 (shares -> value)
            if drainpoint > checkpoint {
                // Drain invalidates historical pricing -> reset to share-only basis
                value_fixed = T::Rational::saturating_from_integer(shares);
                receipt_bias = One::one();
            };

            let Some(bias) = balance::bias::<T>(balance) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            // Compute relative price change since deposit
            let Some(final_ratio) = bias.checked_div(&receipt_bias) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InadequatePrecision))
            };

            // Apply ratio to derive final value
            let Some(final_value_fixed) = value_fixed.checked_mul(&final_ratio) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::InadequatePrecision))
            };

            // Convert from fixed-point -> asset
            let Some(withdraw_fixed) = final_value_fixed.into_inner().checked_div(&T::Rational::DIV) else {
                // unreachable unless DIV is zero
                debug_assert!(
                    false,
                    "divide by zero during fixed-point scaling during \
                    ShareBalanceFamily::Withdraw for id {:?}, variant {:?} \
                    receipt {:?}",
                    variant, id, receipt,
                );
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::FixedPointScalingFailed))
            };

            // Cap withdrawal to available effective balance
            let withdraw = Into::<T::Asset>::into(withdraw_fixed).min(effective);

            // Apply state updates
            let Some(new_effective) = effective.checked_sub(&withdraw) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::AssetUnderflow))
            };

            let Some(new_issued) = issued.checked_sub(&shares) else {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(ShareBalanceError::SharesUnderflow))
            };

            if let Err(e) = balance::set_effective::<T>(balance, new_effective) {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(e))
            };

            if let Err(e) = balance::set_issued::<T>(balance, new_issued) {
                return <Out as FromTag::<_, Withdraw>>::from_tag(Err(e))
            };

            // Handle terminal states
            if new_issued.is_zero() {

                if !new_effective.is_zero() {
                    // leftover value -> reset balance and return all
                    if let Err(e) = balance::init_balance::<T>(balance) {
                        return <Out as FromTag::<_, Withdraw>>::from_tag(Err(e))
                    };

                    // Last shareholder -> receives full remaining balance i.e., dusts
                    return <Out as FromTag::<_, Withdraw>>::from_tag(
                        Ok(Cow::Owned(withdraw.saturating_add(new_effective)))
                    )
                }

                // fully empty -> reset pricing
                balance::set_bias::<T>(balance, One::one());
            }

            <Out as FromTag::<_, Withdraw>>::from_tag(Ok(Cow::Owned(withdraw)))
        }
    );

    plugin_model!(
        /// [`CanWithdraw`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::CanWithdraw`].
        ///
        /// ## Overview
        ///
        /// Performs pre-checks to determine whether a receipt can be
        /// successfully withdrawn, without mutating balance state.
        ///
        /// ## Validation Rules
        ///
        /// - balance must be initialized (must have issued supply)
        /// - receipt must be structurally valid
        /// - receipt must carry:
        ///   - `shares` (ownership)
        ///   - `bias` (pricing context at deposit)
        ///   - `checkpoint` (time anchor)
        /// - `shares` must be non-zero
        /// - `issued` must be non-zero
        /// - receipt cannot claim more shares than total issued
        ///
        /// ## Semantics
        ///
        /// This mirrors [`ModelWithdraw`] constraints without applying state changes,
        /// ensuring withdrawal execution will succeed if validation passes.
        ///
        /// A valid receipt represents a **bounded ownership claim** over the
        /// current balance, which can be safely resolved at execution time.
        name: pub ModelCanWithdraw,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok((balance, _variant, _id, receipt)) = TryIntoTag::<_, CanWithdraw>::try_into_tag(input) else {
                return <Out as FromTag::<_, CanWithdraw>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            let Some(issued) = balance::issued::<T>(&balance) else {
                // balance must be initialized via deposit
                return <Out as FromTag::<_, CanWithdraw>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            let Some(shares) =  receipt::shares::<T>(&receipt) else {
                // invalid receipt structure
                return <Out as FromTag::<_, CanWithdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            };

            // receipt must carry pricing context (bias at deposit time)
            if receipt::bias::<T>(&receipt).is_none() {
                return <Out as FromTag::<_, CanWithdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            }

            // receipt must carry time anchor (checkpoint)
            if receipt::checkpoint::<T>(&receipt).is_none() {
                return <Out as FromTag::<_, CanWithdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            }

            // Validate ownership claim:
            // - shares must be non-zero
            // - issued supply must exist
            // - receipt cannot claim more shares than total issued
            if shares > issued || shares.is_zero() || issued.is_zero() {
                return <Out as FromTag::<_, CanWithdraw>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            }

            <Out as FromTag::<_, CanWithdraw>>::from_tag(Ok(()))
        }
    );

    plugin_model!(
        /// [`TotalValue`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::TotalValue`].
        ///
        /// ## Overview
        ///
        /// Returns the total effective value currently held by the balance.
        ///
        /// This represents the aggregate value backing all issued shares,
        /// independent of any individual receipt.
        ///
        /// ## Semantics
        ///
        /// - `effective` reflects the current total value of the balance
        /// - includes all value changes from:
        ///   - deposits
        ///   - mint (value injection)
        ///   - reap (value removal)
        ///
        /// If the balance is uninitialized (fresh), the total value is treated as zero.
        ///
        /// This operation does not depend on receipts and does not mutate state.
        name: pub ModelTotalValue,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok((balance, _variant, _id)) =  TryIntoTag::<_, TotalValue>::try_into_tag(input) else {
                return <Out as FromTag::<_, TotalValue>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            let Some(effective) = balance::effective::<T>(&balance) else {
                // fresh balance -> no value accumulated
                return <Out as FromTag::<_, TotalValue>>::from_tag(Ok(Cow::Owned(Zero::zero())))
            };

            // return current aggregate value backing all shares
            <Out as FromTag::<_, TotalValue>>::from_tag(Ok(Cow::Owned(effective)))
        }
    );

    plugin_model!(
        /// [`ReceiptActiveValue`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::ReceiptActiveValue`].
        ///
        /// ## Overview
        ///
        /// Computes the current redeemable value of a receipt without mutating
        /// the original balance state.
        ///
        /// This simulates a withdrawal using a cloned balance, returning the
        /// value that would be obtained if the receipt were withdrawn now.
        ///
        /// ## Semantics
        ///
        /// - performs full [`CanWithdraw`] validation before derivation
        /// - executes [`ModelWithdraw`] on a cloned balance
        /// - preserves original state (read-only evaluation)
        ///
        /// The returned value reflects:
        ///
        /// - current `bias` (price per share)
        /// - receipt's `shares`
        /// - checkpoint / drainpoint adjustments
        ///
        /// ## Guarantees
        ///
        /// - no mutation of original balance
        /// - consistent with [`ModelWithdraw`] execution
        /// - safe preview of withdrawal outcome
        ///
        /// This acts as a **pure evaluation layer** over the withdrawal logic.
        name: pub ModelReceiptActiveValue,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
           let Ok((balance, variant, id, receipt)) =  TryIntoTag::<_, ReceiptActiveValue>::try_into_tag(input) else {
                return <Out as FromTag::<_, ReceiptActiveValue>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Validate withdrawal feasibility using a cloned balance
            let can_withdraw_input = <In as FromTag::<_, CanWithdraw>>::from_tag(
                (
                    Cow::Owned((*balance).clone()),
                    variant.clone(),
                    id.clone(),
                    receipt.clone()
                )
            );

            let raw = T::can_withdraw(can_withdraw_input);

            let Ok(result) =  TryIntoTag::<_, CanWithdraw>::try_into_tag(raw) else {
                return <Out as FromTag::<_, ReceiptActiveValue>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Propagate validation failure
            if let Err(e) = result {
                return <Out as FromTag::<_, ReceiptActiveValue>>::from_tag(Err(e));
            }

            // Simulate withdrawal on a cloned balance (no state mutation)
            let withdraw_input = <In as FromTag::<_, Withdraw>>::from_tag(
                (
                    MutHandle::Owned((*balance).clone()),
                    variant,
                    id,
                    receipt
                )
            );

            let raw = T::withdraw(withdraw_input);

            let Ok(result) =  TryIntoTag::<_, Withdraw>::try_into_tag(raw) else {
                return <Out as FromTag::<_, ReceiptActiveValue>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Return derived value or error from withdrawal simulation
            match result {
                Ok(v) => <Out as FromTag::<_, ReceiptActiveValue>>::from_tag(Ok(v)),
                Err(e) => <Out as FromTag::<_, ReceiptActiveValue>>::from_tag(Err(e)),
            }
        }
    );
    plugin_model!(
        /// [`Drain`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::Drain`].
        ///
        /// ## Overview
        ///
        /// Fully removes all effective value from the balance in a single operation.
        ///
        /// This is implemented as a composition of:
        ///
        /// - [`ModelTotalValue`]: derive current total value
        /// - [`ModelReap`]: remove that value from the balance
        ///
        /// ## Effect
        ///
        /// - `effective -> 0`
        /// - `issued` remains unchanged
        /// - `bias -> 0` (balance enters drained state)
        /// - `drainpoint` is recorded via checkpoint logic
        ///
        /// ## Semantics
        ///
        /// Drain represents a **complete value removal**:
        ///
        /// - all shares lose value (price per share becomes zero)
        /// - no shares are burned
        /// - receipts remain valid but resolve to zero until mint
        ///
        /// This transitions the balance into the **drained state**.
        ///
        /// ## Guarantees
        ///
        /// - deterministic: always removes full value
        /// - equivalent to `Reap(effective)`
        /// - respects all [`ModelReap`] invariants
        ///
        /// This acts as a **convenience operation** for full balance
        /// depletion (a very unpractical edge case for production systems visited).
        name: pub ModelDrain,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
           let Ok((balance, variant, id)) =  TryIntoTag::<_, Drain>::try_into_tag(input) else {
                return <Out as FromTag::<_, Drain>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Clone balance to safely derive total value without mutation
            let b = (*balance).clone();

            // Compute current total value backing all shares
            let total_value_input = <In as FromTag::<_, TotalValue>>::from_tag(
                (
                    Cow::Owned(b),
                    variant.clone(),
                    id.clone(),
                )
            );

            let raw = T::total_value(total_value_input);

            let Ok(result) =  TryIntoTag::<_, TotalValue>::try_into_tag(raw) else {
                return <Out as FromTag::<_, Drain>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Propagate total value derivation result
            let value = match result {
                Ok(v) => v,
                Err(e) => return <Out as FromTag::<_, Drain>>::from_tag(Err(e)),
            };

            // Remove entire value via reap (force exact execution)
            let reap_input = <In as FromTag::<_, Reap>>::from_tag(
                (
                    balance,
                    variant,
                    id,
                    value,
                    Cow::Owned(Directive::new(Precision::Exact, Fortitude::Force))
                )
            );

            let raw = T::reap(reap_input);

            let Ok(result) =  TryIntoTag::<_, Reap>::try_into_tag(raw) else {
                return <Out as FromTag::<_, Drain>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Return result of full reap
            match result {
                Ok(v) => <Out as FromTag::<_, Drain>>::from_tag(Ok(v)),
                Err(e) => <Out as FromTag::<_, Drain>>::from_tag(Err(e)),
            }
        }
    );

    plugin_model!(
        /// [`HasDeposits`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::HasDeposits`].
        ///
        /// ## Overview
        ///
        /// Checks whether the balance currently has active deposits
        /// (i.e., issued shares exist).
        ///
        /// ## Validation Rules
        ///
        /// - balance must not be fresh (must be initialized)
        /// - balance must have issued shares (`issued > 0`)
        ///
        /// ## Semantics
        ///
        /// - `issued > 0` -> balance has active deposits
        /// - `issued == 0` -> balance has been fully withdrawn
        ///
        /// This distinguishes:
        ///
        /// - fresh balance (never initialized)
        /// - active balance (has deposits)
        /// - fully withdrawn balance (no remaining shares)
        ///
        /// This operation does not mutate state.
        name: pub ModelHasDeposits,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok((balance, _variant, _id)) =  TryIntoTag::<_, HasDeposits>::try_into_tag(input) else {
                return <Out as FromTag::<_, HasDeposits>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            // Fresh balance -> no deposits have ever been made
            if *balance == Default::default() {
                return <Out as FromTag::<_, HasDeposits>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            let Some(issued) = balance::issued::<T>(&balance) else {
                // unreachable unless balance virtual fields are corrupted
                return <Out as FromTag::<_, HasDeposits>>::from_tag(Err(ShareBalanceError::BalanceNotInitiatedViaDeposit));
            };

            // No issued shares -> balance fully withdrawn
            if issued.is_zero() {
                return <Out as FromTag::<_, HasDeposits>>::from_tag(Err(ShareBalanceError::AllDepositsWithdrawn));
            }

            // Active deposits exist
            <Out as FromTag::<_, HasDeposits>>::from_tag(Ok(()))
        }
    );

    plugin_model!(
        /// [`ReceiptDepositValue`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::ReceiptDepositValue`].
        ///
        /// ## Overview
        ///
        /// Returns the original deposited value associated with a receipt.
        ///
        /// This reflects the principal amount supplied at deposit time,
        /// independent of any subsequent balance state changes.
        ///
        /// ## Semantics
        ///
        /// - corresponds to receipt's `principal`
        /// - does not depend on current `bias` (price per share)
        /// - does not account for mint/reap effects
        ///
        /// This represents the **initial contribution**, not the current value.
        ///
        /// ## Guarantees
        ///
        /// - pure read (no state mutation)
        /// - invariant across all balance transitions
        /// - independent of withdrawal logic
        ///
        /// This acts as a **historical reference value** for the receipt.
        name: pub ModelReceiptDepositValue,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |input, _context| {
            let Ok(receipt) =  TryIntoTag::<_, ReceiptDepositValue>::try_into_tag(input) else {
                return <Out as FromTag::<_, ReceiptDepositValue>>::from_tag(Err(ShareBalanceError::InvalidPluginParams));
            };

            let Some(deposit) = receipt::principal::<T>(&receipt) else {
                // invalid receipt structure
                return <Out as FromTag::<_, ReceiptDepositValue>>::from_tag(Err(ShareBalanceError::InvalidReceipt))
            };

            // return original deposited value (principal)
            <Out as FromTag::<_, ReceiptDepositValue>>::from_tag(Ok(Cow::Owned(deposit)))
        }
    );

    plugin_model!(
        /// [`DepositLimits`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::DepositLimits`].
        ///
        /// ## Overview
        ///
        /// Provides deposit constraints for the balance.
        ///
        /// ## Semantics
        ///
        /// This implementation defines **no limits**:
        ///
        /// - deposits are fully permissive
        /// - no min/max bounds are enforced
        ///
        /// The balance operates purely on a share-based model,
        /// where value distribution is determined by `bias`
        /// (price per share).
        ///
        /// ## Notes
        ///
        /// - no safeguards against extreme deposits
        /// - relies on external discipline or higher-level controls (callers)
        ///
        /// Returns default (unbounded) limits.
        name: pub ModelDepositLimits,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |_input, _context| {
            // No limits -> return default (unbounded)
            <Out as FromTag::<_, DepositLimits>>::from_tag(Ok(Cow::Owned(Default::default())))
        }
    );

    plugin_model!(
        /// [`MintLimits`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::MintLimits`].
        ///
        /// ## Overview
        ///
        /// Provides mint constraints for the balance.
        ///
        /// ## Semantics
        ///
        /// This implementation defines **no limits**:
        ///
        /// - mint operations are fully permissive
        /// - no bounds on value injection
        ///
        /// Mint directly affects `bias` (price per share),
        /// increasing value across all existing shares.
        ///
        /// ## Notes
        ///
        /// - unrestricted minting can skew share price
        /// - value distribution may become imbalanced
        ///
        /// Returns default (unbounded) limits.
        name: pub ModelMintLimits,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |_input, _context| {
            // No limits -> return default (unbounded)
            <Out as FromTag::<_, MintLimits>>::from_tag(Ok(Cow::Owned(Default::default())))
        }
    );

    plugin_model!(
        /// [`ReapLimits`] plugin family's child model over the
        /// [`LazyBalance`]'s compile-time marker via [`ShareBalanceContext`].
        ///
        /// Bound to [`ShareBalanceFamily`] via [`LazyBalanceRoot::ReapLimits`].
        ///
        /// ## Overview
        ///
        /// Provides reap constraints for the balance.
        ///
        /// ## Semantics
        ///
        /// This implementation defines **no limits**:
        ///
        /// - reap operations are fully permissive
        /// - no bounds on value removal
        ///
        /// Reap directly affects `bias` (price per share),
        /// decreasing value across all existing shares.
        ///
        /// ## Notes
        ///
        /// - unrestricted reaping can distort share pricing
        /// - combined misuse of mint/reap may skew withdrawal distribution
        ///
        /// Returns default (unbounded) limits.
        name: pub ModelReapLimits,
        input: In,
        output: Out,
        others: ['a, T, ],
        context: ShareBalanceContext<T>,
        bounds : [
            T: LazyBalance<Input<'a> = In, Output<'a> = Out>,
            Context<T>: VirtualError<LazyBalanceError, Error = ShareBalanceError>,
            Out: LazyBalanceOutput<'a, T::Asset, T::Receipt, T::SnapShot, T::Time, T::Limits, T>,
            In: LazyBalanceInput<'a, T::Balance, T::Variant, T::Id, T::Asset, T::Receipt, T>,
        ],
        compute: |_input, _context| {
            // No limits -> return default (unbounded)
            <Out as FromTag::<_, ReapLimits>>::from_tag(Ok(Cow::Owned(Default::default())))
        }
    );

    // ===============================================================================
    // ````````````````````````````` VIRTUAL STRUCTURES ``````````````````````````````
    // ===============================================================================

    /// Balance-level accessors and initialization utilities.
    ///
    /// Provides a field-oriented interface over [`LazyBalance::Balance`],
    /// treating it as a **virtual struct** composed via discriminants.
    ///
    /// ## Logical Structure
    ///
    /// ```ignore
    /// struct <T as LazyBalance>::Balance {
    ///     BalanceAsset.0: T::Asset,       // effective
    ///     BalanceAsset.1: T::Asset,       // issued
    ///     BalanceRational: T::Rational,   // bias
    ///     BalanceTime.0: T::Time,         // checkpoint
    ///     BalanceTime.1: T::Time,         // drainpoint
    /// }
    /// ```
    ///
    /// - discriminants = field identifiers
    /// - `.0`, `.1` = multiple values (`Many`)
    ///
    /// ```ignore
    /// BalanceAsset    => Many(T::Asset)
    /// BalanceRational => Some(T::Rational)
    /// BalanceTime     => Many(T::Time)
    /// ```
    ///
    /// This is a **type projection**:
    /// - `T` defines the schema
    /// - storage is discriminant-keyed and resolved via [`VirtualDynField`]
    /// - this decouples logical structure from storage layout.
    ///
    /// ## Semantics
    ///
    /// - **Asset**: `effective`, `issued`
    /// - **Rational**: `bias`
    /// - **Time**: `checkpoint`, `drainpoint`
    ///
    /// ## Initialization
    ///
    /// [`balance::is_fresh_balance`] lazily initializes:
    ///
    /// - `effective = 0`, `issued = 0`
    /// - `bias = 1`
    /// - `checkpoint = 0`, `drainpoint = 0`
    ///
    /// ## Context
    ///
    /// [`ShareBalanceContext`] supplies:
    /// - bounds ([`VirtualDynBound`])
    /// - empty extensions ([`empty_virtual_extension!`](frame_suite::empty_virtual_extension))
    ///
    /// Structure stays abstract (but here implemented concretely);
    /// layout and limits come from the context.
    mod balance {
        use super::*;

        /// Returns the current effective value of the balance.
        ///
        /// [`LazyBalance::Balance`] virtual field: `effective`
        ///
        /// Internally resolved from the discriminant [`BalanceAsset`] field at index `0`.
        pub fn effective<T: LazyBalance>(balance: &T::Balance) -> Option<T::Asset> {
            <T::Balance as DynFieldHelpers<BalanceAsset>>::index_get(balance, 0)
        }

        /// Sets the current effective value of the balance.
        ///
        /// [`LazyBalance::Balance`] virtual field: `effective`
        ///
        /// Writes to the discriminant [`BalanceAsset`] field at index `0`.
        pub fn set_effective<T: LazyBalance>(
            balance: &mut T::Balance,
            value: T::Asset,
        ) -> Result<(), ShareBalanceError> {
            <T::Balance as DynFieldHelpers<BalanceAsset>>::index_set(balance, 0, value)
                .map_err(|_| ShareBalanceError::CorruptedVirtualField)
        }

        /// Returns the base value (total shares) backing the balance.
        ///
        /// [`LazyBalance::Balance`] virtual field: `issued`
        ///
        /// Internally resolved from the discriminant [`BalanceAsset`] field at index `1`.
        pub fn issued<T: LazyBalance>(balance: &T::Balance) -> Option<T::Asset> {
            <T::Balance as DynFieldHelpers<BalanceAsset>>::index_get(balance, 1)
        }

        /// Sets the base value (total shares) of the balance.
        ///
        /// [`LazyBalance::Balance`] virtual field: `issued`
        ///
        /// Writes to the discriminant [`BalanceAsset`] field at index `1`.
        pub fn set_issued<T: LazyBalance>(
            balance: &mut T::Balance,
            value: T::Asset,
        ) -> Result<(), ShareBalanceError> {
            <T::Balance as DynFieldHelpers<BalanceAsset>>::index_set(balance, 1, value)
                .map_err(|_| ShareBalanceError::CorruptedVirtualField)
        }

        /// Returns the scaling factor (share-price) applied to the balance value.
        ///
        /// [`LazyBalance::Balance`] virtual field: `bias`
        ///
        /// Internally resolved from the discriminant [`BalanceRational`] field.
        pub fn bias<T: LazyBalance>(balance: &T::Balance) -> Option<T::Rational> {
            <T::Balance as DynFieldHelpers<BalanceRational>>::get(balance)
        }

        /// Sets the scaling factor (share-price) applied to the balance value.
        ///
        /// [`LazyBalance::Balance`] virtual field: `bias`
        ///
        /// Writes to the discriminant [`BalanceRational`] field.
        pub fn set_bias<T: LazyBalance>(balance: &mut T::Balance, value: T::Rational) {
            <T::Balance as DynFieldHelpers<BalanceRational>>::set(balance, value)
        }

        /// Returns the most recent time at which the balance state was adjusted i.e.,
        /// reap or mint.
        ///
        /// [`LazyBalance::Balance`] virtual field: `checkpoint`
        ///
        /// Internally resolved from the discriminant [`BalanceTime`] field at index 0.
        pub fn checkpoint<T: LazyBalance>(balance: &T::Balance) -> Option<T::Time> {
            <T::Balance as DynFieldHelpers<BalanceTime>>::index_get(balance, 0)
        }

        /// Sets the most recent adjusted (reap/mint) time of the balance.
        ///
        /// [`LazyBalance::Balance`] virtual field: `checkpoint`
        ///
        /// Writes to the discriminant [`BalanceTime`] field at index 0.
        pub fn set_checkpoint<T: LazyBalance>(
            balance: &mut T::Balance,
            value: T::Time,
        ) -> Result<(), ShareBalanceError> {
            <T::Balance as DynFieldHelpers<BalanceTime>>::index_set(balance, 0, value)
                .map_err(|_| ShareBalanceError::CorruptedVirtualField)
        }

        /// Returns the most recent time at which the balance state was drained.
        ///
        /// [`LazyBalance::Balance`] virtual field: `drainpoint`
        ///
        /// Internally resolved from the discriminant [`BalanceTime`] field at index 1.
        pub fn drainpoint<T: LazyBalance>(balance: &T::Balance) -> Option<T::Time> {
            <T::Balance as DynFieldHelpers<BalanceTime>>::index_get(balance, 1)
        }

        /// Sets the most recent drained time of the balance.
        ///
        /// [`LazyBalance::Balance`] virtual field: `drainpoint`
        ///
        /// Writes to the discriminant [`BalanceTime`] field at index 1.
        pub fn set_drainpoint<T: LazyBalance>(
            balance: &mut T::Balance,
            value: T::Time,
        ) -> Result<(), ShareBalanceError> {
            <T::Balance as DynFieldHelpers<BalanceTime>>::index_set(balance, 1, value)
                .map_err(|_| ShareBalanceError::CorruptedVirtualField)
        }

        /// Initializes a [`LazyBalance::Balance`] **only if not already initialized**.
        pub fn is_fresh_balance<T: LazyBalance>(
            balance: &mut T::Balance,
        ) -> Result<(), ShareBalanceError> {
            if balance::effective::<T>(balance).is_none() {
                self::init_balance::<T>(balance)?;
            }
            Ok(())
        }

        /// Initializes a [`LazyBalance::Balance`] forcefully.
        ///
        /// Ensures all fields are set to sensible defaults:
        /// - `effective` = 0
        /// - `issued` = 0
        /// - `bias` = 1
        /// - `checkpoint` = 0
        pub fn init_balance<T: LazyBalance>(
            balance: &mut T::Balance,
        ) -> Result<(), ShareBalanceError> {
            set_effective::<T>(balance, Zero::zero())?;
            set_issued::<T>(balance, Zero::zero())?;
            set_bias::<T>(balance, One::one());
            set_checkpoint::<T>(balance, Zero::zero())?;
            set_drainpoint::<T>(balance, Zero::zero())?;
            Ok(())
        }

        /// [`LazyBalance::Balance`] virtual field layout for the
        /// [`BalanceAsset`] discriminant
        ///
        /// Allocates two asset fields:
        /// - `effective`  (internally index `0`)
        /// - `issued`  (internally index `1`)
        impl<T> VirtualDynBound<BalanceAsset> for ShareBalanceContext<T> {
            type Bound = ConstU32<2>;
        }

        /// [`LazyBalance::Balance`] virtual field layout for the
        /// [`BalanceRational`] discriminant
        ///
        /// Allocates one rational field:
        /// - `bias`
        impl<T> VirtualDynBound<BalanceRational> for ShareBalanceContext<T> {
            type Bound = ConstU32<1>;
        }

        /// [`LazyBalance::Balance`] virtual field layout for the
        /// [`BalanceTime`] discriminant
        ///
        /// Allocates two asset fields:
        /// - `checkpoint`  (internally index `0`)
        /// - `drainpoint`  (internally index `1`)
        impl<T> VirtualDynBound<BalanceTime> for ShareBalanceContext<T> {
            type Bound = ConstU32<2>;
        }

        // `LazyBalance::Balance` virtual extension schema for the
        // `BalanceAddon` discriminant
        //
        // Defines an empty extension schema.
        //
        // Balance do not support addon-backed fields, and always behave
        // as having no extension data.
        empty_virtual_extension!(
            target: T::Balance,
            tag: BalanceAddon,
            schema: ShareBalanceContext<T>,
            generics: [T]
        );
    }

    /// [`LazyBalance::SnapShot`] virtual field layout.
    ///
    /// This configuration defines a **zero-sized snapshot**:
    ///
    /// ```ignore
    /// struct <T as LazyBalance>::SnapShot {}
    /// ```
    ///
    /// No fields are allocated:
    /// - [`SnapShotAsset`]     -> 0
    /// - [`SnapShotRational`]  -> 0
    /// - [`SnapShotTime`]      -> 0
    ///
    /// ## Semantics
    ///
    /// Snapshots are **not used** in the [`ShareBalanceFamily`] model:
    ///
    /// - no historical state
    /// - no time-based projections
    /// - no additional storage
    ///
    /// The type exists only to satisfy the [`LazyBalance`] contract.
    ///
    /// ## Extension
    ///
    /// No extensions are supported:
    ///
    /// ```ignore
    /// empty_virtual_extension!(...)
    /// ```
    ///
    /// Snapshot behaves as a **pure placeholder type**.
    mod snapshot {
        use super::*;

        impl<T> VirtualDynBound<SnapShotAsset> for ShareBalanceContext<T> {
            type Bound = ConstU32<0>;
        }

        impl<T> VirtualDynBound<SnapShotRational> for ShareBalanceContext<T> {
            type Bound = ConstU32<0>;
        }

        impl<T> VirtualDynBound<SnapShotTime> for ShareBalanceContext<T> {
            type Bound = ConstU32<0>;
        }

        empty_virtual_extension!(
            target: T::SnapShot,
            tag: SnapShotAddon,
            schema: ShareBalanceContext<T>,
            generics: [T]
        );
    }

    /// Receipt-level accessors and utilities.
    ///
    /// Provides a field-oriented interface over [`LazyBalance::Receipt`],
    /// treating it as a **virtual struct**
    /// composed via discriminants.
    ///
    /// ## Semantics
    ///
    /// A receipt represents a **claim over deposited value**:
    ///
    /// ```text
    /// deposit -> issue receipt
    /// balance mutation -> affects value
    /// withdraw -> resolve receipt
    /// ```
    ///
    /// Captures:
    /// - `principal`, `shares`
    /// - `bias`
    /// - `checkpoint`
    ///
    /// ## Logical Structure
    ///
    /// ```ignore
    /// struct <T as LazyBalance>::Receipt {
    ///     ReceiptAsset.0: T::Asset,       // principal
    ///     ReceiptAsset.1: T::Asset,       // shares
    ///     ReceiptRational: T::Rational,   // bias
    ///     ReceiptTime: T::Time,           // checkpoint
    /// }
    /// ```
    ///
    /// - discriminants = field identifiers
    /// - `.0`, `.1` = multiple values (`Many`)
    ///
    /// ```ignore
    /// ReceiptAsset    => Many(T::Asset)
    /// ReceiptRational => Some(T::Rational)
    /// ReceiptTime     => Some(T::Time)
    /// ```
    ///
    /// Type projection:
    /// - `T` defines schema
    /// - storage via [`VirtualDynField`]
    ///
    /// ## Context
    ///
    /// [`ShareBalanceContext`] supplies:
    /// - bounds ([`VirtualDynBound`])
    /// - empty extensions ([`empty_virtual_extension!`](frame_suite::empty_virtual_extension))
    ///
    /// Structure is abstract; layout comes from context.
    mod receipt {
        use super::*;

        /// Returns the original deposit value of the receipt.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `principal`
        ///
        /// Internally resolved from the discriminant [`ReceiptAsset`] field at index `0`.
        pub fn principal<T: LazyBalance>(receipt: &T::Receipt) -> Option<T::Asset> {
            <T::Receipt as DynFieldHelpers<ReceiptAsset>>::index_get(receipt, 0)
        }

        /// Sets the original deposit value of the receipt.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `principal`
        ///
        /// Writes to the discriminant [`ReceiptAsset`] field at index `0`.
        pub fn set_principal<T: LazyBalance>(
            receipt: &mut T::Receipt,
            value: T::Asset,
        ) -> Result<(), ShareBalanceError> {
            <T::Receipt as DynFieldHelpers<ReceiptAsset>>::index_set(receipt, 0, value)
                .map_err(|_| ShareBalanceError::CorruptedVirtualField)
        }

        /// Returns the total shares provided for the receipt.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `shares`
        ///
        /// Internally resolved from the discriminant [`ReceiptAsset`] field at index `1`.
        pub fn shares<T: LazyBalance>(receipt: &T::Receipt) -> Option<T::Asset> {
            <T::Receipt as DynFieldHelpers<ReceiptAsset>>::index_get(receipt, 1)
        }

        /// Sets the total shares provided for the receipt.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `shares`
        ///
        /// Writes to the discriminant [`ReceiptAsset`] field at index `1`.
        pub fn set_shares<T: LazyBalance>(
            receipt: &mut T::Receipt,
            value: T::Asset,
        ) -> Result<(), ShareBalanceError> {
            <T::Receipt as DynFieldHelpers<ReceiptAsset>>::index_set(receipt, 1, value)
                .map_err(|_| ShareBalanceError::CorruptedVirtualField)
        }

        /// Returns the scaling factor (share-price) associated with the receipt
        /// at the time of deposit.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `bias`
        ///
        /// Internally resolved from the discriminant [`ReceiptRational`] field.
        pub fn bias<T: LazyBalance>(receipt: &T::Receipt) -> Option<T::Rational> {
            <T::Receipt as DynFieldHelpers<ReceiptRational>>::get(receipt)
        }

        /// Sets the scaling factor (share-price) associated with the receipt
        /// at the time of deposit.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `bias`
        ///
        /// Writes to the discriminant [`ReceiptRational`] field.
        pub fn set_bias<T: LazyBalance>(receipt: &mut T::Receipt, value: T::Rational) {
            <T::Receipt as DynFieldHelpers<ReceiptRational>>::set(receipt, value)
        }

        /// Returns the checkpoint time of the receipt.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `checkpoint`
        ///
        /// Internally resolved from the discriminant [`ReceiptTime`] field.
        pub fn checkpoint<T: LazyBalance>(receipt: &T::Receipt) -> Option<T::Time> {
            <T::Receipt as DynFieldHelpers<ReceiptTime>>::get(receipt)
        }

        /// Sets the checkpoint time of the receipt.
        ///
        /// [`LazyBalance::Receipt`] virtual field: `checkpoint`
        ///
        /// Writes to the discriminant [`ReceiptTime`] field.
        pub fn set_checkpoint<T: LazyBalance>(receipt: &mut T::Receipt, value: T::Time) {
            <T::Receipt as DynFieldHelpers<ReceiptTime>>::set(receipt, value)
        }

        /// [`LazyBalance::Receipt`] virtual field layout for the
        /// [`ReceiptAsset`] discriminant
        ///
        /// Allocates two asset fields:
        /// - `principal`  (internally index `0`)
        /// - `shares`  (internally index `1`)
        impl<T> VirtualDynBound<ReceiptAsset> for ShareBalanceContext<T> {
            type Bound = ConstU32<2>;
        }

        /// [`LazyBalance::Receipt`] virtual field layout for the
        /// [`ReceiptRational`] discriminant
        ///
        /// Allocates one rational field:
        /// - `bias`
        impl<T> VirtualDynBound<ReceiptRational> for ShareBalanceContext<T> {
            type Bound = ConstU32<1>;
        }

        /// [`LazyBalance::Receipt`] virtual field layout for the
        /// [`ReceiptTime`] discriminant
        ///
        /// Allocates one time field:
        /// - `checkpoint`
        impl<T> VirtualDynBound<ReceiptTime> for ShareBalanceContext<T> {
            type Bound = ConstU32<1>;
        }

        // `LazyBalance::Receipt` [`virtual`](frame_suite::virtuals) extension schema for the
        // `ReceiptAddon` discriminant
        //
        // Defines an empty extension schema.
        //
        // Receipts do not support addon-backed fields, and always behave
        // as having no extension data.
        empty_virtual_extension!(
            target: T::Receipt,
            tag: ReceiptAddon,
            schema: ShareBalanceContext<T>,
            generics: [T]
        );
    }

    // ===============================================================================
    // ```````````````````````````` SHARE-BALANCE ERRORS `````````````````````````````
    // ===============================================================================

    /// Errors that can occur during [`ShareBalanceFamily`]
    /// plugin operations.
    ///
    /// Covers:
    /// - validation failures (invalid inputs, receipts)
    /// - state violations (uninitialized, drained, inconsistent)
    /// - arithmetic issues (overflow, underflow, precision)
    #[derive(
        Clone,
        Copy,
        PartialEq,
        Eq,
        Debug,
        Encode,
        Decode,
        MaxEncodedLen,
        DecodeWithMemTracking,
        TypeInfo,
    )]
    pub enum ShareBalanceError {
        /// Internal inconsistency in virtual field storage (corrupted or missing data).
        CorruptedVirtualField,

        /// Invalid input parameters passed to the plugin (tag/discriminant mismatch or malformed input).
        InvalidPluginParams,

        /// Deposit amount must be non-zero.
        ZeroDepositNotAllowed,

        /// Operation requires an initialized balance (must be created via deposit first).
        BalanceNotInitiatedViaDeposit,

        /// Cannot deposit into a fully drained balance (`bias == 0`); requires mint to recover.
        BalanceDrainedCannotDeposit,

        /// Fixed-point arithmetic failed due to insufficient precision or invalid scaling.
        InadequatePrecision,

        /// Deposit too small relative to balance, resulting in floored zero shares after conversion.
        LessThanOneShareDerived,

        /// Overflow occurred while updating total effective asset value.
        AssetOverflow,

        /// Underflow occurred while reducing effective asset value.
        AssetUnderflow,

        /// Overflow occurred while updating total issued shares.
        SharesOverflow,

        /// Underflow occurred while reducing issued shares.
        SharesUnderflow,

        /// Operation requires existing deposits (issued shares must be non-zero).
        RequiresExistingDeposits,

        /// Adjustment (mint/reap) amount must be non-zero.
        ZeroAdjustmentNotAllowed,

        /// Receipt is invalid (missing fields, malformed, or inconsistent with balance).
        InvalidReceipt,

        /// Failure during fixed-point scaling conversion (e.g., division by scaling factor).
        FixedPointScalingFailed,

        /// All deposits have already been withdrawn (no remaining shares).
        AllDepositsWithdrawn,
    }

    impl Into<DispatchError> for ShareBalanceError {
        fn into(self) -> DispatchError {
            match self {
                ShareBalanceError::CorruptedVirtualField => {
                    DispatchError::Other("CorruptedVirtualField")
                }
                ShareBalanceError::InvalidPluginParams => {
                    DispatchError::Other("InvalidPluginParams")
                }
                ShareBalanceError::InadequatePrecision => {
                    DispatchError::Other("InadequatePrecision")
                }
                ShareBalanceError::AssetOverflow => DispatchError::Other("AssetOverflow"),
                ShareBalanceError::SharesOverflow => DispatchError::Other("SharesOverflow"),
                ShareBalanceError::FixedPointScalingFailed => {
                    DispatchError::Other("FixedPointScalingFailed")
                }
                ShareBalanceError::AssetUnderflow => DispatchError::Other("AssetUnderflow"),
                ShareBalanceError::InvalidReceipt => DispatchError::Other("InvalidReceipt"),
                ShareBalanceError::SharesUnderflow => DispatchError::Other("SharesUnderflow"),
                ShareBalanceError::BalanceNotInitiatedViaDeposit => {
                    DispatchError::Other("BalanceNotInitiatedViaDeposit")
                }
                ShareBalanceError::BalanceDrainedCannotDeposit => {
                    DispatchError::Other("BalanceDrainedCannotDeposit")
                }
                ShareBalanceError::ZeroDepositNotAllowed => {
                    DispatchError::Other("ZeroDepositNotAllowed")
                }
                ShareBalanceError::AllDepositsWithdrawn => {
                    DispatchError::Other("AllDepositsWithdrawn")
                }
                ShareBalanceError::RequiresExistingDeposits => {
                    DispatchError::Other("RequiresExistingDeposits")
                }
                ShareBalanceError::LessThanOneShareDerived => {
                    DispatchError::Other("LessThanOneShareDerived")
                }
                ShareBalanceError::ZeroAdjustmentNotAllowed => DispatchError::Other("ZeroAdjustmentNotAllowed"),
            }
        }
    }

    /// Provides the concrete error type for the [`LazyBalance`] system
    ///
    /// This binds the [`LazyBalanceError`] discriminant to [`ShareBalanceError`],
    /// allowing all LazyBalance plugin models with context [`ShareBalanceContext`]
    /// to resolve their error type.
    impl<T> VirtualError<LazyBalanceError> for ShareBalanceContext<T> {
        type Error = ShareBalanceError;
    }

    // ===============================================================================
    // `````````````````````` SHARE-BALANCE MODEL-CHECKER TESTS ``````````````````````
    // ===============================================================================

    #[cfg(test)]
    mod model_checker {

        // ===============================================================================
        // ``````````````````````````````````` IMPORTS ```````````````````````````````````
        // ===============================================================================

        // --- Local module imports ---
        use super::{mock::*, *};

        // --- Scale-codec crates ---
        use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
        use scale_info::TypeInfo;

        // --- Substrate primitives ---
        use sp_runtime::{
            traits::{CheckedAdd, CheckedDiv, CheckedSub, One, Saturating, Zero},
            FixedPointNumber, FixedU128,
        };

        // --- Standard library ---
        use std::{
            collections::BTreeMap,
            env,
            fmt::Debug,
            hash::{DefaultHasher, Hash, Hasher},
            marker::PhantomData,
            path::{Path, PathBuf},
            u128,
        };

        // ===============================================================================
        // `````````````````````````````````` CONSTANTS ``````````````````````````````````
        // ===============================================================================

        // --- Test identities ---

        /// Primary test user (baseline subject).
        const ALICE: UserID = UserID(0u32);

        /// Secondary test user (used in multi-user scenarios).
        #[allow(unused)]
        const BOB: UserID = UserID(1u32);

        /// Additional test user for extended scenarios.
        #[allow(unused)]
        const CHARLIE: UserID = UserID(2u32);

        /// Additional test user for extended scenarios.
        #[allow(unused)]
        const DAVE: UserID = UserID(3u32);

        /// Additional test user for extended scenarios.
        #[allow(unused)]
        const EVE: UserID = UserID(3u32);

        /// Default set of users used in most tests.
        const USERS: &[UserID] = &[ALICE, BOB];

        // --- Deposit test values ---

        /// Comprehensive deposit values covering edge cases, powers, primes, and stress inputs.
        const STRESS_DEPOSITS: &[u128] = &[
            // Identity / base
            0,
            1,
            2,
            3,
            // Powers of two
            4,
            8,
            16,
            32,
            64,
            256,
            1024,
            65536,
            1_048_576,
            // Boundaries (2^n +/- 1)
            7,
            15,
            31,
            63,
            127,
            1023,
            1025,
            65535,
            65537,
            1_048_575,
            1_048_577,
            // Primes (spread out)
            11,
            73,
            101,
            509,
            997,
            5003,
            99_991,
            123_457,
            999_983,
            1_000_003,
            // "Ugly" composites
            6,
            12,
            60,
            120,
            360,
            840,
            2520,
            5040,
            // Patterned numbers
            111,
            333,
            777,
            999,
            // Large awkward values
            16_777_213,
            2_147_483_647,
            // Ratio extremes
            10_000_000,
        ];

        /// Adjustment values used to test proportional changes and edge conditions.
        const STRESS_ADJUSTMENTS: &[u128] = &[
            // Identity
            0, 1, 2, 3, // Powers of two
            4, 8, 16, 32, 64, 128, 256, // Boundaries
            7, 15, 31, 63, 255, 257, 1023, 1025, 2047, 4095, 8191, // Primes
            5, 77, 101, 333, 777, 999, 4093, 8191, 16381, 32749, // Dense composites
            6, 12, 24, 60, 120, 360, // High ratio stress
            10_000, 100_000,
        ];

        // --- Safe test ranges ---

        /// Conservative deposit values that avoid overflow and extreme ratios.
        const PRACTICAL_DEPOSITS: &[u128] = &[500, 750, 1000, 1250, 1500, 1750, 2000, 2500, 3000];

        /// Conservative adjustment values for stable, low-risk test scenarios.
        const PRACTICAL_ADJUSTMENTS: &[u128] = &[50, 75, 100, 150, 200];

        // --- Tolerances ---

        /// Maximum allowed basis points deviation (withdraw drifts between lazy
        /// and manual balance model) for stress tests.
        const STRESS_BPS: u32 = 20;

        /// Maximum allowed absolute difference (withdraw drifts between lazy
        /// and manual balance model) for stress tests.
        const STRESS_DIFF: u32 = 5;

        /// Maximum allowed basis points deviation (withdraw drifts between lazy
        /// and manual balance model) for practical tests.
        const PRACTICAL_BPS: u32 = 10;

        /// Maximum allowed absolute difference (withdraw drifts between lazy
        /// and manual balance model) for practial tests.
        const PRACTICAL_DIFF: u32 = 2;

        // --- Limits ---

        /// Maximum balance-operations sequence depth for model-check scenarios.
        const MAX_DEPTH: u32 = 9;

        // --- Subjects ---

        /// Collection of test subjects which is empty by default, since
        /// [`ShareBalanceFamily`] provides unbounded limits.
        const SUBJECTS: &[TestSubject] = &[];

        /// Resolves a the model-checker results directory path relative to the current source file.
        ///
        /// This function walks up from the current working directory until it finds
        /// the source file corresponding to `file!()`. Once found, it returns a path
        /// by joining the source file's directory with the provided `name`.
        ///
        /// - `name`: The name of the results directory to append.
        ///
        /// ## Example
        /// ```ignore
        /// let path = results_dir("outputs");
        /// ```
        fn results_dir(name: &str) -> PathBuf {
            let rel_file = Path::new(file!());
            let mut base = env::current_dir().unwrap();

            let src_file = loop {
                let candidate = base.join(rel_file);
                if candidate.exists() {
                    break candidate;
                }
                if !base.pop() {
                    panic!("Could not resolve source file path");
                }
            };

            let src_dir = src_file.parent().unwrap();
            src_dir.join(name)
        }

        // ===============================================================================
        // ````````````````````````````````` MODEL-CHECKS ````````````````````````````````
        // ===============================================================================

        #[test]
        #[ignore]
        fn model_practical_check() {
            let mut results = Tester::initiate_results();

            Tester::explore(
                USERS,
                PRACTICAL_DEPOSITS,
                PRACTICAL_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                PRACTICAL_BPS,
                PRACTICAL_DIFF,
                &mut results,
            );

            Tester::write_reports(results_dir("model_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn model_stress_check() {
            let mut results = Tester::initiate_results();

            Tester::explore(
                USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                &mut results,
            );

            Tester::write_reports(results_dir("stress_check"), &results, false, true, false);
        }

        // ===============================================================================
        // ```````````````````````````` TRAP-CHECKS (DEPOSIT) ````````````````````````````
        // ===============================================================================

        #[test]
        #[ignore]
        fn trap_empty_deposit() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |_, op| match op {
                    BalanceOp::Deposit(_, v, _) => {
                        let empty_deposit = v.is_zero();
                        empty_deposit
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::ZeroDepositNotAllowed),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_deposit_after_drain() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Deposit(_, v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        if fresh_balance {
                            return false;
                        }

                        let empty_deposit = v.is_zero();

                        let drained = effective.unwrap().is_zero() && bias.unwrap().is_zero();

                        !empty_deposit && drained
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::BalanceDrainedCannotDeposit),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_almost_zero_share_deposit() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Deposit(user, v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);
                        let issued = balance::issued::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        if fresh_balance {
                            return false;
                        }

                        let empty_deposit = v.is_zero();

                        let drained = effective.unwrap().is_zero() && bias.unwrap().is_zero();

                        let zero_share = {
                            match issued.unwrap().is_zero() {
                                true => false,
                                false => {
                                    let adjusted = issued.unwrap().saturating_sub(One::one());
                                    match effective.unwrap().checked_add(adjusted) {
                                        Some(total) => {
                                            let min_required = total / issued.unwrap();
                                            *v < min_required
                                        }
                                        None => false,
                                    }
                                }
                            }
                        };

                        let duplicate = state.receipts.contains_key(&user);

                        !duplicate && !drained && !empty_deposit && zero_share
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::LessThanOneShareDerived),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_deposit_precision() {
            for _ in STRESS_DEPOSITS {
                let mut results = Tester::initiate_results();

                let traps: TrapConfig = BalanceTraps {
                    trap: |state, op| match op {
                        BalanceOp::Deposit(user, v, _) => {
                            let balance = &state.lazy.balance;
                            let effective = balance::effective::<MockShareBalance>(balance);
                            let bias = balance::bias::<MockShareBalance>(balance);
                            let issued = balance::issued::<MockShareBalance>(balance);

                            let fresh_balance =
                                effective.is_none() && bias.is_none() && issued.is_none();

                            if fresh_balance {
                                return false;
                            }

                            let empty_deposit = v.is_zero();

                            let drained = effective.unwrap().is_zero() && bias.unwrap().is_zero();

                            let zero_share = {
                                match issued.unwrap().is_zero() {
                                    true => false,
                                    false => {
                                        let adjusted = issued.unwrap().saturating_sub(One::one());
                                        match effective.unwrap().checked_add(adjusted) {
                                            Some(total) => {
                                                let min_required = total / issued.unwrap();
                                                *v < min_required
                                            }
                                            None => false,
                                        }
                                    }
                                }
                            };

                            let duplicate = state.receipts.contains_key(&user);

                            let derive_fail = FixedU128::saturating_from_integer(*v)
                                .checked_div(&bias.unwrap())
                                .is_none();

                            !duplicate && !drained && !empty_deposit && !zero_share && derive_fail
                        }
                        _ => false,
                    },

                    flow: |_, _| true,

                    reason: format!("{:?}", ShareBalanceError::InadequatePrecision),
                };

                Tester::explore_traps(
                    &USERS,
                    STRESS_DEPOSITS,
                    STRESS_ADJUSTMENTS,
                    SUBJECTS,
                    (MAX_DEPTH + 3).min(11),
                    STRESS_BPS,
                    STRESS_DIFF,
                    Some(traps),
                    &mut results,
                );

                Tester::write_reports(results_dir("trap_check"), &results, false, false, false);

                if !results.trap.is_empty() {
                    break;
                }
            }
        }

        #[test]
        #[ignore]
        fn trap_manual_balance_duplicate_deposit() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Deposit(user, v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);
                        let issued = balance::issued::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        if fresh_balance {
                            return false;
                        }

                        let empty_deposit = v.is_zero();

                        let drained = effective.unwrap().is_zero() && bias.unwrap().is_zero();

                        let zero_share = {
                            match issued.unwrap().is_zero() {
                                true => false,
                                false => {
                                    let adjusted = issued.unwrap().saturating_sub(One::one());
                                    match effective.unwrap().checked_add(adjusted) {
                                        Some(total) => {
                                            let min_required = total / issued.unwrap();
                                            *v < min_required
                                        }
                                        None => false,
                                    }
                                }
                            }
                        };

                        let duplicate = state.receipts.contains_key(&user);

                        let derive_fail = FixedU128::saturating_from_integer(*v)
                            .checked_div(&bias.unwrap())
                            .is_none();

                        !drained && !empty_deposit && !zero_share && !derive_fail && duplicate
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ManualError::DuplicateDeposit),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            if results.trap.is_empty() {
                panic!("None Trapped");
            }

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        // ===============================================================================
        // ```````````````````````````` TRAP-CHECKS (WITHDRAW) ```````````````````````````
        // ===============================================================================

        #[test]
        #[ignore]
        fn trap_unknown_withdrawal_for_manual() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Withdraw(user) => {
                        let exists = state.receipts.contains_key(&user);
                        !exists
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: "ModelChecker::WithdrawReceiptMissing".to_string(),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        // ===============================================================================
        // `````````````````````````````` TRAP-CHECKS (MINT) `````````````````````````````
        // ===============================================================================

        #[test]
        #[ignore]
        fn trap_mint_fresh_balance() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        let zero_mint = v.is_zero();

                        fresh_balance && !zero_mint
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::BalanceNotInitiatedViaDeposit),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_manual_fresh_balance_zero_mint() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        let zero_mint = v.is_zero();

                        fresh_balance && zero_mint
                    }
                    _ => false,
                },

                flow: |state, op| match op {
                    BalanceOp::Mint(..) => {
                        if state.trace.len().is_zero() {
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                },

                reason: format!("{:?}", ManualError::MintWithoutDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                &[0],
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_mint_without_deposits() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let no_receipts = state.receipts.is_empty();

                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        // lazy balance accepts zero mint but manual doesn't
                        let zero_mint = v.is_zero();

                        no_receipts && !fresh_balance && !zero_mint
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::RequiresExistingDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_manual_balance_without_deposit_zero_mint() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let no_receipts = state.receipts.is_empty();

                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        let zero_mint = v.is_zero();

                        no_receipts && !fresh_balance && zero_mint
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ManualError::MintWithoutDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_zero_mint_fresh_balance_manual_trap() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh_balance = effective.is_none() && bias.is_none();

                        let zero_mint = v.is_zero();

                        fresh_balance && zero_mint
                    }
                    _ => false,
                },

                flow: |state, op| match op {
                    BalanceOp::Mint(..) => {
                        if state.trace.len().is_zero() {
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                },

                reason: format!("{:?}", ManualError::MintWithoutDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                &[0],
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_big_width_mint() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return false;
                        }

                        let overflow = v.checked_add(&effective.unwrap()).is_none();

                        let no_deposits = state.receipts.is_empty();

                        let zero_manual = state.manual.total_fixed().is_zero();

                        let manual_no_users = state.manual.users.is_empty();

                        let manual_drain_shares = state.manual.before_drain.is_some();

                        let manual_collapse =
                            !manual_no_users && zero_manual && !manual_drain_shares;

                        !overflow && !no_deposits && !manual_collapse
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::InadequatePrecision),
            };

            let adjustments = {
                let mut v = STRESS_ADJUSTMENTS.to_vec();
                v.push(u128::MAX);
                v
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                &adjustments,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_mint_overflow() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return false;
                        }

                        let overflow = v.checked_add(&effective.unwrap()).is_none();

                        let no_deposits = state.receipts.is_empty();

                        overflow && !no_deposits
                    }
                    _ => false,
                },

                flow: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let mut drained = false;
                        let mut reaped_till = u128::zero();
                        for op in state.trace.iter().rev() {
                            match op {
                                BalanceOp::Drain => {
                                    drained = true;
                                    break;
                                }
                                BalanceOp::Deposit(_, v, _) => {
                                    if !v.is_zero() && *v > reaped_till {
                                        break;
                                    } else {
                                        drained = true;
                                        break;
                                    }
                                }
                                BalanceOp::Reap(v, _) => reaped_till += v,
                                BalanceOp::Mint(v, _) => {
                                    if !v.is_zero() && *v > reaped_till {
                                        break;
                                    } else {
                                        drained = true;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if drained && *v == u128::MAX {
                            return false;
                        }
                        true
                    }
                    _ => true,
                },

                reason: format!("{:?}", ShareBalanceError::AssetOverflow),
            };

            let adjustments = {
                let mut v = STRESS_ADJUSTMENTS.to_vec();
                v.push(u128::MAX);
                v
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                &adjustments,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_manual_collapse() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Mint(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return false;
                        }

                        let zero_value = v.is_zero();

                        let overflow = v.checked_add(&effective.unwrap()).is_none();

                        let no_deposits = state.receipts.is_empty();

                        let zero_manual = state.manual.total_fixed().is_zero();

                        let manual_no_users = state.manual.users.is_empty();

                        let manual_drain_shares = state.manual.before_drain.is_some();

                        let manual_collapse =
                            !manual_no_users && zero_manual && !manual_drain_shares;

                        !zero_value && !overflow && !no_deposits && manual_collapse
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ManualError::CollapsedState),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                (MAX_DEPTH + 2).min(10),
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        // ===============================================================================
        // `````````````````````````````` TRAP-CHECKS (REAP) `````````````````````````````
        // ===============================================================================

        #[test]
        #[ignore]
        fn trap_reap_underflow() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Reap(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return false;
                        }

                        let no_deposits = state.receipts.is_empty();

                        let lazy_underflow = effective.unwrap().checked_sub(*v).is_none();

                        let manual_underflow = state.manual.total().checked_sub(*v).is_none();

                        !no_deposits && lazy_underflow && manual_underflow
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::AssetUnderflow),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_reap_without_deposits() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Reap(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return false;
                        }

                        let no_deposits = state.receipts.is_empty();

                        let zero_value = v.is_zero();

                        no_deposits && !zero_value
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::RequiresExistingDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_manual_zero_reap_without_deposits() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Reap(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return false;
                        }

                        let no_deposits = state.receipts.is_empty();

                        let zero_value = v.is_zero();

                        no_deposits && zero_value
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ManualError::ReapWithoutDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_reap_fresh_balance() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Reap(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh = effective.is_none() && bias.is_none();

                        let zero_value = v.is_zero();

                        fresh & !zero_value
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ShareBalanceError::BalanceNotInitiatedViaDeposit),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        #[test]
        #[ignore]
        fn trap_manual_zero_reap_fresh_balance() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Reap(v, _) => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        let fresh = effective.is_none() && bias.is_none();

                        let zero_value = v.is_zero();

                        fresh & zero_value
                    }
                    _ => false,
                },

                flow: |state, op| match op {
                    BalanceOp::Reap(..) => {
                        if state.trace.len().is_zero() {
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                },

                reason: format!("{:?}", ManualError::ReapWithoutDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                &[0],
                SUBJECTS,
                (MAX_DEPTH + 2).min(10),
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        // ===============================================================================
        // ````````````````````````````` TRAP-CHECKS (DRAIN) `````````````````````````````
        // ===============================================================================

        #[test]
        #[ignore]
        fn trap_manual_zero_drain() {
            let mut results = Tester::initiate_results();

            let traps: TrapConfig = BalanceTraps {
                trap: |state, op| match op {
                    BalanceOp::Drain => {
                        let balance = &state.lazy.balance;
                        let effective = balance::effective::<MockShareBalance>(balance);
                        let bias = balance::bias::<MockShareBalance>(balance);

                        if effective.is_none() && bias.is_none() {
                            return true;
                        }

                        let zero_effective = effective.unwrap().is_zero();

                        zero_effective
                    }
                    _ => false,
                },

                flow: |_, _| true,

                reason: format!("{:?}", ManualError::DrainWithoutDeposits),
            };

            Tester::explore_traps(
                &USERS,
                STRESS_DEPOSITS,
                STRESS_ADJUSTMENTS,
                SUBJECTS,
                MAX_DEPTH,
                STRESS_BPS,
                STRESS_DIFF,
                Some(traps),
                &mut results,
            );

            Tester::write_reports(results_dir("trap_check"), &results, false, false, false);
        }

        // ===============================================================================
        // ````````````````````````` MODEL-CHECKER UTILITY IMPLS `````````````````````````
        // ===============================================================================

        /// Concrete test harness implementing [`LazyBalanceModelChecker`].
        struct Tester;

        impl LazyBalanceModelChecker for Tester {
            /// The lazy (optimized) balance model under test.
            type LazyBalance = MockShareBalance;

            /// The manual/reference implementation used for verification.
            type ManualBalance = ManualBalance<MockShareBalance>;

            /// Predicate that detects invalid or trap states.
            type TrapFn = fn(
                &BalanceState<Self::LazyBalance, Self::ManualBalance>,
                &BalanceOp<Self::LazyBalance, Self::ManualBalance>,
            ) -> bool;

            /// Predicate that validates whether a state transition is allowed.
            type FlowFn = fn(
                &BalanceState<Self::LazyBalance, Self::ManualBalance>,
                &BalanceOp<Self::LazyBalance, Self::ManualBalance>,
            ) -> bool;

            /// Additional Hashing function used to identify or deduplicate states.
            ///
            /// Although not utilized in current test-cases.
            type Hasher = fn(&BalanceState<Self::LazyBalance, Self::ManualBalance>) -> u64;
        }

        /// Configuration type for balance trap handling.
        ///
        /// Combines:
        /// - Trap predicate ([`LazyBalanceModelChecker::TrapFn`]) detects invalid states
        /// - Flow predicate ([`LazyBalanceModelChecker::FlowFn`]) validates allowed op-sequences
        type TrapConfig = BalanceTraps<
            <Tester as LazyBalanceModelChecker>::TrapFn,
            <Tester as LazyBalanceModelChecker>::FlowFn,
        >;

        /// Simple User type from a given [`ManualBalanceModel::User`].
        type User<T> = <ManualBalance<T> as ManualBalanceModel<T>>::User;

        /// Asset type associated from [`LazyBalance::Asset`].
        type AssetOf<T> = <T as LazyBalance>::Asset;

        /// Receipt (deposit bill) type associated from [`LazyBalance::Receipt`].
        type ReceiptOf<T> = <T as LazyBalance>::Receipt;

        /// Hashes a [`BalanceState`] based on its execution trace (sequence of operations),
        /// producing a compact identifier for path-based state exploration.
        ///
        /// ## Model Context
        /// In our share balance models ([`ShareBalanceFamily`]), all operations are
        /// **value-insensitive*, their correctness depends only on the sequence and type
        /// of operations, not on the specific numeric values involved.
        ///
        /// Because of this, we intentionally hash only the **operation trace**
        /// (`state.trace`) and ignore parameters.
        ///
        /// ## Design choice
        /// This is a **path-based hashing strategy**, not a full state hash:
        /// - Efficient and lightweight
        /// - Correct for value-insensitive models like [`ShareBalanceFamily`]
        /// - Does NOT distinguish different parameter values
        ///
        /// ## Example
        /// ```ignore
        /// // First exploration:
        /// Deposit(10) -> Withdraw(5)
        /// -> trace: [Deposit, Withdraw]
        /// -> hash stored
        ///
        /// // Later:
        /// Deposit(1000) -> Withdraw(1)
        /// -> trace: [Deposit, Withdraw]
        /// -> same hash -> skipped (already explored)
        /// ```
        impl<T, M> BalanceStateHasher<T, M> for ManualBalance<T>
        where
            T: LazyBalance + Clone + Debug,
            M: ManualBalanceModel<T>,
            M::User: Hash,
        {
            /// Computes a hash for the given state based solely on its operation trace.
            ///
            /// Each operation contributes a fixed discriminator:
            /// - Deposit -> 0
            /// - Withdraw -> 1
            /// - Mint -> 2
            /// - Reap -> 3
            /// - Drain -> 4
            ///
            /// The resulting hash uniquely identifies the **sequence of operations**
            /// (not the resulting balances), and is used by the model checker to
            /// detect and skip already-explored execution paths.
            fn hash(state: &BalanceState<T, M>) -> u64 {
                let mut h = DefaultHasher::new();

                for op in &state.trace {
                    match op {
                        BalanceOp::Deposit(..) => {
                            0u8.hash(&mut h);
                        }
                        BalanceOp::Withdraw(_) => {
                            1u8.hash(&mut h);
                        }
                        BalanceOp::Mint(..) => {
                            2u8.hash(&mut h);
                        }
                        BalanceOp::Reap(..) => {
                            3u8.hash(&mut h);
                        }
                        BalanceOp::Drain => {
                            4u8.hash(&mut h);
                        }
                    }
                }

                h.finish()
            }
        }

        /// Guard implementation for validating balance operations of [`ShareBalanceFamily`]
        /// and its manual model [`ManualBalance`].
        ///
        /// These guards define whether a given operation is **allowed to proceed**
        /// from the current [`BalanceState`]. They act as preconditions that ensure
        /// only valid transitions are explored during model checking.
        impl<T> BalanceGuards<T, ManualBalance<T>> for ManualBalance<T>
        where
            T: LazyBalance + Clone + Debug,
            <T as LazyBalance>::Asset: From<u128>,
        {
            /// Validates whether a deposit operation is allowed for the given state.
            ///
            /// ## Checks performed
            /// - Rejects deposits into a **drained state**
            /// - Prevents **duplicate deposits** for the same user
            /// - Disallows **zero-value deposits**
            /// - Ensures deposit is large enough to produce a valid share
            /// - Prevents arithmetic failures during ratio derivation
            ///
            /// ## Behavior
            /// - If the system is uninitialized (no effective, bias, issued),
            ///   any non-zero deposit is allowed.
            ///
            /// ## Returns
            /// - `true`: [`BalanceOp::Deposit`] is valid and can be applied
            /// - `false`: deposit is invalid and should be skipped
            fn deposit(
                state: &BalanceState<T, ManualBalance<T>>,
                user: &User<T>,
                amount: &T::Asset,
                _subject: &T::Subject,
            ) -> bool {
                let balance = &state.lazy.balance;
                let effective = balance::effective::<T>(balance);
                let bias = balance::bias::<T>(balance);
                let issued = balance::issued::<T>(balance);

                // Initial state: allow any non-zero deposit (Fast-track)
                if effective.is_none() && bias.is_none() && issued.is_none() {
                    if amount.is_zero() {
                        return false;
                    }
                    return true;
                }

                // State is considered drained if both effective and bias are zero
                let drained = effective.unwrap().is_zero() && bias.unwrap().is_zero();

                // Prevent duplicate deposits from the same user
                let duplicate = state.receipts.contains_key(user);

                // Reject zero-value deposits
                let zero_deposit = amount.is_zero();

                // Reject deposits that would result in zero share issuance
                let zero_share = {
                    match issued.unwrap().is_zero() {
                        true => false,
                        false => {
                            let adjusted = issued.unwrap().saturating_sub(One::one());
                            match effective.unwrap().checked_add(&adjusted) {
                                Some(total) => {
                                    let min_required = total / issued.unwrap();
                                    *amount < min_required
                                }
                                None => false,
                            }
                        }
                    }
                };

                // Prevent failure in rational derivation (e.g. division by zero)
                let derive_fail = T::Rational::saturating_from_integer(*amount)
                    .checked_div(&bias.unwrap())
                    .is_none();

                !drained && !duplicate && !zero_deposit && !zero_share && !derive_fail
            }

            /// Validates whether a withdraw operation is allowed for the given state.
            ///
            /// ## Checks performed
            /// - Ensures the user has an existing receipt (i.e. has previously deposited)
            /// (tracked via `state.receipts`).
            ///
            /// ## Returns
            /// - `true`: [`BalanceOp::Withdraw`] is valid and can be applied
            /// - `false`: withdraw is invalid and should be skipped
            fn withdraw(state: &BalanceState<T, ManualBalance<T>>, user: &User<T>) -> bool {
                let exists = state.receipts.contains_key(user);
                exists
            }

            /// Validates whether a mint operation is allowed for the given state.
            ///
            /// ## Checks performed
            /// - Rejects minting in an uninitialized state (no effective or bias)
            /// - Disallows zero-value mint operations
            /// - Ensures there is at least one active deposit (non-empty receipts)
            /// - Prevents arithmetic overflow when updating effective balance
            /// - Avoids inconsistent manual model states (e.g. collapsed shares)
            ///
            /// ## Returns
            /// - `true`: [`BalanceOp::Mint`] is valid and can be applied
            /// - `false`: mint is invalid and should be skipped
            fn mint(
                state: &BalanceState<T, ManualBalance<T>>,
                value: &T::Asset,
                _subject: &T::Subject,
            ) -> bool {
                let balance = &state.lazy.balance;
                let effective = balance::effective::<T>(balance);
                let bias = balance::bias::<T>(balance);

                // Reject if system is not initialized
                if effective.is_none() && bias.is_none() {
                    return false;
                }

                let zero_value = value.is_zero();

                // No deposits -> nothing to mint against
                let no_deposits = state.receipts.is_empty();

                // Prevent overflow when increasing effective balance
                let overflow = value.checked_add(&effective.unwrap()).is_none();

                // Manual model consistency checks
                let zero_manual = state.manual.total_fixed().is_zero();
                let manual_no_users = state.manual.users.is_empty();
                let manual_drain_shares = state.manual.before_drain.is_some();

                // Detect collapsed manual state (invalid share distribution)
                let manual_collapse = !manual_no_users && zero_manual && !manual_drain_shares;

                !zero_value && !no_deposits && !overflow && !manual_collapse
            }

            /// Validates whether a reap operation is allowed for the given state.
            ///
            /// ## Checks performed
            /// - Rejects reaping in an uninitialized state (no effective or bias)
            /// - Disallows zero-value reap operations
            /// - Ensures there is at least one active deposit (non-empty receipts)
            /// - Prevents underflow in the lazy model (effective balance)
            /// - Prevents underflow in the manual model (total balance)
            ///
            /// ## Behavior
            /// - Reaping is only allowed when the system is initialized and active
            /// - The operation must not reduce balances below zero (underflow) in
            /// either model
            ///
            /// ## Returns
            /// - `true`: [`BalanceOp::Reap`] is valid and can be applied
            /// - `false`: reap is invalid and should be skipped
            fn reap(
                state: &BalanceState<T, ManualBalance<T>>,
                value: &T::Asset,
                _subject: &T::Subject,
            ) -> bool {
                let balance = &state.lazy.balance;
                let effective = balance::effective::<T>(balance);
                let bias = balance::bias::<T>(balance);

                // Reject if system is not initialized
                if effective.is_none() && bias.is_none() {
                    return false;
                }

                let zero_value = value.is_zero();

                // No deposits -> nothing to reap from
                let no_deposits = state.receipts.is_empty();

                // Prevent underflow in lazy model
                let lazy_underflow = effective.unwrap().checked_sub(value).is_none();

                // Prevent underflow in manual model
                let manual_underflow = state.manual.total().checked_sub(value).is_none();

                !zero_value && !no_deposits && !lazy_underflow && !manual_underflow
            }

            /// Validates whether a drain operation is allowed for the given state.
            ///
            /// ## Checks performed
            /// - Ensures there is at least one active deposit (non-empty receipts)
            ///
            /// ## Behavior
            /// - Drain is only meaningful when there are existing deposits to clear
            /// - If no users have deposited, the operation is skipped
            ///
            /// ## Returns
            /// - `true`: [`BalanceOp::Drain`] is valid and can be applied
            /// - `false`: drain is invalid and should be skipped
            fn drain(state: &BalanceState<T, ManualBalance<T>>) -> bool {
                let no_deposits = state.receipts.is_empty();
                !no_deposits
            }

            /// Validates core invariants of the balance state.
            ///
            /// ## Invariant enforced
            /// - If there are active deposits (i.e. receipts exist),
            ///   then the total shares (`issued`) must be non-zero.
            ///
            /// ## Rationale
            /// In the [`ShareBalanceFamily`] model:
            /// - Deposits correspond to issued shares
            /// - Therefore, the existence of deposits implies that shares must exist
            ///
            /// A state where:
            /// - deposits exist, but
            /// - total shares are zero
            ///
            /// is considered invalid and indicates a broken or collapsed system state.
            ///
            /// ## Behavior
            /// - If no deposits exist, then invariant is trivially satisfied
            /// - If deposits exist:
            ///   - Ensures the `issued` (total shares) field is present
            ///   - Ensures the `issued` is non-zero
            fn invariant(state: &BalanceState<T, ManualBalance<T>>) -> Result<(), String> {
                // If Deposits Exists, Total Shares of Balance cannot be zero
                if !state.receipts.is_empty() {
                    let balance = &state.lazy.balance;
                    let repr = <T::Balance as VirtualDynField<BalanceAsset>>::access(balance);
                    let vec = IntoTag::<_, ManyTag>::into_tag(repr);
                    let Some(principal) = vec.as_ref().get(1) else {
                        return Err("Invariant::TotalSharesMissing".to_string());
                    };
                    if *principal == Zero::zero() {
                        return Err("Invariant::ZeroTotalShares".to_string());
                    }
                }
                Ok(())
            }
        }

        // ===============================================================================
        // ```````````````````````````````` MANUAL BALANCE ```````````````````````````````
        // ===============================================================================

        /// A simple counter-based User-ID wrapper
        #[derive(
            Encode,
            Decode,
            Debug,
            Clone,
            Copy,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            DecodeWithMemTracking,
            MaxEncodedLen,
            TypeInfo,
        )]
        struct UserID(u32);

        /// Manual (reference) balance model used for verification against the lazy model.
        ///
        /// This struct maintains an explicit, user-level representation of balances,
        /// serving as a **ground truth** for validating correctness of [`ShareBalanceFamily`]
        /// lazy balance model.
        ///
        /// ## Representation
        /// - User balances are stored as [`FixedU128`] with maximum precision
        /// - When interacting with the lazy model, values are **floored** to the
        ///   underlying asset type ([`LazyBalance::Asset`])
        /// - No explicit "share" abstraction exists here, to assume subjectively:
        ///   - Each user balance directly represents their proportional ownership
        ///   - The total balance represents the total capital (i.e. sum of all shares)
        ///
        /// ## Ledger Model (Intuition)
        /// This model behaves like a **ledger book**:
        /// - Every operation (e.g. `mint`, `reap`) is immediately reflected across
        ///   all user balances
        /// - Each user's balance always represents their up-to-date proportional ownership
        ///
        /// This makes the system:
        /// - Simple and explicit
        /// - Easy to reason about
        /// - Straightforward to validate
        ///
        /// In contrast, the [`LazyBalance`] uses **deferred receipt-based accounting**,
        /// where withdrawals are derived indirectly. This manual model provides a
        /// clear baseline to verify those deferred computations.
        ///
        /// ## Drain / Revival Semantics
        /// - A `drain` operation removes all balances (system reset, no shares remain)
        /// - Before draining, user balances are stored in `before_drain`
        /// - When the system is revived (e.g. via `mint`):
        ///   - Balances are **reconstructed proportionally**
        ///   - Distribution is based on the pre-drain snapshot
        ///   - This ensures continuity of ownership without explicit share tracking
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct ManualBalance<T>
        where
            T: LazyBalance,
        {
            /// Mapping of users to their high-precision proportional balances.
            ///
            /// Each value represents the user's share of total capital directly,
            /// without a separate share abstraction.
            users: BTreeMap<UserID, FixedU128>,

            /// Snapshot of user balances before a drain operation.
            ///
            /// Used to restore proportional ownership when the system is
            /// revived and immediately prunes itself to `None`.
            before_drain: Option<BTreeMap<UserID, FixedU128>>,

            /// Marker for [`LazyBalance`].
            _marker: PhantomData<T>,
        }

        impl<T> ManualBalance<T>
        where
            T: LazyBalance,
        {
            /// Creates an empty manual balance state.
            fn new() -> Self {
                Self {
                    users: BTreeMap::new(),
                    before_drain: None,
                    _marker: PhantomData,
                }
            }

            /// Returns the total balance across all users (high-precision)
            ///
            /// [`ManualBalanceModel::total`] may utilize this and floor to give a unsigned
            /// asset balance value.
            fn total_fixed(&self) -> FixedU128 {
                self.users.values().fold(FixedU128::zero(), |a, b| a + *b)
            }
        }
        /// Errors representing invalid operations or states in the manual balance model.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum ManualError {
            /// Deposit attempted for a user that already has a position.
            DuplicateDeposit,

            /// Mint attempted when no deposits exist.
            MintWithoutDeposits,

            /// Reap attempted when no deposits exist.
            ReapWithoutDeposits,

            /// Drain attempted when no deposits exist.
            DrainWithoutDeposits,

            /// Deposit with zero value is not allowed.
            ZeroDeposit,

            /// Withdraw attempted after a drain without a valid
            /// `before_drain` snapshot to restore balances.
            WithdrawAfterDrainedSnapshotNotFound,

            /// Withdraw attempted by a user with no recorded deposit.
            ///
            /// In the lazy model, withdrawals operate on receipts without explicit
            /// user identity. The manual model requires a corresponding user entry,
            /// so a missing deposit makes the operation invalid.
            WithdrawWithoutDeposit,

            /// Invalid collapsed state where users exist but total balance is zero.
            ///
            /// This can occur due to precision differences between:
            /// - lazy model (share-based, integer)
            /// - manual model (fixed-point, proportional)
            ///
            /// In extreme cases, a "silent full reap" may occur:
            /// - all value is effectively removed
            /// - but no `before_drain` snapshot was captured
            ///
            /// This creates ambiguity, as the system appears drained without
            /// explicit drain semantics.
            ///
            /// [`ShareBalanceFamily`] handles this internally, but the manual model
            /// cannot reliably detect it without duplicating core logic. Hence,
            /// this condition is surfaced as an error and mitigated via guards/traps.
            CollapsedState,
        }

        impl<T> ManualBalanceModel<T> for ManualBalance<T>
        where
            T: LazyBalanceMarker,
            <T as LazyBalance>::Asset: From<u128>,
        {
            /// User identifier type.
            type User = UserID;

            /// Creates a new empty manual balance model.
            fn new() -> Self {
                ManualBalance::new()
            }

            /// Returns total balance floored to asset representation.
            fn total(&self) -> AssetOf<T> {
                let total_fixed = self.total_fixed();
                (total_fixed.into_inner() / FixedU128::DIV).into()
            }

            /// Error type for manual model operations.
            type Error = ManualError;

            /// Adds a new user with an initial balance.
            ///
            /// - Rejects duplicate users
            /// - Rejects zero deposits
            fn deposit(
                &mut self,
                id: Self::User,
                amount: AssetOf<T>,
                _lazy: &(AssetOf<T>, ReceiptOf<T>),
            ) -> Result<(), Self::Error> {
                if self.users.contains_key(&id) {
                    return Err(ManualError::DuplicateDeposit);
                }

                if amount.is_zero() {
                    return Err(ManualError::ZeroDeposit);
                }

                self.users
                    .insert(id, FixedU128::saturating_from_integer(amount));

                self.before_drain = None;

                Ok(())
            }

            /// Removes a user and returns their balance.
            ///
            /// - Validates existence of user
            /// - Handles post-drain snapshot consistency
            fn withdraw(
                &mut self,
                id: Self::User,
                _lazy: &AssetOf<T>,
            ) -> Result<AssetOf<T>, Self::Error> {
                if let Some(snapshot) = &mut self.before_drain {
                    snapshot
                        .remove(&id)
                        .ok_or(ManualError::WithdrawAfterDrainedSnapshotNotFound)?;
                }

                let fixed = self
                    .users
                    .remove(&id)
                    .ok_or(ManualError::WithdrawWithoutDeposit)?;

                Ok((fixed.into_inner() / FixedU128::DIV).into())
            }

            /// Distributes value proportionally across all users.
            ///
            /// - Requires existing deposits
            /// - Uses proportional distribution based on current balances
            /// - If total is zero (post-drain), uses `before_drain` snapshot
            fn mint(&mut self, value: AssetOf<T>, _lazy: &AssetOf<T>) -> Result<(), Self::Error> {
                if self.users.is_empty() {
                    return Err(ManualError::MintWithoutDeposits);
                }

                if value.is_zero() {
                    return Ok(());
                }

                let total_before = self.total_fixed();
                let v = FixedU128::saturating_from_integer(value);

                // Revival path after drain
                if total_before.is_zero() {
                    let shares = self
                        .before_drain
                        .as_ref()
                        .ok_or(ManualError::CollapsedState)?;

                    let total_shares = shares.values().fold(Zero::zero(), |a: FixedU128, b| a + *b);

                    if total_shares.is_zero() {
                        return Ok(());
                    }

                    for (id, bal) in self.users.iter_mut() {
                        let weight = shares.get(id).cloned().unwrap_or_default();
                        let gain = (weight / total_shares) * v;
                        *bal = *bal + gain;
                    }

                    return Ok(());
                }

                // Normal proportional mint
                for bal in self.users.values_mut() {
                    let gain = (*bal / total_before) * v;
                    *bal = *bal + gain;
                }

                self.before_drain = None;

                Ok(())
            }

            /// Removes value proportionally from all users.
            ///
            /// - Requires existing deposits
            /// - Performs proportional reduction
            /// - Converts to drain if full depletion
            fn reap(&mut self, value: AssetOf<T>, _lazy: &AssetOf<T>) -> Result<(), Self::Error> {
                if self.users.is_empty() {
                    return Err(ManualError::ReapWithoutDeposits);
                }

                if value.is_zero() {
                    return Ok(());
                }

                let total_before = self.total_fixed();
                let total_int = (total_before.into_inner() / FixedU128::DIV).into();

                // Full reap -> drain
                if value >= total_int {
                    self.drain()?;
                    return Ok(());
                }

                let v = FixedU128::saturating_from_integer(value);

                for bal in self.users.values_mut() {
                    let loss = (*bal / total_before) * v;
                    *bal = *bal - loss;
                }

                Ok(())
            }

            /// Resets all balances to zero while preserving proportional snapshot.
            ///
            /// - Stores pre-drain balances in `before_drain`
            /// - Used for later proportional revival via mint
            fn drain(&mut self) -> Result<(), Self::Error> {
                if self.users.is_empty() {
                    return Err(ManualError::DrainWithoutDeposits);
                }

                if self.total_fixed().is_zero() {
                    return Ok(());
                }

                self.before_drain = Some(self.users.clone());

                for bal in self.users.values_mut() {
                    *bal = FixedU128::zero();
                }

                Ok(())
            }
        }
    }

    // ===============================================================================
    // ````````````````````````` LAZY BALANCE MOCK PROVIDERS ````````````````````````
    // ===============================================================================

    #[cfg(test)]
    /// Mock Providers implementing [`LazyBalance`] using [`ShareBalanceFamily`]
    mod mock {

        // ===============================================================================
        // ``````````````````````````````````` IMPORTS ```````````````````````````````````
        // ===============================================================================

        // --- Local ---
        use super::*;

        // --- Scale / codec ---
        use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
        use scale_info::TypeInfo;

        // --- FRAME Suite ---
        use frame_suite::{misc::Extent, plugin_context};

        // --- FRAME Support ---
        use frame_support::{
            pallet_prelude::NMapKey,
            storage::types::{OptionQuery, StorageNMap},
            traits::StorageInstance,
            Blake2_128Concat,
        };

        // --- Substrate ---
        use sp_core::ConstU32;
        use sp_runtime::FixedU128;

        // --- std ---
        use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, marker::PhantomData};

        // ===============================================================================
        // `````````````````````````````` MOCK SHARE BALANCE `````````````````````````````
        // ===============================================================================

        #[derive(Debug, Clone, Default)]
        /// Implements mock [`LazyBalance`] by utilizing [`ShareBalanceFamily`]
        pub struct MockShareBalance;

        impl LazyBalance for MockShareBalance {
            type Asset = u128;
            type Rational = FixedU128;
            type Time = u32;
            type Variant = u8;
            type Id = u8;
            type Subject = TestSubject;

            type Balance = TestBalance;
            type SnapShot = TestSnapshot;
            type Receipt = TestReceipt;
            type Limits = TestLimit;

            type Input<'a> = TestInput<'a>;
            type Output<'a> = TestOutput<'a>;

            type BalanceContext = MyShareBalance<Self>;
            type BalanceFamily<'a> = ShareBalanceFamily<'a>;
        }

        plugin_context! {
            name: pub MyShareBalance,
            context: ShareBalanceContext<T>,
            marker: [T],
            value: ShareBalanceContext(PhantomData)
        }

        // ===============================================================================
        // `````````````````````` MOCK LAZY-BALANCE VIRTUAL STRUCTS ``````````````````````
        // ===============================================================================

        /// Mock backing storage for [`LazyBalance::Balance`] using a virtual schema.
        ///
        /// Fields are accessed via [`VirtualDynField`] and encoded using [`SumDynType`],
        /// a convenient default virtual-field representation.
        ///
        /// Layout (by convention):
        /// - `asset[0]` -> effective
        /// - `asset[1]` -> issued
        /// - `bias[0]`  -> price per share
        /// - `time[0]`  -> checkpoint
        /// - `time[1]`  -> drainpoint
        #[derive(
            Clone,
            Default,
            Debug,
            Eq,
            PartialEq,
            Encode,
            Decode,
            DecodeWithMemTracking,
            TypeInfo,
            MaxEncodedLen,
        )]
        pub struct TestBalance {
            /// Asset fields (effective, issued)
            pub asset: SumDynType<u128, ConstU32<2>>,

            /// Price per share (bias)
            pub bias: SumDynType<FixedU128, ConstU32<1>>,

            /// Time fields (checkpoint, drainpoint)
            pub time: SumDynType<u32, ConstU32<2>>,
        }

        /// Mock backing storage for [`LazyBalance::SnapShot`] using a virtual schema.
        ///
        /// Fields are accessed via [`VirtualDynField`] and encoded using [`SumDynType`],
        /// a convenient default virtual-field representation.
        ///
        /// Layout (by convention):
        /// - `bias[0]` -> price per share at snapshot
        #[derive(
            Clone,
            Default,
            Debug,
            Eq,
            PartialEq,
            Encode,
            Decode,
            DecodeWithMemTracking,
            TypeInfo,
            MaxEncodedLen,
        )]
        pub struct TestSnapshot {
            /// Snapshot bias (price per share)
            pub bias: SumDynType<FixedU128, ConstU32<1>>,
        }

        /// Mock backing storage for [`LazyBalance::Receipt`] using a virtual schema.
        ///
        /// Fields are accessed via [`VirtualDynField`] and encoded using [`SumDynType`],
        /// a convenient default virtual-field representation.
        ///
        /// Layout (by convention):
        /// - `asset[0]` -> principal (original deposit)
        /// - `asset[1]` -> shares (ownership units)
        /// - `bias[0]`  -> deposit-time price per share
        /// - `time[0]`  -> checkpoint (time anchor)
        #[derive(
            Clone,
            Default,
            Debug,
            Eq,
            PartialEq,
            Encode,
            Decode,
            DecodeWithMemTracking,
            TypeInfo,
            MaxEncodedLen,
        )]
        pub struct TestReceipt {
            /// Asset fields (deposit-value, shares)
            pub asset: SumDynType<u128, ConstU32<2>>,

            /// Deposit-time price per share
            pub bias: SumDynType<FixedU128, ConstU32<1>>,

            /// Time field (checkpoint)
            pub time: SumDynType<u32, ConstU32<1>>,
        }

        /// Mock backing storage for [`LazyBalance::Limits`] using a virtual schema.
        ///
        /// Fields are accessed via [`VirtualDynField`] and encoded using [`SumDynType`],
        /// a convenient default virtual-field representation.
        ///
        /// Used with [`Extent`] to express optional bounds.
        ///
        /// Layout (by convention):
        /// - `asset[0]` -> minimum
        /// - `asset[1]` -> maximum
        /// - `asset[2]` -> optimal
        #[derive(
            Clone,
            Default,
            Debug,
            Eq,
            PartialEq,
            Encode,
            Decode,
            DecodeWithMemTracking,
            TypeInfo,
            MaxEncodedLen,
        )]
        pub struct TestLimit {
            /// Asset bounds (min, max, optimal)
            pub asset: SumDynType<u128, ConstU32<3>>,
        }

        /// Mock implementation of [`Directive`] for [`LazyBalance::Subject`] execution.
        ///
        /// Encodes execution preferences:
        /// - `precise` -> [`Precision::Exact`] vs [`Precision::BestEffort`]
        /// - `force`   -> [`Fortitude::Force`] vs [`Fortitude::Polite`]
        ///
        /// In [`ShareBalanceFamily`], limits are unbounded, so these flags have no
        /// practical effect and are primarily included for interface completeness.
        #[derive(
            Clone, Eq, PartialEq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
        )]
        pub struct TestSubject {
            /// Precision preference (exact vs best-effort)
            pub precise: bool,

            /// Execution strictness (force vs polite)
            pub force: bool,
        }

        /// Custom debug output for [`TestSubject`].
        ///
        /// In [`ShareBalanceFamily`], balance operations are effectively unbounded
        /// and do not enforce limits ([`LazyBalance::Limits`]). As a result:
        /// - All operations are inherently precise
        /// - No forced execution is required
        ///
        /// Therefore, `precision` and `force` flags are not meaningful here,
        /// and are omitted from debug output.
        impl std::fmt::Debug for TestSubject {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "-")
            }
        }

        impl Directive for TestSubject {
            fn precision(&self) -> Precision {
                if self.precise {
                    return Precision::Exact;
                };
                Precision::BestEffort
            }

            fn fortitude(&self) -> Fortitude {
                if self.force {
                    return Fortitude::Force;
                };
                Fortitude::Polite
            }

            fn new(precision: Precision, fortitude: Fortitude) -> Self {
                Self {
                    precise: matches!(precision, Precision::Exact),
                    force: matches!(fortitude, Fortitude::Force),
                }
            }
        }

        impl Default for TestSubject {
            fn default() -> Self {
                Self {
                    precise: false,
                    force: false,
                }
            }
        }

        // Implements [`VirtualDynField`] for a target type using a [`SumDynType`] field.

        macro_rules! impl_v_field {
            ($target:ty, $tag:ty, $field:ident, $some:ty, $bound:ty) => {
                impl VirtualDynField<$tag> for $target {
                    type None = ();
                    type Some = $some;
                    type Many = Vec<$some>;
                    type Repr = SumDynType<$some, $bound>;

                    fn access(&self) -> Self::Repr {
                        self.$field.clone()
                    }

                    fn mutate(&mut self, v: Self::Repr) {
                        self.$field = v
                    }

                    fn len(&self) -> usize {
                        match &self.$field {
                            SumDynType::None => 0,
                            SumDynType::Some(_) => 1,
                            SumDynType::Many(v) => v.len(),
                        }
                    }

                    fn min(&self) -> usize {
                        match &self.$field {
                            SumDynType::None => 0,
                            SumDynType::Some(_) => 1,
                            SumDynType::Many(_) => 0,
                        }
                    }

                    fn max(&self) -> usize {
                        match &self.$field {
                            SumDynType::None => 0,
                            SumDynType::Some(_) => 1,
                            SumDynType::Many(_) => <$bound as sp_core::Get<u32>>::get() as usize,
                        }
                    }
                }
            };
        }

        /// Implements an empty [`VirtualDynField`] for a target
        /// virtual struct.
        ///
        /// Field is always `None` with zero capacity (`ConstU32<0>`).
        /// All accessors return empty / no-op.
        macro_rules! impl_empty_v_field {
            ($target:ty, $tag:ty, $some:ty) => {
                impl VirtualDynField<$tag> for $target {
                    type None = ();
                    type Some = $some;
                    type Many = Vec<$some>;
                    type Repr = SumDynType<$some, ConstU32<0>>;

                    fn access(&self) -> Self::Repr {
                        SumDynType::None
                    }

                    fn mutate(&mut self, _: Self::Repr) {}

                    fn len(&self) -> usize {
                        0
                    }
                    fn min(&self) -> usize {
                        0
                    }
                    fn max(&self) -> usize {
                        0
                    }
                }
            };
        }

        /// Implements an empty [`VirtualDynExtension`] for a target
        /// virtual struct.
        ///
        /// Returns `Default` on access. Mutation is a no-op (no backing storage).
        macro_rules! impl_empty_extension {
            ($target:ty, $addon:ty, $provider:ty) => {
                impl VirtualDynExtension<$addon> for $target {
                    type TypesVia = $provider;

                    fn access(
                        &self,
                    ) -> <Self::TypesVia as VirtualDynExtensionSchema<$addon>>::Repr {
                        Default::default()
                    }

                    fn mutate(
                        &mut self,
                        _: <Self::TypesVia as VirtualDynExtensionSchema<$addon>>::Repr,
                    ) {
                    }
                }
            };
        }

        impl_v_field!(TestBalance, BalanceAsset, asset, u128, ConstU32<2>);
        impl_v_field!(TestBalance, BalanceRational, bias, FixedU128, ConstU32<1>);
        impl_v_field!(TestBalance, BalanceTime, time, u32, ConstU32<2>);
        impl_empty_extension!(
            TestBalance,
            BalanceAddon,
            ShareBalanceContext<MockShareBalance>
        );

        impl_v_field!(TestSnapshot, SnapShotRational, bias, FixedU128, ConstU32<1>);
        impl_empty_v_field!(TestSnapshot, SnapShotAsset, u128);
        impl_empty_v_field!(TestSnapshot, SnapShotTime, u32);
        impl_empty_extension!(
            TestSnapshot,
            SnapShotAddon,
            ShareBalanceContext<MockShareBalance>
        );

        impl_v_field!(TestReceipt, ReceiptAsset, asset, u128, ConstU32<2>);
        impl_v_field!(TestReceipt, ReceiptRational, bias, FixedU128, ConstU32<1>);
        impl_v_field!(TestReceipt, ReceiptTime, time, u32, ConstU32<1>);
        impl_empty_extension!(
            TestReceipt,
            ReceiptAddon,
            ShareBalanceContext<MockShareBalance>
        );

        impl_v_field!(TestLimit, LimitsAsset, asset, u128, ConstU32<3>);

        /// Binds [`LimitsAsset`] for [`Extent`] semantics to a fixed
        /// capacity (`ConstU32<3>`) for use with [`VirtualDynField`].
        impl VirtualDynBound<LimitsAsset> for TestLimit {
            type Bound = ConstU32<3>;
        }

        /// Implements [`Extent`] for [`TestLimit`] with unbounded semantics.
        ///
        /// All bounds (`minimum`, `maximum`, `optimal`) return `None`,
        /// indicating no constraints on asset values.
        impl Extent<LimitsAsset> for TestLimit {
            type Scalar = <MockShareBalance as LazyBalance>::Asset;

            fn minimum(&self) -> Option<Self::Scalar> {
                None
            }

            fn maximum(&self) -> Option<Self::Scalar> {
                None
            }

            fn optimal(&self) -> Option<Self::Scalar> {
                None
            }

            /// Returns an empty extent with no bounds set
            /// (default state, no virtual fields populated).
            fn none() -> Self {
                Default::default()
            }
        }

        /// Storage prefix for snapshot entries used by [`VirtualNMap`] for [`LazyBalance`]
        /// [`virtual`](frame_suite::virtuals) storage bounds.
        ///
        /// Defined for interface completeness; unused in [`ShareBalanceFamily`].
        pub struct SnapshotPrefix;

        impl StorageInstance for SnapshotPrefix {
            const STORAGE_PREFIX: &'static str = "Snapshots";
            fn pallet_prefix() -> &'static str {
                "LazyBalance"
            }
        }

        // In-memory snapshot storage for mock `VirtualNMap` implementation.
        //
        // Key: `(variant, id, time)`
        // Value: [`TestSnapshot`]
        //
        // Used for testing in place of on-chain storage, although unused
        // in `ShareBalanceFamily`
        thread_local! {
            static SNAPSHOTS: RefCell<BTreeMap<(u8,u8,u32), TestSnapshot>> =
                RefCell::new(BTreeMap::new());
        }

        /// Mock [`VirtualNMap`] implementation for snapshot storage although not
        /// utilized in [`ShareBalanceFamily`].
        ///
        /// Uses thread-local in-memory map instead of persistent storage.
        /// Provides basic `get`, `insert`, and `remove` operations.
        ///
        /// Key layout:
        /// - `(variant, id, time)` -> snapshot at a given checkpoint
        impl VirtualNMap<TestBalance, SnapShotStorage> for MockShareBalance {
            type Key = (u8, u8, u32);
            type Value = TestSnapshot;

            type KeyGen = (
                NMapKey<Blake2_128Concat, u8>,
                NMapKey<Blake2_128Concat, u8>,
                NMapKey<Blake2_128Concat, u32>,
            );

            type Map = StorageNMap<SnapshotPrefix, Self::KeyGen, TestSnapshot, OptionQuery>;

            type Query = Option<TestSnapshot>;

            fn get(key: Self::Key) -> Self::Query {
                SNAPSHOTS.with(|m| m.borrow().get(&key).cloned())
            }

            fn insert(key: Self::Key, value: Self::Value) {
                SNAPSHOTS.with(|m| m.borrow_mut().insert(key, value));
            }

            fn remove(key: Self::Key) {
                SNAPSHOTS.with(|m| m.borrow_mut().remove(&key));
            }
        }

        /// Helper macro to define [`LazyBalance::Input`] enums where each variant
        /// satisfies the blanket [`VirtualCollector`] bounds.
        ///
        /// For each variant:
        /// - [`FromTag`] constructs enum variant from a tuple (actual input)
        /// - [`TryIntoTag`] extracts tuple from enum (collector enum)
        macro_rules! mock_lazy_input {
            (
                $name:ident < $lt:lifetime > {
                    $(
                        $variant:ident (
                            $( $field:ident : $ty:ty ),* $(,)?
                        )
                    ),* $(,)?
                }
            ) => {

                pub enum $name<$lt> {
                    $(
                        $variant( $( $ty ),* ),
                    )*
                }

                $(
                #[allow(unused_parens)]
                impl<$lt> FromTag<( $( $ty ),* ), $variant> for $name<$lt> {
                    fn from_tag(t: ( $( $ty ),* )) -> Self {
                        let ( $( $field ),* ) = t;
                        Self::$variant( $( $field ),* )
                    }
                }

                #[allow(unused_parens)]
                impl<$lt> TryIntoTag<( $( $ty ),* ), $variant> for $name<$lt> {
                    type Error = ();

                    fn try_into_tag(self) -> Result<( $( $ty ),* ), Self::Error> {
                        match self {
                            Self::$variant( $( $field ),* ) => Ok(( $( $field ),* )),
                            _ => Err(()),
                        }
                    }
                }
                )*
            };
        }

        /// Helper macro to define [`LazyBalance::Output`] enums where each variant
        /// satisfies the blanket [`VirtualCollector`] bounds.
        ///
        /// For each variant:
        /// - [`FromTag`] constructs enum variant from a tuple (actual output)
        /// - [`TryIntoTag`] extracts tuple from enum (collector enum)
        macro_rules! mock_lazy_output {
            (
                $name:ident < $lt:lifetime > {
                    $(
                        $variant:ident ( $ty:ty )
                    ),* $(,)?
                }
            ) => {

                pub enum $name<$lt> {
                    $(
                        $variant($ty),
                    )*
                }

                $(
                impl<$lt> FromTag<$ty, $variant> for $name<$lt> {
                    fn from_tag(t: $ty) -> Self {
                        Self::$variant(t)
                    }
                }

                impl<$lt> TryIntoTag<$ty, $variant> for $name<$lt> {
                    type Error = ();

                    fn try_into_tag(self) -> Result<$ty, Self::Error> {
                        match self {
                            Self::$variant(v) => Ok(v),
                            _ => Err(()),
                        }
                    }
                }
                )*
            };
        }

        mock_lazy_input!(
            TestInput<'a> {

                Deposit(
                    balance: MutHandle<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    asset: Cow<'a, u128>,
                    subject: Cow<'a, TestSubject>,
                ),

                Mint(
                    balance: MutHandle<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    asset: Cow<'a, u128>,
                    subject: Cow<'a, TestSubject>,
                ),

                Reap(
                    balance: MutHandle<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    asset: Cow<'a, u128>,
                    subject: Cow<'a, TestSubject>,
                ),

                Withdraw(
                    balance: MutHandle<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    receipt: Cow<'a, TestReceipt>,
                ),

                Drain(
                    balance: MutHandle<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                ),

                CanDeposit(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    asset: Cow<'a, u128>,
                    subject: Cow<'a, TestSubject>,
                ),

                CanMint(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    asset: Cow<'a, u128>,
                    subject: Cow<'a, TestSubject>,
                ),

                CanReap(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    asset: Cow<'a, u128>,
                    subject: Cow<'a, TestSubject>,
                ),

                CanWithdraw(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    receipt: Cow<'a, TestReceipt>,
                ),

                TotalValue(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                ),

                ReceiptActiveValue(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    receipt: Cow<'a, TestReceipt>,
                ),

                HasDeposits(
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                ),

                ReceiptDepositValue(
                    receipt: Cow<'a, TestReceipt>,
                ),

                DepositLimits (
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    subject: Cow<'a, TestSubject>,
                ),

                MintLimits (
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    subject: Cow<'a, TestSubject>,
                ),

                ReapLimits (
                    balance: Cow<'a, TestBalance>,
                    variant: Cow<'a, u8>,
                    id: Cow<'a, u8>,
                    subject: Cow<'a, TestSubject>,
                ),
            }
        );

        mock_lazy_output!(
            TestOutput<'a> {

                Deposit(Result<(Cow<'a, u128>, Cow<'a, TestReceipt>), ShareBalanceError>),

                Mint(Result<Cow<'a, u128>, ShareBalanceError>),

                Reap(Result<Cow<'a, u128>, ShareBalanceError>),

                Withdraw(Result<Cow<'a, u128>, ShareBalanceError>),

                Drain(Result<Cow<'a, u128>, ShareBalanceError>),

                CanDeposit(Result<(), ShareBalanceError>),

                CanMint(Result<(), ShareBalanceError>),

                CanReap(Result<(), ShareBalanceError>),

                CanWithdraw(Result<(), ShareBalanceError>),

                TotalValue(Result<Cow<'a, u128>, ShareBalanceError>),

                ReceiptActiveValue(Result<Cow<'a, u128>, ShareBalanceError>),

                HasDeposits(Result<(), ShareBalanceError>),

                ReceiptDepositValue(Result<Cow<'a, u128>, ShareBalanceError>),

                DepositLimits(Result<Cow<'a, TestLimit>, ShareBalanceError>),

                MintLimits(Result<Cow<'a, TestLimit>, ShareBalanceError>),

                ReapLimits(Result<Cow<'a, TestLimit>, ShareBalanceError>),
            }
        );
    }
}
