use crate::attribute_helpers::contains_skip;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Fields, Ident, ItemEnum};

pub fn enum_ser(input: &ItemEnum) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let mut body = TokenStream::new();
    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_idx = variant_idx as u8;
        let variant_ident = &variant.ident;
        let mut variant_header = TokenStream::new();
        let mut variant_body = TokenStream::new();
        match &variant.fields {
            Fields::Named(fields) => {
                for field in &fields.named {
                    let field_name = field.ident.as_ref().unwrap();
                    if contains_skip(&field.attrs) {
                        variant_header.extend(quote! { _#field_name, });
                        continue;
                    } else {
                        variant_header.extend(quote! { #field_name, });
                    }
                    variant_body.extend(quote! {
                         oasis_borsh::BorshSerialize::serialize(#field_name, writer)?;
                    })
                }
                variant_header = quote! { { #variant_header }};
            }
            Fields::Unnamed(fields) => {
                for (field_idx, field) in fields.unnamed.iter().enumerate() {
                    let field_idx = field_idx as u32;
                    if contains_skip(&field.attrs) {
                        let field_ident =
                            Ident::new(format!("_id{}", field_idx).as_str(), Span::call_site());
                        variant_header.extend(quote! { #field_ident, });
                        continue;
                    } else {
                        let field_ident =
                            Ident::new(format!("id{}", field_idx).as_str(), Span::call_site());
                        variant_header.extend(quote! { #field_ident, });
                        variant_body.extend(quote! {
                            oasis_borsh::BorshSerialize::serialize(#field_ident, writer)?;
                        })
                    }
                }
                variant_header = quote! { ( #variant_header )};
            }
            Fields::Unit => {}
        }
        body.extend(quote!(
            #name::#variant_ident #variant_header => {
                let variant_idx: u8 = #variant_idx;
                writer.write_all(&variant_idx.to_le_bytes())?;
                #variant_body
            }
        ))
    }

    let generics = crate::util::add_ser_constraints(input.generics.clone());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics oasis_borsh::ser::BorshSerialize for #name #ty_generics #where_clause {
            fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::result::Result<(), std::io::Error> {
                match self {
                    #body
                }
                Ok(())
            }
        }
    })
}
