use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, Expr};

#[proc_macro_attribute]
pub fn shinkai_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = input.sig.ident.clone();
    let attrs = input.attrs.clone();
    let body = input.block.clone();

    let config_expr: Expr = syn::parse_quote!(shinkai_test_framework::TestConfig::default());

    let gen = quote! {
        #(#attrs)*
        #[test]
        fn #fn_name() {
            use shinkai_test_framework::{run_test_one_node_network, TestContext};
            let config = #config_expr;
            run_test_one_node_network(config, |ctx: TestContext| Box::pin(async move { #body }));
        }
    };
    gen.into()
}
