use crate::attribute_helpers::{contains_initialize_with, contains_skip};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, ItemStruct};

pub fn struct_de(input: &ItemStruct) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let init_method = contains_initialize_with(&input.attrs)?;
    let return_value = match &input.fields {
        Fields::Named(fields) => {
            let mut body = TokenStream::new();
            for field in &fields.named {
                let field_name = field.ident.as_ref().unwrap();
                let delta = if contains_skip(&field.attrs) {
                    quote! {
                        #field_name: Default::default(),
                    }
                } else {
                    quote! {
                        #field_name: oasis_borsh::BorshDeserialize::deserialize(reader)?,
                    }
                };
                body.extend(delta);
            }
            quote! {
                Self { #body }
            }
        }
        Fields::Unnamed(fields) => {
            let mut body = TokenStream::new();
            for _ in 0..fields.unnamed.len() {
                let delta = quote! {
                    oasis_borsh::BorshDeserialize::deserialize(reader)?,
                };
                body.extend(delta);
            }
            quote! {
                Self( #body )
            }
        }
        Fields::Unit => {
            quote! {
                Self {}
            }
        }
    };

    let generics = crate::util::add_de_constraints(input.generics.clone());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    if let Some(method_ident) = init_method {
        Ok(quote! {
            impl #impl_generics oasis_borsh::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize<R: std::io::Read>(reader: &mut R) -> std::result::Result<Self, std::io::Error> {
                    let mut return_value = #return_value;
                    return_value.#method_ident();
                    Ok(return_value)
                }
            }
        })
    } else {
        Ok(quote! {
            impl #impl_generics oasis_borsh::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize<R: std::io::Read>(reader: &mut R) -> std::result::Result<Self, std::io::Error> {
                    Ok(#return_value)
                }
            }
        })
    }
}
