//! # `ext_trait`
//! `ext_trait` is a procedural macro which provides you with a shortcut to the [extension trait pattern][1]
//!
//! [1]: https://github.com/rust-lang/rfcs/blob/master/text/0445-extension-trait-conventions.md
//! # Examples
//! Note that IntelliJ Rust really dislikes this macro, i.e. displays errors even if there are none.
//! This stems from the plugin's linter's limited understanding of procedural macros.
//!
//! - Simple
//! ```
//! use ext_trait::ext;
//!
//! // No argument to the macro => A trait name is generated
//! #[ext]
//! impl u8 {
//!     fn foo(self) -> u8 { self + 1 }
//! }
//!
//! // this controls the visibility of the generated trait (you can also provide `pub(crate)` etc.)
//! #[ext(pub)]
//! impl u16 {
//!     fn bar(self) -> u16 { self + 2 }
//! }
//!
//! assert_eq!(1u8.foo(), 2);
//! assert_eq!(1u16.bar(), 3);
//! ```
//!
//! - One large example
//! ```
//! use ext_trait::ext;
//!
//! macro_rules! foo {
//!     () => { const BAR: usize = 2; }
//! }
//!
//! trait SameType<T> {
//!     const OK: () = ();
//! }
//! impl<T> SameType<T> for T {}
//!
//! // The argument is the name of the generated trait
//! #[ext(MyVecU8Ext)] // this trait is private (no `pub`)
//! impl Vec<u8> {
//!     const FOO: usize = 1;
//!
//!     type Foo = usize;
//!
//!     fn foo(&self) -> usize { 1 }
//!
//!     // this specific case works as of now
//!     // but macros are expanded in the trait *and* in the impl
//!     // which can lead to problems
//!     foo!();
//! }
//!
//! // named ext traits can also be public
//! #[ext(pub MyVecU16Ext)]
//! impl Vec<u16> {
//!     const FOO: usize = 2;
//! }
//!
//! let v: Vec<u8> = vec![1, 2, 3];
//! assert_eq!(Vec::<u8>::FOO, 1);
//! let _assert_same_type: () = <usize as SameType<<Vec<u8> as MyVecU8Ext>::Foo>>::OK;
//! assert_eq!(v.foo(), 1);
//! assert_eq!(Vec::<u8>::BAR, 2);
//! assert_eq!(Vec::<u16>::FOO, 2);
//! ```
//!
//! - Generics
//! ```
//! use ext_trait::ext;
//!
//! #[ext(MyVecExt)]
//! impl<T: Clone> Vec<T>
//! where T: Eq, (): Copy
//! {
//!     pub fn second(&self) -> Option<&T> {
//!         self.get(1)
//!     }
//! }
//!
//! let mut v = vec![1];
//! assert_eq!(v.second(), None);
//! v.push(2);
//! assert_eq!(v.second(), Some(&2));
//! ```
//!
//! # Comparison to similar crates
//! - [`easy_ext`](https://crates.io/crates/easy-ext) only supports methods and constants, not types and macro invokations; also, the implementation is different
//!     - to be fair, macro invokations are impossible to fully support with this pattern (as far as I can see)
//!
//! # Quirks
//! - The generated trait doesn't retain implicit trait bounds, specifically impls for (implicitly) `Sized` types are not
//!    converted into traits that require `Self: Sized`
//!     - Mostly, this leads to no problem since the type is often either explicitly `?Sized` or
//!         the ext trait only gets implemented for sized types
//!     - In case of any problems, just add a `where Self: Sized` bound to the impl and all is good (see example below)
//! - Because the random trait names are created using hashing of the input, there is a tiny chance of a collision.
//!     - In that case, you can define a macro that expands to nothing and insert it into the impl. That should shake up the hash a bit.
//!
//! ## Example: Fixing `Sized`-Issue
//! The following code will not compile:
//! ```compile_fail
//! use std::marker::PhantomData;
//! use ext_trait::ext;
//!
//! pub struct AssertSized<T>(PhantomData<T>);
//!
//! #[ext]
//! impl<T> T {
//!     fn foo(self) -> AssertSized<Self> { AssertSized(PhantomData) }
//! }
//! ```
//!
//! Fixed code:
//! ```no_run
//! use std::marker::PhantomData;
//! use ext_trait::ext;
//!
//! pub struct AssertSized<T>(PhantomData<T>);
//!
//! #[ext]
//! impl<T> T
//!     where Self: Sized
//! {
//!     fn foo(self) -> AssertSized<Self> { AssertSized(PhantomData) }
//! }
//! ```
//!
//! Alternative Fix (in this case):
//! ```no_run
//! use std::marker::PhantomData;
//! use ext_trait::ext;
//!
//! pub struct AssertSized<T>(PhantomData<T>);
//!
//! #[ext]
//! impl<T: Sized> T {
//!     fn foo(self) -> AssertSized<Self> { AssertSized(PhantomData) }
//! }
//! ```
//!
//! Note also that something like `#[ext] impl<T> [T] where Self: Sized { … }` will compile, but won't do anything since `[T]` is never `Sized`.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, Ident, ItemImpl, Path, PathArguments, PathSegment, Token, Visibility,
};
// for some reason IntelliJ doesn't detect the other Token import so this is a quick fix
#[allow(unused_imports)]
use syn::token::Token;

mod impl_to_trait;
mod process_impl;

fn hash(input: &TokenStream) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(input.to_string().as_bytes());
    hasher.finish()
}

fn ident_to_path(ident: Ident) -> Path {
    let mut segments = Punctuated::new();
    segments.push(PathSegment {
        ident,
        arguments: PathArguments::None,
    });

    Path {
        leading_colon: None,
        segments,
    }
}

struct ExtArgs {
    pub vis: Visibility,
    ident: Option<Ident>,
}

impl ExtArgs {
    pub fn trait_ident(&self, input_hash: u64) -> Ident {
        self.ident
            .clone()
            .unwrap_or_else(|| Ident::new(&format!("__ExtTrait{}", input_hash), Span::call_site()))
    }
}

impl Parse for ExtArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ExtArgs {
            vis: input.parse()?,
            ident: input.parse()?,
        })
    }
}

#[proc_macro_attribute]
pub fn ext(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_hash = hash(&input);

    let mut item = parse_macro_input!(input as ItemImpl);
    if item.trait_.is_some() {
        panic!("Only inherent impls can become an ext trait");
    }
    process_impl::move_bounds_to_where_clause(&mut item);

    let args = parse_macro_input!(args as ExtArgs);
    let name = args.trait_ident(input_hash);

    process_impl::make_trait_impl(&mut item, ident_to_path(name.clone()));
    process_impl::copy_appropriate_where_clause_type_from_and_to_self(&mut item);

    let trait_def = impl_to_trait::to_trait(item.clone(), args.vis, name);

    quote!(#trait_def #item).into()
}
