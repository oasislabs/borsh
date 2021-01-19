use proc_macro2::TokenStream;
use syn::ItemUnion;

pub fn union_ser(_input: &ItemUnion) -> syn::Result<TokenStream> {
    unimplemented!()
}
