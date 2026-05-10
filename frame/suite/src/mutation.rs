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
// ``````````````````````````````` MUTATION SUITE ````````````````````````````````
// ===============================================================================

//! Mutation-focused abstractions over owned and borrowed values,
//! treating mutability as the primary capability rather than ownership.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Substrate primitives ---
use sp_runtime::Cow;

// ===============================================================================
// ````````````````````````````````` MUT-HANDLE ``````````````````````````````````
// ===============================================================================

/// A mutable access abstraction over a value that may be either borrowed or owned.
///
/// `MutHandle` represents the idea that mutation is the primary capability,
/// while ownership is incidental. It allows code to operate on a value
/// uniformly without caring whether that value is owned or borrowed.
///
/// The key semantic guarantee is:
/// - mutation is always permitted
/// - ownership is never implicitly changed or upgraded
///
/// This makes it suitable for contexts where:
/// - mutation must be expressed generically
/// - ownership should remain explicit and stable
/// - no hidden allocation or cloning is allowed
///
/// Conceptually, it models a "mutable view" over data with a fixed ownership mode,
/// enabling APIs to focus on behavior (mutation) rather than representation (ownership).
pub enum MutHandle<'a, T> {
    Borrowed(&'a mut T),
    Owned(T),
}

impl<'a, T> core::ops::Deref for MutHandle<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            MutHandle::Borrowed(v) => &*v,
            MutHandle::Owned(v) => v,
        }
    }
}

impl<'a, T> core::ops::DerefMut for MutHandle<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        match self {
            MutHandle::Borrowed(v) => *v,
            MutHandle::Owned(v) => v,
        }
    }
}

impl<'a, T> core::convert::AsRef<T> for MutHandle<'a, T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<'a, T> core::convert::AsMut<T> for MutHandle<'a, T> {
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

impl<'a, T> core::borrow::Borrow<T> for MutHandle<'a, T> {
    fn borrow(&self) -> &T {
        self
    }
}
impl<'a, T> core::borrow::BorrowMut<T> for MutHandle<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<'a, T> From<&'a mut T> for MutHandle<'a, T> {
    fn from(v: &'a mut T) -> Self {
        MutHandle::Borrowed(v)
    }
}

impl<'a, T> From<T> for MutHandle<'a, T> {
    fn from(v: T) -> Self {
        MutHandle::Owned(v)
    }
}

impl<'a, T: Clone> From<MutHandle<'a, T>> for Cow<'a, T> {
    fn from(v: MutHandle<'a, T>) -> Self {
        match v {
            MutHandle::Borrowed(v) => Cow::Borrowed(v),
            MutHandle::Owned(v) => Cow::Owned(v),
        }
    }
}

impl<'a, T: Clone> From<Cow<'a, T>> for MutHandle<'a, T> {
    fn from(c: Cow<'a, T>) -> Self {
        match c {
            Cow::Borrowed(v) => MutHandle::Owned(v.clone()),
            Cow::Owned(v) => MutHandle::Owned(v),
        }
    }
}

impl<'a, T: core::fmt::Debug> core::fmt::Debug for MutHandle<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: Clone> From<&'a T> for MutHandle<'a, T> {
    fn from(v: &'a T) -> Self {
        MutHandle::Owned(v.clone())
    }
}

impl<'a, T: core::hash::Hash> core::hash::Hash for MutHandle<'a, T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}

impl<'a, T: PartialEq> PartialEq for MutHandle<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<'a, T: Eq> Eq for MutHandle<'a, T> {}

impl<'a, T: PartialOrd> PartialOrd for MutHandle<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<'a, T: Ord> Ord for MutHandle<'a, T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<'a, T: Default> Default for MutHandle<'a, T> {
    fn default() -> Self {
        MutHandle::Owned(T::default())
    }
}
