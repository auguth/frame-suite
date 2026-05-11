
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_chain_manager`.
pub trait WeightInfo {
fn validate() -> Weight;
fn chill() -> Weight;
fn declare() -> Weight;
fn elect() -> Weight;
fn force_allow_affidavits() -> Weight;
fn force_affidavit_begins_at() -> Weight;
fn force_affidavit_ends_at() -> Weight;
fn force_election_begins_at() -> Weight;
fn force_election_runner_points_upgrade() -> Weight;
fn force_validate_tx_priority() -> Weight;
fn force_election_tx_priority() -> Weight;
fn force_affidavit_tx_priority() -> Weight;
fn force_finality_after() -> Weight;
fn force_finality_ticks() -> Weight;
fn inspect_elects() -> Weight;
fn prepare_validation_payload() -> Weight;
fn inspect_affidavit() -> Weight;
fn on_offence(n: u32, ) -> Weight;
fn on_initialize_with_author() -> Weight;
}

impl WeightInfo for () {
    fn validate() -> Weight { Weight::zero() }
    fn chill() -> Weight { Weight::zero() }
    fn declare() -> Weight { Weight::zero() }
    fn elect() -> Weight { Weight::zero() }
    fn force_allow_affidavits() -> Weight { Weight::zero() }
    fn force_affidavit_begins_at() -> Weight { Weight::zero() }
    fn force_affidavit_ends_at() -> Weight { Weight::zero() }
    fn force_election_begins_at() -> Weight { Weight::zero() }
    fn force_election_runner_points_upgrade() -> Weight { Weight::zero() }
    fn force_validate_tx_priority() -> Weight { Weight::zero() }
    fn force_election_tx_priority() -> Weight { Weight::zero() }
    fn force_affidavit_tx_priority() -> Weight { Weight::zero() }
    fn force_finality_after() -> Weight { Weight::zero() }
    fn force_finality_ticks() -> Weight { Weight::zero() }
    fn inspect_elects() -> Weight { Weight::zero() }
    fn prepare_validation_payload() -> Weight { Weight::zero() }
    fn inspect_affidavit() -> Weight { Weight::zero() }

    fn on_offence(_n: u32) -> Weight {
        Weight::zero()
    }

    fn on_initialize_with_author() -> Weight {
        Weight::zero()
    }
}