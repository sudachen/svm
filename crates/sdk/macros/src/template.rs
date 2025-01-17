use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{Error, Item, ItemMod, ItemType, ItemUse, Result};

use super::{function, r#struct};
use crate::{json, meta, Function, Struct, TemplateMeta};

use r#function::{func_attrs, has_default_fundable_hook_attr};
use r#struct::has_storage_attr;

pub struct Template {
    name: Ident,
    functions: Vec<Function>,
    structs: Vec<Struct>,
    imports: Vec<ItemUse>,
    aliases: Vec<ItemType>,
    default_fundable_hook: Option<Ident>,
}

impl Template {
    pub fn name(&self) -> String {
        self.name.to_string()
    }

    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    pub fn structs(&self) -> &[Struct] {
        &self.structs
    }

    pub fn imports(&self) -> &[ItemUse] {
        &self.imports
    }

    pub fn aliases(&self) -> &[ItemType] {
        &self.aliases
    }

    pub fn default_fundable_hook(&self) -> Option<Ident> {
        self.default_fundable_hook.clone()
    }

    pub fn set_default_fundable_hook(&mut self, hook: Ident) {
        self.default_fundable_hook = Some(hook)
    }
}

pub fn expand(_args: TokenStream, input: TokenStream) -> Result<(TokenStream, TemplateMeta)> {
    let module = syn::parse2(input)?;
    let template = parse_template(module)?;
    let meta = meta::template_meta(&template)?;

    let _imports = template.imports();
    let _aliases = template.aliases();

    let structs = expand_structs(&template)?;
    let functions = expand_functions(&template)?;
    let verify_export = export_verify_ast();
    let alloc_export = export_alloc_ast();

    let ast = quote! {
        #verify_export

        #alloc_export

        #structs

        #functions
    };

    Ok((ast, meta))
}

pub fn parse_template(mut raw_template: ItemMod) -> Result<Template> {
    let name = raw_template.ident.clone();

    let mut functions = Vec::new();
    let mut structs = Vec::new();
    let mut imports = Vec::new();
    let mut aliases = Vec::new();

    let (_, content) = raw_template.content.take().unwrap();

    for item in content {
        // TODO: Is is possible to extract the `item` real `Span`?
        // See <https://github.com/spacemeshos/svm/issues/278>.
        let span = Span::call_site();

        match item {
            Item::Fn(item) => {
                let func = Function::new(item, functions.len());
                functions.push(func);
            }
            Item::Struct(item) => {
                let strukt = Struct::new(item);
                structs.push(strukt);
            }
            Item::Use(item) => imports.push(item),
            Item::Type(item) => aliases.push(item),
            Item::Const(..) => {
                let msg = "declaring `const` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Enum(..) => {
                let msg = "declaring `enum` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::ExternCrate(..) => {
                let msg = "using `extern crate` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::ForeignMod(..) => {
                let msg =
                    "using foreign items such as `extern \"C\"` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Impl(..) => {
                let msg = "using `impl` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Macro(..) => {
                let msg = "declaring `macro_rules!` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Macro2(..) => {
                let msg = "declaring `macro` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Mod(..) => {
                let msg = "declaring new modules inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Static(..) => {
                let msg = "declaring new `static` items inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Trait(..) => {
                let msg = "declaring new traits inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::TraitAlias(..) => {
                let msg = "using trait aliases inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Union(..) => {
                let msg = "declaring `union` inside `#[template]` is not supported.";
                return Err(Error::new(span, msg));
            }
            Item::Verbatim(item) => {
                let msg = format!("invalid Rust code: {}", item);
                return Err(Error::new(span, msg));
            }
            Item::__TestExhaustive(..) => unreachable!(),
        }
    }

    let mut template = Template {
        name,
        functions,
        structs,
        imports,
        aliases,
        default_fundable_hook: None,
    };

    let default = extract_default_fundable_hook(&template)?;

    if default.is_some() {
        template.set_default_fundable_hook(default.unwrap());
    }

    Ok(template)
}

fn extract_default_fundable_hook(template: &Template) -> Result<Option<Ident>> {
    let span = Span::call_site();
    let mut seen_default_fundable_hook = false;
    let mut default = None;

    for func in template.functions().iter() {
        let attrs = func_attrs(func).unwrap();

        if has_default_fundable_hook_attr(&attrs) {
            if seen_default_fundable_hook {
                return Err(Error::new(
                    span,
                    "There can be only a single default `fundable hook`",
                ));
            }

            seen_default_fundable_hook = true;

            default = Some(func.raw_name());
        }
    }

    Ok(default)
}

fn expand_structs(template: &Template) -> Result<TokenStream> {
    let mut structs = Vec::new();

    validate_structs(template)?;

    for strukt in template.structs() {
        let strukt = r#struct::expand(strukt)?;

        structs.push(strukt);
    }

    let ast = quote! {
        #(#structs)*
    };

    Ok(ast)
}

fn validate_structs(template: &Template) -> Result<()> {
    let mut seen_storage = false;

    for strukt in template.structs() {
        match strukt.attrs() {
            Ok(attrs) => {
                if has_storage_attr(attrs) {
                    if seen_storage {
                        let msg = format!("A Template can have only a single `#[storage]`");
                        let span = Span::call_site();

                        return Err(Error::new(span, msg));
                    }

                    seen_storage = true;
                }
            }
            Err(err) => return Err(err.clone()),
        }
    }

    Ok(())
}

fn expand_functions(template: &Template) -> Result<TokenStream> {
    let mut funcs = Vec::new();

    for func in template.functions() {
        let func = function::expand(func, template)?;

        funcs.push(func);
    }

    let implicit_fundable_hook = if template.default_fundable_hook().is_some() {
        quote! {}
    } else {
        function::fundable_hook::expand_default()?
    };

    let ast = quote! {
        #(#funcs)*

        #implicit_fundable_hook
    };

    Ok(ast)
}

fn export_alloc_ast() -> TokenStream {
    quote! {
        // using the `Allocator` of `svm_sdk`
        extern crate svm_sdk;

        #[no_mangle]
        pub extern "C" fn svm_alloc(size: u32) -> u32 {
            let ptr = svm_sdk::alloc(size as usize);
            ptr.offset() as u32
        }
    }
}

fn export_verify_ast() -> TokenStream {
    quote! {
        #[no_mangle]
        pub extern "C" fn svm_verify() -> u32 {
            // TODO:
            // This is a temporary stub
            //
            // The Template Author will have to come up with
            // his own implementation for `svm_verify`
            0
        }
    }
}
