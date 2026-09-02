use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn causm_plugin(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let expanded = quote! {
        #input_fn

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn causm_plugin_alloc(len: u32) -> *mut u8 {
            ::causm_plugin_sdk::abi::alloc(len)
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn causm_plugin_dealloc(ptr: *mut u8, len: u32) {
            unsafe {
                ::causm_plugin_sdk::abi::dealloc(ptr, len);
            }
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn causm_plugin_transform(ptr: *mut u8, len: u32) -> u64 {
            unsafe {
                ::causm_plugin_sdk::abi::dispatch(ptr, len, #fn_name)
            }
        }
    };

    TokenStream::from(expanded)
}
