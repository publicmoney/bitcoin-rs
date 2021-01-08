extern crate syn;
#[macro_use]
extern crate quote;

mod de;
mod ser;

use de::impl_deserializable;
use proc_macro::TokenStream;
use ser::impl_serializable;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Serializable)]
pub fn serializable(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let token_stream = impl_serializable(&ast);
	TokenStream::from(token_stream)
}

#[proc_macro_derive(Deserializable)]
pub fn deserializable(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let token_stream = impl_deserializable(&ast);
	TokenStream::from(token_stream)
}
