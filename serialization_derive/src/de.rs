use quote::quote;
use syn::export::TokenStream2 as TokenStream;
use syn::{Data, DeriveInput, Field, Index, Type};

pub fn impl_deserializable(ast: &DeriveInput) -> TokenStream {
	let body = match ast.data {
		Data::Struct(ref s) => s,
		_ => panic!("#[derive(Deserializable)] is only defined for structs."),
	};

	let stmts: Vec<_> = body
		.fields
		.iter()
		.enumerate()
		.map(|(i, field)| deserialize_field(i, field))
		.collect();

	let name = &ast.ident;

	let dummy_const = format_ident!("_IMPL_DESERIALIZABLE_FOR_{}", name);
	let impl_block = quote! {
		impl serialization::Deserializable for #name {
			fn deserialize<T>(reader: &mut serialization::Reader<T>) -> Result<Self, serialization::Error> where T: io::Read {
				let result = #name {
					#(#stmts)*
				};

				Ok(result)
			}
		}
	};

	quote! {
		#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
		const #dummy_const: () = {
			extern crate serialization;
			use std::io;
			#impl_block
		};
	}
}

fn deserialize_field(index: usize, field: &Field) -> TokenStream {
	match &field.ty {
		Type::Path(type_path) => {
			let ident = &type_path.path.segments.first().expect("there must be at least 1 segment").ident;
			if &ident.to_string() == "Vec" {
				match field.ident {
					Some(ref ident) => quote! { #ident: reader.read_list()?, },
					None => {
						let index = Index::from(index);
						quote! { #index: reader.read_list()?, }
					}
				}
			} else {
				match field.ident {
					Some(ref ident) => quote! { #ident: reader.read()?, },
					None => {
						let index = Index::from(index);
						quote! { #index: reader.read()?, }
					}
				}
			}
		}
		_ => panic!("serialization not supported"),
	}
}
