
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_commitment`.
pub trait WeightInfo {
fn deposit_reserve() -> Weight;
fn withdraw_reserve() -> Weight;
fn withdraw_reserve_partial() -> Weight;
fn inspect_digest_model() -> Weight;
fn inspect_commit_value() -> Weight;
fn inspect_index_value() -> Weight;
fn inspect_entry_value() -> Weight;
fn inspect_entries_value() -> Weight;
fn inspect_pool_value() -> Weight;
fn inspect_slot_value() -> Weight;
fn inspect_slots_value() -> Weight;
fn inspect_pool_commission() -> Weight;
fn inspect_pool_manager() -> Weight;
fn inspect_asset_to_issue() -> Weight;
fn inspect_asset_to_reap() -> Weight;
fn inspect_reason_value() -> Weight;
fn reason_value() -> Weight;
}

impl WeightInfo for () {
    fn deposit_reserve() -> Weight { Weight::zero() }
    fn withdraw_reserve() -> Weight { Weight::zero() }
    fn withdraw_reserve_partial() -> Weight { Weight::zero() }
    fn inspect_digest_model() -> Weight { Weight::zero() }
    fn inspect_commit_value() -> Weight { Weight::zero() }
    fn inspect_index_value() -> Weight { Weight::zero() }
    fn inspect_entry_value() -> Weight { Weight::zero() }
    fn inspect_entries_value() -> Weight { Weight::zero() }
    fn inspect_pool_value() -> Weight { Weight::zero() }
    fn inspect_slot_value() -> Weight { Weight::zero() }
    fn inspect_slots_value() -> Weight { Weight::zero() }
    fn inspect_pool_commission() -> Weight { Weight::zero() }
    fn inspect_pool_manager() -> Weight { Weight::zero() }
    fn inspect_asset_to_issue() -> Weight { Weight::zero() }
    fn inspect_asset_to_reap() -> Weight { Weight::zero() }
    fn inspect_reason_value() -> Weight { Weight::zero() }
    fn reason_value() -> Weight { Weight::zero() }
}