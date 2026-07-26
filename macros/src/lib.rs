extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields};

/// Derive macro for the Diffable trait.
/// Generates an incremental patch structure and implements Diffable.
#[proc_macro_derive(Diffable, attributes(diff))]
pub fn derive_diffable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let patch_name = format_ident!("{}Patch", name);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Diffable derive only supports structs with named fields"),
        },
        _ => panic!("Diffable derive only supports structs"),
    };

    // 1. Generate patch struct fields definition
    let patch_fields_def = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_ty = &f.ty;
        let field_attrs = parse_field_attrs(f);

        if field_attrs.is_required {
            // required fields (like id) are always sent, so they are not Option
            quote! { pub #field_name: #field_ty }
        } else if field_attrs.is_immutable {
            // immutable fields are never sent in patch updates, but we define them as Option in the patch type
            quote! { pub #field_name: Option<#field_ty> }
        } else {
            // standard fields: resolve patch type using GetPatchType associated type
            quote! { pub #field_name: <#field_ty as ::notifiapp_protocol_core::diff::GetPatchType>::FieldPatch }
        }
    });

    // 2. Generate diff method implementation
    let diff_fields = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_ty = &f.ty;
        let field_attrs = parse_field_attrs(f);

        if field_attrs.is_required {
            quote! {
                #field_name: self.#field_name.clone()
            }
        } else if field_attrs.is_immutable {
            quote! {
                #field_name: None
            }
        } else {
            quote! {
                #field_name: <#field_ty as ::notifiapp_protocol_core::diff::GetPatchType>::resolve_diff(&self.#field_name, &new.#field_name)
            }
        }
    });

    // 3. Generate has_changes check: a diff patch is Some only if at least one non-required, non-immutable field changed.
    let has_changes_checks = fields.iter().filter_map(|f| {
        let field_name = &f.ident;
        let field_attrs = parse_field_attrs(f);

        if !field_attrs.is_required && !field_attrs.is_immutable {
            Some(quote! { patch.#field_name.is_some() })
        } else {
            None
        }
    }).collect::<Vec<_>>();

    let has_changes_expr = if has_changes_checks.is_empty() {
        quote! { false }
    } else {
        quote! { #( #has_changes_checks )||* }
    };

    // 4. Generate apply_patch method implementation
    let apply_fields = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_ty = &f.ty;
        let field_attrs = parse_field_attrs(f);

        if field_attrs.is_required {
            quote! {
                self.#field_name = patch.#field_name.clone();
            }
        } else if field_attrs.is_immutable {
            quote! {}
        } else {
            quote! {
                <#field_ty as ::notifiapp_protocol_core::diff::GetPatchType>::resolve_apply(&mut self.#field_name, &patch.#field_name);
            }
        }
    });

    // 5. Generate to_full_patch implementation
    let full_fields = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_ty = &f.ty;
        let field_attrs = parse_field_attrs(f);

        if field_attrs.is_required {
            quote! {
                #field_name: self.#field_name.clone()
            }
        } else if field_attrs.is_immutable {
            quote! {
                #field_name: None
            }
        } else {
            quote! {
                #field_name: <#field_ty as ::notifiapp_protocol_core::diff::GetPatchType>::resolve_full(&self.#field_name)
            }
        }
    });

    let expanded = quote! {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        pub struct #patch_name {
            #( #patch_fields_def, )*
        }

        impl ::notifiapp_protocol_core::diff::Diffable for #name {
            type Patch = #patch_name;

            fn diff(&self, new: &Self) -> Option<Self::Patch> {
                let patch = #patch_name {
                    #( #diff_fields, )*
                };

                if #has_changes_expr {
                    Some(patch)
                } else {
                    None
                }
            }

            fn apply_patch(&mut self, patch: &Self::Patch) {
                #( #apply_fields )*
            }

            fn to_full_patch(&self) -> Self::Patch {
                #patch_name {
                    #( #full_fields, )*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

struct FieldAttrs {
    is_required: bool,
    is_immutable: bool,
}

fn parse_field_attrs(field: &Field) -> FieldAttrs {
    let mut is_required = false;
    let mut is_immutable = false;

    for attr in &field.attrs {
        if attr.path().is_ident("diff") {
            // In syn 2.0, parse nested meta using parse_nested_meta helper
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("required") {
                    is_required = true;
                } else if meta.path.is_ident("immutable") {
                    is_immutable = true;
                }
                Ok(())
            });
        }
    }

    FieldAttrs {
        is_required,
        is_immutable,
    }
}
