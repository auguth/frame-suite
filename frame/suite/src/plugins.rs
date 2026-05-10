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
// ```````````````````````````````` PLUGINS SUITE ````````````````````````````````
// ===============================================================================

//! Pluggable, type-safe execution framework for composing runtime behavior via plugin
//! models and families.
//!
//! This module offers two complementary abstractions for extensible, type-safe
//! runtime behaviour:
//!
//! - Read [Plugin Model](#plugin-model) to understand the **fundamental unit**
//!   of computation - a single operation plugin, analogous to a pure function
//!   or a procedure that may mutate its input and produce an output.
//!
//! - Read [Plugin Families](#plugin-families) when modelling a **cohesive
//!   state-machine-like component** composed of multiple related operations
//!   (methods). A family groups several operation-specific plugin models under
//!   one logical root, while the concrete implementation variant of each
//!   operation is plugged via a single family model.
//!
//! > **Note:** Plugin families are built on top of plugin models.  
//! > To correctly design or use families, one must first understand the
//! > plugin model abstraction, since families internally orchestrate multiple
//! > models as their operational building blocks.
//!
//! In short:
//!
//! ```text
//! Simple, single-step transformation?                 -> Plugin Model
//! Multi-operation logical component / state machine? -> Plugin Family (built from Plugin Models)
//! ```
//!
//! Both approaches share the same execution infrastructure and compile-time
//! type-safety guarantees, but differ in how behaviour is structured,
//! composed, and ultimately resolved.
//!
//! # Plugin Model
//!
//! A **plugin model** is a type-safe, swappable unit of computation with optional
//! context. It enables deterministic, composable, and runtime-configurable behavior.
//!
//! Each model:
//! - Implements [`PurePluginModel<Input, Context, Output>`] or
//!   [`MutablePluginModel<Input, Context, Output>`]
//! - Produces an output from input (and optional context)
//!
//! Execution:
//! - `compute(input, &context) -> output`
//! - `compute_mut(&mut input, &context) -> output`
//!
//! Context:
//! - Provided via [`ModelContext`]
//! - Defined using [`plugin_context`](crate::plugin_context)
//!
//! Tooling:
//! - Declare via [`plugin_types`](crate::plugin_types)
//! - Define via [`plugin_model`](crate::plugin_model)
//! - Execute via [`plugin_output`](crate::plugin_output)
//! - Test with [`plugin_test`](crate::plugin_test)
//!
//! ## Motivation
//!
//! Conventional trait-based designs couple the **contract** and the
//! **implementation** into a single resolution step:
//!
//! ```text
//! Sub-set Contract (Trait Bounds)
//!        |
//!        v
//! Concrete Type (Implementation)
//! ```
//!
//! The implementation is chosen first, and the contract is something it must
//! satisfy. Any **stronger bounds or richer behavior** remain internal to the
//! concrete type and cannot be independently selected or composed.
//!
//! This leads to key limitations:
//!
//! - Behavior is fixed once the type is chosen
//! - Stronger capabilities cannot be surfaced or selected explicitly
//! - Context-driven or configuration-based behavior becomes difficult
//!
//! ## Behaviour Model
//!
//! Plugin models treat behavior as a **compatibility problem between two
//! independent entities**:
//!
//! ```text
//! Sub-set Contract (Pallet)
//!        <->
//! Super-set Capability (Model + Bounds + Context)
//! ```
//!
//! These are defined independently and only come together through
//! **compatibility matching**.
//!
//! Matching rule:
//!
//! - The super-set must satisfy all requirements of the sub-set
//! - The sub-set must allow the super-set's stronger bounds
//!
//! Only when both conditions hold does a valid composition exist.
//!
//! ## Benefits
//!
//! - Decoupled contract and behavior
//! - Multiple interchangeable implementations
//! - Context-driven execution
//! - Late selection via configuration
//! - Full compile-time verification
//!
//! Behaviour is not implemented, it is resolved by matching a sub-set contract
//! with a compatible super-set capability.
//!
//! ## Example: Sorter Plugin
//!
//! This example shows how a pallet can dynamically select sorting strategies
//! at runtime via plugin types.
//!
//! ```ignore
//!
//! // ----- Support Crate -------
//!
//! /// A generic sorter plugin trait.
//! ///
//! /// This trait defines a plugin point where the actual sorting logic
//! /// is provided by an associated plugin model and its context.
//! pub trait Sorter<Input> {
//!     /// The output type produced by the sorter.
//!     type Output;
//!
//!     // Declare the associated plugin model and context types.
//!     // These will be supplied by downstream crates (e.g., pallets or runtime).
//!     plugin_types! {
//!         input: Input,
//!         output: Self::Output,
//!         model: Model,
//!         context: Context,
//!     }
//!
//!     plugin_output! {
//!         /// Execute the sorting logic using the injected plugin model.
//!         ///
//!         /// The actual implementation is resolved at compile time based on
//!         /// the associated `Model` and `Context` types.
//!         fn sort
//!         input: values,
//!         model: Self::Model,
//!         context: Self::Context,
//!     }
//! }
//!
//! // ----- Pallet Crate -------
//!
//! /// Pallet configuration exposing plugin hook points.
//! ///
//! /// The runtime will decide which concrete model and context to use.
//! pub trait Config: frame_system::Config {
//!     /// Input type consumed by the sorter plugin.
//!     type InputX;
//!
//!     /// Output type produced by the sorter plugin.
//!     type OutputX;
//!
//!     // Declare pallet-level plugin types that must satisfy the plugin contract.
//!     plugin_types! {
//!         input: Self::InputX,
//!         output: Self::OutputX,
//!         model: ModelX,     // Concrete model chosen by the runtime
//!         context: ContextX, // Concrete context provider chosen by the runtime
//!     }
//! }
//!
//! /// Implement the generic sorter plugin for the pallet.
//! ///
//! /// The pallet simply forwards execution to the configured model.
//! impl<T: Config> Sorter<T::InputX> for Pallet<T> {
//!     type Output = T::OutputX;
//!     type Model = T::ModelX;
//!     type Context = T::ContextX;
//! }
//!
//! /// Helper function demonstrating how the plugin is executed generically.
//! fn try_sort<T: Config>(values: &T::InputX) -> T::OutputX {
//!     <Pallet<T> as Sorter<T::InputX>>::sort(values)
//! }
//!
//! // ----- Runtime Crate -------
//!
//! /// Define a generic plugin model.
//! /// This model sorts the generic type `Vector` in ascending order and then
//! /// purges all elements greater than the runtime `until` threshold.
//! /// Such many models like this can live in plugin-registries
//! plugin_model! {
//!     name: CappedSort,
//!     input: Vector,
//!     output: Vector,
//!     context: UntilConfig<Number>,
//!     others: [Number],
//!     bounds: [
//!         // Elements must be comparable and clonable
//!         Number: Unsigned + Clone + Ord,
//!         // Vector must be iterable and rebuildable after filtering
//!         Vector: IntoIterator<Item = Number> + FromIterator<Number> + Clone,
//!     ],
//!     compute: |values, ctx| {
//!         // Clone input so original remains unchanged
//!         let mut v: Vector = values.clone();
//!
//!         // Retrieve runtime threshold from context
//!         let until = ctx.0.clone();
//!
//!         // Sort from small to large
//!         let mut temp: Vec<Number> = v.into_iter().collect();
//!         temp.sort();
//!
//!         // Find first element greater than `until`
//!         // Purge that element and everything after it
//!         let filtered = temp
//!             .into_iter()
//!             .take_while(|x| *x <= until)
//!             .collect::<Vec<_>>();
//!
//!         // Rebuild the output vector from the filtered values
//!         filtered.into_iter().collect()
//!     }
//! }
//!
//! /// Context data structure holding the threshold value.
//! /// Such many models's contexts like this can live in
//! /// plugin-registries, as models and its contexts are tightly coupled
//! struct UntilConfig<Number>(Number);
//!
//! /// Define a concrete context provider supplying the threshold.
//! plugin_context! {
//!     name: MyContext,
//!     context: UntilConfig<u8>,
//!     value: UntilConfig(10),
//! }
//!
//! /// Inject the concrete model and context into the runtime configuration.
//! impl Config for Runtime {
//!     type InputX = Vec<u8>;
//!     type OutputX = Vec<u8>;
//!     type ModelX = CappedSort;   // Uses capped sorting logic
//!     type ContextX = MyContext;  // Provides the `until` threshold
//! }
//!
//! // Example behavior:
//! // Input:  vec![12, 3, 8, 25, 5]
//! // Sorted: [3, 5, 8, 12, 25]
//! // until = 10
//! // Output: [3, 5, 8]   // elements > 10 are purged
//!
//! ```
//!
//! The pallet only assumes a generic "sorter" transformation. The runtime injects
//! `CappedSort`, which sorts values ascending and purges elements greater than a
//! contextual `until` threshold. The pallet sees only the minimal contract,
//! while the runtime provides richer, context-driven logic.
//!
//! # Plugin Families
//!
//! A **plugin family** extends a single plugin model into a **unified logical
//! plugin** with multiple related operations.
//!
//! Unlike a model (one computation), a family represents a cohesive component
//! (e.g., state machine or service) whose operations are selected via *child*
//! markers, while a **family type** maps them to concrete models.
//!
//! This lets callers use a single interface while deferring implementation
//! choice to runtime configuration.
//!
//! ## Motivation
//!
//! A single model fits simple transformations:
//!
//! ```text
//! Model   -> Implementation
//! Context -> Parameters
//! ```
//!
//! But real systems need:
//!
//! - Multiple related operations
//! - Multiple strategy variants
//! - Configurable behaviour
//!
//! A **plugin family** groups these operations under one unit, where:
//!
//! - Children = operations
//! - Family type = model mapping
//!
//! Result: structured, state-machine-like design with compile-time resolution.
//!
//! ## State-Machine Style Logical Plugin
//!
//! A plugin family acts as a logical component exposing multiple operations,
//! similar to a state machine or service:
//!
//! ```text
//! Family Root (Unified Interface)
//!   |-- Child A -> Operation A
//!   |-- Child B -> Operation B
//!   |-- Child C -> Operation C -> Concrete Model (via Family Type)
//! ```
//!
//! - **Root**: unified special-interface
//! - **Child**: operation selector
//! - **Family type**: maps each operation to a concrete model
//!
//! Calling a child is equivalent to invoking a method on the plugin.
//! The concrete model is resolved by the family type, with context passed
//! through to execution.
//!
//! ### Family Contract Consistency
//!
//! All models within a **family type** are expected to share the same
//! execution contract:
//!
//! ```text
//! Input   -> shared
//! Output  -> shared
//! Context -> shared
//! ```
//!
//! This allows the family to behave as a **uniform pluggable component**,
//! where operations can be invoked without knowledge of the underlying model.
//!
//! ```text
//! Family Type
//!   |-- Child A -> ModelA<Input, Context, Output>
//!   |-- Child B -> ModelB<Input, Context, Output>
//!   |-- Child C -> ModelC<Input, Context, Output>
//! ```
//!
//! With a consistent `(Input, Output, Context)` signature, callers interact
//! through the root interface while the compiler resolves the concrete model.
//!
//! While not strictly enforced, consistent contracts are recommended for
//! clarity and interchangeability.
//!
//! ### Trait Bound Consistency
//!
//! Plugin models define `(Input, Output, Context)` generically via trait bounds.
//!
//! Within a **family type**, the concrete types must satisfy the **combined
//! bounds** of all possible models.
//!
//! ```text
//! ModelA requires: Input: Ord
//! ModelB requires: Input: Clone
//! ```
//!
//! -> Caller must provide:
//!
//! ```text
//! Input: Ord + Clone
//! ```
//!
//! The family contract therefore reflects the **union of required bounds**:
//!
//! ```text
//! Family Contract
//!   Input: Ord + Clone
//!   Output: ...
//! ```
//!
//! This guarantees that any selected model can be resolved safely at compile time.
//!
//! ## Plugin Family as a Logical Plugin State Machine
//!
//! ```text
//!                         +---------------------------------------+
//!                         |           FAMILY ROOT                  |
//!                         |   Unified logical plugin interface    |
//!                         +--------------------+------------------+
//!                                              |
//!                                      Concrete Family Type
//!                                              |
//!          +-----------------------------------+-----------------------------------+
//!          |                                   |                                   |
//!   +--------------+                    +--------------+                    +--------------+
//!   |   Child A    |                    |   Child B    |                    |   Child C    |
//!   | Operation A  |                    | Operation B  |                    | Operation C  |
//!   +------+-------+                    +------+-------+                    +------+-------+
//!          |                                   |                                   |
//!   +------+-------+                    +------+-------+                    +------+-------+
//!   |   Model A    |                    |   Model B    |                    |   Model C    |
//!   | selected by  |                    | selected by  |                    | selected by  |
//!   | Family Type  |                    | Family Type  |                    | Family Type  |
//!   +------+-------+                    +------+-------+                    +------+-------+
//!          |                                   |                                   |
//!          +---------------------------- Context ----------------------------------+
//!                                   (shared across models)
//! ```
//!
//! In this structure:
//!
//! - The **family root** represents the unified logical plugin interface.
//! - Each **child marker** represents one operation of that interface.
//! - The **family type** determines which concrete model implements each
//!   operation.
//!
//! All models belonging to the same family share the **same context type**.
//! The context is therefore represented as a single input flowing into the
//! resolved model during execution.
//!
//! ### Resolution Flow
//!
//! ```text
//! Caller invokes:
//!   FamilyRoot + FamilyType + ChildX
//!
//! Compiler resolves:
//!   (FamilyType, ChildX) -> ConcreteModel
//!
//! Execution:
//!   ConcreteModel.compute(input, context)
//! ```
//!
//! This allows callers to treat the family as a single logical plugin while
//! the compiler statically resolves the concrete model for each operation.
//!
//! ## Declaration Model
//!
//! A plugin family is constructed using three complementary macros:
//!
//! - [`declare_family`](crate::declare_family)
//! - [`plugin_model`](crate::plugin_model)
//! - [`define_family`](crate::define_family)
//!
//! Together they define the **operations**, **models**, and **family
//! implementation** that make up a plugin family.
//!
//! ### 1. Declaring the Family Interface
//!
//! The [`declare_family`](crate::declare_family) macro defines the **family
//! root trait** and a set of **child marker types** representing operations
//! of the logical plugin.
//!
//! ```text
//! Family Root
//!   |-- ChildA
//!   |-- ChildB
//!   |-- ChildC
//! ```
//!
//! The root trait represents the unified plugin interface, while each child
//! marker identifies one operation that the family exposes.
//!
//!
//! ### 2. Defining Plugin Models
//!
//! Concrete behaviour is implemented using [`plugin_model`](crate::plugin_model).
//!
//! Each plugin model implements a specific `(Input, Context, Output)`
//! computation and can later be attached to a family operation.
//!
//! ```text
//! ModelA<Input, Context, Output>
//! ModelB<Input, Context, Output>
//! ModelC<Input, Context, Output>
//! ```
//!
//! Models remain independent units of computation and can be reused across
//! different families.
//!
//!
//! ### 3. Defining the Family Implementation
//!
//! The [`define_family`](crate::define_family) macro creates a **concrete
//! family type** that binds each child operation to a specific model.
//!
//! ```text
//! FamilyType
//!   |-- ChildA -> ModelA
//!   |-- ChildB -> ModelB
//!   |-- ChildC -> ModelC
//! ```
//!
//! This family type represents a concrete implementation of the family root
//! and determines which models are used for each operation.
//!
//!
//! ### Resolution Model
//!
//! When a caller invokes an operation, it refers only to the **family root**
//! and a **child marker**.
//!
//! The compiler then resolves the concrete model using the configured
//! family type:
//!
//! ```text
//! (FamilyType, Child) -> ConcreteModel
//! ```
//!
//! The resolved model is then executed using the provided `(Input, Context)`
//! values.
//!
//! This design allows callers to interact with the family as a single logical
//! plugin while the runtime configuration determines the concrete behaviour
//! through the selected **family type**.
//!
//!
//! ## Immutable vs Mutable Operational Variants
//!
//! A family may host immutable (`PurePluginModel`) and/or mutable
//! (`MutablePluginModel`) variants for its operations. However, mutability forms
//! part of the execution contract:
//!
//! - Immutable execution resolves only to pure models.
//! - Mutable execution resolves only to mutable models.
//!
//! Even if both coexist in the same family hierarchy, they are not
//! interchangeable at the usage site because the caller's expected execution
//! semantics are part of the type-level interface.
//!
//! ## Example: Family-Based Model Resolution
//!
//! This example demonstrates how a plugin **family** defines a semantic
//! extension point on a caller trait and how concrete models attach
//! themselves to that family. The runtime then selects the active model
//! by supplying an appropriate context.
//!
//! In this design the **family is declared by the caller trait**, because
//! the trait owns the extension point. Concrete plugin models merely
//! register themselves under that family.
//!
//! ### Caller Trait - Declaring the Plugin Family
//!
//! The caller trait defines the **plugin contract** and declares the family
//! that models may attach to.
//!
//! ```ignore
//! declare_family! {
//!     root: pub MathFamilyRoot,
//!     child: [MaybePlusOne]
//! }
//!
//! pub trait MathTrait {
//!     type Input: AtLeast8BitUnsigned;
//!     type Output: AtLeast8BitUnsigned;
//!
//!     plugin_types! {
//!         input: Self::Input,
//!         output: Self::Output,
//!         root: MathFamilyRoot,
//!         family: MathFamily
//!         context: MathContext,
//!     }
//!
//!     plugin_output! {
//!         fn request,
//!         input: Self::Input,
//!         output: Self::Output,
//!         root: MathFamilyRoot,
//!         family: Self::MathFamily
//!         child: MaybePlusOne,
//!         context: Self::MathContext,
//!     }
//! }
//! ```
//!
//! Here:
//!
//! - `MathFamily` defines the semantic plugin domain.
//! - `MaybePlusOne` acts as a **child selector**, representing an optional
//!   increment strategy.
//!
//! The trait itself does **not specify which model is used**.
//!
//! ### Plugin Models - Registering Implementations
//!
//! Plugin models implement behavior for a specific `(Family, Child, Context)`
//! combination. Multiple models may attach to the same child selector.
//!
//! ```ignore
//! pub struct AddOneContext;
//!
//! plugin_model! {
//!     name: AddOne,
//!     input: Value,
//!     context: AddOneContext,
//!     bounds: [Value: AtLeast8BitUnsigned],
//!     compute: |v, _ctx| {
//!         v.clone().saturating_add(One::one())
//!     }
//! }
//!
//! define_family! {
//!     root: MathFamilyRoot,
//!     family: OneFamily,
//!     input: Value,
//!     context: AddOneContext
//!     bounds: [Value: AtLeast8BitUnsigned]
//!     child: [                      
//!         MaybePlusOne => AddOne,
//!     ],
//! }
//!```
//!
//! ```ignore
//! pub struct AddNothingContext;
//!
//! plugin_model! {
//!     name: AddNothing,
//!     input: mut Value,
//!     context: AddNothingContext,
//!     bounds: [Value: Clone],
//!     compute: |v, _ctx| {
//!         v.clone()
//!     }
//! }
//!
//! define_family! {
//!     root: MathFamilyRoot,
//!     family: NoneFamily,
//!     input: Value,
//!     context: AddNothingContext
//!     bounds: [Value: Clone]
//!     child: [                      
//!         MaybePlusOne => AddNothing,
//!     ],
//! }
//! ```
//!
//! Both models attach to the same family and child selector but differ
//! in unified family type, context and execution behavior.
//!
//! ### Pallet Wiring - Remaining Generic
//!
//! The pallet implements the caller trait without committing to a concrete
//! model. It simply forwards the family and context from its configuration.
//!
//! ```ignore
//! struct Pallet<T: Config>(PhantomData<T>);
//!
//! impl<T: Config> MathTrait for Pallet<T> {
//!     type Input = T::XInput;
//!     type Output = T::XOutput;
//!     type MathFamily = T::XMathFamily;
//!     type MathContext = T::XMathContext;
//! }
//! ```
//!
//! This keeps the pallet generic and reusable across runtimes.
//!
//! ### Runtime Injection - Selecting the Active Model
//!
//! The runtime chooses the concrete behavior by supplying a context that
//! matches one of the registered models.
//!
//! ```ignore
//! pub trait Config {
//!     type XInput: AtLeast8BitUnsigned;
//!     type XOutput: AtLeast8BitUnsigned;
//!
//!     plugin_types! {
//!         input: Self::XInput,
//!         output: Self::XOutput,
//!         root: MathFamilyRoot,
//!         family: XMathFamily,
//!         context: XMathContext,
//!     }
//! }
//!
//! plugin_context! {
//!     name: MyContext,
//!     context: AddOneContext,
//!     value: AddOneContext,
//! }
//!
//! pub struct Runtime;
//!
//! impl Config for Runtime {
//!     type XInput = u8;
//!     type XOutput = u8;
//!     type XMathFamily = OneFamily;
//!     type XMathContext = AddOneContext;
//!
//!     // Also can be plugged towards
//!     // type XMathFamily = NoneFamily;
//!     // type XMathContext = AddNothingContext;
//! }
//! ```
//!
//! ### Resolution Flow
//!
//! ```text
//! Runtime selects:
//!   Family  = OneFamily
//!   Child   = MaybePlusOne
//!   Context = AddOneContext
//!
//! Matching Model:
//!   AddOne<Input=u8, Context=AddOneContext, Output=u8>
//! ```
//!
//! If the runtime instead supplied `NoneFamily` & `AddNothingContext`, the alternative model
//! would be selected automatically.
//!
//! The caller trait never names a concrete model. Instead, the compiler resolves
//! the correct implementation purely from the type-level contract:
//!
//! ```text
//! (Family, Child, Context) -> Model
//! ```
//!
//! This enables fully static, type-safe plugin resolution without runtime
//! dispatch or registration tables.

// ===============================================================================
// ````````````````````````````````` CORE TRAITS `````````````````````````````````
// ===============================================================================

/// Core trait implemented by all **immutable plugin models**.
///
/// A plugin model is typically a zero-sized (stateless) struct that defines
/// a specific computation strategy. Each model represents a logically distinct
/// variant within a plugin and may optionally depend on external context
/// to compute its result.
///
/// This trait defines the **pure computation contract**: the input is owned by
/// caller immutably and must not be mutated. The model returns a new output value
/// derived from the input and context.
///
/// ## Generics
/// - `Input`: Type of owned-data consumed by the model.
/// - `Context`: External parameters or configuration required by the model.
/// - `Output`: Type of value produced by the model.
///
/// ## Determinism
/// Implementations are expected to be stateless and deterministic, producing
/// the same output for the same input and context.
pub trait PurePluginModel<Input, Context, Output>: Default {
    /// Computes the model's output for a given immutable input and context.
    fn compute(&self, input: Input, context: &Context) -> Output;
}

/// Trait implemented by **mutable plugin models** that may transform their
/// input in-place while still producing an output.
///
/// Unlike [`PurePluginModel`], this trait explicitly allows mutation of the input,
/// making it suitable for in-place normalization, sorting, accumulation,
/// or other performance-sensitive transformations that avoid extra allocations.
///
/// Mutation is **explicit and opt-in**, preserving clarity between pure and
/// state-transforming computations.
///
/// ## Generics
/// - `Mutate`: Type of data that will be mutated in-place.
/// - `Context`: External parameters or configuration required by the model.
/// - `Output`: Type of value produced by the model.
///
/// ## Semantics
/// - The input may be modified during computation.
/// - The returned output may be derived from either the original or mutated state.
/// - Implementations should still remain stateless with respect to internal storage.
pub trait MutablePluginModel<Mutate, Context, Output>: Default {
    /// Computes the model's output while mutating the input in-place.
    fn compute_mut(&self, input: &mut Mutate, context: &Context) -> Output;
}

/// Represents a source of context for models.
///
/// Models can retrieve context from an implementor of this trait.
pub trait ModelContext {
    /// Associated type representing the actual context.
    type Context;

    /// Returns the context for a model.
    fn context() -> Self::Context;
}

/// Placeholder type for models that **do not require any external context**.
impl ModelContext for () {
    type Context = ();

    fn context() -> () {
        ()
    }
}

// ===============================================================================
// ```````````````````````````````` PLUGIN TYPES `````````````````````````````````
// ===============================================================================

/// Declares **associated plugin types** inside a trait.
///
/// Supports both:
/// - **concrete plugin model binding**, or
/// - **plugin family binding** for late model selection.
///
/// Exactly one of `model` or `family` must be specified.
/// Exactly one of `input` or `input: mut` must be specified.
///
/// ## Syntax
///
/// ### Immutable Concrete Model
///
/// ```ignore
/// plugin_types! {
///     input: InputType,        // Required: immutable input type
///     output: OutputType,      // Required: output type
///     model: ModelAssoc,       // Required: associated plugin model
///     context: ContextAssoc,   // Required: associated context provider
/// }
/// ```
///
/// ### Mutable Concrete Model
///
/// ```ignore
/// plugin_types! {
///     input: mut MutateType,   // Required: mutable input type
///     output: OutputType,      // Required: output type
///     model: ModelAssoc,       // Required: associated plugin model
///     context: ContextAssoc,   // Required: associated context provider
/// }
/// ```
///
/// ### Plugin Family (Immutable or Mutable)
///
/// ```ignore
/// plugin_types! {
///     input: InputType,        // or: input: mut MutateType
///     output: OutputType,      // Required: output type
///     borrow: ['a],            // Optional: lifetime parameters of input/output
///     root: PluginFamilyRoot,  // Required: plugin family root trait
///     family: FamilyAssoc,     // Required: associated plugin family type
///     context: ContextAssoc,   // Required: associated context provider
///     provides: [Send + Sync], // Optional: bounds on context
/// }
/// ```
///
/// ## Lifetimes
///
/// - `lifetimes` expands to `<...>` on the **family associated type**
/// - Enables lifetime-parameterized associated types (GATs)
///
/// ## Context Bounds
///
/// You may restrict the family's **context type** using `provides`.
///
/// ```ignore
/// plugin_types! {
///     input: Input,
///     output: Output,
///     root: PluginFamilyRoot,
///     family: FamilyAssoc,    
///     context: MyContext,
///     provides: [Send + Sync + 'static],
/// }
/// ```
///
/// Expands roughly to:
///
/// ```ignore
/// type MyContext: ModelContext<Context: Send + Sync + 'static>;
/// ```
///
/// ## Input / Output Constraints
///
/// ### Plugin Models (Immutable & Mutable)
/// `Input` and `Output` must not contain generics or lifetime-based types (no GATs)
///
/// ### Plugin Families
/// - `Input` and `Output` may include **lifetimes**
/// - Enabled via parameter `borrow: ['a]`
/// - Type generics are still not supported
///
/// ```text
/// Model   -> concrete associated types only (no generics, no lifetimes, no GATS)
/// Family  -> supports lifetimes only (via borrow)
/// ```
///
/// ## Examples
///
/// ### Concrete Model
///
/// ```ignore
/// pub trait Increment {
///     type Input;
///     type Output;
///
///     plugin_types! {
///         input: Self::Input,
///         output: Self::Output,
///         model: AddOneModel,
///         context: AddOneCtx,
///     }
/// }
/// ```
///
/// ### Plugin Family
///
/// ```ignore
/// pub trait MathOps {
///     type Input;
///     type Output;
///
///     plugin_types! {
///         input: Self::Input,
///         output: Self::Output,
///         root: MathFamilyRoot,
///         family: MathFamily,
///         context: MathContext,
///     }
/// }
/// ```
#[macro_export]
macro_rules! plugin_types {

    // Immutable model arm
    (
        input: $InputTy:ty,
        output: $OutputTy:ty,
        $(#[$model_meta:meta])*
        model: $ModelAssoc:ident,
        $(#[$ctx_meta:meta])*
        context: $ContextAssoc:ident $(,)?
    ) => {
        $(#[$model_meta])*
        type $ModelAssoc: $crate::plugins::PurePluginModel<
                $InputTy,
                <Self::$ContextAssoc as $crate::plugins::ModelContext>::Context,
                $OutputTy,
            > + Default;

        $(#[$ctx_meta])*
        type $ContextAssoc:
            $crate::plugins::ModelContext;
    };

    // Mutable model arm
    (
        input: mut $MutateTy:ty,
        output: $OutputTy:ty,
        $(#[$model_meta:meta])*
        model: $ModelAssoc:ident,
        $(#[$ctx_meta:meta])*
        context: $ContextAssoc:ident $(,)?
    ) => {
        $(#[$model_meta])*
        type $ModelAssoc: $crate::plugins::MutablePluginModel<
                $MutateTy,
                <Self::$ContextAssoc as $crate::plugins::ModelContext>::Context,
                $OutputTy,
            > + Default;

        $(#[$ctx_meta])*
        type $ContextAssoc:
            $crate::plugins::ModelContext;
    };

    // Family model arm
    (
        input: $(mut)? $InputTy:ty,
        output: $OutputTy:ty,
        $(borrow: [$($borrow_lt:lifetime)* $(,)?],)?
        root: $Root:ident,
        $(#[$model_meta:meta])*
        family: $FamilyAssoc:ident,
        $(#[$ctx_meta:meta])*
        context: $ContextAssoc:ident
        $(, provides: [$($provider:tt)*])? $(,)?
    ) => {
        $(#[$model_meta])*
        type $FamilyAssoc $(<$($borrow_lt)*>)? : $Root<
                $InputTy,
                <Self::$ContextAssoc as $crate::plugins::ModelContext>::Context,
                $OutputTy,
            >;

        $(#[$ctx_meta])*
        type $ContextAssoc:
            $crate::plugins::ModelContext$(<Context: $($provider)*>)?;
    };

}

// ===============================================================================
// ``````````````````````````````` PLUGIN CONTEXT ````````````````````````````````
// ===============================================================================

/// Generates a stateless plugin context marker type for a plugin model or a
/// plugin family.
///
/// This macro defines:
/// 1. A zero-sized **marker struct** representing the plugin context provider.
/// 2. An implementation of the [`ModelContext`] trait for that marker,
///    including a constructor function `context()` returning a value on demand.
///
/// The `value` construction is on-demand (via [`ModelContext::context`]) which
/// allows the context to depend on other constants, statics, or computed values
/// while remaining compile-time friendly.
///
/// When `marker` is specified, the generated struct stores them in `PhantomData`
/// fields so the type system correctly tracks them without affecting runtime behavior.
///
/// - Type generics are tracked using `PhantomData<(T, ...)>`, preserving the
///   usual marker semantics for type parameters.
///
/// ## Syntax
///
/// ```ignore
/// plugin_context! {
///     #[attributes...]                // Optional: struct-level attributes (docs, derives, etc.)
///     name: pub ContextName,          // Required: visibility and name of the context marker struct
///     context: ContextType,           // Required: type representing the context data
///
///     marker: [T, U],                // Optional: phantom-data parameters applied to the marker
///     bounds: [T: Default, U: Clone], // Optional: trait bounds for the generated impl
///
///     value: ContextExpression,       // Required: expression producing the context value
/// }
/// ```
///
/// ## Attributes
/// Optional doc comments or other attributes can be attached to the generated
/// marker by placing them above the macro invocation.
///
/// ## Generics Support
///
/// - Only **type generics** (`T`, `U`, etc.) are supported via `marker: [...]`
/// - Lifetime generics are **not supported**
/// - The generics are tracked using `PhantomData` and do not affect runtime behavior
///
/// ## Examples
///
/// ### Basic Context
///
/// ```ignore
/// plugin_context! {
///     name: pub ElectionContext,
///     context: PhragmenConfig,
///     value: PhragmenConfig { sequential: true }
/// }
/// ```
///
/// ### Generic Context
///
/// ```ignore
/// plugin_context! {
///     name: pub GenericContext,
///     marker: [T],
///     context: PhragmenConfig<T>,
///     value: PhragmenConfig { sequential: true }
/// }
/// ```
///
#[macro_export]
macro_rules! plugin_context {
    (
        $(#[$meta:meta])*
        name: $vis:vis $Name:ident,
        context: $ContextType:ty,
        $(marker: [$($marker_gen:ident),* $(,)?],)?
        $(bounds: [$($bounds:tt)*],)?
        value: $ContextLiteral:expr $(,)?
    ) => {
        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            []
            [$($($marker_gen),*)?]
        );

        impl $(< $($marker_gen,)* >)?
        $crate::plugins::ModelContext
        for $Name $(< $($marker_gen,)* >)?
        $(where $($bounds)*)?
        {
            type Context = $ContextType;

            fn context() -> Self::Context {
                $ContextLiteral
            }
        }
    };
}

// ===============================================================================
// ```````````````````````````````` PLUGIN OUTPUT ````````````````````````````````
// ===============================================================================

/// Generates a strongly-typed associated function that executes a plugin model
/// and returns its computed output.
///
/// The macro expands to a function which:
/// - Instantiates the plugin model using `Default`
/// - Constructs the execution context via [`ModelContext::context`]
/// - Wraps the input, model, and context into the appropriate execution source
/// - Executes the model and returns the resulting output
///
/// This removes boilerplate wiring of model construction, context resolution,
/// and execution, while preserving full compile-time type safety.
///
/// Exactly one of the following must be specified:
/// - `model` -> directly executes a concrete plugin model
/// - `root` + `family` + `child` -> resolves a model from a plugin family
///
/// Exactly one of:
/// - `input:`     -> immutable execution contract ([`PurePluginModel`])
/// - `input: mut` -> mutable execution contract ([`MutablePluginModel`])
///
/// Optional:
/// - `borrow` declares function-level generic parameters for input and output
/// specialization in family models.
///
/// ## Syntax
///
/// ### Immutable Concrete Model
///
/// ```ignore
/// plugin_output! {
///     pub fn run_model,        // Required: function visibility and name to generate
///     input: MyInput,          // Required: immutable input type
///     output: MyOutput,        // Required: output type produced by the model
///     model: MyModel,          // Required: concrete immutable plugin model type
///     context: MyContext,      // Required: context provider implementing ModelContext
/// }
/// ```
///
/// Expands to:
/// `pub fn run_model(input: MyInput) -> MyOutput { ... }`
///
/// ### Mutable Concrete Model
///
/// ```ignore
/// plugin_output! {
///     pub fn run_model_mut,    // Required: function visibility and name to generate
///     input: mut MyInput,      // Required: mutable input type
///     output: MyOutput,        // Required: output type produced by the model
///     model: MyModel,          // Required: concrete mutable plugin model type
///     context: MyContext,      // Required: context provider implementing ModelContext
/// }
/// ```
///
/// ### Immutable Family-Selected Model
///
/// ```ignore
/// plugin_output! {
///     pub fn run_family,       // Required: function visibility and name to generate
///     input: MyInput<'a>,      // Required: immutable input type
///     output: MyOutput,        // Required: output type produced by the model
///     borrow: ['a],            // Optional: function-level liftimes over input/output
///     root: MyFamilyRoot,      // Required: plugin family root trait
///     family: MyFamily,        // Required: concrete plugin family type
///     child: MyChildMarker,    // Required: child model identifier within the family
///     context: MyContext,      // Required: context provider implementing ModelContext
/// }
/// ```
///
/// ### Mutable Family-Selected Model
///
/// ```ignore
/// plugin_output! {
///     pub fn run_family_mut,   // Required: function visibility and name to generate
///     input: mut MyInput,      // Required: mutable input type
///     output: MyOutput<'a>,    // Required: output type produced by the model
///     borrow: ['a],            // Optional: function-level liftimes over input/output
///     root: MyFamilyRoot,      // Required: plugin family root trait
///     family: MyFamily,        // Required: concrete plugin family type
///     child: MyChildMarker,    // Required: child model identifier within the family
///     context: MyContext,      // Required: context provider implementing ModelContext
/// }
/// ```
///
/// ## Semantics
///
/// - `model` form executes a fixed concrete plugin model.
/// - `root` + `family` + `child` form defers model selection to the plugin family,
///   where the concrete model is resolved at compile time using the
///   `(Input, Context, Output, Family)` signature.
/// - `Context` acts as the nominal discriminator within a family, while
///   `Input` and `Output` are validated once concretely resolved at the call site.
/// - Mutable variants may mutate the input in-place, but resolution remains
///   entirely static through trait bounds.
/// - All resolution is performed at compile time; no dynamic dispatch is used.
///
/// ## Input / Output Constraints
///
/// - Plugin models (immutable & mutable) use **non-GAT types only**
///   - No generics
///   - No lifetime-based types (no GATs)
///
/// - Plugin families may use **lifetimes only**
///   - Enabled via `borrow: ['a]`
///   - Type generics are not supported
///
/// ```text
/// Model   -> concrete types only
/// Family  -> supports lifetimes only
/// ```
#[macro_export]
macro_rules! plugin_output {
    // Immutable model function
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident,
        input: $Input:ty,
        output: $Output:ty,
        model: $ModelType:ty,
        context: $ContextType:ty $(,)?
    ) => {
        $(#[$meta])*
        $vis fn $name (input: $Input) -> $Output
        {
            // Instantiate the model
            let model = <$ModelType>::default();

            // Construct the context
            let context: <$ContextType as $crate::plugins::ModelContext>::Context =
                <$ContextType as $crate::plugins::ModelContext>::context();

            // Compute and return output
            $crate::plugins::PurePluginModel::<_, _, _>::compute(&model, input, &context)
        }
    };

    // Mutable model function
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident,
        input: mut $Input:ty,
        output: $Output:ty,
        model: $ModelType:ty,
        context: $ContextType:ty $(,)?
    ) => {
        $(#[$meta])*
        $vis fn $name(input: &mut $Input) -> $Output
        {
            // Instantiate the model
            let model = <$ModelType>::default();

            // Construct the context
            let context: <$ContextType as $crate::plugins::ModelContext>::Context =
                <$ContextType as $crate::plugins::ModelContext>::context();

            // Compute and return output
            $crate::plugins::MutablePluginModel::<_, _, _>::compute_mut(&model, input, &context)
        }
    };

    // Immutable Family Child-specific function
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident,
        input: $Input:ty,
        output: $Output:ty,
        $(borrow: [$($borrow_lt:lifetime)* $(,)?],)?
        root: $Root:ident,
        family: $Family:ty,
        child: $Child:ident,
        context: $ContextType:ty $(,)?
    ) => {
        #[doc = concat!(
            "Plugin invocation pure-function for the child - [`",
            stringify!($Child),
            "`] of the plugin family - [`",
            stringify!($Root),
            "`]"
        )]
        $(#[$meta])*
        $vis fn $name $(<$($borrow_lt)*>)? (input: $Input) -> $Output
        {
            // Resolve the concrete plugin model from the family using the root trait
            let model =
                <$Family as $Root<$Input,<$ContextType as $crate::plugins::ModelContext>::Context,
                    $Output>>::$Child::default();

            // Construct the execution context via the context provider.
            let context =
                <$ContextType as $crate::plugins::ModelContext>::context();

            // Execute the immutable plugin model and return the computed output.
            <<$Family as $Root<$Input,<$ContextType as $crate::plugins::ModelContext>::Context,
                    $Output>>::$Child
                    as $crate::plugins::PurePluginModel<
                        $Input,
                        <$ContextType as $crate::plugins::ModelContext>::Context,
                        $Output,
                    >>::compute(&model, input, &context)
        }
    };

    // Mutable Family Child-specific function
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident,
        input: mut $Input:ty,
        output: $Output:ty,
        $(borrow: [$($borrow_lt:lifetime)* $(,)?],)?
        root: $Root:ident,
        family: $Family:ty,
        child: $Child:ident,
        context: $ContextType:ty $(,)?
    ) => {
        #[doc = concat!(
            "Plugin invocation mutable-function for the child - [`",
            stringify!($Child),
            "`] of the plugin family - [`",
            stringify!($Root),
            "`]"
        )]
        $(#[$meta])*
        $vis fn $name $(<$($borrow_lt)*>)? (input: &mut $Input) -> $Output
        {
            // Resolve the concrete plugin model from the family using the root trait
            let model =
                <$Family as $Root<$Input,<$ContextType as $crate::plugins::ModelContext>::Context,
                    $Output>>::$Child::default();

            // Construct the execution context via the context provider.
            let context =
                <$ContextType as $crate::plugins::ModelContext>::context();

            // Execute the immutable plugin model and return the computed output.
            <<$Family as $Root<$Input,<$ContextType as $crate::plugins::ModelContext>::Context,
                    $Output>>::$Child
                    as $crate::plugins::MutablePluginModel<
                        $Input,
                        <$ContextType as $crate::plugins::ModelContext>::Context,
                        $Output,
                    >>::compute_mut(&model, input, &context)
        }
    };
}

// ===============================================================================
// ```````````````````````````````` PLUGIN TESTS `````````````````````````````````
// ===============================================================================

/// Generates **table-driven unit tests** for plugin models.
///
/// It supports both **immutable** and **mutable** models, with or without context,
/// and with either explicit or inferred output types.
///
/// For each test case, the macro:
/// - Instantiates the plugin model using `Default`
/// - Constructs the required context (if any)
/// - Executes the model's computation (`compute` for immutable models,
///   `compute_mut` for mutable models)
/// - Asserts that the computed output matches the expected value
/// - Optionally asserts the final mutated input state for mutable models
///
/// Each test case expands into an **independent `#[test]` function**, ensuring
/// clear isolation and accurate failure reporting.
///
/// ## Features
///
/// - Supports **immutable** (`PurePluginModel`) and **mutable** (`MutablePluginModel`) models
/// - Supports **context-aware** and **context-free** plugin models
/// - Supports **explicit output types** or **implicit output = input**
/// - Optional assertion of the **mutated input value** for mutable models
/// - Generates one `#[test]` function per case
/// - Avoids boilerplate while preserving full type safety
/// - Mirrors the exact runtime execution contract of plugin models
///
/// ## Supported Forms
///
/// The macro supports the same four combinations for both immutable and mutable models:
///
/// | Context | Output |
/// +-------+------+
/// | Yes    | Explicit |
/// | Yes    | Inferred (output = input) |
/// | No     | Explicit |
/// | No     | Inferred (output = input) |
///
/// Mutable models are declared by using `input: mut Type`, which indicates that the
/// model will receive `&mut Type` and may transform the input in-place.
///
/// ## Syntax
///
/// ```ignore
/// plugin_test! {
///     model: ModelType,              // Plugin model type to test
///     input: InputType | mut InputType, // `mut` enables mutable model testing
///     output: OutputType,            // Optional: defaults to `InputType` if omitted
///     context: ContextType,          // Optional: required if model uses context
///     value: context_expr,           // Optional: expression constructing the context
///     cases: {
///         (test_name, input_expr, expected_output),
///         (test_name_2, input_expr_2, expected_output_2, expected_mutated_input), // mutable only
///     }
/// }
/// ```
///
/// - `model`: Plugin model type implementing `PurePluginModel` or `MutablePluginModel`
/// - `input`: Input type consumed by the model (`mut` indicates in-place mutation)
/// - `output`: Output type produced by the model (defaults to input type if omitted)
/// - `context`: Context type required by the model (omit for `()`)
/// - `value`: Expression that constructs the context instance
/// - `cases`: List of test tuples
///
/// Each case tuple has the form:
/// - `(name, input, expected_output)` for immutable models
/// - `(name, input, expected_output)` for mutable models when only output is asserted
/// - `(name, input, expected_output, expected_mutated_input)` to also verify
///   the final mutated state of the input
///
/// When the fourth element is provided, the macro additionally checks that the
/// input was correctly transformed in-place.
///
/// ## Notes
///
/// - Each test case expands into a **separate `#[test]` function**
/// - Context and input types must match the model's trait implementation
/// - Output inference (`output = input`) follows the same rule as `plugin_model!`
/// - Compilation fails if input, context, or output types are incompatible
///
/// This macro is intended for **testing plugin model logic in isolation** and
/// should not be used for testing pallet storage, dispatchables, or runtime configuration.
#[macro_export]
macro_rules! plugin_test {
    // Helper: choose output type, defaulting to input if not provided
    (@output_ty $InputType:ty) => { $InputType };
    (@output_ty $InputType:ty, $OutputType:ty) => { $OutputType };

    // WITH CONTEXT, EXPLICIT OUTPUT
    (
        model: $ModelName:ty,
        input: $InputTy:ty,
        output: $OutputTy:ty,
        context: $ContextTy:ty,
        value: $ContextExpr:expr,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr)),* $(,)? }
    ) => {
        $(
            #[test] // generate a #[test] function for each case
            fn $test_name() {
                let model = <$ModelName>::default();           // instantiate model
                let context: $ContextTy = $ContextExpr;        // construct context
                let input: $InputTy = $input_expr;             // test input
                let result: $OutputTy =                        // compute output
                    <$ModelName as $crate::plugins::PurePluginModel<
                        $InputTy,
                        $ContextTy,
                        $OutputTy
                    >>::compute(&model, input, &context);
                assert_eq!(result, $expected);                // verify result
            }
        )*
    };

    // WITH CONTEXT, OUTPUT = INPUT
    (
        model: $ModelName:ty,
        input: $InputTy:ty,
        context: $ContextTy:ty,
        value: $ContextExpr:expr,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                type Output = $InputTy;
                let model = <$ModelName>::default();
                let context: $ContextTy = $ContextExpr;
                let input: $InputTy = $input_expr;
                let result: Output =
                    <$ModelName as $crate::plugins::PurePluginModel<
                        $InputTy,
                        $ContextTy,
                        Output
                    >>::compute(&model, input, &context);
                assert_eq!(result, $expected);
            }
        )*
    };

    // No Context, EXPLICIT OUTPUT
    (
        model: $ModelName:ty,
        input: $InputTy:ty,
        output: $OutputTy:ty,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                let model = <$ModelName>::default();
                let context: () = Default::default();
                let input: $InputTy = $input_expr;
                let result: $OutputTy =
                    <$ModelName as $crate::plugins::PurePluginModel<
                        $InputTy,
                        (),
                        $OutputTy
                    >>::compute(&model, input, &context);
                assert_eq!(result, $expected);
            }
        )*
    };

    // No Context, OUTPUT = INPUT
    (
        model: $ModelName:ty,
        input: $InputTy:ty,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                type Output = $InputTy;
                let model = <$ModelName>::default();
                let context: () = Default::default();
                let input: $InputTy = $input_expr;
                let result: Output =
                    <$ModelName as $crate::plugins::PurePluginModel<
                        $InputTy,
                        (),
                        Output
                    >>::compute(&model, input, &context);
                assert_eq!(result, $expected);
            }
        )*
    };

    // WITH CONTEXT, EXPLICIT OUTPUT (MUTABLE)
    (
        model: $ModelName:ty,
        input: mut $InputTy:ty,
        output: $OutputTy:ty,
        context: $ContextTy:ty,
        value: $ContextExpr:expr,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr $(, $expected_input:expr)?)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                let model = <$ModelName>::default();
                let context: $ContextTy = $ContextExpr;
                let mut input: $InputTy = $input_expr;

                let result: $OutputTy =
                    <$ModelName as $crate::plugins::MutablePluginModel<
                        $InputTy,
                        $ContextTy,
                        $OutputTy
                    >>::compute_mut(&model, &mut input, &context);

                assert_eq!(result, $expected);

                $(
                    assert_eq!(input, $expected_input);
                )?
            }
        )*
    };

    // WITH CONTEXT, OUTPUT = INPUT (MUTABLE)
    (
        model: $ModelName:ty,
        input: mut $InputTy:ty,
        context: $ContextTy:ty,
        value: $ContextExpr:expr,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr $(, $expected_input:expr)?)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                type Output = $InputTy;
                let model = <$ModelName>::default();
                let context: $ContextTy = $ContextExpr;
                let mut input: $InputTy = $input_expr;

                let result: Output =
                    <$ModelName as $crate::plugins::MutablePluginModel<
                        $InputTy,
                        $ContextTy,
                        Output
                    >>::compute_mut(&model, &mut input, &context);

                assert_eq!(result, $expected);

                $(
                    assert_eq!(input, $expected_input);
                )?
            }
        )*
    };

    // No Context, EXPLICIT OUTPUT (MUTABLE)
    (
        model: $ModelName:ty,
        input: mut $InputTy:ty,
        output: $OutputTy:ty,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr $(, $expected_input:expr)?)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                let model = <$ModelName>::default();
                let context: () = Default::default();
                let mut input: $InputTy = $input_expr;

                let result: $OutputTy =
                    <$ModelName as $crate::plugins::MutablePluginModel<
                        $InputTy,
                        (),
                        $OutputTy
                    >>::compute_mut(&model, &mut input, &context);

                assert_eq!(result, $expected);

                $(
                    assert_eq!(input, $expected_input);
                )?
            }
        )*
    };

    // No Context, OUTPUT = INPUT (MUTABLE)
    (
        model: $ModelName:ty,
        input: mut $InputTy:ty,
        cases: { $(($test_name:ident, $input_expr:expr, $expected:expr $(, $expected_input:expr)?)),* $(,)? }
    ) => {
        $(
            #[test]
            fn $test_name() {
                type Output = $InputTy;
                let model = <$ModelName>::default();
                let context: () = Default::default();
                let mut input: $InputTy = $input_expr;

                let result: Output =
                    <$ModelName as $crate::plugins::MutablePluginModel<
                        $InputTy,
                        (),
                        Output
                    >>::compute_mut(&model, &mut input, &context);

                assert_eq!(result, $expected);

                $(
                    assert_eq!(input, $expected_input);
                )?
            }
        )*
    };
}

// ===============================================================================
// ```````````````````````````````` PLUGIN MODEL `````````````````````````````````
// ===============================================================================

/// Defines a plugin model in a fully generic, and type-safe way.
///
/// The macro generates:
/// - A `struct` representing the plugin model (deriving `Default`)
/// - An implementation of either [`PurePluginModel`] or [`MutablePluginModel`]
///
/// This removes repetitive boilerplate while ensuring that the relationships
/// between input, output, and context types are enforced at compile time.
///
/// Exactly one of:
/// - `input:`     -> immutable model (`PurePluginModel`)
/// - `input: mut` -> mutable model (`MutablePluginModel`)
///
/// Optionally:
/// - `context:` enables contextual execution (otherwise context defaults to `()`)
/// - `root:` + `child:` attaches the model to a plugin family for late resolution
///
/// ## Syntax
///
/// ### Immutable Model (No Context)
///
/// ```ignore
/// plugin_model! {
///     name: pub ModelName,          // Required: struct visibility and struct name of the plugin model
///     input: InputType,             // Required: generic immutable input type
///     output: OutputType,           // Optional: output type (defaults to input if omitted)
///     others: [T1, T2],             // Optional: additional generic parameters
///     bounds: [TraitBounds],        // Required: trait bounds for generics
///     compute: |input, ctx| { ... } // Required: compute logic (`ctx` is `()`)
/// }
/// ```
///
/// ### Immutable Model with Context
///
/// ```ignore
/// plugin_model! {
///     name: pub ModelName,          // Required: struct visibility and struct name of the plugin model
///     input: InputType,             // Required: generic immutable input type
///     output: OutputType,           // Optional: output type (defaults to input if omitted)
///     others: [T1, T2],             // Optional: additional generic parameters
///     context: ContextType,         // Required: context struct used during execution
///     bounds: [TraitBounds],        // Required: trait bounds for generics
///     compute: |input, ctx| { ... } // `ctx: &ContextType`
/// }
/// ```
///
/// ### Mutable Model
///
/// ```ignore
/// plugin_model! {
///     name: pub ModelName,          // Required: struct visibility and struct name of the plugin model
///     input: mut InputType,         // Required: mutable input type (`&mut InputType`)
///     output: OutputType,           // Optional: output type (defaults to immutable input type)
///     others: [T1, T2],             // Optional: additional generic parameters
///     context: ContextType,         // Optional: context struct (defaults to `()`)
///     bounds: [TraitBounds],        // Required: trait bounds for generics
///     compute: |input, ctx| { ... } // Uses `compute_mut`
/// }
/// ```
///
/// ## Output Type Semantics
///
/// - If `output` is omitted, the output type defaults to the **immutable input type**,
///   even for mutable models (it does **not** default to `()`).
/// - This rule applies to both immutable and mutable plugin models.
/// - If a unit output `()` is desired, it must be specified explicitly as:
///
/// ```ignore
/// output: Output,
/// bounds: [Output: Default]
/// ```
///
/// The `Default` bound is required so `compute`'s block can construct the output value.
///
/// ## Semantics
///
/// - Each model is fully generic over its input, output, and optional context.
/// - If `context` is omitted, the model uses `()` as its context type.
/// - If `root` and `child` are provided, the model becomes a member of a plugin
///   family and is selected indirectly using the `(root, child, context)`
///   resolution lattice.
/// - Immutable variants use `compute` with shared input references.
/// - Mutable variants use `compute_mut` and may mutate the input in-place.
///
/// All constraints are enforced purely through trait bounds and associated
/// types, guaranteeing compile-time correctness of model wiring and resolution.
#[macro_export]
macro_rules! plugin_model {

    // Helper Rule: `@output_ty`
    (@output_ty $Input:tt) => { $Input };
    (@output_ty $Input:tt, $Output:tt) => { $Output };

    // Variant 1: No Context, Single Input, Single Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: $Input:ident,
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $Output)?
        > $crate::plugins::PurePluginModel<
            $Input,
            (),
            $crate::plugin_model!(@output_ty $Input $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: $Input,
                $ctx_arg: &()
            ) -> $crate::plugin_model!(@output_ty $Input $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 2: No Context, Single Input, Tuple Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: $Input:ident,
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $($Output),+)?
        > $crate::plugins::PurePluginModel<
            $Input,
            (),
            $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: $Input,
                $ctx_arg: &()
            ) -> $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 3: No Context, Tuple Input, Single Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: ($($Input:ident),+),
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $Output)?
        > $crate::plugins::PurePluginModel<
            ($($Input),+),
            (),
            $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: ($($Input),+),
                $ctx_arg: &()
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 4: No Context, Tuple Input, Tuple Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: ($($Input:ident),+),
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $($Output),+)?
        > $crate::plugins::PurePluginModel<
            ($($Input),+),
            (),
            $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: ($($Input),+),
                $ctx_arg: &()
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 5: Context, Single Input, Single Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: $Input:ident,
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $Output)?
        > $crate::plugins::PurePluginModel<
            $Input,
            $Context,
            $crate::plugin_model!(@output_ty $Input $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: $Input,
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty $Input $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 6: Context, Single Input, Tuple Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: $Input:ident,
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $($Output),+)?
        > $crate::plugins::PurePluginModel<
            $Input,
            $Context,
            $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: $Input,
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 7: Context, Tuple Input, Single Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: ($($Input:ident),+),
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $Output)?
        > $crate::plugins::PurePluginModel<
            ($($Input),+),
            $Context,
            $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: ($($Input),+),
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 8: Context, Tuple Input, Tuple Output
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: ($($Input:ident),+),
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $($Output),+)?
        > $crate::plugins::PurePluginModel<
            ($($Input),+),
            $Context,
            $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute(
                &self,
                $input_arg: ($($Input),+),
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 9: No Context, Single Input, Single Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut $Input:ident,
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $Output)?
        > $crate::plugins::MutablePluginModel<
            $Input,
            (),
            $crate::plugin_model!(@output_ty $Input $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut $Input,
                $ctx_arg: &()
            ) -> $crate::plugin_model!(@output_ty $Input $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 10: No Context, Single Input, Tuple Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut $Input:ident,
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $($Output),+)?
        > $crate::plugins::MutablePluginModel<
            $Input,
            (),
            $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut $Input,
                $ctx_arg: &()
            ) -> $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 11: No Context, Tuple Input, Single Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut ($($Input:ident),+),
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $Output)?
        > $crate::plugins::MutablePluginModel<
            ($($Input),+),
            (),
            $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut ($($Input),+),
                $ctx_arg: &(),
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 12: No Context, Tuple Input, Tuple Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut ($($Input:ident),+),
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $($Output),+)?
        > $crate::plugins::MutablePluginModel<
            ($($Input),+),
            (),
            $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut ($($Input),+),
                $ctx_arg: &(),
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 13: Context, Single Input, Single Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut $Input:ident,
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $Output)?
        > $crate::plugins::MutablePluginModel<
            $Input,
            $Context,
            $crate::plugin_model!(@output_ty $Input $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut $Input,
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty $Input $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 14: Context, Single Input, Tuple Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut $Input:ident,
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $Input
            $(, $($Output),+)?
        > $crate::plugins::MutablePluginModel<
            $Input,
            $Context,
            $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut $Input,
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty $Input $(, ($($Output),+))?) {
                $body
            }
        }
    };

    // Variant 15: Context, Tuple Input, Single Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut ($($Input:ident),+),
        $(output: $Output:ident ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $Output)?
        > $crate::plugins::MutablePluginModel<
            ($($Input),+),
            $Context,
            $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut ($($Input),+),
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, $Output)?) {
                $body
            }
        }
    };

    // Variant 16: Context, Tuple Input, Tuple Output (MUTABLE)
    (
        $(#[$name_meta:meta])*
        name: $vis:vis $ModelName:ident,
        input: mut ($($Input:ident),+),
        $(output: ($($Output:ident),+) ,)?
        $(others: [$($other_gen:tt),* $(,)? ] ,)?
        context: $Context:ty,
        bounds: [$($bounds:tt)*],
        $(#[$compute_meta:meta])*
        compute: |$input_arg:ident, $ctx_arg:ident| $body:block $(,)?
    ) => {
        #[derive(Debug, Default)]
        $(#[$name_meta])*
        $vis struct $ModelName;

        impl<
            $($($other_gen ,)*)?
            $($Input),+
            $(, $($Output),+)?
        > $crate::plugins::MutablePluginModel<
            ($($Input),+),
            $Context,
            $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?)
        > for $ModelName
        where
            $($bounds)*
        {
            $(#[$compute_meta])*
            fn compute_mut(
                &self,
                $input_arg: &mut ($($Input),+),
                $ctx_arg: &$Context
            ) -> $crate::plugin_model!(@output_ty ($($Input),+) $(, ($($Output),+))?) {
                $body
            }
        }

    };

}

// ===============================================================================
// ``````````````````````````````` DECLARE FAMILY ````````````````````````````````
// ===============================================================================

/// Declares a plugin model family using marker types.
///
/// This macro generates a **family root trait** and one or more **child markers**
/// used to represent operations within that family.
///
/// The generated markers are purely type-level and contain no runtime data.
/// Concrete plugin implementations attach to a `(Root, Child)` pair via
/// [`plugin_model!`](crate::plugin_model), allowing models to be selected later through context
/// and trait bounds.
///
/// ## Parameters
/// - `root`: Declares the **family root trait**.
///   - Can include a visibility modifier (`pub`, `pub(crate)`, etc.).
///   - The same visibility is applied to all child marker structs.
///   - Prefix with `mut` to create a mutable plugin family.
/// - `child`: A list of **child marker types** representing operations
///   within the family.
///
/// ## Syntax
///
/// ```ignore
/// declare_family! {
///     root: pub FamilyRoot,
///     child: [OperationA, OperationB, OperationC]
/// }
/// ```
///
/// Mutable families use the `mut` keyword:
///
/// ```ignore
/// declare_family! {
///     root: mut pub FamilyRoot,
///     child: [OperationA, OperationB]
/// }
/// ```
///
/// ## Generated Types
///
/// The macro generates:
///
/// - A **child marker struct** for each entry in `child`.
/// - A **family root trait** defining associated plugin model types
///   for each child operation.
///
/// The child marker structs are zero-sized types used purely as
/// identifiers for operations within the family.
///
/// The root trait declares an associated model type per child:
///
/// - Immutable families require models implementing [`PurePluginModel`].
/// - Mutable families require models implementing [`MutablePluginModel`].
///
/// Each associated type corresponds to a concrete plugin implementation
/// bound to that operation.
///
/// These associated model types are later used by [`plugin_model!`](crate::plugin_model) to bind
/// concrete implementations to specific `(Root, Child)` combinations.
///
/// ## Example
///
/// ```ignore
/// declare_family! {
///     root: pub VotingFamily,
///     child: [Phragmen, STV]
/// }
/// ```
///
/// Expands roughly to:
///
/// ```ignore
/// pub struct Phragmen;
/// pub struct STV;
///
/// pub trait VotingFamily<Input, Context, Output> {
///     type Phragmen: PurePluginModel<Input, Context, Output>;
///     type STV: PurePluginModel<Input, Context, Output>;
/// }
/// ```
///
/// Mutable family example:
///
/// ```ignore
/// declare_family! {
///     root: mut pub StorageFamily,
///     child: [Insert, Remove]
/// }
/// ```
///
/// Expands roughly to:
///
/// ```ignore
/// pub struct Insert;
/// pub struct Remove;
///
/// pub trait StorageFamily<Input, Context, Output> {
///     type Insert: MutablePluginModel<Input, Context, Output>;
///     type Remove: MutablePluginModel<Input, Context, Output>;
/// }
/// ```
#[macro_export]
macro_rules! declare_family {
    // Immutable Family
    (
        $(#[$meta:meta])*
        root: $vis:vis $Root:ident,
        child: [
            $(
                $(#[$child_meta:meta])*
                $Child:ident
            ),+ $(,)?
        ]
    ) => {
        $(
            $(#[$child_meta])*
            $vis struct $Child;
        )+

        $(#[$meta])*
        $vis trait $Root<Input, Context, Output>{
            $(
                $(#[$child_meta])*
                type $Child: $crate::plugins::PurePluginModel<Input, Context, Output>;
            )+
        }

    };

    // Mutable Family
    (
        $(#[$meta:meta])*
        root: mut $vis:vis $Root:ident,
        child: [
            $(
                $(#[$child_meta:meta])*
                $Child:ident
            ),+ $(,)?
        ]
    ) => {
        $(
            $(#[$child_meta])*
            $vis struct $Child;
        )+

        $(#[$meta])*
        $vis trait $Root<Input, Context, Output> {
            $(
                $(#[$child_meta])*
                type $Child: $crate::plugins::MutablePluginModel<Input, Context, Output>;
            )+
        }

    };
}

// ===============================================================================
// ```````````````````````````````` DEFINE FAMILY ````````````````````````````````
// ===============================================================================

/// Declares a concrete **plugin family implementation** for a given family root.
///
/// This macro generates:
/// 1. A **family marker struct** representing a concrete implementation of a
///    plugin family.
/// 2. An implementation of the **family root trait** mapping each declared
///    child operation to a concrete plugin model.
///
/// The generated family struct is purely a **type-level marker** and contains
/// no runtime data. It is used to bind concrete plugin models to a specific
/// `(FamilyType, Child)` combination through associated types.
///
/// When `borrow` are specified, the generated family marker stores them
/// in `PhantomData` fields so the type system correctly tracks them
/// without affecting runtime behavior.
///
/// ## Syntax
///
/// ### Family With Context
///
/// ```ignore
/// define_family! {
///     root: FamilyRoot,             // Required: family root trait
///
///     family: pub MyFamily,         // Required: visibility and concrete family marker struct
///     borrow: ['a],                 // Optional: lifetime parameters for family marker
///
///     input: Input,                 // Required: input type parameter
///     output: Output,               // Optional: output type (defaults to input if omitted)
///
///     context: MyContext,           // Required: context type
///     marker: [T],                  // Optional: generic parameters for the context
///
///     bounds: [T: Clone],           // Optional: trait bounds for the generated impl
///
///     child: [                      // Required: child -> model mapping
///         OperationA => ModelA,
///         OperationB => ModelB,
///     ]
/// }
/// ```
///
/// ### Family Without Context
///
/// ```ignore
/// define_family! {
///     root: FamilyRoot,           // Required: family root trait
///
///     family: pub MyFamily,       // Required: visibility and concrete family marker struct
///     borrow: ['a],               // Optional: lifetime parameters for family marker
///
///     input: Input,               // Required: input type parameter
///     output: Output,             // Optional: output type (defaults to input if omitted)
///
///     bounds: [T: Clone],         // Optional: trait bounds for the generated impl
///
///     child: [                    // Required: child -> model mapping
///         OperationA => ModelA,
///         OperationB => ModelB,
///     ]
/// }
/// ```
///
/// In the second form, the context parameter of the root trait defaults to `()`.
///
/// ## Lifetimes and Generics
///
/// - `borrow` apply to the **family marker type**
///   - used to model execution-time borrowing
///   - stored via `PhantomData`
///
/// ```ignore
/// borrow: ['a]
/// ```
///
/// - `marker` apply only when a `context` is specified
///   - used to parameterize the **context type**
///   - introduced on the generated `impl`, not the family struct
///
/// ```ignore
/// context: MyContext,
/// marker: [T]
/// ```
///
/// When no `context` is provided:
///
/// - the context defaults to `()`
/// - `marker` is not used
///
/// ## Example
///
/// ```ignore
///
/// // ----- Crate A ------
///
/// declare_family! {
///     root: pub VotingFamily,
///     child: [Phragmen, STV]
/// }
///
/// // ----- Crate B ------
///
/// plugin_model! {
///     name: PhragmenModel,
///     ...
/// }
///
/// plugin_model! {
///     name: STVModel,
///     ...
/// }
///
/// define_family! {
///     root: VotingFamily,
///
///     family: pub RuntimeVoting,
///     input: AccountId,
///     output: Balance,
///     context: RuntimeContext,
///
///     child: [
///         Phragmen => PhragmenModel,
///         STV => STVModel,
///     ]
/// }
/// ```
///
/// This binds the `Phragmen` and `Approval` operations of the
/// `VotingFamily` root to concrete plugin models for the
/// `RuntimeVoting` family implementation.
#[macro_export]
macro_rules! define_family {

    // Helper Rule: `@output_ty`
    (@output_ty $Input:tt) => { $Input };
    (@output_ty $Input:tt, $Output:tt) => { $Output };


    // With Context, Single Input, Single Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: $Input:ident,
        $(output: $Output:ident ,)?
        context: $Context:ty,
        $(marker: [$($marker_gen:ident),* $(,)?],)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $($($marker_gen,)*)?
            $Input
            $(, $Output)?
        > $Root<
            $Input,
            $Context,
            $crate::define_family!(@output_ty $Input $(, $Output)?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+
        }
    };

    // With Context, Single Input, Tuple Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: $Input:ident,
        $(output: ($($Output:ident),+) ,)?
        context: $Context:ty,
        $(marker: [$($marker_gen:ident),* $(,)?],)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $($($marker_gen,)*)?
            $Input
            $(, $($Output),+)?
        > $Root<
            $Input,
            $Context,
            $crate::define_family!(@output_ty $Input $(, ($($Output),+))?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+

        }
    };

    // With Context, Tuple Input, Single Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: ($($Input:ident),+),
        $(output: $Output:ident ,)?
        context: $Context:ty,
        $(marker: [$($marker_gen:ident),* $(,)?],)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $($($marker_gen,)*)?
            $($Input),+
            $(, $Output)?
        > $Root<
            ($($Input),+),
            $Context,
            $crate::define_family!(@output_ty ($($Input),+) $(, $Output)?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+

        }
    };

    // With Context, Tuple Input, Tuple Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: ($($Input:ident),+),
        $(output: ($($Output:ident),+) ,)?
        context: $Context:ty,
        $(marker: [$($marker_gen:ident),* $(,)?],)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $($($marker_gen,)*)?
            $($Input),+
            $(, $($Output),+)?
        > $Root<
            ($($Input),+),
            $Context,
            $crate::define_family!(@output_ty ($($Input),+) $(, ($($Output),+))?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+

        }
    };

    // No Context, Single Input, Single Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: $Input:ident,
        $(output: $Output:ident ,)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $Input
            $(, $Output)?
        > $Root<
            $Input,
            (),
            $crate::define_family!(@output_ty $Input $(, $Output)?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+
        }
    };

    // No Context, Single Input, Tuple Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: $Input:ident,
        $(output: ($($Output:ident),+) ,)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $Input
            $(, $($Output),+)?
        > $Root<
            $Input,
            (),
            $crate::define_family!(@output_ty $Input $(, ($($Output),+))?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+
        }
    };

    // No Context, Tuple Input, Single Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: ($($Input:ident),+),
        $(output: $Output:ident ,)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $($Input),+
            $(, $Output)?
        > $Root<
            ($($Input),+),
            (),
            $crate::define_family!(@output_ty ($($Input),+) $(, $Output)?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+
        }
    };

    // No Context, Tuple Input, Tuple Output
    (
        root: $Root:ident,
        $(#[$meta:meta])*
        family: $vis:vis $Name:ident,
        $(borrow: [$($borrow_lt:lifetime),* $(,)?],)?
        input: ($($Input:ident),+),
        $(output: ($($Output:ident),+) ,)?
        $(bounds: [$($bounds:tt)*],)?
        child: [
            $($child:ident => $model:ty,)+
        $(,)? ] $(,)?
    ) => {

        $crate::__phantom_struct!(
            $(#[$meta])*
            #[allow(unused)]
            $vis
            $Name
            [$($($borrow_lt),*)?]
            []
        );

        impl<
            $($($borrow_lt,)*)?
            $($Input),+
            $(, $($Output),+)?
        > $Root<
            ($($Input),+),
            (),
            $crate::define_family!(@output_ty ($($Input),+) $(, ($($Output),+))?)
        >
        for $Name$(<
            $($borrow_lt,)*
        >)?
        $(where $($bounds)*)?
        {
            $(
                type $child = $model;
            )+
        }
    };

}

// ===============================================================================
// ```````````````````````````````` HELPER MACROS ````````````````````````````````
// ===============================================================================

/// Generates a zero-sized or PhantomData-backed marker struct.
///
/// This is an internal helper used by `plugin_context`, `define_family`,
/// and other macros that need to produce marker structs which may carry
/// lifetime or type parameters purely at the type level without any
/// runtime storage.
///
/// ## Syntax
///
/// ```ignore
/// __phantom_struct!(
///     #[attributes]   // optional
///     VISIBILITY      // pub, pub(crate), or empty
///     NAME            // struct identifier
///     [LIFETIMES]     // e.g. ['a, 'b] or []
///     [GENERICS]      // e.g. [T, U]   or []
/// )
/// ```
///
/// ## Variants
///
/// | Lifetimes | Generics | Generated struct                               |
/// |-----------|----------|------------------------------------------------|
/// | `[]`      | `[]`     | `struct Foo;`                                  |
/// | `['a]`    | `[]`     | `struct Foo<'a>(PhantomData<(&'a (),)>)`       |
/// | `[]`      | `[T]`    | `struct Foo<T>(PhantomData<(T,)>)`             |
/// | `['a]`    | `[T]`    | `struct Foo<'a, T>(PhantomData<(T, &'a ())>)`  |
///
#[macro_export]
macro_rules! __phantom_struct {

    // Arm 1: No lifetimes, no generics -> plain unit struct.
    (
        $(#[$meta:meta])*
        $vis:vis
        $Name:ident
        []
        []
    ) => {
        $(#[$meta])*
        $vis struct $Name;
    };

    // Arm 2: Lifetimes only -> struct with a PhantomData reference tuple
    // that is covariant over each declared lifetime independently.
    (
        $(#[$meta:meta])*
        $vis:vis
        $Name:ident
        [$($lt:lifetime),+ $(,)?]
        []
    ) => {
        $(#[$meta])*
        $vis struct $Name<$($lt),*>(
            core::marker::PhantomData<($(&$lt (),)*)>
        );
    };

    // Arm 3: Generics only -> struct with a PhantomData tuple field that
    // tracks each type parameter independently.
    (
        $(#[$meta:meta])*
        $vis:vis
        $Name:ident
        []
        [$($gen:ident),+ $(,)?]
    ) => {
        $(#[$meta])*
        $vis struct $Name<$($gen),*>(
            core::marker::PhantomData<($($gen,)*)>
        );
    };

    // Arm 4: Both lifetimes and generics -> single PhantomData tuple field
    // combining both, so the struct has one field instead of two and the
    // variance of each parameter remains independent.
    (
        $(#[$meta:meta])*
        $vis:vis
        $Name:ident
        [$($lt:lifetime),+ $(,)?]
        [$($gen:ident),+ $(,)?]
    ) => {
        $(#[$meta])*
        $vis struct $Name<$($lt),*, $($gen),*>(
            core::marker::PhantomData<($($gen,)* $(&$lt (),)*)>
        );
    };
}

// ===============================================================================
// `````````````````````````````````` MOCK TEST ``````````````````````````````````
// ===============================================================================

#[cfg(test)]
#[allow(unused)]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use super::*;

    // --- Core / Std ---
    use core::marker::PhantomData;
    use std::mem::take;

    // --- Substrate primitives ---
    use sp_arithmetic::traits::AtLeast8BitUnsigned;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` STRUCTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    //--- Mock structs ---

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BasicConfig {
        value: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct GenericConfig<T> {
        value: T,
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````` PLUGIN CONTEXT ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    //--- Basic context form ---

    plugin_context! {
        name: pub BasicConfigProvider,
        context: BasicConfig,
        value: BasicConfig {value: 10}
    }

    #[test]
    fn plugin_context_basic_form_returns_expected_context() {
        let ctx = <BasicConfigProvider as ModelContext>::context();
        assert_eq!(ctx, BasicConfig { value: 10 });
    }

    //--- Generic marker form ---

    plugin_context! {
        name: GenericConfigProvider,
        context: GenericConfig<T>,
        marker: [T],
        bounds: [T: AtLeast8BitUnsigned + Default],
        value: GenericConfig {value: T::default()}
    }

    #[test]
    fn plugin_context_marker_form_returns_expected_context() {
        let ctx = <GenericConfigProvider<u8> as ModelContext>::context();
        assert_eq!(ctx, GenericConfig { value: 0u8 });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` PLUGIN MODEL `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    //--- Variant 1: No Context, Single Input, Single Output ---

    plugin_model! {
        name: pub PureNoCtxSingleSingle,
        input: Input,
        output: Output,
        bounds: [Input: Into<u8>, Output: From<u8>],
        compute: |input, _ctx| {
            let x = input.into();
            Output::from(x + 1)
        }
    }

    plugin_test! {
        model: PureNoCtxSingleSingle,
        input: u8,
        output: u8,
        cases: {
            (pure_no_ctx_single_single_case1, 10, 11),
            (pure_no_ctx_single_single_case2, 0, 1),
        }
    }

    plugin_test! {
        model: PureNoCtxSingleSingle,
        input: u8,
        cases: {
            (pure_no_ctx_single_single_case3, 99, 100),
            (pure_no_ctx_single_single_case4, 24, 25),
        }
    }

    //--- Variant 2: No Context, Single Input, Tuple Output ---

    plugin_model! {
        name: pub PureNoCtxSingleTuple,
        input: Input,
        output: (OutA, OutB),
        bounds: [Input: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        compute: |input, _ctx| {
            let x = input.into();
            (OutA::from(x), OutB::from(x + 1))
        }
    }

    plugin_test! {
        model: PureNoCtxSingleTuple,
        input: u8,
        output: (u8, u8),
        cases: {
            (pure_no_ctx_single_tuple_case1, 10, (10, 11)),
            (pure_no_ctx_single_tuple_case2, 0, (0, 1)),
        }
    }

    //--- Variant 3: No Context, Tuple Input, Single Output ---

    plugin_model! {
        name: pub PureNoCtxTupleSingle,
        input: (InpA, InpB),
        output: Output,
        bounds: [InpA: Into<u8>, InpB: Into<u8>, Output: From<u8>],
        compute: |input, _ctx| {
            let (a, b) = input;
            Output::from(a.into() + b.into())
        }
    }

    plugin_test! {
        model: PureNoCtxTupleSingle,
        input: (u8, u8),
        output: u8,
        cases: {
            (pure_no_ctx_tuple_single_case1, (10, 11), 21),
            (pure_no_ctx_tuple_single_case2, (0, 5), 5),
        }
    }

    //--- Variant 4: No Context, Tuple Input, Tuple Output ---

    plugin_model! {
        name: pub PureNoCtxTupleTuple,
        input: (InpA, InpB),
        output: (OutA, OutB),
        bounds: [InpA: Into<u8>, InpB: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        compute: |input, _ctx| {
            let (a, b) = input;
            (OutA::from(a.into() * 10), OutB::from(b.into() * 10))
        }
    }

    plugin_test! {
        model: PureNoCtxTupleTuple,
        input: (u8, u8),
        output: (u8, u8),
        cases: {
            (pure_no_ctx_tuple_tuple_case1, (5, 10), (50, 100)),
            (pure_no_ctx_tuple_tuple_case2, (1, 0), (10, 0)),
        }
    }

    // --- Variant 5: Context, Single Input, Single Output ---

    plugin_model! {
        name: pub PureCtxSingleSingle,
        input: Input,
        output: Output,
        context: BasicConfig,
        bounds: [Input: Into<u8>, Output: From<u8>],
        compute: |input, ctx| {
            let v = ctx.value;
            Output::from(input.into() + v)
        }
    }

    plugin_test! {
        model: PureCtxSingleSingle,
        input: u8,
        output: u8,
        context: BasicConfig,
        value: BasicConfig{ value: 10 },
        cases: {
            (pure_ctx_single_single_case1, 10, 20),
            (pure_ctx_single_single_case2, 1, 11),
        }
    }

    plugin_test! {
        model: PureCtxSingleSingle,
        input: u8,
        context: BasicConfig,
        value: BasicConfig{ value: 10 },
        cases: {
            (pure_ctx_single_single_case3, 90, 100),
            (pure_ctx_single_single_case4, 0, 10),
        }
    }

    //--- Variant 6: Context, Single Input, Tuple Output ---

    plugin_model! {
        name: pub PureCtxSingleTuple,
        input: Input,
        output: (OutA, OutB),
        context: BasicConfig,
        bounds: [Input: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        compute: |input, ctx| {
            let x = input.into();
            let v = ctx.value;
            (OutA::from(x), OutB::from(x + v))
        }
    }

    plugin_test! {
        model: PureCtxSingleTuple,
        input: u8,
        output: (u8, u8),
        context: BasicConfig,
        value: BasicConfig{ value: 1 },
        cases: {
            (pure_ctx_single_tuple_case1, 5, (5, 6)),
            (pure_ctx_single_tuple_case2, 1, (1, 2)),
        }
    }

    //--- Variant 7: Context, Tuple Input, Single Output ---

    plugin_model! {
        name: pub PureCtxTupleSingle,
        input: (InpA, InpB),
        output: Output,
        context: BasicConfig,
        bounds: [InpA: Into<u8>, InpB: Into<u8>, Output: From<u8>],
        compute: |input, ctx| {
            let (a, b) = input;
            let v = ctx.value;
            Output::from(a.into() + b.into() + v)
        }
    }

    plugin_test! {
        model: PureCtxTupleSingle,
        input: (u8, u8),
        output: u8,
        context: BasicConfig,
        value: BasicConfig { value: 10},
        cases: {
            (pure_ctx_tuple_single_case1, (10, 10), 30),
            (pure_ctx_tuple_single_case2, (5, 30), 45),
        }
    }

    //--- Variant 8: Context, Tuple Input, Tuple Output ---

    plugin_model! {
        name: pub PureCtxTupleTuple,
        input: (InpA, InpB),
        output: (OutA, OutB),
        context: BasicConfig,
        bounds: [InpA: Into<u8>, InpB: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        compute: |input, ctx| {
            let (a, b) = input;
            let v = ctx.value;
            (OutA::from(a.into() + v), OutB::from(b.into() + v))
        }
    }

    plugin_test! {
        model: PureCtxTupleTuple,
        input: (u8, u8),
        output: (u8, u8),
        context: BasicConfig,
        value: BasicConfig { value: 5 },
        cases: {
            (pure_ctx_tuple_tuple_case1, (5, 10), (10, 15)),
            (pure_ctx_tuple_tuple_case2, (1, 0), (6, 5)),
        }
    }

    //--- Variant 9: No Context, Single Input, Single Output (MUTABLE) ---

    plugin_model! {
        name: pub MutNoCtxSingleSingle,
        input: mut Input,
        output: Output,
        bounds: [Input: From<Vec<u8>> + Into<Vec<u8>> + Default, Output: From<usize>],
        compute: |input, _ctx| {
            let mut v: Vec<u8> = take(input).into();
            v.push(1);
            let len = v.len();
            *input = Input::from(v);
            Output::from(len)
        }
    }

    plugin_test! {
        model: MutNoCtxSingleSingle,
        input: mut Vec<u8>,
        output: usize,
        cases: {
            (mut_no_ctx_single_single_case1, vec![1, 2], 3usize, vec![1, 2, 1]),
            (mut_no_ctx_single_single_case2, vec![5, 4, 3, 2], 5usize, vec![5, 4, 3, 2, 1]),
        }
    }

    //--- Variant 10: No Context, Single Input, Tuple Output (MUTABLE) ---

    plugin_model! {
        name: pub MutNoCtxSingleTuple,
        input: mut Input,
        output: (OutA, OutB),
        bounds: [Input: From<Vec<u8>> + Into<Vec<u8>> + Default, OutA: From<usize>, OutB: From<u8>],
        compute: |input, _ctx| {
            let mut v:  Vec<u8> = take(input).into();
            v.push(1);
            v.push(0);
            let len = v.len();
            let last = *v.last().unwrap();
            *input = Input::from(v);
            (OutA::from(len), OutB::from(last))
        }
    }

    plugin_test! {
        model: MutNoCtxSingleTuple,
        input: mut Vec<u8>,
        output: (usize, u8),
        cases: {
            (mut_no_ctx_single_tuple_case1, vec![1, 0], (4usize, 0), vec![1, 0, 1, 0]),
            (mut_no_ctx_single_tuple_case2, vec![5, 4, 3, 2], (6usize, 0)),
        }
    }

    //--- Variant 11: No Context, Tuple Input, Single Output (MUTABLE) ---

    plugin_model! {
        name: pub MutNoCtxTupleSingle,
        input: mut (InpA, InpB),
        output: Output,
        bounds: [InpA: From<u8> + Into<u8> + Copy, InpB: From<u8> + Into<u8> + Copy, Output: From<u8>],
        compute: |input, _ctx| {
            input.0 = InpA::from(input.0.into() + 1);
            input.1 = InpB::from(input.1.into() + 1);
            Output::from(input.0.into() + input.1.into())
        }
    }

    plugin_test! {
        model: MutNoCtxTupleSingle,
        input: mut (u8, u8),
        output: u8,
        cases: {
            (mut_no_ctx_tuple_single_case1, (11, 12), 25, (12, 13)),
            (mut_no_ctx_tuple_single_case2, (0, 0), 2, (1, 1)),

        }
    }

    //--- Variant 12: No Context, Tuple Input, Tuple Output (MUTABLE) ---

    plugin_model! {
        name: pub MutNoCtxTupleTuple,
        input: mut (A, B),
        output: (OutA, OutB),
        bounds: [A: From<u8> + Into<u8> + Copy, B: From<u8> + Into<u8> + Copy, OutA: From<u8>, OutB: From<u8>],
        compute: |input, _ctx| {
            input.0 = A::from(input.0.into() * 10);
            input.1 = B::from(input.1.into() * 0);
            (OutA::from(input.0.into()), OutB::from(input.1.into()))
        }
    }

    plugin_test! {
        model: MutNoCtxTupleTuple,
        input: mut (u8, u8),
        output: (u8, u8),
        cases: {
            (mut_no_ctx_tuple_tuple_case1, (10, 10), (100, 0), (100, 0)),
            (mut_no_ctx_tuple_tuple_case2, (20, 35), (200, 0)),
        }
    }

    plugin_test! {
        model: MutNoCtxTupleTuple,
        input: mut (u8, u8),
        cases: {
            (mut_no_ctx_tuple_tuple_case3, (1, 10), (10, 0), (10, 0)),
            (mut_no_ctx_tuple_tuple_case4, (0, 0), (0, 0)),
        }
    }

    //--- Variant 13: Context, Single Input, Single Output (MUTABLE) ---

    plugin_model! {
        name: pub MutCtxSingleSingle,
        input: mut Input,
        output: Output,
        context: BasicConfig,
        bounds: [Input: From<Vec<u8>> + Into<Vec<u8>> + Default, Output: From<usize>],
        compute: |input, ctx| {
            let mut v: Vec<u8> = take(input).into();
            v.push(ctx.value);
            let len = v.len();
            *input = Input::from(v);
            Output::from(len)
        }
    }

    plugin_test! {
        model: MutCtxSingleSingle,
        input: mut Vec<u8>,
        output: usize,
        context: BasicConfig,
        value: BasicConfig { value: 0 },
        cases: {
            (mut_ctx_single_single_case1, vec![1], 2usize, vec![1, 0]),
            (mut_ctx_single_single_case2, vec![1, 5, 3], 4usize, vec![1, 5, 3, 0]),

        }
    }

    //--- Variant 14: Context, Single Input, Tuple Output (MUTABLE) --

    plugin_model! {
        name: pub MutCtxSingleTuple,
        input: mut Input,
        output: (OutA, OutB),
        context: BasicConfig,
        bounds: [Input: From<Vec<u8>> + Into<Vec<u8>> + Default, OutA: From<usize>, OutB: From<u8>],
        compute: |input, ctx| {
            let mut v: Vec<u8> = take(input).into();
            v.push(ctx.value);
            let len = v.len();
            let last = *v.last().unwrap();
            *input = Input::from(v);
            (OutA::from(len), OutB::from(last))
        }
    }

    plugin_test! {
        model: MutCtxSingleTuple,
        input: mut Vec<u8>,
        output: (usize, u8),
        context: BasicConfig,
        value: BasicConfig{value: 4},
        cases: {
            (mut_ctx_single_tuple_case1, vec![2, 3], (3usize, 4), vec![2, 3, 4]),
            (mut_ctx_single_tuple_case2, vec![20, 16, 12, 8], (5usize, 4), vec![20, 16, 12, 8, 4]),
        }
    }

    //--- Variant 15: Context, Tuple Input, Single Output (MUTABLE) ---

    plugin_model! {
        name: pub MutCtxTupleSingle,
        input: mut (InpA, InpB),
        output: Output,
        context: BasicConfig,
        bounds: [InpA: From<u8> + Into<u8> + Copy, InpB: From<u8> + Into<u8> + Copy, Output: From<u8>],
        compute: |input, ctx| {
            input.0 = InpA::from(input.0.into() + ctx.value);
            input.1 = InpB::from(input.1.into() + ctx.value);
            Output::from(input.0.into() + input.1.into())
        }
    }

    plugin_test! {
        model: MutCtxTupleSingle,
        input: mut (u8, u8),
        output: u8,
        context: BasicConfig,
        value: BasicConfig{value: 2},
        cases: {
            (mut_ctx_tuple_single_case1, (1, 2), 7, (3, 4)),
            (mut_ctx_tuple_single_case2, (8, 8), 20, (10, 10)),
        }
    }

    //--- Variant 16: Context, Tuple Input, Tuple Output (MUTABLE) ---

    plugin_model! {
        name: pub MutCtxTupleTuple,
        input: mut (A, B),
        output: (OutA, OutB),
        context: BasicConfig,
        bounds: [A: From<u8> + Into<u8> + Copy, B: From<u8> + Into<u8> + Copy, OutA: From<u8>, OutB: From<u8>],
        compute: |input, ctx| {
            input.0 = A::from(input.0.into() + ctx.value);
            input.1 = B::from(input.1.into() + ctx.value + 1);
            (OutA::from(input.0.into()), OutB::from(input.1.into()))
        }
    }

    plugin_test! {
        model: MutCtxTupleTuple,
        input: mut (u8, u8),
        output: (u8, u8),
        context: BasicConfig,
        value: BasicConfig{value: 2},
        cases: {
            (mut_ctx_tuple_tuple_case1, (5, 6), (7, 9), (7, 9)),
            (mut_ctx_tuple_tuple_case2, (0, 0), (2, 3), (2, 3)),
        }
    }

    plugin_test! {
        model: MutCtxTupleTuple,
        input: mut (u8, u8),
        context: BasicConfig,
        value: BasicConfig{value: 2},
        cases: {
            (mut_ctx_tuple_tuple_case3, (8, 7), (10, 10), (10, 10)),
            (mut_ctx_tuple_tuple_case4, (1, 1), (3, 4), (3, 4)),
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` PLUGIN TYPES ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    plugin_context! {
        name: UnitContextProvider,
        context: (),
        value: ()
    }

    //--- Variant 1: Immutable concrete model arm ---

    trait ImmutablePluginTrait {
        plugin_types! {
            input: u8,
            output: u8,
            model: Model,
            context: Context,
        }
    }

    struct ImmutableHost;

    impl ImmutablePluginTrait for ImmutableHost {
        type Model = PureNoCtxSingleSingle;
        type Context = UnitContextProvider;
    }

    fn run_immutable_plugin<T: ImmutablePluginTrait>(input: u8) -> u8 {
        let model = T::Model::default();
        let ctx = <T::Context as ModelContext>::context();

        <T::Model as PurePluginModel<u8, <T::Context as ModelContext>::Context, u8>>::compute(
            &model, input, &ctx,
        )
    }

    #[test]
    fn plugin_types_immutable_model_arm_works() {
        assert_eq!(run_immutable_plugin::<ImmutableHost>(10), 11);
        assert_eq!(run_immutable_plugin::<ImmutableHost>(0), 1);
    }

    //--- Variant 2: Mutable concrete model arm ---

    trait MutablePluginTrait {
        plugin_types! {
            input: mut Vec<u8>,
            output: usize,
            model: Model,
            context: Context,
        }
    }

    struct MutableHost;

    impl MutablePluginTrait for MutableHost {
        type Model = MutNoCtxSingleSingle;
        type Context = UnitContextProvider;
    }

    fn run_mutable_plugin<T: MutablePluginTrait>(mut input: Vec<u8>) -> (Vec<u8>, usize) {
        let model = T::Model::default();
        let ctx = <T::Context as ModelContext>::context();

        let out = <T::Model as MutablePluginModel<
            Vec<u8>,
            <T::Context as ModelContext>::Context,
            usize,
        >>::compute_mut(&model, &mut input, &ctx);

        (input, out)
    }

    #[test]
    fn plugin_types_mutable_model_arm_works() {
        let (input, out) = run_mutable_plugin::<MutableHost>(vec![1, 2]);
        assert_eq!(input, vec![1, 2, 1]);
        assert_eq!(out, 3);

        let (input, out) = run_mutable_plugin::<MutableHost>(vec![5, 4, 3, 2]);
        assert_eq!(input, vec![5, 4, 3, 2, 1]);
        assert_eq!(out, 5);
    }

    //--- Variant 3: Family arm ---

    trait SimpleFamilyRoot<Input, Context, Output> {
        type Op: PurePluginModel<Input, Context, Output> + Default;
    }

    struct SimpleFamily;

    impl SimpleFamilyRoot<u8, (), u8> for SimpleFamily {
        type Op = PureNoCtxSingleSingle;
    }

    trait FamilyPluginTrait {
        plugin_types! {
            input: u8,
            output: u8,
            root: SimpleFamilyRoot,
            family: Family,
            context: Context,
        }
    }

    struct FamilyHost;

    impl FamilyPluginTrait for FamilyHost {
        type Family = SimpleFamily;
        type Context = UnitContextProvider;
    }

    fn run_family_plugin<T: FamilyPluginTrait>(input: u8) -> u8 {
        let model = <T::Family as SimpleFamilyRoot<
            u8,
            <T::Context as ModelContext>::Context,
            u8,
        >>::Op::default();

        let ctx = <T::Context as ModelContext>::context();

        <<T::Family as SimpleFamilyRoot<
            u8,
            <T::Context as ModelContext>::Context,
            u8
        >>::Op as PurePluginModel<
            u8,
            <T::Context as ModelContext>::Context,
            u8
        >>::compute(&model, input, &ctx)
    }

    #[test]
    fn plugin_types_family_arm_works() {
        assert_eq!(run_family_plugin::<FamilyHost>(10), 11);
        assert_eq!(run_family_plugin::<FamilyHost>(0), 1);
    }

    //--- Varaint 4: Family arm with `provides` ---

    trait ProvidedFamilyRoot<Input, Context, Output> {
        type Op: PurePluginModel<Input, Context, Output> + Default;
    }

    struct ProvidedFamily;

    impl ProvidedFamilyRoot<u8, BasicConfig, u8> for ProvidedFamily {
        type Op = PureCtxSingleSingle;
    }

    trait ProvidedFamilyPluginTrait {
        plugin_types! {
            input: u8,
            output: u8,
            root: ProvidedFamilyRoot,
            family: Family,
            context: Context,
            provides: [Send + Sync + 'static],
        }
    }

    struct ThreadSafeConfigProvider;

    impl ModelContext for ThreadSafeConfigProvider {
        type Context = BasicConfig;

        fn context() -> Self::Context {
            BasicConfig { value: 10 }
        }
    }

    struct ProvidedFamilyHost;

    impl ProvidedFamilyPluginTrait for ProvidedFamilyHost {
        type Family = ProvidedFamily;
        type Context = ThreadSafeConfigProvider;
    }

    fn run_family_with_provides<T: ProvidedFamilyPluginTrait>(input: u8) -> u8 {
        let model = <T::Family as ProvidedFamilyRoot<
            u8,
            <T::Context as ModelContext>::Context,
            u8,
        >>::Op::default();

        let ctx = <T::Context as ModelContext>::context();

        <<T::Family as ProvidedFamilyRoot<
            u8,
            <T::Context as ModelContext>::Context,
            u8
        >>::Op as PurePluginModel<
            u8,
            <T::Context as ModelContext>::Context,
            u8
        >>::compute(&model, input, &ctx)
    }

    #[test]
    fn plugin_types_family_arm_with_provides_works() {
        assert_eq!(run_family_with_provides::<ProvidedFamilyHost>(10), 20);
        assert_eq!(run_family_with_provides::<ProvidedFamilyHost>(1), 11);
    }

    //--- Variant 5: Family arm with `borrow`

    #[derive(Default)]
    struct BorrowIdentityModel;

    impl<'a> PurePluginModel<&'a [u8], (), &'a [u8]> for BorrowIdentityModel {
        fn compute(&self, input: &'a [u8], _context: &()) -> &'a [u8] {
            input
        }
    }

    trait BorrowFamilyRoot<Input, Context, Output> {
        type Op: PurePluginModel<Input, Context, Output> + Default;
    }

    struct BorrowFamily<'a>(PhantomData<&'a ()>);

    impl<'a> BorrowFamilyRoot<&'a [u8], (), &'a [u8]> for BorrowFamily<'a> {
        type Op = BorrowIdentityModel;
    }

    trait BorrowedFamilyPluginTrait {
        plugin_types! {
            input: &'a [u8],
            output: &'a [u8],
            borrow: ['a],
            root: BorrowFamilyRoot,
            family: Family,
            context: Context,
        }
    }

    struct BorrowedFamilyHost;

    impl BorrowedFamilyPluginTrait for BorrowedFamilyHost {
        type Family<'a> = BorrowFamily<'a>;
        type Context = UnitContextProvider;
    }

    fn run_borrowed_family_plugin<'a, T: BorrowedFamilyPluginTrait>(input: &'a [u8]) -> &'a [u8] {
        let model = <T::Family<'a> as BorrowFamilyRoot<
            &'a [u8],
            <T::Context as ModelContext>::Context,
            &'a [u8],
        >>::Op::default();

        let ctx = <T::Context as ModelContext>::context();

        <<T::Family<'a> as BorrowFamilyRoot<
            &'a [u8],
            <T::Context as ModelContext>::Context,
            &'a [u8]
        >>::Op as PurePluginModel<
            &'a [u8],
            <T::Context as ModelContext>::Context,
            &'a [u8]
        >>::compute(&model, input, &ctx)
    }

    #[test]
    fn plugin_types_family_arm_with_borrow_works() {
        let data = [1u8, 2, 3];
        assert_eq!(
            run_borrowed_family_plugin::<BorrowedFamilyHost>(&data),
            &data
        );
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` PLUGIN OUTPUT ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // Variant 1: Immutable concrete model arm

    struct OutputPureModelRunner;

    impl OutputPureModelRunner {
        plugin_output! {
            pub fn run_pure_model,
            input: u8,
            output: u8,
            model: PureCtxSingleSingle,
            context: BasicConfigProvider,
        }
    }

    #[test]
    fn plugin_output_immutable_model_arm_works() {
        assert_eq!(OutputPureModelRunner::run_pure_model(10), 20);
        assert_eq!(OutputPureModelRunner::run_pure_model(0), 10);
    }

    // Variant 2: concrete model arm

    struct OutputMutableModelRunner;

    impl OutputMutableModelRunner {
        plugin_output! {
            pub fn run_mutable_model,
            input: mut Vec<u8>,
            output: usize,
            model: MutNoCtxSingleSingle,
            context: UnitContextProvider,
        }
    }

    #[test]
    fn plugin_output_mutable_model_arm_works() {
        let mut input = vec![1, 2];
        let out = OutputMutableModelRunner::run_mutable_model(&mut input);

        assert_eq!(out, 3);
        assert_eq!(input, vec![1, 2, 1]);

        let mut input = vec![5, 4, 3, 2];
        let out = OutputMutableModelRunner::run_mutable_model(&mut input);

        assert_eq!(out, 5);
        assert_eq!(input, vec![5, 4, 3, 2, 1]);
    }

    // Variant 3: Immutable family-selected model arm

    declare_family! {
        root: pub OutputFamilyRoot,
        child: [Run]
    }

    define_family! {
        root: OutputFamilyRoot,
        family: OutputFamily,
        input: Input,
        output: Output,
        context: (),
        bounds: [
            Input: Into<u8>,
            Output: From<u8>,
        ],
        child: [
            Run => PureNoCtxSingleSingle,
        ],
    }

    struct OutputFamilyRunner;

    impl OutputFamilyRunner {
        plugin_output! {
            pub fn run_family_model,
            input: u8,
            output: u8,
            root: OutputFamilyRoot,
            family: OutputFamily,
            child: Run,
            context: UnitContextProvider,
        }
    }

    #[test]
    fn plugin_output_immutable_family_arm_works() {
        assert_eq!(OutputFamilyRunner::run_family_model(10), 11);
        assert_eq!(OutputFamilyRunner::run_family_model(0), 1);
    }

    // Variant 4: Mutable family-selected model arm

    declare_family! {
        root: mut pub OutputMutFamilyRoot,
        child: [RunMut]
    }

    define_family! {
        root: OutputMutFamilyRoot,
        family: OutputMutFamily,
        input: Input,
        output: Output,
        context: (),
        bounds: [
            Input: From<Vec<u8>> + Into<Vec<u8>> + Default,
            Output: From<usize>,
        ],
        child: [
            RunMut => MutNoCtxSingleSingle,
        ],
    }

    struct OutputMutFamilyRunner;

    impl OutputMutFamilyRunner {
        plugin_output! {
            pub fn run_mut_family_model,
            input: mut Vec<u8>,
            output: usize,
            root: OutputMutFamilyRoot,
            family: OutputMutFamily,
            child: RunMut,
            context: UnitContextProvider,
        }
    }

    #[test]
    fn plugin_output_mutable_family_arm_works() {
        let mut input = vec![1, 2];
        let out = OutputMutFamilyRunner::run_mut_family_model(&mut input);

        assert_eq!(out, 3);
        assert_eq!(input, vec![1, 2, 1]);

        let mut input = vec![9];
        let out = OutputMutFamilyRunner::run_mut_family_model(&mut input);

        assert_eq!(out, 2);
        assert_eq!(input, vec![9, 1]);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````` DECLARE FAMILY ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    //--- Variant 1: Immutable Family ---

    declare_family! {
        root: pub ImmutableDeclaredRoot,
        child: [DeclaredRun, DeclaredEcho]
    }

    struct ImmutableDeclaredFamily;

    impl ImmutableDeclaredRoot<u8, (), u8> for ImmutableDeclaredFamily {
        type DeclaredRun = PureNoCtxSingleSingle;
        type DeclaredEcho = PureNoCtxSingleSingle;
    }

    fn run_declared_immutable_family(input: u8) -> u8 {
        let model =
            <ImmutableDeclaredFamily as ImmutableDeclaredRoot<u8, (), u8>>::DeclaredRun::default();
        let context = ();

        <<ImmutableDeclaredFamily as ImmutableDeclaredRoot<u8, (), u8>>::DeclaredRun
            as PurePluginModel<u8, (), u8>>::compute(&model, input, &context)
    }

    #[test]
    fn declare_family_immutable_arm_creates_root_and_children() {
        assert_eq!(run_declared_immutable_family(10), 11);
        assert_eq!(run_declared_immutable_family(0), 1);
    }

    #[test]
    fn declare_family_immutable_arm_child_markers_exist() {
        let _run = DeclaredRun;
        let _echo = DeclaredEcho;
    }

    //--- Variant 2: Mutable Family ---

    declare_family! {
        root: mut pub MutableDeclaredRoot,
        child: [DeclaredRunMut, DeclaredNormalize]
    }

    struct MutableDeclaredFamily;

    impl MutableDeclaredRoot<Vec<u8>, (), usize> for MutableDeclaredFamily {
        type DeclaredRunMut = MutNoCtxSingleSingle;
        type DeclaredNormalize = MutNoCtxSingleSingle;
    }

    fn run_declared_mutable_family(mut input: Vec<u8>) -> (Vec<u8>, usize) {
        let model =
            <MutableDeclaredFamily as MutableDeclaredRoot<Vec<u8>, (), usize>>::DeclaredRunMut::default();
        let context = ();

        let out =
            <<MutableDeclaredFamily as MutableDeclaredRoot<Vec<u8>, (), usize>>::DeclaredRunMut
                as MutablePluginModel<Vec<u8>, (), usize>>::compute_mut(
                    &model,
                    &mut input,
                    &context,
                );

        (input, out)
    }

    #[test]
    fn declare_family_mutable_arm_creates_root_and_children() {
        let (input, out) = run_declared_mutable_family(vec![1, 2]);
        assert_eq!(input, vec![1, 2, 1]);
        assert_eq!(out, 3);
    }

    #[test]
    fn declare_family_mutable_arm_child_markers_exist() {
        let _run = DeclaredRunMut;
        let _normalize = DeclaredNormalize;
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` DEFINE FAMILY ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    //--- With Context, Single Input, Single Output ---

    declare_family! {
        root: pub DefFamCtxSingleSingleRoot,
        child: [DefFamCtxSingleSingleChild]
    }

    define_family! {
        root: DefFamCtxSingleSingleRoot,
        family: DefFamCtxSingleSingleFamily,
        input: Input,
        output: Output,
        context: BasicConfig,
        bounds: [Input: Into<u8>, Output: From<u8>],
        child: [
            DefFamCtxSingleSingleChild => PureCtxSingleSingle,
        ],
    }

    fn run_define_family_ctx_single_single(input: u8) -> u8 {
        let model = <DefFamCtxSingleSingleFamily as DefFamCtxSingleSingleRoot<
            u8,
            BasicConfig,
            u8,
        >>::DefFamCtxSingleSingleChild::default();

        let ctx = BasicConfig { value: 10 };

        <<DefFamCtxSingleSingleFamily as DefFamCtxSingleSingleRoot<u8, BasicConfig, u8>>
            ::DefFamCtxSingleSingleChild as PurePluginModel<u8, BasicConfig, u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_with_context_single_input_single_output_works() {
        assert_eq!(run_define_family_ctx_single_single(10), 20);
        assert_eq!(run_define_family_ctx_single_single(1), 11);
    }

    //--- With Context, Single Input, Tuple Output ---

    declare_family! {
        root: pub DefFamCtxSingleTupleRoot,
        child: [DefFamCtxSingleTupleChild]
    }

    define_family! {
        root: DefFamCtxSingleTupleRoot,
        family: DefFamCtxSingleTupleFamily,
        input: Input,
        output: (OutA, OutB),
        context: BasicConfig,
        bounds: [Input: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        child: [
            DefFamCtxSingleTupleChild => PureCtxSingleTuple,
        ],
    }

    fn run_define_family_ctx_single_tuple(input: u8) -> (u8, u8) {
        let model = <DefFamCtxSingleTupleFamily as DefFamCtxSingleTupleRoot<
            u8,
            BasicConfig,
            (u8, u8),
        >>::DefFamCtxSingleTupleChild::default();

        let ctx = BasicConfig { value: 1 };

        <<DefFamCtxSingleTupleFamily as DefFamCtxSingleTupleRoot<u8, BasicConfig, (u8, u8)>>
            ::DefFamCtxSingleTupleChild as PurePluginModel<u8, BasicConfig, (u8, u8)>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_with_context_single_input_tuple_output_works() {
        assert_eq!(run_define_family_ctx_single_tuple(5), (5, 6));
        assert_eq!(run_define_family_ctx_single_tuple(1), (1, 2));
    }

    // With Context, Tuple Input, Single Output

    declare_family! {
        root: pub DefFamCtxTupleSingleRoot,
        child: [DefFamCtxTupleSingleChild]
    }

    define_family! {
        root: DefFamCtxTupleSingleRoot,
        family: DefFamCtxTupleSingleFamily,
        input: (InpA, InpB),
        output: Output,
        context: BasicConfig,
        bounds: [InpA: Into<u8>, InpB: Into<u8>, Output: From<u8>],
        child: [
            DefFamCtxTupleSingleChild => PureCtxTupleSingle,
        ],
    }

    fn run_define_family_ctx_tuple_single(input: (u8, u8)) -> u8 {
        let model = <DefFamCtxTupleSingleFamily as DefFamCtxTupleSingleRoot<
            (u8, u8),
            BasicConfig,
            u8,
        >>::DefFamCtxTupleSingleChild::default();

        let ctx = BasicConfig { value: 10 };

        <<DefFamCtxTupleSingleFamily as DefFamCtxTupleSingleRoot<(u8, u8), BasicConfig, u8>>
            ::DefFamCtxTupleSingleChild as PurePluginModel<(u8, u8), BasicConfig, u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_with_context_tuple_input_single_output_works() {
        assert_eq!(run_define_family_ctx_tuple_single((10, 10)), 30);
        assert_eq!(run_define_family_ctx_tuple_single((5, 30)), 45);
    }

    //--- With Context, Tuple Input, Tuple Output ---

    declare_family! {
        root: pub DefFamCtxTupleTupleRoot,
        child: [DefFamCtxTupleTupleChild]
    }

    define_family! {
        root: DefFamCtxTupleTupleRoot,
        family: DefFamCtxTupleTupleFamily,
        input: (InpA, InpB),
        output: (OutA, OutB),
        context: BasicConfig,
        bounds: [InpA: Into<u8>, InpB: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        child: [
            DefFamCtxTupleTupleChild => PureCtxTupleTuple,
        ],
    }

    fn run_define_family_ctx_tuple_tuple(input: (u8, u8)) -> (u8, u8) {
        let model = <DefFamCtxTupleTupleFamily as DefFamCtxTupleTupleRoot<
            (u8, u8),
            BasicConfig,
            (u8, u8),
        >>::DefFamCtxTupleTupleChild::default();

        let ctx = BasicConfig { value: 5 };

        <<DefFamCtxTupleTupleFamily as DefFamCtxTupleTupleRoot<(u8, u8), BasicConfig, (u8, u8)>>
            ::DefFamCtxTupleTupleChild as PurePluginModel<(u8, u8), BasicConfig, (u8, u8)>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_with_context_tuple_input_tuple_output_works() {
        assert_eq!(run_define_family_ctx_tuple_tuple((5, 10)), (10, 15));
        assert_eq!(run_define_family_ctx_tuple_tuple((1, 0)), (6, 5));
    }

    //--- No Context, Single Input, Single Output ---

    declare_family! {
        root: pub DefFamNoCtxSingleSingleRoot,
        child: [DefFamNoCtxSingleSingleChild]
    }

    define_family! {
        root: DefFamNoCtxSingleSingleRoot,
        family: DefFamNoCtxSingleSingleFamily,
        input: Input,
        output: Output,
        bounds: [Input: Into<u8>, Output: From<u8>],
        child: [
            DefFamNoCtxSingleSingleChild => PureNoCtxSingleSingle,
        ],
    }

    fn run_define_family_no_ctx_single_single(input: u8) -> u8 {
        let model =
            <DefFamNoCtxSingleSingleFamily as DefFamNoCtxSingleSingleRoot<u8, (), u8>>
                ::DefFamNoCtxSingleSingleChild::default();

        let ctx = ();

        <<DefFamNoCtxSingleSingleFamily as DefFamNoCtxSingleSingleRoot<u8, (), u8>>
            ::DefFamNoCtxSingleSingleChild as PurePluginModel<u8, (), u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_no_context_single_input_single_output_works() {
        assert_eq!(run_define_family_no_ctx_single_single(10), 11);
        assert_eq!(run_define_family_no_ctx_single_single(0), 1);
    }

    //--- No Context, Single Input, Tuple Output ---

    declare_family! {
        root: pub DefFamNoCtxSingleTupleRoot,
        child: [DefFamNoCtxSingleTupleChild]
    }

    define_family! {
        root: DefFamNoCtxSingleTupleRoot,
        family: DefFamNoCtxSingleTupleFamily,
        input: Input,
        output: (OutA, OutB),
        bounds: [Input: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        child: [
            DefFamNoCtxSingleTupleChild => PureNoCtxSingleTuple,
        ],
    }

    fn run_define_family_no_ctx_single_tuple(input: u8) -> (u8, u8) {
        let model = <DefFamNoCtxSingleTupleFamily as DefFamNoCtxSingleTupleRoot<
            u8,
            (),
            (u8, u8),
        >>::DefFamNoCtxSingleTupleChild::default();

        let ctx = ();

        <<DefFamNoCtxSingleTupleFamily as DefFamNoCtxSingleTupleRoot<u8, (), (u8, u8)>>
            ::DefFamNoCtxSingleTupleChild as PurePluginModel<u8, (), (u8, u8)>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_no_context_single_input_tuple_output_works() {
        assert_eq!(run_define_family_no_ctx_single_tuple(10), (10, 11));
        assert_eq!(run_define_family_no_ctx_single_tuple(0), (0, 1));
    }

    //--- No Context, Tuple Input, Single Output ---

    declare_family! {
        root: pub DefFamNoCtxTupleSingleRoot,
        child: [DefFamNoCtxTupleSingleChild]
    }

    define_family! {
        root: DefFamNoCtxTupleSingleRoot,
        family: DefFamNoCtxTupleSingleFamily,
        input: (InpA, InpB),
        output: Output,
        bounds: [InpA: Into<u8>, InpB: Into<u8>, Output: From<u8>],
        child: [
            DefFamNoCtxTupleSingleChild => PureNoCtxTupleSingle,
        ],
    }

    fn run_define_family_no_ctx_tuple_single(input: (u8, u8)) -> u8 {
        let model = <DefFamNoCtxTupleSingleFamily as DefFamNoCtxTupleSingleRoot<
            (u8, u8),
            (),
            u8,
        >>::DefFamNoCtxTupleSingleChild::default();

        let ctx = ();

        <<DefFamNoCtxTupleSingleFamily as DefFamNoCtxTupleSingleRoot<(u8, u8), (), u8>>
            ::DefFamNoCtxTupleSingleChild as PurePluginModel<(u8, u8), (), u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_no_context_tuple_input_single_output_works() {
        assert_eq!(run_define_family_no_ctx_tuple_single((10, 11)), 21);
        assert_eq!(run_define_family_no_ctx_tuple_single((0, 5)), 5);
    }

    //--- No Context, Tuple Input, Tuple Output ---

    declare_family! {
        root: pub DefFamNoCtxTupleTupleRoot,
        child: [DefFamNoCtxTupleTupleChild]
    }

    define_family! {
        root: DefFamNoCtxTupleTupleRoot,
        family: DefFamNoCtxTupleTupleFamily,
        input: (InpA, InpB),
        output: (OutA, OutB),
        bounds: [InpA: Into<u8>, InpB: Into<u8>, OutA: From<u8>, OutB: From<u8>],
        child: [
            DefFamNoCtxTupleTupleChild => PureNoCtxTupleTuple,
        ],
    }

    fn run_define_family_no_ctx_tuple_tuple(input: (u8, u8)) -> (u8, u8) {
        let model = <DefFamNoCtxTupleTupleFamily as DefFamNoCtxTupleTupleRoot<
            (u8, u8),
            (),
            (u8, u8),
        >>::DefFamNoCtxTupleTupleChild::default();

        let ctx = ();

        <<DefFamNoCtxTupleTupleFamily as DefFamNoCtxTupleTupleRoot<(u8, u8), (), (u8, u8)>>
            ::DefFamNoCtxTupleTupleChild as PurePluginModel<(u8, u8), (), (u8, u8)>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_no_context_tuple_input_tuple_output_works() {
        assert_eq!(run_define_family_no_ctx_tuple_tuple((5, 10)), (50, 100));
        assert_eq!(run_define_family_no_ctx_tuple_tuple((1, 0)), (10, 0));
    }

    //--- With Context + marker ---

    plugin_model! {
        name: pub PureGenericCtxSingleSingle,
        input: Input,
        output: Output,
        others: [T],
        context: GenericConfig<T>,
        bounds: [Input: Into<u8>, Output: From<u8>, T: Into<u8> + Clone],
        compute: |input, ctx| {
            Output::from(input.into() + ctx.value.clone().into())
        },
    }

    declare_family! {
        root: pub DefFamCtxMarkerRoot,
        child: [DefFamCtxMarkerChild]
    }

    define_family! {
        root: DefFamCtxMarkerRoot,
        family: DefFamCtxMarkerFamily,
        input: Input,
        output: Output,
        context: GenericConfig<T>,
        marker: [T],
        bounds: [Input: Into<u8>, Output: From<u8>, T: Into<u8> + Clone],
        child: [
            DefFamCtxMarkerChild => PureGenericCtxSingleSingle,
        ],
    }

    fn run_define_family_ctx_marker(input: u8) -> u8 {
        let model =
            <DefFamCtxMarkerFamily as DefFamCtxMarkerRoot<u8, GenericConfig<u8>, u8>>
                ::DefFamCtxMarkerChild::default();

        let ctx = GenericConfig { value: 7u8 };

        <<DefFamCtxMarkerFamily as DefFamCtxMarkerRoot<u8, GenericConfig<u8>, u8>>
            ::DefFamCtxMarkerChild as PurePluginModel<u8, GenericConfig<u8>, u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_with_context_marker_works() {
        assert_eq!(run_define_family_ctx_marker(10), 17);
        assert_eq!(run_define_family_ctx_marker(0), 7);
    }

    //--- With Context + borrow + marker ---

    plugin_model! {
        name: pub PureGenericBorrowCtx,
        input: Input,
        output: Output,
        others: [T],
        context: GenericConfig<T>,
        bounds: [Input: AsRef<[u8]>, Output: From<usize>, T: Clone],
        compute: |input, _ctx| {
            Output::from(input.as_ref().len())
        },
    }

    declare_family! {
        root: pub DefFamCtxBorrowMarkerRoot,
        child: [DefFamCtxBorrowMarkerChild]
    }

    define_family! {
        root: DefFamCtxBorrowMarkerRoot,
        family: DefFamCtxBorrowMarkerFamily,
        borrow: ['a],
        input: Input,
        output: Output,
        context: GenericConfig<T>,
        marker: [T],
        bounds: [Input: AsRef<[u8]> + 'a, Output: From<usize>, T: Clone],
        child: [
            DefFamCtxBorrowMarkerChild => PureGenericBorrowCtx,
        ],
    }

    fn run_define_family_ctx_borrow_marker<'a>(input: &'a [u8]) -> usize {
        let model = <DefFamCtxBorrowMarkerFamily<'a> as DefFamCtxBorrowMarkerRoot<
            &'a [u8],
            GenericConfig<u8>,
            usize,
        >>::DefFamCtxBorrowMarkerChild::default();

        let ctx = GenericConfig { value: 99u8 };

        <<DefFamCtxBorrowMarkerFamily<'a> as DefFamCtxBorrowMarkerRoot<&'a [u8], GenericConfig<u8>, usize>>
            ::DefFamCtxBorrowMarkerChild as PurePluginModel<&'a [u8], GenericConfig<u8>, usize>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_with_context_borrow_and_marker_works() {
        let data = [1u8, 2, 3, 4];
        assert_eq!(run_define_family_ctx_borrow_marker(&data), 4);

        let data = [9u8];
        assert_eq!(run_define_family_ctx_borrow_marker(&data), 1);
    }

    //--- No Context + multiple children ---

    declare_family! {
        root: pub DefFamNoCtxMultiChildRoot,
        child: [DefFamNoCtxMultiChildA, DefFamNoCtxMultiChildB]
    }

    define_family! {
        root: DefFamNoCtxMultiChildRoot,
        family: DefFamNoCtxMultiChildFamily,
        input: Input,
        output: Output,
        bounds: [Input: Into<u8>, Output: From<u8>],
        child: [
            DefFamNoCtxMultiChildA => PureNoCtxSingleSingle,
            DefFamNoCtxMultiChildB => PureNoCtxSingleSingle,
        ],
    }

    fn run_define_family_no_ctx_multi_child_a(input: u8) -> u8 {
        let model =
            <DefFamNoCtxMultiChildFamily as DefFamNoCtxMultiChildRoot<u8, (), u8>>
                ::DefFamNoCtxMultiChildA::default();

        let ctx = ();

        <<DefFamNoCtxMultiChildFamily as DefFamNoCtxMultiChildRoot<u8, (), u8>>
            ::DefFamNoCtxMultiChildA as PurePluginModel<u8, (), u8>>
            ::compute(&model, input, &ctx)
    }

    fn run_define_family_no_ctx_multi_child_b(input: u8) -> u8 {
        let model =
            <DefFamNoCtxMultiChildFamily as DefFamNoCtxMultiChildRoot<u8, (), u8>>
                ::DefFamNoCtxMultiChildB::default();

        let ctx = ();

        <<DefFamNoCtxMultiChildFamily as DefFamNoCtxMultiChildRoot<u8, (), u8>>
            ::DefFamNoCtxMultiChildB as PurePluginModel<u8, (), u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_multiple_children_work() {
        assert_eq!(run_define_family_no_ctx_multi_child_a(10), 11);
        assert_eq!(run_define_family_no_ctx_multi_child_b(0), 1);

        let _a = DefFamNoCtxMultiChildA;
        let _b = DefFamNoCtxMultiChildB;
    }

    //--- No Context + multiple children with distinct models ---

    plugin_model! {
        name: pub PureNoCtxSingleSingleDouble,
        input: Input,
        output: Output,
        bounds: [Input: Into<u8>, Output: From<u8>],
        compute: |input, _ctx| {
            let x = input.into();
            Output::from(x * 2)
        }
    }

    declare_family! {
        root: pub DefFamNoCtxMultiChildDistinctRoot,
        child: [DefFamNoCtxMultiChildInc, DefFamNoCtxMultiChildDouble]
    }

    define_family! {
        root: DefFamNoCtxMultiChildDistinctRoot,
        family: DefFamNoCtxMultiChildDistinctFamily,
        input: Input,
        output: Output,
        bounds: [Input: Into<u8>, Output: From<u8>],
        child: [
            DefFamNoCtxMultiChildInc => PureNoCtxSingleSingle,
            DefFamNoCtxMultiChildDouble => PureNoCtxSingleSingleDouble,
        ],
    }

    fn run_define_family_no_ctx_multi_child_inc(input: u8) -> u8 {
        let model = <DefFamNoCtxMultiChildDistinctFamily as DefFamNoCtxMultiChildDistinctRoot<
            u8,
            (),
            u8,
        >>::DefFamNoCtxMultiChildInc::default();

        let ctx = ();

        <<DefFamNoCtxMultiChildDistinctFamily as DefFamNoCtxMultiChildDistinctRoot<u8, (), u8>>
            ::DefFamNoCtxMultiChildInc as PurePluginModel<u8, (), u8>>
            ::compute(&model, input, &ctx)
    }

    fn run_define_family_no_ctx_multi_child_double(input: u8) -> u8 {
        let model = <DefFamNoCtxMultiChildDistinctFamily as DefFamNoCtxMultiChildDistinctRoot<
            u8,
            (),
            u8,
        >>::DefFamNoCtxMultiChildDouble::default();

        let ctx = ();

        <<DefFamNoCtxMultiChildDistinctFamily as DefFamNoCtxMultiChildDistinctRoot<u8, (), u8>>
            ::DefFamNoCtxMultiChildDouble as PurePluginModel<u8, (), u8>>
            ::compute(&model, input, &ctx)
    }

    #[test]
    fn define_family_multiple_children_with_distinct_models_work() {
        assert_eq!(run_define_family_no_ctx_multi_child_inc(10), 11);
        assert_eq!(run_define_family_no_ctx_multi_child_inc(0), 1);

        assert_eq!(run_define_family_no_ctx_multi_child_double(10), 20);
        assert_eq!(run_define_family_no_ctx_multi_child_double(3), 6);

        let _inc = DefFamNoCtxMultiChildInc;
        let _double = DefFamNoCtxMultiChildDouble;
    }
        
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` HELPER MACROS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn phantom_struct_no_lifetime_no_generic_compiles() {
        __phantom_struct!(pub Plain [] []);
        let _x = Plain;
    }

    #[test]
    fn phantom_struct_lifetime_only_compiles() {
        __phantom_struct!(pub WithLt ['a] []);
        let _x: WithLt<'static>;
    }

    #[test]
    fn phantom_struct_generic_only_compiles() {
        __phantom_struct!(pub WithGen [] [T]);
        let _x: WithGen<u8>;
    }

    #[test]
    fn phantom_struct_lifetime_and_generic_compiles() {
        __phantom_struct!(pub WithLtGen ['a] [T]);
        let _x: WithLtGen<'static, u8>;
    }

    #[test]
    fn phantom_struct_generic_is_covariant() {
        __phantom_struct!(pub CovGen [] [T]);
        // covariance: Foo<&'static str> can be used where Foo<&'short str> is expected
        fn accepts<'a>(_: CovGen<&'a str>) {}
        let x: CovGen<&'static str> = CovGen(PhantomData);
        accepts(x);
    }

    #[test]
    fn phantom_struct_lifetime_and_generic_is_covariant() {
        __phantom_struct!(pub CovLtGen ['a] [T]);
        fn accepts<'a>(_: CovLtGen<'a, &'a str>) {}
        let x: CovLtGen<'static, &'static str> = CovLtGen(PhantomData);
        accepts(x);
    }

    #[test]
    fn phantom_struct_field_types_are_correct() {
        // Arm 2: PhantomData<(&'a (),)>
        __phantom_struct!(pub LtField ['a] []);
        let _: LtField<'static> = LtField(PhantomData::<(&'static (),)>);

        // Arm 3: PhantomData<(T,)>
        __phantom_struct!(pub GenField [] [T]);
        let _: GenField<u8> = GenField(PhantomData::<(u8,)>);

        // Arm 4: PhantomData<(T, &'a ())> generics first, then lifetime references
        __phantom_struct!(pub LtGenField ['a] [T]);
        let _: LtGenField<'static, u8> = LtGenField(PhantomData::<(u8, &'static ())>);
    }
}
