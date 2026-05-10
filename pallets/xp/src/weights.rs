#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_xp`.
pub trait WeightInfo {
fn handover() -> Weight;
fn dispose() -> Weight;
fn force_handover() -> Weight;
fn force_update_min_time_stamp() -> Weight;
fn call() -> Weight;
fn force_update_init_xp() -> Weight;
fn force_update_min_pulse() -> Weight;
fn force_update_pulse_factor() -> Weight;
fn inspect_xp_keys_of() -> Weight;
fn inspect_my_xp() -> Weight;
}

impl WeightInfo for () {
    fn handover() -> Weight { Weight::zero() }
    fn dispose() -> Weight { Weight::zero() }
    fn force_handover() -> Weight { Weight::zero() }
    fn force_update_min_time_stamp() -> Weight { Weight::zero() }
    fn call() -> Weight { Weight::zero() }
    fn force_update_init_xp() -> Weight { Weight::zero() }
    fn force_update_min_pulse() -> Weight { Weight::zero() }
    fn force_update_pulse_factor() -> Weight { Weight::zero() }
    fn inspect_xp_keys_of() -> Weight { Weight::zero() }
    fn inspect_my_xp() -> Weight { Weight::zero() }
}