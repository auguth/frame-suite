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
// ````````````````````````````````` KEYS SUITE ``````````````````````````````````
// ===============================================================================

//! Utilities for deterministic identifier derivation.
//!
//! Provides a generic mechanism to generate reproducible identifiers (`Id`)
//! from a combination of:
//! - a base key (`Id`)
//! - an associated item (`Item`)
//! - a salt (`Salt`)
//!
//! using a hashing algorithm (via [`Hash`]).
//!
//! The same input tuple `(Id, Item, Salt)` will always produce the same output,
//! enabling stable and namespaced key derivation.
//!
//! ## Components
//!
//! - [`KeySeedFor`]: Encodes the derivation inputs and produces a derived key.
//! - [`KeyGenFor`]: Trait providing a generic interface for key generation.
//!
//! ## Guarantees
//!
//! - Deterministic: identical inputs yield identical outputs
//! - Domain separation via `(Item, Salt)` inputs
//!
//! ## Notes
//!
//! - Uniqueness depends on correct salt usage
//! - Decoding from hash output must succeed for valid key generation

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core ---
use core::marker::PhantomData;

// --- SCALE & metadata ---
use codec::{Decode, Encode, FullCodec, MaxEncodedLen};
use scale_info::TypeInfo;

// --- Substrate crates ---
use sp_runtime::{traits::Hash, RuntimeDebug};

// ===============================================================================
// ``````````````````````````````` STRUCTURES ````````````````````````````````````
// ===============================================================================

/// A seed structure for deterministic identifier derivation.
///
/// `KeySeedFor` enables the generation of a unique, deterministic identifier (`Id`)
/// from a combination of:
/// - a target key type (`Id`)
/// - a meta-data value (`Item`)
/// - a unique salt (`Salt`)
///
/// within the context of a specific runtime (`T`) and hashing algorithm (`Hash`).
///
/// ### Overview
/// This struct serves as the canonical pre-image for generating new deterministic
/// IDs, based on a source identity (`target`), a value of interest (`item`), and a
/// unique `salt`.
///
/// ### Type Parameters
/// - `Item`: The value associated with the derived identifier. May be low entropy
/// or even default.
/// - `Id`: The identifier type, used both as the source (`key`) and the derived
/// output (`target`).
/// - `Salt`: A unique value to ensure uniqueness per `(Id, Item)` pair.
/// - `Hash`: The hashing algorithm used for deterministic ID derivation.
/// - `T`: The runtime context.
///
/// ### Constraints & Responsibilities
/// - `target` (`Id`): Must be a high-entropy, globally unique identifier (e.g., account ID, hash, public key).
/// - `item` (`Item`): May be low-entropy; a single `Id` can be associated with multiple `Item` types,
///   but each `Item` type must be uniquely tied to a single `Id` type.
/// - `salt`: Must be unique per `(Id, Item)` pair; implementers must ensure salts are not reused.
///
/// ### Example Use Cases
/// - Sub-identities derived from a parent identity
/// - Capability delegation
/// - Namespaced object IDs
/// - Resource handles scoped to specific keys
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct KeySeedFor<Id, Item, Salt, Hash, T> {
    target: Id,
    item: Item,
    salt: Salt,

    #[codec(skip)]
    #[scale_info(skip)]
    _hash: PhantomData<Hash>,

    #[codec(skip)]
    #[scale_info(skip)]
    _marker: PhantomData<T>,
}

// ===============================================================================
// ````````````````````````````` INHERENT IMPLS ``````````````````````````````````
// ===============================================================================

/// Implementation of utility methods for `KeySeedFor`.
impl<Id, Item, Salt, Hasher, T> KeySeedFor<Id, Item, Salt, Hasher, T>
where
    Id: Clone + FullCodec,
    Item: Clone + FullCodec,
    Salt: Clone + FullCodec,
    Hasher: Hash,
{
    /// Gets the current value of the target key.
    ///
    pub fn target(&self) -> Id {
        self.target.clone()
    }

    /// Gets the current value of the item (metadata for the target key).
    ///
    pub fn item(&self) -> Item {
        self.item.clone()
    }

    /// Gets the current value of the salt used for key derivation.
    ///
    pub fn salt(&self) -> Salt {
        self.salt.clone()
    }

    /// Provides mutable access to the target key.
    ///
    pub fn mut_target(&mut self) -> &mut Id {
        &mut self.target
    }

    /// Provides mutable access to the item (metadata).
    ///
    pub fn mut_item(&mut self) -> &mut Item {
        &mut self.item
    }

    /// Provides mutable access to the salt.
    ///
    pub fn mut_salt(&mut self) -> &mut Salt {
        &mut self.salt
    }

    /// Constructs a new `KeySeedFor` from the given target, item and salt.
    ///
    pub fn new(target: Id, item: Item, salt: Salt) -> Self {
        Self {
            target,
            item,
            salt,
            _marker: PhantomData,
            _hash: PhantomData,
        }
    }

    /// Generates a deterministic identifier (`Id`) by hashing the encoded
    /// target, item, and salt.
    ///
    /// Returns `Some(Id)` if decoding from the hash output succeeds, or
    /// `None` otherwise.
    pub fn key_gen(&self) -> Option<Id> {
        let mut encoded = self.item().encode();
        encoded.extend(self.target().encode());
        encoded.extend(self.salt().encode());

        let hash = <Hasher as Hash>::hash(&encoded);
        let Ok(id) = Id::decode(&mut hash.as_ref()) else {
            return None;
        };
        Some(id)
    }
}

// ===============================================================================
// ````````````````````````````````` TRAITS ``````````````````````````````````````
// ===============================================================================

/// Trait for generating deterministic identifiers from a combination of key, item,
/// and salt.
///
/// This trait abstracts the process of deriving a unique, reproducible identifier (`Id`)
/// from a source key (`target`), associated item (metadata), and a salt value, using
/// a specified hashing algorithm.
///
/// It is intended for use cases where deterministic, namespaced, or context-specific
/// IDs are required, such as sub-identities, capability delegation, or resource handles.
///
/// ## Type Parameters
/// - `Id`: The identifier type to be generated and used as the source key.
/// - `Item`: The metadata or context value associated with the identifier.
/// - `Salt`: A unique value to ensure uniqueness per `(Id, Item)` pair.
/// - `Hasher`: The hashing algorithm used for deterministic ID derivation.
/// - `T`: The runtime context (e.g., a Substrate pallet's `Config`).
pub trait KeyGenFor<Id, Item, Salt, Hasher, T>
where
    Id: Clone + FullCodec,
    Item: Clone + FullCodec,
    Salt: Clone + FullCodec,
    Hasher: Hash,
{
    /// Generates a deterministic identifier (`Id`) from the given target, item,
    /// and salt.
    ///
    /// This method constructs a [`KeySeedFor`] instance and invokes its `key_gen`
    /// method, ensuring that the same input combination always produces the same
    /// output identifier.
    ///
    /// Returns `Some(Id)` if key generation succeeds, or `None` if decoding from
    /// the hash output fails.
    fn gen_key(target: &Id, item: &Item, salt: Salt) -> Option<Id> {
        let key = KeySeedFor::<Id, Item, Salt, Hasher, T>::new(target.clone(), item.clone(), salt)
            .key_gen()?;
        Some(key)
    }
}

/// Blanket implementation of [`KeyGenFor`] for [`KeySeedFor`].
///
/// This allows any `KeySeedFor` instance to use the `gen_key` utility
///
impl<Id, Item, Salt, Hasher, T> KeyGenFor<Id, Item, Salt, Hasher, T>
    for KeySeedFor<Id, Item, Salt, Hasher, T>
where
    Id: Clone + FullCodec,
    Item: Clone + FullCodec,
    Salt: Clone + FullCodec,
    Hasher: Hash,
{
}

// ===============================================================================
// ```````````````````````````` TEST UTILITIES ```````````````````````````````````
// ===============================================================================

/// Internal test utilities for validating [`KeyGenFor`] implementations.
///
/// This module provides reusable checks to ensure deterministic behavior
/// and basic input separation properties of key generation logic.
pub mod test_utils {
    use super::*;

    /// Verifies deterministic key generation.
    ///
    /// Ensures that identical `(target, item, salt)` inputs produce
    /// the same derived key across multiple invocations.
    pub fn run_keygen_deterministic_check<
        Id: Clone + FullCodec + PartialEq + core::fmt::Debug,
        Item: Clone + FullCodec,
        Salt: Clone + FullCodec,
        Hasher: Hash,
        T,
        Impl: KeyGenFor<Id, Item, Salt, Hasher, T>,
    >(
        target: Id,
        item: Item,
        salt: Salt,
    ) {
        // Determinism
        let gen_key_1 = Impl::gen_key(&target, &item, salt.clone()).unwrap();
        let gen_key_2 = Impl::gen_key(&target, &item, salt.clone()).unwrap();
        assert_eq!(gen_key_1, gen_key_2);
    }

    /// Verifies input sensitivity of key generation.
    ///
    /// Ensures that changes in any of the inputs (`target`, `item`, or `salt`)
    /// result in a different derived key.
    pub fn run_keygen_collision_check<
        Id: Clone + FullCodec + PartialEq + core::fmt::Debug,
        Item: Clone + FullCodec + PartialEq + core::fmt::Debug,
        Salt: Clone + FullCodec + PartialEq + core::fmt::Debug,
        Hasher: Hash,
        T,
        Impl: KeyGenFor<Id, Item, Salt, Hasher, T>,
    >(
        target: Id,
        item: Item,
        salt: Salt,
        dif_target: Id,
        dif_item: Item,
        dif_salt: Salt,
    ) {
        let base_key = Impl::gen_key(&target, &item, salt.clone()).unwrap();
        let dif_salt = Impl::gen_key(&target, &item, dif_salt).unwrap();
        let dif_item = Impl::gen_key(&target, &dif_item, salt.clone()).unwrap();
        let dif_target = Impl::gen_key(&dif_target, &item, salt.clone()).unwrap();

        // asserts
        assert_ne!(base_key, dif_salt);
        assert_ne!(base_key, dif_item);
        assert_ne!(base_key, dif_target);
    }
}
