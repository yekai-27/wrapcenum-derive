/*!
A quick and dirty derive macro for use in [nvml-wrapper](https://github.com/Cldfire/nvml-wrapper).
It is **not for use by the general public**, a status that may or may not change
in the future.
*/

#![recursion_limit = "1024"]

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use quote::Tokens;
use syn::MetaItem::*;
use syn::Lit::*;
use syn::Body::*;
use syn::NestedMetaItem::*;

// TODO: Tests.
// TODO: Maybe clean this up if I feel like it.

// THIS MUST BE USED TO WRAP ENUMS IN CONSTANT FORM
// See https://docs.rs/bindgen/0.25.1/bindgen/struct.Builder.html#method.constified_enum
// REASON: https://github.com/rust-lang/rust/issues/36927
//
// Use it like this
//
// #[derive(EnumWrapper)]
// #[wrap(c_enum = "libSomeEnum_t")]
// // This is used to map an unknown variant returned from C to a default value.
// #[wrap(default = "LIB_COUNT_VARIANT")] (optional)
// pub enum SomeEnum {
//     #[wrap(c_variant = LIB_SOME_VARIANT)]
//     SomeVariant,
//     #[wrap(c_variant = LIB_OTHER_VARIANT)]
//     OtherVariant,
// }

struct VariantInfo {
    rust_name: syn::Ident,
    rust_variant: syn::Ident,
    c_name: syn::Ident,
    c_variant: syn::Ident,
}

impl VariantInfo {
    fn from(variant: syn::Variant, c_name: syn::Ident, rust_name: syn::Ident) -> Self {
        let c_variant: syn::Ident = variant_attr_val_for_str("c_variant", &variant).into();

        VariantInfo {
            rust_name: rust_name,
            rust_variant: variant.ident,
            c_name: c_name,
            c_variant: c_variant,
        }
    }

    fn tokens_for_as_c(&self) -> Tokens {
        let ref rust_name = self.rust_name;
        let ref rust_variant = self.rust_variant;
        let ref c_name = self.c_name;
        let ref c_variant = self.c_variant;

        let c_joined = syn::Ident::new(c_name.to_string() + "_" + 
            &c_variant.to_string());

        quote! {
            #rust_name::#rust_variant => #c_joined,
        }
    }

    fn tokens_for_from_c(&self) -> Tokens {
        let ref rust_name = self.rust_name;
        let ref rust_variant = self.rust_variant;
        let ref c_name = self.c_name;
        let ref c_variant = self.c_variant;

        let c_joined = syn::Ident::new(c_name.to_string() + "_" + 
            &c_variant.to_string());

        quote! {
            #c_joined => #rust_name::#rust_variant,
        }
    }

    fn tokens_for_try_from_c(&self) -> Tokens {
        let ref rust_name = self.rust_name;
        let ref rust_variant = self.rust_variant;
        let ref c_name = self.c_name;
        let ref c_variant = self.c_variant;

        let c_joined = syn::Ident::new(c_name.to_string() + "_" + 
            &c_variant.to_string());

        quote! {
            #c_joined => Ok(#rust_name::#rust_variant),
        }
    }
}

#[proc_macro_derive(EnumWrapper, attributes(wrap))]
pub fn enum_wrapper(input: TokenStream) -> TokenStream {
    let source = input.to_string();
    let ast = syn::parse_derive_input(&source).expect("Could not parse derive input");

    let expanded = wrap_enum(ast);

    expanded.parse().expect("Could not parse expanded output")
}

fn wrap_enum(ast: syn::DeriveInput) -> Tokens {
    let rust_name = &ast.ident;
    let c_name: syn::Ident = attr_val_for_str("c_enum", &ast).unwrap().into();
    let default_variant = attr_val_for_str("default", &ast);

    match ast.body {
        Enum(variant_vec) => {
            let info_vec: Vec<VariantInfo> = variant_vec.iter().map(|v| {
                VariantInfo::from(v.clone(), c_name.clone(), rust_name.clone())
            }).collect();
            
            if let Some(v) = default_variant {
                gen_impl(&info_vec[..], Some(v.into()))
            } else {
                gen_impl(&info_vec[..], None)
            }
        },
        Struct(_) => panic!("This derive macro does not support structs"),
    }

}

fn gen_impl(variant_slice: &[VariantInfo], default_variant: Option<syn::Ident>) -> Tokens {
    let ref c_name = variant_slice[0].c_name;
    let ref rust_name = variant_slice[0].rust_name;

    let for_arms: Vec<Tokens> = variant_slice.iter().map(|v| {
        v.tokens_for_as_c()
    }).collect();

    let from_arms: Vec<Tokens> = variant_slice.iter().map(|v| {
        v.tokens_for_from_c()
    }).collect();

    let try_from_arms: Vec<Tokens> = variant_slice.iter().map(|v| {
        v.tokens_for_try_from_c()
    }).collect();

    if let Some(v) = default_variant {
        quote! {
            impl #rust_name {
                /// Returns the C enum variant equivalent for the given Rust enum variant.
                pub fn as_c(&self) -> #c_name {
                    match *self {
                        #(#for_arms)*
                    }
                }
            }

            impl From<#c_name> for #rust_name {
                fn from(enum_: #c_name) -> Self {
                    match enum_ {
                        #(#from_arms)*
                        _ => #c_name::#v
                    }
                }
            }
        }
    } else {
        quote! {
            impl #rust_name {
                /// Returns the C enum variant equivalent for the given Rust enum variant.
                pub fn as_c(&self) -> #c_name {
                    match *self {
                        #(#for_arms)*
                    }
                }

                /// Waiting for `TryFrom` to be stable. In the meantime, we do this.
                ///
                /// # Errors
                /// * `UnexpectedVariant`, for which you can read the docs for
                pub fn try_from(enum_: #c_name) -> Result<Self> {
                    match enum_ {
                        #(#try_from_arms)*
                        _ => Err(Error::from_kind(ErrorKind::UnexpectedVariant)),
                    }
                }
            }
        }
    }
}

 fn attr_val_for_str<S: AsRef<str>>(string: S, ast: &syn::DeriveInput) -> Option<String> {
    let mut return_string: Option<String> = None;
    // Iterate through attributes on this variant, match on the MetaItem
    ast.attrs.iter().find(|ref a| match a.value {
        // If this value is a List...
        List(ref ident, ref nested_items_vec) => {
            let mut real_return_val = false;
            // If the ident matches our derive's prefix...
            if ident == "wrap" {
                // Iterate through nested attributes in this attribute and match on NestedMetaItem...
                let item = nested_items_vec.iter().find(|ref i| match i {
                    // If it's another MetaItem
                    &&&MetaItem(ref item) => match item {
                        // If it's a name value pair
                        &NameValue(ref ident, ref lit) => {
                            let mut return_val = false;
                            // If the name matches what was passed in for us to look for
                            if ident == string.as_ref() {
                                // Match on the value paired with the name
                                return_string = match lit {
                                    // If it's a string, return it. Then go beg for mercy after
                                    // having read through this code.
                                    &Str(ref the_value, _) => Some(the_value.to_string()),
                                    _ => panic!("Attribute value was not a string")
                                };
                                return_val = true;
                            }
                            return_val
                        },
                        _ => panic!("Attribute was was not a namevalue"),
                    },
                    _ => false,
                });

                if let Some(_) = item {
                    real_return_val = true;
                }
            }
            real_return_val
        },
        _ => false,
    });

    return_string
}

// TODO: This should at least be cleaned up to be like the above
fn variant_attr_val_for_str<S: AsRef<str>>(string: S, variant: &syn::Variant) -> String {
    let mut return_string = "this_is_not_to_be_returned".to_string();
    variant.attrs.iter().find(|ref a| match a.value {
        List(ref ident, ref nested_items_vec) => {
            let mut real_return_val = false;
            if ident == "wrap" {
                let item = nested_items_vec.iter().find(|ref i| match i {
                    &&&MetaItem(ref item) => match item {
                        &NameValue(ref ident, ref lit) => {
                            let mut return_val = false;
                            if ident == string.as_ref() {
                                return_string = match lit {
                                    &Str(ref the_value, _) => the_value.to_string(),
                                    _ => panic!("Attribute value was not a string")
                                };
                                return_val = true;
                            }
                            return_val
                        },
                        _ => panic!("Attribute was was not a namevalue"),
                    },
                    _ => false,
                });

                if let Some(_) = item {
                    real_return_val = true;
                }
            }
            real_return_val
        },
        _ => false,
    });

    if return_string != "this_is_not_supposed_to_be_returned" {
        return_string
    } else {
        panic!("Could not find attribute for {:?}", string.as_ref())
    }
}
