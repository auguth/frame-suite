// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{AccountId, BalancesConfig, RuntimeGenesisConfig, SudoConfig};
use alloc::{vec, vec::Vec};
use frame_benchmarking::{account};
use frame_support::build_struct_json_patch;
use pallet_xp::types::GenesisAcc;
use serde_json::Value;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;

const SEED: u32 = 1;

fn generate_genesis_author_acc(prefix: &'static str, count: u32) -> Vec<GenesisAcc<AccountId, AccountId>> {
    let mut gen_accounts = Vec::default();
    for i in 0..count {
        let acc = account(prefix, i, SEED);
        let prefix_id = match prefix {
            "validator" => "validator_id",
            _ => panic!("prefix mismatch"),
        };
        let acc_id = account(prefix_id, i, SEED);
        gen_accounts.push(GenesisAcc {
            owner: acc,
            id: acc_id,
        });
    }
    gen_accounts
}

fn generate_genesis_funder_acc(prefix: &'static str, count: u32) -> Vec<GenesisAcc<AccountId, AccountId>> {
    let mut gen_accounts = Vec::default();
    for i in 0..count {
        let acc = account(prefix, i, SEED);
        let prefix_id = match prefix {
            "backer" => "backer_id",
            _ => panic!("prefix mismatch"),
        };
        let acc_id = account(prefix_id, i, SEED);
        gen_accounts.push(GenesisAcc {
            owner: acc,
            id: acc_id,
        });
    }
    gen_accounts
}

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
    initial_authorities: Vec<(AuraId, GrandpaId)>,
    endowed_accounts: Vec<AccountId>,
    root: AccountId,
) -> Value {
    let mut genesis_accounts = vec![
        GenesisAcc {
            owner: account("whitelisted_caller", 0, SEED),
            id: account("whitelisted_caller", 0, SEED),
        },
        GenesisAcc {
            owner: account("alice", 0, SEED),
            id: account("alice_id", 0, SEED),
        },
        GenesisAcc {
            owner: account("bob", 0, SEED),
            id: account("bob_id", 0, SEED),
        },
        GenesisAcc {
            owner: account("charlie", 0, SEED),
            id: account("charlie_id", 0, SEED),
        },
        GenesisAcc {
            owner: account("alan", 0, SEED),
            id: account("alan_id", 0, SEED),
        },
        GenesisAcc {
            owner: account("mike", 0, SEED),
            id: account("mike_id", 0, SEED),
        },
    ];
    genesis_accounts.extend(generate_genesis_author_acc("validator", 250));
    genesis_accounts.extend(generate_genesis_funder_acc("backer", 250));
	
    build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1u128 << 60))
                .collect::<Vec<_>>(),
        },
        aura: pallet_aura::GenesisConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>(),
        },
        grandpa: pallet_grandpa::GenesisConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.1.clone(), 1))
                .collect::<Vec<_>>(),
        },
        sudo: SudoConfig { key: Some(root) },
        xp: pallet_xp::GenesisConfig {
            genesis_acc: genesis_accounts,
            ..Default::default()
        }
    })
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
    testnet_genesis(
        vec![(
            sp_keyring::Sr25519Keyring::Alice.public().into(),
            sp_keyring::Ed25519Keyring::Alice.public().into(),
        )],
        vec![
            Sr25519Keyring::Alice.to_account_id(),
            Sr25519Keyring::Bob.to_account_id(),
            Sr25519Keyring::AliceStash.to_account_id(),
            Sr25519Keyring::BobStash.to_account_id(),
        ],
        sp_keyring::Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
    testnet_genesis(
        vec![
            (
                sp_keyring::Sr25519Keyring::Alice.public().into(),
                sp_keyring::Ed25519Keyring::Alice.public().into(),
            ),
            (
                sp_keyring::Sr25519Keyring::Bob.public().into(),
                sp_keyring::Ed25519Keyring::Bob.public().into(),
            ),
        ],
        Sr25519Keyring::iter()
            .filter(|v| v != &Sr25519Keyring::One && v != &Sr25519Keyring::Two)
            .map(|v| v.to_account_id())
            .collect::<Vec<_>>(),
        Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work. qed.")
            .into_bytes(),
    )
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
    ]
}
