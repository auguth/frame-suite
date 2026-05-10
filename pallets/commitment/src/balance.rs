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
// ```````````````````````````````` LAZY BALANCE `````````````````````````````````
// ===============================================================================

//! [`Pallet`] implementation of [`LazyBalance`] using [`virtual`](frame_suite::virtuals)
//! structs and [`plugins`](frame_suite::plugins).
//!
//! Provides a generic, plugin-driven balance system where behavior is defined
//! by [`LazyBalance::BalanceFamily`] thin-delegated to [`Config::BalanceFamily`],
//! rather than being hardcoded.
//!
//! State is encoded using [`SumDynType`] and accessed through
//! [`VirtualDynField`], enabling flexible virtual schemas.
//!
//! Supports:
//! - lazy evaluation of balances
//! - snapshot-based time tracking via [`VirtualNMap`]
//! - typed dispatch through tagged input/output enums

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{types::*, BalanceSnapShots, Config, Pallet};

// --- Scale-codec crates ---
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

// --- FRAME Suite ---
use frame_suite::{
    assets::*,
    base::Delimited,
    misc::{Directive, Extent},
    mutation::MutHandle,
    virtuals::*,
};

// --- FRAME Support ---
use frame_support::{
    pallet_prelude::NMapKey,
    traits::tokens::{Fortitude, Precision},
    Blake2_128Concat,
};

// --- Substrate primitives ---
use sp_core::{ConstU32, Get};
use sp_runtime::{Cow, DispatchError, Vec};

// ===============================================================================
// `````````````````````` LAZY BALANCE OPERATIONS UTILITIES ``````````````````````
// ===============================================================================

/// Deposits value into a [`VirtualBalance`] via [`LazyBalance::deposit`] execution.
///
/// Wraps [`plugin`](frame_suite::plugins) dispatch using [`LazyInput::Deposit`]
/// and returns (`effective_asset`, [`VirtualReceipt`]) on success.
pub fn deposit<'a, T: Config<I>, I: 'static>(
    balance: &'a mut VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    value: &'a AssetOf<T, I>,
    qualify: &'a DispatchPolicy,
) -> Result<(AssetOf<T, I>, VirtualReceipt<T, I>), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, Deposit>>::from_tag((
        MutHandle::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(value),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::deposit(input);

    let Ok(result) = TryIntoTag::<_, Deposit>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok((asset, receipt)) => Ok((asset.into_owned(), receipt.into_owned())),
        Err(e) => Err(e.into()),
    }
}

/// Withdraws value from a [`VirtualBalance`] using a [`VirtualReceipt`] via
/// [`LazyBalance::withdraw`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::Withdraw`]
/// and returns the actual withdrawn asset value.
pub fn withdraw<'a, T: Config<I>, I: 'static>(
    balance: &'a mut VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    receipt: &'a VirtualReceipt<T, I>,
) -> Result<AssetOf<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, Withdraw>>::from_tag((
        MutHandle::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(receipt),
    ));

    let raw = Pallet::<T, I>::withdraw(input);

    let Ok(result) = TryIntoTag::<_, Withdraw>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(*v),
        Err(e) => Err(e.into()),
    }
}

/// Mints value into a [`VirtualBalance`] (e.g. rewards/inflation) via
/// [`LazyBalance::mint`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::Mint`]
/// and returns the actual minted asset value.
pub fn mint<'a, T: Config<I>, I: 'static>(
    balance: &'a mut VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    value: &'a AssetOf<T, I>,
    qualify: &'a DispatchPolicy,
) -> Result<AssetOf<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, Mint>>::from_tag((
        MutHandle::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(value),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::mint(input);

    let Ok(result) = TryIntoTag::<_, Mint>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(v.into_owned()),
        Err(e) => Err(e.into()),
    }
}

/// Reaps (removes) value from a [`VirtualBalance`] (e.g. penalties/deflation)
/// via [`LazyBalance::reap`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::Reap`]
/// and returns the actual reaped asset value.
pub fn reap<'a, T: Config<I>, I: 'static>(
    balance: &'a mut VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    value: &'a AssetOf<T, I>,
    qualify: &'a DispatchPolicy,
) -> Result<AssetOf<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, Reap>>::from_tag((
        MutHandle::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(value),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::reap(input);

    let Ok(result) = TryIntoTag::<_, Reap>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(v.into_owned()),
        Err(e) => Err(e.into()),
    }
}

/// Drains all value associated with a `(variant, digest)` pair via
/// [`LazyBalance::drain`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::Drain`]
/// and performs full state cleanup.
#[allow(unused)]
pub fn drain<'a, T: Config<I>, I: 'static>(
    balance: &'a mut VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
) -> Result<(), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, Drain>>::from_tag((
        MutHandle::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
    ));

    let raw = <Pallet<T, I> as LazyBalance>::drain(input);

    let Ok(result) = TryIntoTag::<_, Drain>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Checks if a deposit is allowed under current constraints via
/// [`LazyBalance::can_deposit`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::CanDeposit`]
/// and returns `Ok(())` if permitted.
pub fn can_deposit<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    value: &'a AssetOf<T, I>,
    qualify: &'a DispatchPolicy,
) -> Result<(), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, CanDeposit>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(value),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::can_deposit(input);

    let Ok(result) = TryIntoTag::<_, CanDeposit>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Checks if a withdrawal is allowed for a given [`VirtualReceipt`] via
/// [`LazyBalance::can_withdraw`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::CanWithdraw`]
/// and returns `Ok(())` if permitted.
pub fn can_withdraw<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    receipt: &'a VirtualReceipt<T, I>,
) -> Result<(), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, CanWithdraw>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(receipt),
    ));

    let raw = Pallet::<T, I>::can_withdraw(input);

    let Ok(result) = TryIntoTag::<_, CanWithdraw>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Checks if minting is allowed under current constraints via
/// [`LazyBalance::can_withdraw`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::CanMint`]
/// and returns `Ok(())` if permitted.
pub fn can_mint<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    value: &'a AssetOf<T, I>,
    qualify: &'a DispatchPolicy,
) -> Result<(), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, CanMint>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(value),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::can_mint(input);

    let Ok(result) = TryIntoTag::<_, CanMint>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Checks if reaping is allowed under current constraints via
/// [`LazyBalance::can_reap`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::CanReap`]
/// and returns `Ok(())` if permitted.
pub fn can_reap<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    value: &'a AssetOf<T, I>,
    qualify: &'a DispatchPolicy,
) -> Result<(), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, CanReap>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(value),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::can_reap(input);

    let Ok(result) = TryIntoTag::<_, CanReap>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Returns total value of a [`VirtualBalance`] for a `(variant, digest)` pair via
/// [`LazyBalance::total_value`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::TotalValue`].
pub fn balance_total<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
) -> Result<AssetOf<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, TotalValue>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
    ));

    let raw = Pallet::<T, I>::total_value(input);

    let Ok(result) = TryIntoTag::<_, TotalValue>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(*v),
        Err(e) => Err(e.into()),
    }
}

/// Returns the current (lazy-evaluated) value of a [`VirtualReceipt`] via
/// [`LazyBalance::receipt_active_value`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::ReceiptActiveValue`].
pub fn receipt_active_value<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    receipt: &'a VirtualReceipt<T, I>,
) -> Result<AssetOf<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, ReceiptActiveValue>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(receipt),
    ));

    let raw = Pallet::<T, I>::receipt_active_value(input);

    let Ok(result) = TryIntoTag::<_, ReceiptActiveValue>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(*v),
        Err(e) => Err(e.into()),
    }
}

/// Returns the original deposited value of a [`VirtualReceipt`] via
/// [`LazyBalance::receipt_deposit_value`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::ReceiptDepositValue`].
pub fn receipt_deposit_value<'a, T: Config<I>, I: 'static>(
    receipt: &'a VirtualReceipt<T, I>,
) -> Result<AssetOf<T, I>, DispatchError> {
    let input =
        <LazyInput<'a, T, I> as FromTag<_, ReceiptDepositValue>>::from_tag(Cow::Borrowed(receipt));

    let raw = Pallet::<T, I>::receipt_deposit_value(input);

    let Ok(result) = TryIntoTag::<_, ReceiptDepositValue>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(*v),
        Err(e) => Err(e.into()),
    }
}

/// Checks whether any deposits exist for a `(variant, digest)` pair via
/// [`LazyBalance::has_deposits`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::HasDeposits`].
pub fn has_deposits<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
) -> Result<(), DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, HasDeposits>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
    ));

    let raw = Pallet::<T, I>::has_deposits(input);

    let Ok(result) = TryIntoTag::<_, HasDeposits>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Returns deposit limits implementing [`Extent`] for a [`VirtualBalance`]
/// context via [`LazyBalance::deposit_limits`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::DepositLimits`]
/// and returns [`LimitsProduct`].
pub fn deposit_limits_of<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    qualify: &'a DispatchPolicy,
) -> Result<LimitsProduct<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, DepositLimits>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::deposit_limits(input);

    let Ok(result) = TryIntoTag::<_, DepositLimits>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(v.into_owned()),
        Err(e) => Err(e.into()),
    }
}

/// Returns mint limits implementing [`Extent`] for a [`VirtualBalance`]
/// context via [`LazyBalance::mint_limits`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::MintLimits`]
/// and returns [`LimitsProduct`].
pub fn mint_limits_of<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    qualify: &'a DispatchPolicy,
) -> Result<LimitsProduct<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, MintLimits>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::mint_limits(input);

    let Ok(result) = TryIntoTag::<_, MintLimits>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(v.into_owned()),
        Err(e) => Err(e.into()),
    }
}

/// Returns reap limits implementing [`Extent`] for a [`VirtualBalance`]
/// context via [`LazyBalance::reap_limits`] execution.
///
/// Wraps plugin dispatch using [`LazyInput::ReapLimits`]
/// and returns [`LimitsProduct`].
pub fn reap_limits_of<'a, T: Config<I>, I: 'static>(
    balance: &'a VirtualBalance<T, I>,
    variant: &'a T::Position,
    id: &'a Digest<T>,
    qualify: &'a DispatchPolicy,
) -> Result<LimitsProduct<T, I>, DispatchError> {
    let input = <LazyInput<'a, T, I> as FromTag<_, ReapLimits>>::from_tag((
        Cow::Borrowed(balance),
        Cow::Borrowed(variant),
        Cow::Borrowed(id),
        Cow::Borrowed(qualify),
    ));

    let raw = Pallet::<T, I>::reap_limits(input);

    let Ok(result) = TryIntoTag::<_, ReapLimits>::try_into_tag(raw) else {
        return Err(crate::Error::<T, I>::CorruptedPlugin.into());
    };

    match result {
        Ok(v) => Ok(v.into_owned()),
        Err(e) => Err(e.into()),
    }
}

// ===============================================================================
// `````````````````````````````` LAZY BALANCE IMPL ``````````````````````````````
// ===============================================================================

impl<T, I> LazyBalance for Pallet<T, I>
where
    T: Config<I>,
    I: 'static,
{
    type Asset = AssetOf<T, I>;
    type Rational = T::Bias;
    type Time = T::Time;

    type Balance = VirtualBalance<T, I>;
    type Variant = T::Position;
    type Id = Digest<T>;
    type Limits = LimitsProduct<T, I>;
    type Subject = DispatchPolicy;

    type SnapShot = VirtualSnapShot<T, I>;
    type Receipt = VirtualReceipt<T, I>;

    type Input<'a> = LazyInput<'a, T, I>;
    type Output<'a> = LazyOutput<'a, T, I>;

    type BalanceFamily<'a> = T::BalanceFamily<'a>;
    type BalanceContext = T::BalanceContext;
}

// ===============================================================================
// ````````````````````` BALANCE PLUGIN CONTEXT TRAIT BOUNDS `````````````````````
// ===============================================================================

/// Schema provider for [`ProductType`] typical via [`BalanceModelContext`].
///
/// Combines:
/// - [`VirtualDynBound`] for core discriminants (`Asset`, `Rational`, `Time`)
/// - [`VirtualDynExtensionSchema`] for extension layout (`Addon`)
///
/// Implementors define the **field bounds and extension schema**
/// used to interpret a [`virtual`](frame_suite::virtuals) product.
pub trait ProductProvider<Asset, Rational, Time, Addon>:
    VirtualDynExtensionSchema<Addon>
    + VirtualDynBound<Asset>
    + VirtualDynBound<Rational>
    + VirtualDynBound<Time>
where
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
}

impl<T, Asset, Rational, Time, Addon> ProductProvider<Asset, Rational, Time, Addon> for T
where
    T: VirtualDynExtensionSchema<Addon>
        + VirtualDynBound<Asset>
        + VirtualDynBound<Rational>
        + VirtualDynBound<Time>,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
}

// ===============================================================================
// ``````````````````````````` VIRTUAL STRUCT PRODUCT ````````````````````````````
// ===============================================================================

/// A generic **[`virtual`](frame_suite::virtuals) product structure** used
/// by [`LazyBalance`] components.
///
/// Core backing type for:
/// - [`VirtualBalance`]
/// - [`VirtualReceipt`]
/// - [`VirtualSnapShot`]
///
/// Each field is stored as a [`SumDynType`] and accessed via
/// [`VirtualDynField`] or [`VirtualDynExtension`].
///
/// Enables a **schema-less, context-driven layout**, where:
/// - field multiplicity is dynamic (`None | Some | Many`)
/// - bounds are enforced via [`VirtualDynBound`]
/// - extensions are defined via [`VirtualDynExtensionSchema`]
///
/// This allows reuse across multiple virtual types without redefining structs.
///
/// - [`ProductProvider`] implemented by [`BalanceModelContext`] supplies bounds and
/// extension schema
/// - generics (`Asset`, `Rational`, `Time`) resolve discriminant fields of the respective
/// lazy balance virtual structs.
/// - access is mediated through generic-traits, not direct struct usage
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[codec(encode_bound(
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    <Provider as VirtualDynExtensionSchema<Addon>>::Repr: Delimited + Default
))]
#[codec(decode_bound(
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    <Provider as VirtualDynExtensionSchema<Addon>>::Repr:  Delimited + Default
))]
#[codec(decode_with_mem_tracking_bound(
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    <Provider as VirtualDynExtensionSchema<Addon>>::Repr: Delimited + Default
))]
#[codec(mel_bound(
    <Provider as VirtualDynExtensionSchema<Addon>>::Repr: MaxEncodedLen
))]
#[scale_info(skip_type_params(T, I, Provider, Asset, Rational, Time, Addon))]
pub struct ProductType<T, I, Provider, Asset, Rational, Time, Addon>
where
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    T: Config<I>,
    I: 'static,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
    /// Asset-related fields resolved via [`VirtualDynField`].
    ///
    /// Uses [`SumDynType`] with capacity bounded by [`VirtualDynBound`] for `Asset`.
    asset: SumDynType<AssetOf<T, I>, <Provider as VirtualDynBound<Asset>>::Bound>,

    /// Rational fields (e.g. bias / price factors) resolved via [`VirtualDynField`].
    ///
    /// Backed by [`SumDynType`] and bounded by [`VirtualDynBound`] for `Rational`.
    bias: SumDynType<T::Bias, <Provider as VirtualDynBound<Rational>>::Bound>,

    /// Time-related fields (e.g. checkpoints) resolved via [`VirtualDynField`].
    ///
    /// Encoded as [`SumDynType`] with bounds provided by [`VirtualDynBound`] for `Time`.
    time: SumDynType<T::Time, <Provider as VirtualDynBound<Time>>::Bound>,

    /// Extension storage defined by [`VirtualDynExtensionSchema`].
    ///
    /// Layout and semantics are fully provided by [`ProductProvider`] via
    /// the associated extension schema (`Addon`).
    addon: <Provider as VirtualDynExtensionSchema<Addon>>::Repr,
}

impl<T, I, Provider, Asset, Rational, Time, Addon> Clone
    for ProductType<T, I, Provider, Asset, Rational, Time, Addon>
where
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    T: Config<I>,
    I: 'static,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
    fn clone(&self) -> Self {
        Self {
            asset: self.asset.clone(),
            bias: self.bias.clone(),
            time: self.time.clone(),
            addon: self.addon.clone(),
        }
    }
}

impl<T, I, Provider, Asset, Rational, Time, Addon> core::fmt::Debug
    for ProductType<T, I, Provider, Asset, Rational, Time, Addon>
where
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    T: Config<I>,
    I: 'static,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProductType")
            .field("asset", &self.asset)
            .field("bias", &self.bias)
            .field("time", &self.time)
            .field("addon", &self.addon)
            .finish()
    }
}

impl<T, I, Provider, Asset, Rational, Time, Addon> Default
    for ProductType<T, I, Provider, Asset, Rational, Time, Addon>
where
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    T: Config<I>,
    I: 'static,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
    fn default() -> Self {
        Self {
            asset: Default::default(),
            bias: Default::default(),
            time: Default::default(),
            addon: Default::default(),
        }
    }
}

impl<T, I, Provider, Asset, Rational, Time, Addon> PartialEq
    for ProductType<T, I, Provider, Asset, Rational, Time, Addon>
where
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    T: Config<I>,
    I: 'static,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
    fn eq(&self, other: &Self) -> bool {
        self.asset == other.asset
            && self.bias == other.bias
            && self.time == other.time
            && self.addon == other.addon
    }
}

impl<T, I, Provider, Asset, Rational, Time, Addon> Eq
    for ProductType<T, I, Provider, Asset, Rational, Time, Addon>
where
    Provider: ProductProvider<Asset, Rational, Time, Addon>,
    T: Config<I>,
    I: 'static,
    Addon: DiscriminantTag,
    Rational: DiscriminantTag,
    Time: DiscriminantTag,
    Asset: DiscriminantTag,
{
}

// ===============================================================================
// ```````````````````````` VIRTUAL FIELD ALLOCATIONS ````````````````````````````
// ===============================================================================

/// Implements [`VirtualDynField`] for a [`ProductType`] field.
///
/// Maps a discriminant tag to a concrete virtual struct field
/// using [`SumDynType`].
///
/// Bounds are enforced via [`VirtualDynBound`] provided by [`ProductProvider`].
macro_rules! impl_v_field {
    (
        $tag:ty,
        $field:ident,
        $product:ty,
        $asset:ty,
        $rational:ty,
        $time:ty,
        $addon:ty,
        $value:ty
    ) => {
        impl<T, I, Provider> VirtualDynField<$tag> for $product
        where
            Provider: ProductProvider<$asset, $rational, $time, $addon>,
            T: Config<I>,
            I: 'static,
        {
            type None = ();
            type Some = $value;
            type Many = Vec<$value>;
            type Repr = SumDynType<$value, <Provider as VirtualDynBound<$tag>>::Bound>;

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
                    SumDynType::Many(_) => {
                        <Provider as VirtualDynBound<$tag>>::Bound::get() as usize
                    }
                }
            }
        }
    };
}

/// Implements [`VirtualDynExtension`] for a [`ProductType`] extension field.
///
/// Maps an extension discriminant to the virtual field,
/// with layout defined by [`VirtualDynExtensionSchema`] via [`ProductProvider`].
macro_rules! impl_v_ext {
    (
        $tag:ty,
        $field:ident,
        $product:ty,
        $asset:ty,
        $rational:ty,
        $time:ty,
        $addon:ty
    ) => {
        impl<T, I, Provider> VirtualDynExtension<$tag> for $product
        where
            Provider: ProductProvider<$asset, $rational, $time, $addon>,
            T: Config<I>,
            I: 'static,
        {
            type TypesVia = Provider;

            fn access(&self) -> <Provider as VirtualDynExtensionSchema<$addon>>::Repr {
                self.$field.clone()
            }

            fn mutate(&mut self, v: <Provider as VirtualDynExtensionSchema<$addon>>::Repr) {
                self.$field = v
            }
        }
    };
}

/// Implements all virtual field and extension bindings for a [`ProductType`].
///
/// Expands [`VirtualDynField`] for core discriminants (`Asset`, `Rational`, `Time`)
/// and [`VirtualDynExtension`] for the addon field, using [`ProductProvider`]
/// for bounds and schema.
macro_rules! impl_product_alloc {
    (
        $asset:ty,
        $rational:ty,
        $time:ty,
        $addon:ty
    ) => {

        impl_v_field!(
            $asset,
            asset,
            ProductType<T,I,Provider,$asset,$rational,$time,$addon>,
            $asset,
            $rational,
            $time,
            $addon,
            AssetOf<T,I>
        );

        impl_v_field!(
            $rational,
            bias,
            ProductType<T,I,Provider,$asset,$rational,$time,$addon>,
            $asset,
            $rational,
            $time,
            $addon,
            T::Bias
        );

        impl_v_field!(
            $time,
            time,
            ProductType<T,I,Provider,$asset,$rational,$time,$addon>,
            $asset,
            $rational,
            $time,
            $addon,
            T::Time
        );

        impl_v_ext!(
            $addon,
            addon,
            ProductType<T,I,Provider,$asset,$rational,$time,$addon>,
            $asset,
            $rational,
            $time,
            $addon
        );
    };
}

impl_product_alloc!(BalanceAsset, BalanceRational, BalanceTime, BalanceAddon);

impl_product_alloc!(SnapShotAsset, SnapShotRational, SnapShotTime, SnapShotAddon);

impl_product_alloc!(ReceiptAsset, ReceiptRational, ReceiptTime, ReceiptAddon);

// ===============================================================================
// `````````````````````````` CONVENIENCE TYPE ALIASES ```````````````````````````
// ===============================================================================

/// Convenient alias for the pallet's [`virtual`](frame_suite::virtuals) balance type.
type LazyBalanceOf<T, I> = VirtualBalance<T, I>;

/// Convenient alias for the pallet's [`virtual`](frame_suite::virtuals) receipt type.
type LazyReceiptOf<T, I> = VirtualReceipt<T, I>;

/// Convenient alias for the pallet's underlying asset type.
type LazyAssetOf<T, I> = AssetOf<T, I>;

/// Convenient alias for the variant (position) used in lazy balance.
type LazyVariantOf<T, I> = <Pallet<T, I> as LazyBalance>::Variant;

/// Convenient alias for the digest identifier used in lazy balance.
type LazyIdOf<T, I> = <Pallet<T, I> as LazyBalance>::Id;

/// Convenient alias for the error type resolved from the lazy balance context.
type LazyErrorOf<T, I> = <Context<Pallet<T, I>> as VirtualError<LazyBalanceError>>::Error;

// ===============================================================================
// ````````````````````````` VIRTUAL COLLECTORS (ENUMS) ``````````````````````````
// ===============================================================================

/// Defines the [`LazyInput`] enum for [`LazyBalance`] operations.
///
/// Each variant represents an operation input, with typed fields encoded
/// as tuples and mapped via [`FromTag`] / [`TryIntoTag`].
///
/// Enables type-safe, tag-driven dispatch into [`LazyBalance::BalanceFamily`]
/// [`plugins`](frame_suite::plugins).
macro_rules! lazy_input {
    (
        $(
            $variant:ident (
                $( $field:ident : $ty:ty ),* $(,)?
            )
        ),* $(,)?
    ) => {

        pub enum LazyInput<'a, T, I>
        where
            T: Config<I>,
            I: 'static,
        {
            $(
                $variant( $( $ty ),* ),
            )*
        }

        $(
            #[allow(unused_parens)]
        impl<'a, T, I>
            FromTag<( $( $ty ),* ), $variant>
        for LazyInput<'a, T, I>
        where
            T: Config<I>,
            I: 'static,
        {
            fn from_tag(t: ( $( $ty ),* )) -> Self {
                let ( $( $field ),* ) = t;
                LazyInput::$variant( $( $field ),* )
            }
        }

            #[allow(unused_parens)]
        impl<'a, T, I>
            TryIntoTag<( $( $ty ),* ), $variant>
        for LazyInput<'a, T, I>
        where
            T: Config<I>,
            I: 'static,
        {
            type Error = ();

            fn try_into_tag(self)
            -> Result<( $( $ty ),* ), Self::Error>
            {
                match self {
                    LazyInput::$variant( $( $field ),* ) =>
                        Ok(( $( $field ),* )),
                    _ => Err(()),
                }
            }
        }
        )*
    };
}

lazy_input! {

    Deposit(
        balance: MutHandle<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        asset: Cow<'a, LazyAssetOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    Mint(
        balance: MutHandle<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        asset: Cow<'a, LazyAssetOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    Reap(
        balance: MutHandle<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        asset: Cow<'a, LazyAssetOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    Drain(
        balance: MutHandle<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
    ),

    Withdraw(
        balance: MutHandle<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        receipt: Cow<'a, LazyReceiptOf<T, I>>,
    ),

    CanDeposit(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        asset: Cow<'a, LazyAssetOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    CanMint(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        asset: Cow<'a, LazyAssetOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    CanReap(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        asset: Cow<'a, LazyAssetOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    CanWithdraw(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        receipt: Cow<'a, LazyReceiptOf<T, I>>,
    ),

    TotalValue(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
    ),

    ReceiptActiveValue(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        receipt: Cow<'a, LazyReceiptOf<T, I>>,
    ),

    HasDeposits(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
    ),

    ReceiptDepositValue(
        receipt: Cow<'a, LazyReceiptOf<T, I>>,
    ),

    DepositLimits(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    MintLimits(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

    ReapLimits(
        balance: Cow<'a, LazyBalanceOf<T, I>>,
        variant: Cow<'a, LazyVariantOf<T, I>>,
        id: Cow<'a, LazyIdOf<T, I>>,
        subject: Cow<'a, DispatchPolicy>,
    ),

}

/// Defines the [`LazyOutput`] enum for [`LazyBalance`] operations.
///
/// Each variant represents an operation input, with typed fields encoded
/// as tuples and mapped via [`FromTag`] / [`TryIntoTag`].
///
/// Enables type-safe, tag-driven dispatch into [`LazyBalance::BalanceFamily`]
/// [`plugins`](frame_suite::plugins).
macro_rules! lazy_output {
    (
        $(
            $variant:ident ( $ty:ty )
        ),* $(,)?
    ) => {

        pub enum LazyOutput<'a, T, I>
        where
            T: Config<I>,
            I: 'static,
        {
            $(
                $variant($ty),
            )*
        }

        $(
        impl<'a, T, I> FromTag<$ty, $variant>
            for LazyOutput<'a, T, I>
        where
            T: Config<I>,
            I: 'static,
        {
            fn from_tag(t: $ty) -> Self {
                Self::$variant(t)
            }
        }

        impl<'a, T, I> TryIntoTag<$ty, $variant>
            for LazyOutput<'a, T, I>
        where
            T: Config<I>,
            I: 'static,
        {
            type Error = ();

            fn try_into_tag(self) -> Result<$ty, Self::Error> {
                match self {
                    Self::$variant(i) => Ok(i),
                    _ => Err(()),
                }
            }
        }
        )*
    };
}

lazy_output! {
    Deposit(Result<(Cow<'a, AssetOf<T, I>>, Cow<'a, LazyReceiptOf<T, I>>), LazyErrorOf<T, I>>),
    Mint(Result<Cow<'a, AssetOf<T, I>>, LazyErrorOf<T, I>>),
    Reap(Result<Cow<'a, AssetOf<T, I>>, LazyErrorOf<T, I>>),
    Withdraw(Result<Cow<'a, LazyAssetOf<T, I>>, LazyErrorOf<T, I>>),
    Drain(Result<Cow<'a, AssetOf<T, I>>, LazyErrorOf<T, I>>),
    CanDeposit(Result<(), LazyErrorOf<T, I>>),
    CanMint(Result<(), LazyErrorOf<T, I>>),
    CanReap(Result<(), LazyErrorOf<T, I>>),
    CanWithdraw(Result<(), LazyErrorOf<T, I>>),
    TotalValue(Result<Cow<'a, LazyAssetOf<T, I>>, LazyErrorOf<T, I>>),
    ReceiptActiveValue(Result<Cow<'a, LazyAssetOf<T, I>>, LazyErrorOf<T, I>>),
    HasDeposits(Result<(), LazyErrorOf<T, I>>),
    ReceiptDepositValue(Result<Cow<'a, LazyAssetOf<T, I>>, LazyErrorOf<T, I>>),
    DepositLimits(Result<Cow<'a, LimitsProduct<T, I>>, LazyErrorOf<T, I>>),
    MintLimits(Result<Cow<'a, LimitsProduct<T, I>>, LazyErrorOf<T, I>>),
    ReapLimits(Result<Cow<'a, LimitsProduct<T, I>>, LazyErrorOf<T, I>>),
}

// ===============================================================================
// `````````````````````````` LAZY BALANCE VIRTUAL MAP ```````````````````````````
// ===============================================================================

/// [`VirtualNMap`] implementation for snapshot storage of [`VirtualBalance`].
///
/// Implemented for [`Pallet`] since it satisfies the super-bounds of
/// [`LazyBalance`] and provides the concrete virtual storage backend.
///
/// Maps `(digest, variant, time)` -> [`VirtualSnapShot`], enabling
/// time-indexed balance projections via [`BalanceSnapShots`].
impl<T, I> VirtualNMap<VirtualBalance<T, I>, SnapShotStorage> for Pallet<T, I>
where
    T: Config<I>,
    I: 'static,
{
    type Key = (Digest<T>, T::Position, T::Time);

    type Value = VirtualSnapShot<T, I>;

    type KeyGen = (
        NMapKey<Blake2_128Concat, Digest<T>>,
        NMapKey<Blake2_128Concat, T::Position>,
        NMapKey<Blake2_128Concat, T::Time>,
    );

    type Map = BalanceSnapShots<T, I>;

    type Query = Option<VirtualSnapShot<T, I>>;
}

// ===============================================================================
// ```````````````````````` LAZY BALANCE OPERATION LIMITS ````````````````````````
// ===============================================================================

/// Virtual limits container for [`LazyBalance`] operations.
///
/// Stores bounded asset constraints (`min`, `max`, `optimal`) using
/// [`SumDynType`] with fixed capacity (`ConstU32<3>`).
///
/// Interpreted via [`Extent`] and accessed through [`VirtualDynField`].
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T, I))]
pub struct LimitsProduct<T, I>
where
    T: Config<I>,
    I: 'static,
{
    /// Asset bounds (`min`, `max`, `optimal`) encoded as [`SumDynType`].
    ///
    /// Capacity is fixed to 3 via [`ConstU32<3>`].
    asset: SumDynType<AssetOf<T, I>, ConstU32<3>>,
}

impl<T, I> Clone for LimitsProduct<T, I>
where
    T: Config<I>,
    I: 'static,
    SumDynType<AssetOf<T, I>, ConstU32<3>>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            asset: self.asset.clone(),
        }
    }
}

impl<T, I> core::fmt::Debug for LimitsProduct<T, I>
where
    T: Config<I>,
    I: 'static,
    SumDynType<AssetOf<T, I>, ConstU32<3>>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LimitsProduct")
            .field("asset", &self.asset)
            .finish()
    }
}

impl<T, I> Default for LimitsProduct<T, I>
where
    T: Config<I>,
    I: 'static,
    SumDynType<AssetOf<T, I>, ConstU32<3>>: Default,
{
    fn default() -> Self {
        Self {
            asset: Default::default(),
        }
    }
}

impl<T, I> core::cmp::PartialEq for LimitsProduct<T, I>
where
    T: Config<I>,
    I: 'static,
    SumDynType<AssetOf<T, I>, ConstU32<3>>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.asset == other.asset
    }
}

impl<T, I> core::cmp::Eq for LimitsProduct<T, I>
where
    T: Config<I>,
    I: 'static,
    SumDynType<AssetOf<T, I>, ConstU32<3>>: Eq,
{
}

impl<T: Config<I>, I: 'static> VirtualDynField<LimitsAsset> for LimitsProduct<T, I> {
    type None = ();

    type Some = AssetOf<T, I>;

    type Many = Vec<AssetOf<T, I>>;

    type Repr = SumDynType<AssetOf<T, I>, ConstU32<3>>;

    fn access(&self) -> Self::Repr {
        self.asset.clone()
    }

    fn mutate(&mut self, v: Self::Repr) {
        self.asset = v
    }

    fn len(&self) -> usize {
        match &self.asset {
            SumDynType::None => 0,
            SumDynType::Some(_) => 1,
            SumDynType::Many(v) => v.len(),
        }
    }

    fn min(&self) -> usize {
        match &self.asset {
            SumDynType::None => 0,
            SumDynType::Some(_) => 1,
            SumDynType::Many(_) => 0,
        }
    }

    fn max(&self) -> usize {
        match &self.asset {
            SumDynType::None => 0,
            SumDynType::Some(_) => 1,
            SumDynType::Many(_) => 3,
        }
    }
}

impl<T: Config<I>, I: 'static> VirtualDynBound<LimitsAsset> for LimitsProduct<T, I> {
    type Bound = ConstU32<3>;
}

impl<T: Config<I>, I: 'static> Extent<LimitsAsset> for LimitsProduct<T, I> {
    type Scalar = AssetOf<T, I>;

    fn minimum(&self) -> Option<Self::Scalar> {
        self.index_get(0)
    }

    fn maximum(&self) -> Option<Self::Scalar> {
        self.index_get(1)
    }

    fn optimal(&self) -> Option<Self::Scalar> {
        self.index_get(3)
    }

    fn none() -> Self {
        Default::default()
    }
}

impl<T: Config<I>, I: 'static> Extent for LimitsProduct<T, I> {
    type Scalar = AssetOf<T, I>;

    fn minimum(&self) -> Option<Self::Scalar> {
        self.index_get(0)
    }

    fn maximum(&self) -> Option<Self::Scalar> {
        self.index_get(1)
    }

    fn optimal(&self) -> Option<Self::Scalar> {
        self.index_get(3)
    }

    fn none() -> Self {
        Default::default()
    }
}

// ===============================================================================
// ```````````````````````` LAZY BALANCE OPERATION POLICY ````````````````````````
// ===============================================================================

/// Execution policy for [`LazyBalance`] [`plugin`](frame_suite::plugins) operations.
///
/// Encodes dispatch preferences:
/// - `precise`: `true` -> [`Precision::Exact`], `false` -> [`Precision::BestEffort`]
/// - `force`  : `true` -> [`Fortitude::Force`], `false` -> [`Fortitude::Polite`]
///
/// Used to guide behavior during execution.
#[derive(
    Clone, Eq, PartialEq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Debug,
)]
pub struct DispatchPolicy {
    /// `true` for exact execution, `false` for best-effort
    pub precise: bool,

    /// `true` to force execution, `false` for polite (non-forcing)
    pub force: bool,
}

impl Directive for DispatchPolicy {
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

impl Default for DispatchPolicy {
    fn default() -> Self {
        Self {
            precise: false,
            force: false,
        }
    }
}
