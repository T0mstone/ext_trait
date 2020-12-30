use super::Token;
use proc_macro2::{Ident, Span};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{
    ImplItem, ImplItemConst, ImplItemMacro, ImplItemMethod, ImplItemType, ItemImpl, ItemTrait,
    TraitItem, TraitItemConst, TraitItemMacro, TraitItemMethod, TraitItemType, Visibility,
};

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

        // at the time of writing this, all valid ImplItems are covered above
        i => unimplemented!("Unsupported item: {}", i.into_token_stream()),
    }
}

/// Make a trait out of the inherent impl
pub fn to_trait(i: ItemImpl, vis: Visibility, trait_ident: Ident) -> ItemTrait {
    ItemTrait {
        attrs: i.attrs,
        vis,
        unsafety: i.unsafety,
        auto_token: None,
        trait_token: Token![trait](Span::call_site()),
        ident: trait_ident,
        generics: i.generics,
        colon_token: None,
        supertraits: Punctuated::new(),
        brace_token: i.brace_token,
        items: i.items.into_iter().map(convert_item).collect(),
    }
}
