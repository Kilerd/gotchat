use darling::FromMeta;
use proc_macro::TokenStream;
use quote::__private::ext::RepToTokensExt;
use quote::quote;
use syn::{AttributeArgs, ItemFn, Lit, LitStr, Meta, parse_macro_input};
use route::{HttpMethod, RouteMeta};
use proc_macro_error::{proc_macro_error, abort};

mod route;
mod parameter;

macro_rules! handler {
    ($name:tt, $method: expr) => {
        #[proc_macro_attribute]
        pub fn $name(args: TokenStream, input_stream: TokenStream) -> TokenStream {
            route::request_handler($method, args, input_stream)
        }
    }
}
handler!(get, HttpMethod::Get);
handler!(post, HttpMethod::Post);
handler!(put, HttpMethod::Put);
handler!(patch, HttpMethod::Patch);
handler!(delete, HttpMethod::Delete);
handler!(options, HttpMethod::Options);
handler!(connect, HttpMethod::Connect);
handler!(head, HttpMethod::Head);
handler!(trace, HttpMethod::Trace);



#[proc_macro_derive(Parameter, attributes(parameter))]
#[proc_macro_error]
pub fn derive_parameter(input: TokenStream) -> TokenStream {
    let stream2 = proc_macro2::TokenStream::from(input);
    match parameter::handler(stream2) {
        Ok(stream) => proc_macro::TokenStream::from(stream),
        Err((span, msg)) => abort! {span, msg}
    }
}
