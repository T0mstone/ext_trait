use super::{ident_to_path, Token};
use proc_macro2::{Ident, Span};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{
    AngleBracketedGenericArguments, Expr, ExprPath, GenericArgument, GenericParam, ImplItem,
    ItemImpl, Path, PathArguments, Type, TypePath, Visibility, WhereClause, WherePredicate,
};

fn convert_generic_param_to_args(p: GenericParam) -> GenericArgument {
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

/// Make the inherent impl a trait impl
pub fn make_trait_impl(item: &mut ItemImpl, mut trait_ident_path: Path) {
    // remove any `pub`
    for ii in &mut item.items {
        match ii {
            ImplItem::Type(t) => t.vis = Visibility::Inherited,
            ImplItem::Const(c) => c.vis = Visibility::Inherited,
            ImplItem::Method(m) => m.vis = Visibility::Inherited,
            ImplItem::Macro(_) | ImplItem::Verbatim(_) => (),
            _ => unimplemented!("Unsupported item: {}", ii.to_token_stream()),
        }
    }

    // insert the proper generic args
    // (the trait has all generic params too, i.e. `T<A, B>`, so we have to `impl<A, B> T<A, B> for ...`
    // the `<A, B>` from the `T<A, B>` in that last part is what is added here
    if let Some(s) = trait_ident_path.segments.last_mut() {
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
                .map(convert_generic_param_to_args)
                .collect(),
            gt_token: item
                .generics
                .gt_token
                .unwrap_or_else(|| Token![>](Span::call_site())),
        })
    }

    // change it to a trait impl
    item.trait_ = Some((None, trait_ident_path, Token![for](Span::call_site())));
}

fn where_predicate_from_take_generic_bounds(g: &mut GenericParam) -> Option<WherePredicate> {
    use syn::{PredicateLifetime, PredicateType};

    match g {
        GenericParam::Type(t) => {
            if t.bounds.is_empty() {
                None
            } else {
                let pt = PredicateType {
                    lifetimes: None,
                    bounded_ty: Type::Path(TypePath {
                        qself: None,
                        path: ident_to_path(t.ident.clone()),
                    }),
                    colon_token: Token![:](Span::call_site()),
                    bounds: std::mem::take(&mut t.bounds),
                };
                Some(WherePredicate::Type(pt))
            }
        }
        GenericParam::Lifetime(l) => {
            if l.bounds.is_empty() {
                None
            } else {
                let pl = PredicateLifetime {
                    lifetime: l.lifetime.clone(),
                    colon_token: Token![:](Span::call_site()),
                    bounds: std::mem::take(&mut l.bounds),
                };
                Some(WherePredicate::Lifetime(pl))
            }
        }
        GenericParam::Const(_) => None,
    }
}

pub fn move_bounds_to_where_clause(item: &mut ItemImpl) {
    let where_clause = &mut item.generics.where_clause;

    for p in item.generics.params.iter_mut() {
        if let Some(wp) = where_predicate_from_take_generic_bounds(p) {
            if where_clause.is_none() {
                let mut predicates = Punctuated::new();
                predicates.push(wp);
                *where_clause = Some(WhereClause {
                    where_token: Token![where](Span::call_site()),
                    predicates,
                });
            } else {
                where_clause.as_mut().unwrap().predicates.push(wp);
            }
        }
    }
}

pub fn copy_appropriate_where_clause_type_from_and_to_self(item: &mut ItemImpl) {
    if let Some(c) = &mut item.generics.where_clause {
        let mut extra = Punctuated::<WherePredicate, Token![,]>::new();

        for p in c.predicates.iter_mut() {
            if let WherePredicate::Type(t) = p {
                if t.bounded_ty == *item.self_ty {
                    // make a copy and change the bounded type to `Self`
                    let mut t = t.clone();
                    t.bounded_ty = Type::Path(TypePath {
                        qself: None,
                        path: ident_to_path(Ident::new("Self", Span::call_site())),
                    });
                    extra.push(WherePredicate::Type(t));
                } else if let Type::Path(p) = &mut t.bounded_ty {
                    if let Some(seg) = p.path.segments.last_mut() {
                        if seg.ident == "Self" {
                            // make a copy and change the bounded type to the other form of `Self`
                            let mut t = t.clone();
                            t.bounded_ty = (*item.self_ty).clone();
                            extra.push(WherePredicate::Type(t));
                        }
                    }
                }
            }
        }

        c.predicates.extend(extra);
    }
}
