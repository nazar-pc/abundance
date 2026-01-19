//! See and use `ab-riscv-macros` crate instead, this is its implementation detail

mod instruction;
mod instruction_execution;

use proc_macro::TokenStream;

/// Processes `#[instruction]` attribute on both enum definitions and implementations.
///
/// # Enum definition
///
/// When applied to the enum definition, it can be used as simply `#[instruction]` to make an enum
/// with instructions available for inheritance.
///
/// More complex syntax is used when inheriting instructions:
/// ```rust,ignore
/// #[instruction(
///     reorder = [C, Add],
///     inherit = [BaseInstruction],
///     reorder = [D, A],
/// )]
/// struct Extended<Reg> {
///     A(Reg),
///     B(Reg),
///     C(Reg),
///     D(Reg),
/// }
/// ```
///
/// This will generate an enum with both `BaseInstruction` and `Extended` instructions, while also
/// reordering them according to the specified order. So the eventual enum will look like this:
/// ```rust,ignore
/// struct Extended<Reg> {
///     C(Reg),
///     Add { rd: Reg, rs1: Reg, rs2: Reg },
///     // Any other instructions from `BaseInstruction` that were not mentioned explicitly
///     D(Reg),
///     A(Reg),
///     B(Reg),
/// }
/// ```
///
/// Note that both `reorder` and `inherit` attributes can be specified multiple times, and
/// reordering can reference any variant from both the `BaseInstruction` and `Extended` enums.
///
/// This, of course, only works when enums have compatible generics.
///
/// All instruction enums in the project must have unique names. Individual instructions can be
/// repeated between enums, but they must have the same exact variant definition and are assumed to
/// be 100% compatible. Instructions of inherited enums that do not have an explicit position using
/// `reorder` will be placed at the relative position of the enum reference in the `inherit` list.
/// Own instruction variants that do not have an explicit position will be placed at the end of the
/// enum.
///
/// # Enum implementation
///
/// For enum implementations, the macro is applied to the implementation of `Instruction` trait and
/// affects its `try_decode()` method:
/// ```rust,ignore
/// #[instruction]
/// impl<Reg> const Instruction for Rv64MInstruction<Reg>
/// where
///     Reg: [const] Register<Type = u64>,
/// {
/// ```
///
/// `try_decode()` implementation will end up containing decoding logic for the full extended enum
/// as mentioned above. The two major restrictions are that `return` is not allowed in the
/// `try_decode()` method and enum variants must be constructed using `Self::`. The implementation
/// is quite fragile, so if you're calling internal functions, they might have to be re-exported
/// since the macro will simply copy-paste the decoding logic as is. Similarly with missing imports,
/// etc. Compiler should be able to guide you through errors reasonably well.
///
/// # `process_instruction_macros()`
///
/// What this macro "does" is impossible to do in Rust macros. So for completeness,
/// `ab_riscv_macros::process_instruction_macros()` must be called from `build.rs` in a
/// crate that uses `#[instruction]` macro to generate a bunch of special filed, which the macro
/// uses to replace the original code with. This is the only way to get the desired ergonomics
/// withing current constraints of what macros are allowed to do.
///
/// # [package.links]
///
/// `package` section of `Cargo.toml` must contain `links = "crate-name"` in order for metadata to
/// be successfully exported to dependent crates.
#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    instruction::instruction(attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Processes `#[instruction_execution]` attribute on both enum execution implementations.
///
/// It must be applied to enum, whose definition is already annotated with `#[instruction]` macro.
///
/// Similarly to that macro, this macro will process the contents of the `ExecutableInstruction`
/// trait implementation. `execute()` implementation will end up containing both inherited and own
/// execution logic according to the ordering set in `#[instruction]`.
///
/// There are constraints on the `execute()` method body, it must have one or both (but nothing
/// else) of the following:
/// * matching in the following style: `match self { Self::Variant { .. } }`
///   * note that `Self` must be used instead of the explicit type name, such that it works when
///     inherited
/// * `Ok(ControlFlow::Continue(()))` expression
#[proc_macro_attribute]
pub fn instruction_execution(attr: TokenStream, item: TokenStream) -> TokenStream {
    instruction_execution::instruction_execution(attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
