//! # `ext_trait`
//! `ext_trait` is a procedural macro which provides you with a shortcut to the [extension trait pattern][1]
//!
//! [1]: https://github.com/rust-lang/rfcs/blob/master/text/0445-extension-trait-conventions.md
//! # Examples
//! - Simple
//! ```
//! use ext_trait::ext;
//!
//! // IntelliJ complains but it works
//! // No argument to the macro => A trait name is generated
//! #[ext]
//! impl u8 {
//!     fn foo(self) -> u8 { self + 1 }
//! }
//!
//! assert_eq!(1u8.foo(), 2);
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
//! #[ext(MyVecU8Ext)]
//! impl Vec<u8> {
//!     const FOO: usize = 1;
//!
//!     // IntelliJ complains about this one but it works
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
//! let v: Vec<u8> = vec![1, 2, 3];
//! assert_eq!(Vec::<u8>::FOO, 1);
//! let _assert_same_type: () = <usize as SameType<<Vec<u8> as MyVecU8Ext>::Foo>>::OK;
//! assert_eq!(v.foo(), 1);
//! assert_eq!(Vec::<u8>::BAR, 2);
//! ```
//!
//! - Generics
//! ```
//! use ext_trait::ext;
//!
//! #[ext(MyVecExt)]
//! impl<T> Vec<T>
//! where T: Eq
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
//! # Difference to similar crates
//! - [`easy_ext`](https://crates.io/crates/easy-ext) only supports methods and constants, not types and macro invokations; also, the implementation differs by a lot
//!     - to be fair, macro invokations are impossible to fully support with this pattern

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use quote::ToTokens;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, Expr, ExprPath, GenericArgument,
    GenericParam, Ident, ImplItem, ImplItemConst, ImplItemMacro, ImplItemMethod, ImplItemType,
    ItemImpl, ItemTrait, Path, PathArguments, PathSegment, Token, TraitItem, TraitItemConst,
    TraitItemMacro, TraitItemMethod, TraitItemType, Type, TypePath, VisPublic, Visibility,
};
// for some reason IntelliJ doesn't detect the other Token import so this is a quick fix
use syn::parse::Nothing;
#[allow(unused_imports)]
use syn::token::Token;

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

fn convert_method(m: ImplItemMethod) -> TraitItemMethod {
    TraitItemMethod {
        attrs: m.attrs,
        sig: m.sig,
        default: None,
        semi_token: Some(Token![;](Span::call_site())),
    }
}

fn convert_constant(c: ImplItemConst) -> TraitItemConst {
    TraitItemConst {
        attrs: c.attrs,
        const_token: c.const_token,
        ident: c.ident,
        colon_token: c.colon_token,
        ty: c.ty,
        default: None,
        semi_token: c.semi_token,
    }
}

fn convert_type(t: ImplItemType) -> TraitItemType {
    TraitItemType {
        attrs: t.attrs,
        type_token: t.type_token,
        ident: t.ident,
        generics: t.generics,
        colon_token: None,
        bounds: Punctuated::new(),
        default: None,
        semi_token: t.semi_token,
    }
}

fn convert_macro(m: ImplItemMacro) -> TraitItemMacro {
    TraitItemMacro {
        attrs: m.attrs,
        mac: m.mac,
        semi_token: m.semi_token,
    }
}

fn convert_item(i: ImplItem) -> TraitItem {
    match i {
        ImplItem::Const(c) => TraitItem::Const(convert_constant(c)),
        ImplItem::Method(m) => TraitItem::Method(convert_method(m)),
        ImplItem::Type(t) => TraitItem::Type(convert_type(t)),
        ImplItem::Macro(m) => TraitItem::Macro(convert_macro(m)),
        ImplItem::Verbatim(s) => TraitItem::Verbatim(s),
        i => unimplemented!("Unsupported item: {}", i.into_token_stream()),
    }
}

fn convert_impl(i: ItemImpl, vis: Visibility, ident: Ident) -> ItemTrait {
    ItemTrait {
        attrs: i.attrs,
        vis,
        unsafety: i.unsafety,
        auto_token: None,
        trait_token: Token![trait](Span::call_site()),
        ident,
        generics: i.generics,
        colon_token: None,
        supertraits: Punctuated::new(),
        brace_token: i.brace_token,
        items: i.items.into_iter().map(convert_item).collect(),
    }
}

fn convert_generic_params_to_args(p: GenericParam) -> GenericArgument {
    match p {
        GenericParam::Type(t) => GenericArgument::Type(Type::Path(TypePath {
            qself: None,
            path: ident_to_path(t.ident),
        })),
        GenericParam::Lifetime(l) => GenericArgument::Lifetime(l.lifetime),
        GenericParam::Const(c) => GenericArgument::Const(Expr::Path(ExprPath {
            attrs: Vec::new(),
            qself: None,
            path: ident_to_path(c.ident),
        })),
    }
}

fn postprocess_impl(item: &mut ItemImpl, mut path: Path) {
    // remove any `pub`
    for ii in &mut item.items {
        if let ImplItem::Method(m) = ii {
            m.vis = Visibility::Inherited;
        }
    }

    // insert the proper generic args
    // (the trait has all generic params too, i.e. `T<A, B>`, so we have to `impl<A, B> T<A, B> for ...`
    // the `<A, B>` from the `T<A, B>` in that last part is what is added here
    if let Some(s) = path.segments.last_mut() {
        s.arguments = PathArguments::AngleBracketed(AngleBracketedGenericArguments {
            colon2_token: None,
            lt_token: item
                .generics
                .lt_token
                .unwrap_or_else(|| Token![<](Span::call_site())),
            args: item
                .generics
                .params
                .clone()
                .into_iter()
                .map(convert_generic_params_to_args)
                .collect(),
            gt_token: item
                .generics
                .gt_token
                .unwrap_or_else(|| Token![>](Span::call_site())),
        })
    }

    // change it to a trait impl
    item.trait_ = Some((None, path, Token![for](Span::call_site())));
}

#[proc_macro_attribute]
pub fn ext(args: TokenStream, input: TokenStream) -> TokenStream {
    let id = hash(&input);
    let mut item = parse_macro_input!(input as ItemImpl);
    if item.trait_.is_some() {
        panic!("Only inherent impls can become an ext trait");
    }

    let args2 = args.clone();
    let name = match parse_macro_input!(args as Option<Ident>) {
        Some(id) => id,
        None => {
            let _ = parse_macro_input!(args2 as Nothing);
            Ident::new(&format!("__ExtTrait{}", id), Span::call_site())
        }
    };

    let trait_def = convert_impl(
        item.clone(),
        Visibility::Public(VisPublic {
            pub_token: Token![pub](Span::call_site()),
        }),
        name.clone(),
    );

    postprocess_impl(&mut item, ident_to_path(name));

    quote!(#trait_def #item).into()
}
