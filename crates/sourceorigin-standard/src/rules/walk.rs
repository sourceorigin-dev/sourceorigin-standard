//! A walk over a file's expressions that knows where each sits: in test
//! code, or in a `const` or `static` item or `const fn`. The rules over
//! expressions inspect files through it.

use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, Token};

use super::source::Source;
use super::test_code::TestAttributes;

/// Where an expression sits.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Scope {
    /// In test code: a file of tests, or an item `#[cfg(test)]` or a test
    /// attribute marks.
    pub(super) test: bool,
    /// In a `const` or `static` item, or a `const fn`.
    pub(super) constant: bool,
}

/// What a rule inspects as a [`Walk`] passes it. Each method sees every
/// node of its kind, in the macros whose arguments parse as expressions too.
pub(super) trait Inspect {
    /// A function call, such as `Duration::from_secs(5)`.
    fn call(&mut self, _call: &syn::ExprCall, _scope: Scope) {}

    /// A method call, such as `router.route("/", get(index))`.
    fn method_call(&mut self, _call: &syn::ExprMethodCall, _scope: Scope) {}

    /// A macro invocation, such as `json!({})`.
    fn macro_call(&mut self, _call: &syn::Macro, _scope: Scope) {}

    /// A function or method with a body, before the nodes inside it.
    fn function(&mut self, _signature: &syn::Signature, _body: &syn::Block, _scope: Scope) {}
}

/// A walk over one file, passing its nodes to an [`Inspect`].
pub(super) struct Walk<'a, I> {
    inspect: &'a mut I,
    scope: Scope,
}

impl<'a, I: Inspect> Walk<'a, I> {
    /// Passes the nodes of `source` to `inspect`, the whole file test code
    /// when `test`.
    pub(super) fn file(source: &Source, test: bool, inspect: &'a mut I) {
        let mut walk = Self {
            inspect,
            scope: Scope {
                test: test || source.syntax.attrs.test(),
                constant: false,
            },
        };
        walk.visit_file(&source.syntax);
    }
}

/// Visits each item kind in the scope its attributes set, in a constant
/// scope where `const` holds of it, and passes the function `fn` finds in
/// it to [`Inspect::function`].
macro_rules! scoped {
    ($($visit:ident($node:ty) $(const $constant:expr)? $(, fn $function:expr)?;)*) => {
        $(
            fn $visit(&mut self, node: &'ast $node) {
                let outer = self.scope;
                self.scope.test |= node.attrs.test();
                $(self.scope.constant |= $constant(node);)?
                $(
                    if let Some((signature, body)) = $function(node) {
                        self.inspect.function(signature, body, self.scope);
                    }
                )?
                visit::$visit(self, node);
                self.scope = outer;
            }
        )*
    };
}

impl<'ast, I: Inspect> Visit<'ast> for Walk<'_, I> {
    scoped! {
        visit_item_mod(syn::ItemMod);
        visit_item_impl(syn::ItemImpl);
        visit_item_trait(syn::ItemTrait);
        visit_item_fn(syn::ItemFn)
            const |node: &syn::ItemFn| node.sig.constness.is_some(),
            fn |node: &'ast syn::ItemFn| Some((&node.sig, &*node.block));
        visit_impl_item_fn(syn::ImplItemFn)
            const |node: &syn::ImplItemFn| node.sig.constness.is_some(),
            fn |node: &'ast syn::ImplItemFn| Some((&node.sig, &node.block));
        visit_trait_item_fn(syn::TraitItemFn)
            const |node: &syn::TraitItemFn| node.sig.constness.is_some(),
            fn |node: &'ast syn::TraitItemFn| node.default.as_ref().map(|body| (&node.sig, body));
        visit_item_const(syn::ItemConst) const |_| true;
        visit_item_static(syn::ItemStatic) const |_| true;
        visit_impl_item_const(syn::ImplItemConst) const |_| true;
        visit_trait_item_const(syn::TraitItemConst) const |_| true;
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.inspect.call(call, self.scope);
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.inspect.method_call(call, self.scope);
        visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, call: &'ast syn::Macro) {
        self.inspect.macro_call(call, self.scope);
        if let Ok(arguments) = call.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
        {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        }
    }
}
