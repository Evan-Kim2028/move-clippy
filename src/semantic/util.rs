use crate::diagnostics::{Diagnostic, Position, Span};
use crate::level::LintLevel;
use crate::lint::{LintDescriptor, LintSettings};
use move_compiler::shared::files::MappedFiles;
use move_ir_types::location::Loc;

pub(super) fn convert_compiler_diagnostic(
    compiler_diag: &move_compiler::diagnostics::Diagnostic,
    settings: &LintSettings,
    file_map: &MappedFiles,
    descriptor: &'static LintDescriptor,
) -> Option<Diagnostic> {
    // Check if this lint is enabled
    if settings.level_for(descriptor.name) == LintLevel::Allow {
        return None;
    }

    // Get the primary location and message from the compiler diagnostic
    let primary_loc = compiler_diag.primary_loc();
    let primary_msg = compiler_diag.primary_msg();

    // Convert location to our span format
    let (file, span, contents) = diag_from_loc(file_map, &primary_loc)?;

    Some(Diagnostic {
        lint: descriptor,
        level: LintLevel::Warn,
        file: Some(file),
        span,
        message: primary_msg.to_string(),
        help: None,
        suggestion: None,
    })
}

pub(super) fn diag_from_loc(
    file_map: &MappedFiles,
    loc: &Loc,
) -> Option<(String, Span, std::sync::Arc<str>)> {
    let (fname, contents) = file_map.get(&loc.file_hash())?;
    let p = file_map.position_opt(loc)?;

    let file = fname.as_str().to_string();
    let span = Span {
        start: Position {
            row: p.start.line_offset() + 1,
            column: p.start.column_offset() + 1,
        },
        end: Position {
            row: p.end.line_offset() + 1,
            column: p.end.column_offset() + 1,
        },
    };

    Some((file, span, contents))
}

pub(super) fn push_diag(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    lint: &'static LintDescriptor,
    file: String,
    span: Span,
    source: &str,
    anchor_start: usize,
    message: String,
) {
    let module_scope = crate::annotations::module_scope(source);
    let item_scope = crate::annotations::item_scope(source, anchor_start);
    let level = crate::lint::effective_level_for_scopes(settings, lint, &module_scope, &item_scope);
    if level == LintLevel::Allow {
        return;
    }

    out.push(Diagnostic {
        lint,
        level,
        file: Some(file),
        span,
        message,
        help: None,
        suggestion: None,
    });
}

pub(super) fn position_from_byte_offset(source: &str, byte_offset: usize) -> Position {
    let mut row = 1usize;
    let mut col = 1usize;
    let end = byte_offset.min(source.len());
    for b in source.as_bytes().iter().take(end) {
        if *b == b'\n' {
            row += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    Position { row, column: col }
}
