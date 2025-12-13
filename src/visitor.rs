use crate::lint::LintContext;
use tree_sitter::Node;

pub trait MoveVisitor {
    fn visit_module(&mut self, _node: Node, _ctx: &mut LintContext<'_>) {}
    fn visit_function(&mut self, _node: Node, _ctx: &mut LintContext<'_>) {}
    fn visit_struct(&mut self, _node: Node, _ctx: &mut LintContext<'_>) {}
    fn visit_constant(&mut self, _node: Node, _ctx: &mut LintContext<'_>) {}
    fn visit_call(&mut self, _node: Node, _ctx: &mut LintContext<'_>) {}
    fn visit_expression(&mut self, _node: Node, _ctx: &mut LintContext<'_>) {}
}

pub fn walk_tree(root: Node, _source: &str, ctx: &mut LintContext<'_>) {
    struct NoopVisitor;
    impl MoveVisitor for NoopVisitor {}

    let mut visitor = NoopVisitor;
    walk_node(root, &mut visitor, ctx);
}

fn walk_node(node: Node, visitor: &mut impl MoveVisitor, ctx: &mut LintContext<'_>) {
    match node.kind() {
        "module_definition" => visitor.visit_module(node, ctx),
        "function_definition" => visitor.visit_function(node, ctx),
        "struct_definition" => visitor.visit_struct(node, ctx),
        "constant" => visitor.visit_constant(node, ctx),
        "call_expression" => visitor.visit_call(node, ctx),
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, visitor, ctx);
    }
}
