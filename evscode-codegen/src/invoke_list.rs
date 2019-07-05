use quote::quote;
use std::sync::atomic::{AtomicU64, Ordering};
use syn::{export::Span, IntSuffix, LitInt};

pub struct InvocationList {
	id: &'static str,
	counter: AtomicU64,
}
impl InvocationList {
	pub const fn new(id: &'static str) -> InvocationList {
		InvocationList { id, counter: AtomicU64::new(1) }
	}

	pub fn base_definitions(&self, payload_type: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
		let marker = self.marker_struct();
		quote! {
			struct #marker<T>(std::marker::PhantomData<T>);
			impl<T> evscode::internal::macros::InvocChain<T> for crate::#marker<[(); 0]> {
				type Payload = #payload_type;
				fn payload() -> Self::Payload {
					unreachable!()
				}
				default fn is_last() -> bool {
					true
				}
				default type Next = Self;
			}
		}
	}

	pub fn invoke(&self, payload: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
		let iid = self.counter.fetch_add(1, Ordering::Relaxed);
		let marker = self.marker_struct();
		let iid_lit = LitInt::new(iid, IntSuffix::None, Span::call_site());
		let prev_iid_lit = LitInt::new(iid - 1, IntSuffix::None, Span::call_site());
		quote! {
			impl<T> evscode::internal::macros::InvocChain<T> for crate::#marker<[(); #iid_lit]> {
				type Payload = <crate::#marker<[(); #prev_iid_lit]> as evscode::internal::macros::InvocChain<()>>::Payload;
				fn payload() -> Self::Payload {
					#payload
				}
				default fn is_last() -> bool {
					true
				}
				default type Next = Self;
			}
			impl evscode::internal::macros::InvocChain<()> for crate::#marker<[(); #prev_iid_lit]> {
				fn is_last() -> bool {
					false
				}
				type Next = crate::#marker<[(); #iid_lit]>;
			}
		}
	}

	pub fn payloads(&self) -> proc_macro2::TokenStream {
		let marker = self.marker_struct();
		quote! {
			evscode::internal::macros::collect_payloads::<crate::#marker<[(); 0]>>()
		}
	}

	fn marker_struct(&self) -> syn::Ident {
		syn::Ident::new(&format!("__evscode_invokelist_{}", self.id), Span::call_site())
	}
}
