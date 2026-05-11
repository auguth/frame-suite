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
// `````````````````````````````` APPLICATION CRYPTO `````````````````````````````
// ===============================================================================

//! Affidavit crypto and payload types for offchain operations used for validation,
//! affidavit declaration, and author election
//!
//! Utilizes
//! - [`AppCrypto`],
//! - [`SignedPayload`], and
//! - [`SigningTypes`]

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core / Std ---
use core::fmt::Debug;

// --- Scale-codec crates ---
use codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;

// --- FRAME System ---
use frame_system::offchain::{AppCrypto, SignedPayload, SigningTypes};

// --- Substrate primitives ---
use sp_application_crypto::app_crypto;
use sp_core::offchain::KeyTypeId;
use sp_runtime::{MultiSignature, MultiSigner, RuntimeDebug};

// ===============================================================================
// ``````````````````````````````` AFFIDAVIT CRYPTO ``````````````````````````````
// ===============================================================================

// Re-export supported affidavit crypto implementations.
//
// The runtime or pallet configuration may choose either implementation
// depending on desired cryptographic preferences.

pub use ed25519::AffidavitCryptoEd25519;

pub use sr25519::AffidavitCryptoSr25519;

/// Unique application key type identifier for affidavit-related
/// cryptography ([`crate::Config::AffidavitCrypto`]).
///
/// This `KeyTypeId` namespaces affidavit keys in the node's local keystore,
/// allowing the pallet to:
/// - Generate and rotate affidavit keys independently (see `routines`)
/// - Recover keys via OCWs when required
/// - Avoid collisions with other pallets or consensus keys
///
/// The identifier is intentionally short and fixed.
pub const AFDT_KEY_TYPE: KeyTypeId = KeyTypeId(*b"afdt");

/// sr25519-based affidavit cryptography implementation.
pub mod sr25519 {
    use super::*;

    /// Internal application-crypto binding for sr25519 affidavit keys.
    ///
    /// This binds the `AFDT_KEY_TYPE` namespace to sr25519 key material
    /// in the node keystore.
    mod app_sr25519 {
        use super::*;
        use sp_application_crypto::sr25519;
        app_crypto!(sr25519, AFDT_KEY_TYPE);
    }

    // Affidavit key pair type (sr25519).
    //
    // Used by offchain workers for signing affidavit- and
    // election-related payloads.
    sp_application_crypto::with_pair! {
        pub type AffidavitPair = app_sr25519::Pair;
    }

    /// Affidavit signature type (sr25519).
    pub type AffidavitSignature = app_sr25519::Signature;

    /// Affidavit public key type (sr25519).
    pub type AffidavitPublic = app_sr25519::Public;

    /// Runtime crypto adapter for sr25519 affidavit keys.
    ///
    /// This implementation allows affidavit signatures to be verified
    /// using `MultiSignature` while retaining a concrete sr25519
    /// application key internally.
    pub struct AffidavitCryptoSr25519;

    impl AppCrypto<MultiSigner, MultiSignature> for AffidavitCryptoSr25519 {
        type RuntimeAppPublic = AffidavitPublic;
        type GenericSignature = sp_application_crypto::sr25519::Signature;
        type GenericPublic = sp_application_crypto::sr25519::Public;
    }
}

/// ed25519-based affidavit cryptography implementation.
pub mod ed25519 {
    use super::*;

    /// Internal application-crypto binding for ed25519 affidavit keys.
    ///
    /// This binds the `AFDT_KEY_TYPE` namespace to ed25519 key material
    /// in the node keystore.
    mod app_ed25519 {
        use super::*;
        use sp_application_crypto::ed25519;
        app_crypto!(ed25519, AFDT_KEY_TYPE);
    }

    // Affidavit key pair type (ed25519).
    //
    // Used by offchain workers for signing affidavit- and
    // election-related payloads.
    sp_application_crypto::with_pair! {
        pub type AffidavitPair = app_ed25519::Pair;
    }

    /// Affidavit signature type (ed25519).
    pub type AffidavitSignature = app_ed25519::Signature;

    /// Affidavit public key type (ed25519).
    pub type AffidavitPublic = app_ed25519::Public;

    /// Runtime crypto adapter for ed25519 affidavit keys.
    ///
    /// Enables verification of affidavit signatures through
    /// `MultiSignature` while internally using ed25519 keys.
    pub struct AffidavitCryptoEd25519;

    impl AppCrypto<MultiSigner, MultiSignature> for AffidavitCryptoEd25519 {
        type RuntimeAppPublic = AffidavitPublic;
        type GenericSignature = sp_application_crypto::ed25519::Signature;
        type GenericPublic = sp_application_crypto::ed25519::Public;
    }
}

// ===============================================================================
// `````````````````````````````` UNSIGNED PAYLOADS ``````````````````````````````
// ===============================================================================

/// Unsigned payload for [`crate::Pallet::declare`] extrinsic.
///
/// - `public` is the **currently active affidavit key**, used to authorize
///   the affidavit declaration.
/// - `rotate` is the **next affidavit key** that will be registered and
///   rotated in as the active key for the upcoming session.
///
/// The payload is signed offchain using the active affidavit key and
/// verified on-chain via `ValidateUnsigned`.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, DecodeWithMemTracking)]
pub struct AffidavitPayload<Public, AccountId>
where
    Public: Debug + TypeInfo,
    AccountId: Debug + TypeInfo,
{
    /// Active affidavit public key used for authorization.
    pub public: Public,

    /// Next affidavit key to be rotated in for the upcoming session.
    pub rotate: AccountId,
}

impl<T: SigningTypes<Public = Public>, Public, AccountId> SignedPayload<T>
    for AffidavitPayload<Public, AccountId>
where
    Public: Encode + Debug + TypeInfo + Clone,
    AccountId: Encode + Debug + TypeInfo + Clone,
{
    fn public(&self) -> <T as SigningTypes>::Public {
        self.public.clone()
    }
}

/// Unsigned payload for [`crate::Pallet::validate`] extrinsic.
///
/// The `public` field represents the **active affidavit key** whose
/// validity will be verified later during affidavit declaration.
///
/// This payload is signed offchain and authenticated on-chain
/// via `ValidateUnsigned`.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, DecodeWithMemTracking)]
pub struct ValidatePayload<Public>
where
    Public: Debug + TypeInfo,
{
    /// Active affidavit public key to be verified.
    pub public: Public,
}

impl<T: SigningTypes<Public = Public>, Public> SignedPayload<T> for ValidatePayload<Public>
where
    Public: Encode + Debug + TypeInfo + Clone,
{
    fn public(&self) -> <T as SigningTypes>::Public {
        self.public.clone()
    }
}

/// Unsigned payload for [`crate::Pallet::elect`] extrinsic.
///
/// The `public` field represents the **currently active affidavit key**,
/// which was very recently rotated during the latest affidavit declaration.
///
/// Elections require the affidavit key that was registered for the
/// upcoming session's affidavit window. This ensures that only authors
/// who successfully completed affidavit declaration and key rotation
/// are eligible to run the election.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, DecodeWithMemTracking)]
pub struct ElectionPayload<Public>
where
    Public: Debug + TypeInfo,
{
    /// Active affidavit public key authorizing election execution.
    pub public: Public,
}

impl<T: SigningTypes<Public = Public>, Public> SignedPayload<T> for ElectionPayload<Public>
where
    Public: Encode + Debug + TypeInfo + Clone,
{
    fn public(&self) -> <T as SigningTypes>::Public {
        self.public.clone()
    }
}
