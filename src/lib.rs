//! # `ext_trait`
//! `ext_trait` is a procedural macro which provides you with a shortcut to the common pattern of
//! defining a custom trait to extend some type
//!
//! # Examples
//! - One large example
//! ```
//! use ext_trait::ext_trait;
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
//! #[ext_trait(MyVecU8Ext)]
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
//! use ext_trait::ext_trait;
//!
//! #[ext_trait(MyVecExt)]
//! impl<T> Vec<T> {
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

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, Expr, ExprPath, GenericArgument,
    GenericParam, Ident, ImplItem, ImplItemConst, ImplItemMacro, ImplItemMethod, ImplItemType,
    ItemImpl, ItemTrait, Path, PathArguments, PathSegment, Token, TraitItem, TraitItemConst,
    TraitItemMacro, TraitItemMethod, TraitItemType, Type, TypePath, VisPublic, Visibility,
};
// for some reason IntelliJ doesn't detect the other Token import so this is a quick fix
#[allow(unused_imports)]
use syn::token::Token;

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

fn convert_method(m: ImplItemMethod, sp: Span) -> TraitItemMethod {
    TraitItemMethod {
        attrs: m.attrs,
        sig: m.sig,
        default: None,
        semi_token: Some(Token![;](sp)),
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

fn convert_item(i: ImplItem, sp: Span) -> TraitItem {
    match i {
        ImplItem::Const(c) => TraitItem::Const(convert_constant(c)),
        ImplItem::Method(m) => TraitItem::Method(convert_method(m, sp)),
        ImplItem::Type(t) => TraitItem::Type(convert_type(t)),
        ImplItem::Macro(m) => TraitItem::Macro(convert_macro(m)),
        ImplItem::Verbatim(s) => TraitItem::Verbatim(s),
        i => unimplemented!("Unsupported item: {}", i.into_token_stream()),
    }
}

fn convert_impl(i: ItemImpl, sp: Span, vis: Visibility, ident: Ident) -> ItemTrait {
    ItemTrait {
        attrs: i.attrs,
        vis,
        unsafety: i.unsafety,
        auto_token: None,
        trait_token: Token![trait](sp),
        ident,
        generics: i.generics,
        colon_token: None,
        supertraits: Punctuated::new(),
        brace_token: i.brace_token,
        items: i.items.into_iter().map(|ii| convert_item(ii, sp)).collect(),
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

fn postprocess_impl(item: &mut ItemImpl, sp: Span, mut path: Path) {
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
            lt_token: item.generics.lt_token.unwrap_or_else(|| Token![<](sp)),
            args: item
                .generics
                .params
                .clone()
                .into_iter()
                .map(convert_generic_params_to_args)
                .collect(),
            gt_token: item.generics.gt_token.unwrap_or_else(|| Token![>](sp)),
        })
    }

    // change it to a trait impl
    item.trait_ = Some((None, path, Token![for](sp)));
}

#[proc_macro_attribute]
pub fn ext_trait(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as ItemImpl);
    if item.trait_.is_some() {
        panic!("Only inherent impls can become an ext trait");
    }

    let name = parse_macro_input!(args as Ident);
    let span = name.span();

    let trait_def = convert_impl(
        item.clone(),
        span,
        Visibility::Public(VisPublic {
            pub_token: Token![pub](span),
        }),
        name.clone(),
    );

    postprocess_impl(&mut item, span, ident_to_path(name));

    quote!(#trait_def #item).into()
}
