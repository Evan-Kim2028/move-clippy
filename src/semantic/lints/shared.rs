use move_compiler::naming::ast as N;
use move_compiler::shared::Identifier;

pub(super) fn strip_refs(ty: &N::Type_) -> &N::Type_ {
    match ty {
        N::Type_::Ref(_, inner) => strip_refs(&inner.value),
        other => other,
    }
}

/// Check if a type is `sui::coin::Coin<T>`.
///
/// Coin types have the same ability pattern as capabilities (key+store, no copy/drop)
/// but they are value tokens, not access control objects. We exclude them from
/// capability transfer warnings to avoid false positives.
pub(super) fn is_coin_type(ty: &N::Type_) -> bool {
    match ty {
        N::Type_::Apply(_, type_name, _) => {
            if let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value {
                let module_sym = mident.value.module.value();
                let struct_sym = struct_name.value();
                // Match sui::coin::Coin or any coin module's Coin type
                module_sym.as_str() == "coin" && struct_sym.as_str() == "Coin"
            } else {
                false
            }
        }
        N::Type_::Ref(_, inner) => is_coin_type(&inner.value),
        _ => false,
    }
}

/// Check if a type is `sui::balance::Balance<T>`.
pub(super) fn is_balance_type(ty: &N::Type_) -> bool {
    match ty {
        N::Type_::Apply(_, type_name, _) => {
            if let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value {
                let module_sym = mident.value.module.value();
                let struct_sym = struct_name.value();
                module_sym.as_str() == "balance" && struct_sym.as_str() == "Balance"
            } else {
                false
            }
        }
        N::Type_::Ref(_, inner) => is_balance_type(&inner.value),
        _ => false,
    }
}

pub(super) fn is_coin_or_balance_type(ty: &N::Type_) -> bool {
    is_coin_type(ty) || is_balance_type(ty)
}

/// Format a type for display in error messages (using naming::ast::Type_ structure).
pub(super) fn format_type(ty: &N::Type_) -> String {
    match ty {
        N::Type_::Unit => "()".to_string(),
        N::Type_::Ref(is_mut, inner) => {
            let prefix = if *is_mut { "&mut " } else { "&" };
            format!("{}{}", prefix, format_type(&inner.value))
        }
        N::Type_::Apply(_, type_name, type_args) => format_apply_type(type_name, type_args),
        N::Type_::Param(tp) => tp.user_specified_name.value.to_string(),
        N::Type_::Fun(args, ret) => {
            let arg_strs: Vec<_> = args.iter().map(|t| format_type(&t.value)).collect();
            format!(
                "fun({}) -> {}",
                arg_strs.join(", "),
                format_type(&ret.value)
            )
        }
        N::Type_::Var(_) => "_".to_string(),
        N::Type_::Anything => "any".to_string(),
        N::Type_::Void => "void".to_string(),
        N::Type_::UnresolvedError => "error".to_string(),
    }
}

/// Format an Apply type (module::Type<args>) for display.
pub(super) fn format_apply_type(type_name: &N::TypeName, type_args: &[N::Type]) -> String {
    let name = match &type_name.value {
        N::TypeName_::Builtin(builtin) => format!("{:?}", builtin.value),
        N::TypeName_::ModuleType(mident, struct_name) => {
            format!("{}::{}", mident.value.module.value(), struct_name.value())
        }
        N::TypeName_::Multiple(_) => "tuple".to_string(),
    };
    if type_args.is_empty() {
        name
    } else {
        let args: Vec<_> = type_args
            .iter()
            .map(|t| format_type(strip_refs(&t.value)))
            .collect();
        format!("{}<{}>", name, args.join(", "))
    }
}
