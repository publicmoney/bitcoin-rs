use quote::quote;
use syn::export::TokenStream2 as TokenStream;
use syn::{Data, DeriveInput, Field, Index, Type};

pub fn impl_serializable(ast: &DeriveInput) -> TokenStream {
	let body = match ast.data {
		Data::Struct(ref s) => s,
		_ => panic!("#[derive(Serializable)] is only defined for structs."),
	};

	let stmts: Vec<_> = body.fields.iter().enumerate().map(|(i, field)| serialize_field(i, field)).collect();

	let size_stmts: Vec<_> = body
		.fields
		.iter()
		.enumerate()
		.map(|(i, field)| serialize_field_size(i, field))
		.collect();

	let name = &ast.ident;

	let dummy_const = format_ident!("_IMPL_SERIALIZABLE_FOR_{}", name);

	let impl_block = quote! {
		impl serialization::Serializable for #name {
			fn serialize(&self, stream: &mut serialization::Stream) {
				#(#stmts)*
			}

			fn serialized_size(&self) -> usize {
				#(#size_stmts)+*
			}
		}
	};

	quote! {
		#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
		const #dummy_const: () = {
			extern crate serialization;
			#impl_block
		};
	}
}

fn serialize_field_size(index: usize, field: &Field) -> TokenStream {
	match &field.ty {
		Type::Path(type_path) => {
			let ident = &type_path.path.segments.first().expect("there must be at least 1 segment").ident;
			if &ident.to_string() == "Vec" {
				match field.ident {
					Some(ref ident) => quote! { serialization::serialized_list_size(&self.#ident) },
					None => {
						let index = Index::from(index);
						quote! { serialization::serialized_list_size(&self.#index) }
					}
				}
			} else {
				match field.ident {
					Some(ref ident) => quote! { self.#ident.serialized_size() },
					None => {
						let index = Index::from(index);
						quote! { self.#index.serialized_size() }
					}
				}
			}
		}
		_ => panic!("serialization not supported"),
	}
}

fn serialize_field(index: usize, field: &syn::Field) -> TokenStream {
	match &field.ty {
		Type::Path(type_path) => {
			let ident = &type_path.path.segments.first().expect("there must be at least 1 segment").ident;
			if ident.to_string() == "Vec" {
				match field.ident {
					Some(ref ident) => quote! { stream.append_list(&self.#ident); },
					None => {
						let index = Index::from(index);
						quote! { stream.append_list(&self.#index); }
					}
				}
			} else {
				match field.ident {
					Some(ref ident) => quote! { stream.append(&self.#ident); },
					None => {
						let index = Index::from(index);
						quote! { stream.append(&self.#index); }
					}
				}
			}
		}
		_ => panic!("serialization not supported"),
	}
}
