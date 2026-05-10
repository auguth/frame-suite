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
// ``````````````````````````````` XP MOCK RUNTIME ```````````````````````````````
// ===============================================================================

//! Mock runtime and test utilities for the XP pallet.

#![cfg(feature = "std")]
#![allow(unused)]

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate as pallet_xp;
use crate::{types::*, Config};

// --- FRAME Suite ---
use frame_suite::misc::Ignore;

// --- FRAME Support ---
pub use frame_support::instances::Instance1;
pub use frame_support::instances::Instance2;
use frame_support::{derive_impl, pallet_prelude::*, traits::VariantCount};

// --- Substrate primitives ---
use sp_core::ConstBool;
use sp_runtime::{BuildStorage, RuntimeDebug};

// --- External crates ---
use serde::{Deserialize, Serialize};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Block type for the mock runtime.
pub type Block = frame_system::mocking::MockBlock<Test>;

/// AccountId type for the mock runtime.
pub type AccountId = u64;

/// System Account type for the mock runtime.
pub type Account = frame_system::Account<Test>;

/// Mock Typed XP reserved entry.
pub type ReserveId =
    IdXp<crate::types::ReserveReason<Test, Instance1>, <Test as Config<Instance1>>::Xp>;

/// Mock Typed XP lock entry.
pub type LockId = IdXp<crate::types::LockReason<Test, Instance1>, <Test as Config<Instance1>>::Xp>;

/// Mock Pallet Type
pub type Pallet = crate::Pallet<Test, Instance1>;

/// Mock Pallet's Error Type
pub type Error = crate::Error<Test, Instance1>;

/// Mock Pallet's Event Type
pub type Event = crate::Event<Test, Instance1>;

/// Mock Xp Structure
pub type MockXp = crate::types::Xp<Test, Instance1>;

/// Mocked-Instance2 Xp Structure
pub type MockXp2 = crate::types::Xp<Test, Instance2>;

/// Mock Xp Meta-Data Storage-Map
pub type XpOf = crate::XpOf<Test, Instance1>;

/// Mocked-Instance2 Xp Meta-Data Storage-Map
pub type XpOf2 = crate::XpOf<Test, Instance2>;

/// Mock Locked Xp Meta-Data Storage-Map
pub type LockedXpOf = crate::LockedXpOf<Test, Instance1>;

/// Mock Reserved Xp Meta-Data Storage-Map
pub type ReservedXpOf = crate::ReservedXpOf<Test, Instance1>;

/// Mocked-Instance2 Locked Xp Meta-Data Storage-Map
pub type LockedXpOf2 = crate::LockedXpOf<Test, Instance2>;

/// Mocked-Instance2 Reserved Xp Meta-Data Storage-Map
pub type ReservedXpOf2 = crate::ReservedXpOf<Test, Instance2>;

/// Mock Xp-Owners Meta-Data Storage-Map
pub type XpOwners = crate::XpOwners<Test, Instance1>;

/// Mocked-Instance2 Xp-Owners Meta-Data Storage-Map
pub type XpOwners2 = crate::XpOwners<Test, Instance2>;

/// Mock Xp-Reaped Meta-Data Storage-Map
pub type ReapedXp = crate::ReapedXp<Test, Instance1>;

/// Mocked-Instance2 Xp-Reaped Meta-Data Storage-Map
pub type ReapedXp2 = crate::ReapedXp<Test, Instance2>;

/// Mock Initial Xp Points Storage Value
pub type InitXp = crate::InitXp<Test, Instance1>;

/// Mocked-Instance2 Initial Xp Points Storage Value
pub type InitXp2 = crate::InitXp<Test, Instance2>;

/// Mock Discrete Accumulator's Stepper Type
pub type Stepper = crate::types::Stepper<Test, Instance1>;

/// Mocked-Instance2 Discrete Accumulator's Stepper Type
pub type Stepper2 = crate::types::Stepper<Test, Instance2>;

/// Mock Discrete Accumulator for Xp Pulse Tracking
pub type Accumulator = crate::types::Accumulator<Test, Instance1>;

/// Mock Pulse Factor for Reputation Tracking  Storage-Map
pub type PulseFactor = crate::PulseFactor<Test, Instance1>;

/// Mocked-Instance2 Pulse Factor for Reputation Tracking  Storage-Map
pub type PulseFactor2 = crate::PulseFactor<Test, Instance2>;

/// Mock Minimum Pulse Storage Value
pub type MinPulse = crate::MinPulse<Test, Instance1>;

/// Mocked-Instance2 Minimum Pulse Storage Value
pub type MinPulse2 = crate::MinPulse<Test, Instance2>;

/// Mock Minimum Xp Timestamp Storage Value
pub type MinTimeStamp = crate::MinTimeStamp<Test, Instance1>;

/// Mocked-Instance2 Minimum Xp Timestamp Storage Value
pub type MinTimeStamp2 = crate::MinTimeStamp<Test, Instance2>;

/// Mock Runtime Call Enum
pub type Call = <Test as Config<Instance1>>::RuntimeCall;

// ===============================================================================
// `````````````````````````````````` CONSTANTS ``````````````````````````````````
// ===============================================================================

/// AccountId of id `1`
pub const ALICE: AccountId = 1;

/// AccountId of id `2`
pub const BOB: AccountId = 2;

/// AccountId of id `3`
pub const CHARLIE: AccountId = 3;

/// Sample XP identifier (alpha case).
pub const XP_ALPHA: <Test as Config<Instance1>>::Xp = 1;

/// Sample XP identifier (beta case).
pub const XP_BETA: <Test as Config<Instance1>>::Xp = 2;

/// Sample XP identifier (gamma case).
pub const XP_GAMMA: <Test as Config<Instance1>>::Xp = 3;

/// Staking-related XP reason.
pub const STAKING: Reason = Reason::Staking;

/// Governance-related XP reason.
pub const REASON_TREASURY: Reason = Reason::Treasury;

/// Governance-related XP reason.
pub const GOVERNANCE: Reason = Reason::Governance;

/// Default XP amount used in tests.
pub const DEFAULT_POINTS: <Test as Config<Instance1>>::Xp = 10;

/// Represents an invalid or zero XP input.
pub const INVALID_POINTS: <Test as Config<Instance1>>::Xp = 0;

/// XP reward for comment actions.
pub const XP_REWARD_COMMENT: <Test as Config<Instance1>>::Xp = 5;

/// XP reward for proposal actions.
pub const XP_REWARD_PROPOSAL: <Test as Config<Instance1>>::Xp = 10;

/// Maximum XP value (saturation boundary).
pub const SATURATED_MAX: <Test as Config<Instance1>>::Xp = <Test as Config<Instance1>>::Xp::MAX;

// ===============================================================================
// ``````````````````````````````````` STRUCTS ```````````````````````````````````
// ===============================================================================

#[derive(
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    DecodeWithMemTracking,
    RuntimeDebug,
    Encode,
    Decode,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
    Default,
)]
pub enum Reason {
    #[default]
    Staking,
    Treasury,
    Governance,
}

// ===============================================================================
// ````````````````````````````````` TRAIT IMPLS `````````````````````````````````
// ===============================================================================

impl VariantCount for Reason {
    const VARIANT_COUNT: u32 = 3;
}

// ===============================================================================
// ``````````````````````````````` TEST ENV-HELPERS ``````````````````````````````
// ===============================================================================

pub fn xp_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_xp::GenesisConfig::<Test, pallet_xp::Instance1> {
        min_pulse: 1,
        init_xp: 10,
        pulse_factor: Stepper::new(50u8.into(), 10u8.into()).unwrap(),
        genesis_acc: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_xp::GenesisConfig::<Test, pallet_xp::Instance2> {
        min_pulse: 5,
        init_xp: 1,
        pulse_factor: Stepper2::new(20u8.into(), 6u8.into()).unwrap(),
        genesis_acc: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

// ===============================================================================
// ``````````````````````````````````` RUNTIME ```````````````````````````````````
// ===============================================================================

#[frame_support::runtime]
pub mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type Xp = pallet_xp::Pallet<Test, Instance1>;

    #[runtime::pallet_index(2)]
    pub type Xp2 = pallet_xp::Pallet<Test, Instance2>;
}

// ===============================================================================
// ``````````````````````````````````` CONFIGS ```````````````````````````````````
// ===============================================================================

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
}

impl pallet_xp::Config<Instance1> for Test {
    type Xp = u64;
    type Pulse = u32;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type LockReason = Reason;
    type ReserveReason = Reason;
    type Extensions = Ignore<Xp>;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}

impl pallet_xp::Config<Instance2> for Test {
    type Xp = u64;
    type Pulse = u32;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type LockReason = Reason;
    type ReserveReason = Reason;
    type Extensions = Ignore<Xp2>;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}