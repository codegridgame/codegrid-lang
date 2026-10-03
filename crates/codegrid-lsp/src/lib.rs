use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use codegrid_compiler::{check, compile_with_symbols, completion_context, SymbolKind};
use codegrid_model::{
    AttachmentInstruction, Direction, PageDirection, PointerDirection, PrimaryInstruction,
    ShiftDirection,
};
use codegrid_syntax::{
    parse_cell_token, parse_directive_head, parse_syntax, CellToken, DefinitionTarget,
    DirectiveKind, LineIndex, Span, SyntaxItem,
};
use serde_json::{json, Value};

const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
struct Document {
    version: Option<i64>,
    text: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Lifecycle {
    #[default]
    Uninitialized,
    AwaitingInitialized,
    Running,
    Shutdown,
}

#[derive(Default)]
struct Server {
    documents: BTreeMap<String, Document>,
    lifecycle: Lifecycle,
}

/// Runs the native Language Server Protocol process over standard input/output.
/// Standard output is reserved exclusively for framed JSON-RPC messages.
pub fn run_stdio() -> io::Result<bool> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(stdin.lock(), stdout.lock())
}

fn serve<R: BufRead, W: Write>(mut reader: R, mut writer: W) -> io::Result<bool> {
    let mut server = Server::default();
    while let Some(message) = read_message(&mut reader)? {
        if !server.handle(message, &mut writer)? {
            return Ok(server.lifecycle == Lifecycle::Shutdown);
        }
    }
    Ok(server.lifecycle == Lifecycle::Shutdown)
}

impl Server {
    fn handle<W: Write>(&mut self, message: Value, writer: &mut W) -> io::Result<bool> {
        let method = message.get("method").and_then(Value::as_str);
        let Some(method) = method else {
            let id = message.get("id").cloned().unwrap_or(Value::Null);
            write_message(
                writer,
                &error_response(id, -32600, "Invalid JSON-RPC request: missing method."),
            )?;
            return Ok(true);
        };
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            let id = message.get("id").cloned().unwrap_or(Value::Null);
            write_message(
                writer,
                &error_response(id, -32600, "Invalid JSON-RPC version; expected 2.0."),
            )?;
            return Ok(true);
        }
        if method == "exit" {
            if let Some(id) = message.get("id") {
                write_message(
                    writer,
                    &error_response(
                        id.clone(),
                        -32600,
                        "The LSP exit notification must not include an id.",
                    ),
                )?;
                return Ok(true);
            }
            return Ok(false);
        }

        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = message.get("id").cloned() else {
            self.handle_notification(method, &params, writer)?;
            return Ok(true);
        };

        let response = match method {
            "initialize" if self.lifecycle == Lifecycle::Uninitialized => {
                self.lifecycle = Lifecycle::AwaitingInitialized;
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": { "openClose": true, "change": 2 },
                            "hoverProvider": true,
                            "definitionProvider": true,
                            "referencesProvider": true,
                            "documentSymbolProvider": true,
                            "documentFormattingProvider": true,
                            "completionProvider": { "triggerCharacters": ["@", "~", "#", "$", ","] }
                        },
                        "serverInfo": {
                            "name": "CodeGrid Language Server",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                })
            }
            "initialize" => error_response(id, -32600, "The server has already been initialized."),
            "shutdown" if self.lifecycle == Lifecycle::Running => {
                self.lifecycle = Lifecycle::Shutdown;
                json!({ "jsonrpc": "2.0", "id": id, "result": null })
            }
            "shutdown" => error_response(id, -32002, "The server is not in an initialized state."),
            _ if self.lifecycle == Lifecycle::Shutdown => {
                error_response(id, -32600, "The server has already shut down.")
            }
            _ if self.lifecycle != Lifecycle::Running => {
                error_response(id, -32002, "The server is not in an initialized state.")
            }
            _ => match self.request_result(method, &params) {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err((code, message)) => error_response(id, code, &message),
            },
        };
        write_message(writer, &response)?;
        Ok(true)
    }

    fn handle_notification<W: Write>(
        &mut self,
        method: &str,
        params: &Value,
        writer: &mut W,
    ) -> io::Result<()> {
        match method {
            "initialized" if self.lifecycle == Lifecycle::AwaitingInitialized => {
                self.lifecycle = Lifecycle::Running;
            }
            "initialized" => {}
            _ if self.lifecycle != Lifecycle::Running => {}
            "textDocument/didOpen" => {
                let Some(document) = params.get("textDocument") else {
                    return Ok(());
                };
                let (Some(uri), Some(text)) = (
                    document.get("uri").and_then(Value::as_str),
                    document.get("text").and_then(Value::as_str),
                ) else {
                    return Ok(());
                };
                let version = document.get("version").and_then(Value::as_i64);
                self.documents.insert(
                    uri.to_owned(),
                    Document {
                        version,
                        text: text.to_owned(),
                    },
                );
                self.publish_diagnostics(uri, writer)?;
            }
            "textDocument/didChange" => {
                let Some(document_id) = params.get("textDocument") else {
                    return Ok(());
                };
                let (Some(uri), Some(changes)) = (
                    document_id.get("uri").and_then(Value::as_str),
                    params.get("contentChanges").and_then(Value::as_array),
                ) else {
                    return Ok(());
                };
                let Some(version) = document_id.get("version").and_then(Value::as_i64) else {
                    return Ok(());
                };
                let Some(document) = self.documents.get_mut(uri) else {
                    return Ok(());
                };
                if document.version.is_some_and(|old| version <= old) {
                    return Ok(());
                }
                let mut updated_text = document.text.clone();
                for change in changes {
                    let Some(text) = change.get("text").and_then(Value::as_str) else {
                        return Ok(());
                    };
                    if !apply_content_change(&mut updated_text, change, text) {
                        return Ok(());
                    }
                }
                document.text = updated_text;
                document.version = Some(version);
                self.publish_diagnostics(uri, writer)?;
            }
            "textDocument/didClose" => {
                let Some(uri) = params
                    .get("textDocument")
                    .and_then(|document| document.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(());
                };
                self.documents.remove(uri);
                write_message(
                    writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": { "uri": uri, "diagnostics": [] }
                    }),
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn publish_diagnostics<W: Write>(&self, uri: &str, writer: &mut W) -> io::Result<()> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(());
        };
        let index = LineIndex::new(&document.text);
        let diagnostics = check(&document.text)
            .iter()
            .map(|diagnostic| {
                json!({
                    "range": range_json(&document.text, &index, diagnostic.span),
                    "severity": match diagnostic.severity {
                        codegrid_syntax::Severity::Error => 1,
                        codegrid_syntax::Severity::Warning => 2,
                    },
                    "source": "codegrid",
                    "code": diagnostic.code,
                    "data": {"error_number": codegrid_model::error_number("source", diagnostic.code).or_else(|| codegrid_model::error_number("ir", diagnostic.code))},
                    "message": diagnostic.message
                })
            })
            .collect::<Vec<_>>();
        let mut params = json!({ "uri": uri, "diagnostics": diagnostics });
        if let Some(version) = document.version {
            params["version"] = json!(version);
        }
        write_message(
            writer,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": params
            }),
        )
    }

    fn request_result(&self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        if !matches!(
            method,
            "textDocument/hover"
                | "textDocument/definition"
                | "textDocument/references"
                | "textDocument/completion"
                | "textDocument/documentSymbol"
                | "textDocument/formatting"
        ) {
            return Err((-32601, format!("Unsupported method: {method}")));
        }
        let uri = params
            .get("textDocument")
            .and_then(|document| document.get("uri"))
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "Missing textDocument.uri.".to_owned()))?;
        let document = self
            .documents
            .get(uri)
            .ok_or_else(|| (-32602, "The requested document is not open.".to_owned()))?;
        let index = LineIndex::new(&document.text);

        match method {
            "textDocument/hover" => {
                let position = params
                    .get("position")
                    .ok_or_else(|| (-32602, "Missing textDocument position.".to_owned()))?;
                let offset = byte_offset_at_position(&document.text, &index, position)
                    .ok_or_else(|| (-32602, "Invalid textDocument position.".to_owned()))?;
                Ok(hover_cell(&document.text, &index, offset))
            }
            "textDocument/definition" => {
                let position = params
                    .get("position")
                    .ok_or_else(|| (-32602, "Missing textDocument position.".to_owned()))?;
                let offset = byte_offset_at_position(&document.text, &index, position)
                    .ok_or_else(|| (-32602, "Invalid textDocument position.".to_owned()))?;
                Ok(definition_location(uri, &document.text, offset))
            }
            "textDocument/references" => {
                let position = params
                    .get("position")
                    .ok_or_else(|| (-32602, "Missing textDocument position.".to_owned()))?;
                let offset = byte_offset_at_position(&document.text, &index, position)
                    .ok_or_else(|| (-32602, "Invalid textDocument position.".to_owned()))?;
                let include_declaration = params
                    .get("context")
                    .and_then(|context| context.get("includeDeclaration"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                Ok(references_locations(
                    uri,
                    &document.text,
                    offset,
                    include_declaration,
                ))
            }
            "textDocument/completion" => {
                let position = params
                    .get("position")
                    .ok_or_else(|| (-32602, "Missing textDocument position.".to_owned()))?;
                let offset = byte_offset_at_position(&document.text, &index, position)
                    .ok_or_else(|| (-32602, "Invalid textDocument position.".to_owned()))?;
                Ok(json!({
                    "isIncomplete": false,
                    "items": completion_items(&document.text, &index, offset)
                }))
            }
            "textDocument/documentSymbol" => Ok(json!(document_symbols(&document.text, &index))),
            "textDocument/formatting" => {
                if check(&document.text)
                    .iter()
                    .any(|diagnostic| diagnostic.severity == codegrid_syntax::Severity::Error)
                {
                    return Ok(json!([]));
                }
                let Some(formatted) = codegrid_syntax::format_source(&document.text) else {
                    return Ok(json!([]));
                };
                if formatted == document.text {
                    return Ok(json!([]));
                }
                Ok(json!([{
                    "range": range_json(
                        &document.text,
                        &index,
                        Span::new(0, document.text.len())
                    ),
                    "newText": formatted
                }]))
            }
            _ => Err((-32601, format!("Unsupported method: {method}"))),
        }
    }
}

fn document_symbols(source: &str, index: &LineIndex) -> Vec<Value> {
    let Ok(compilation) = compile_with_symbols(source) else {
        return Vec::new();
    };
    let parsed = parse_syntax(source);
    let mut symbols = Vec::new();
    if let Some((range, selection)) = main_symbol_spans(&parsed.items) {
        symbols.push(json!({
            "name": "Main Board",
            "detail": "Outer Main CodeGrid",
            "kind": 2,
            "range": range_json(source, index, range),
            "selectionRange": range_json(source, index, selection)
        }));
    }
    for definition in compilation.symbols.definitions() {
        let (kind, detail) = match definition.key.kind() {
            SymbolKind::Custom => (3, "Custom CodeGrid"),
            SymbolKind::Function => (12, "Function Board"),
            SymbolKind::FoldedBlock => (18, "Folded Block"),
        };
        let name = definition.key.qualified_name();
        symbols.push(json!({
            "name": name,
            "detail": detail,
            "kind": kind,
            "range": range_json(source, index, definition.span),
            "selectionRange": range_json(source, index, definition.span)
        }));
    }
    symbols
}

fn main_symbol_spans(items: &[SyntaxItem]) -> Option<(Span, Span)> {
    let explicit_main = items.iter().position(|item| {
        matches!(
            item,
            SyntaxItem::Directive(directive)
                if matches!(directive.kind, DirectiveKind::Definition(DefinitionTarget::Main))
        )
    });
    if let Some(main_index) = explicit_main {
        let SyntaxItem::Directive(main) = &items[main_index] else {
            return None;
        };
        let mut open_definitions = 1usize;
        let mut end_span = main.span;
        for item in &items[main_index + 1..] {
            match item {
                SyntaxItem::Directive(directive) => match directive.kind {
                    DirectiveKind::Definition(target) => {
                        let one_line_fold = matches!(
                            target,
                            DefinitionTarget::RelativeFoldedBlock(_)
                                | DefinitionTarget::FoldedBlock { .. }
                                | DefinitionTarget::RelativeFunctionFoldedBlock { .. }
                        ) && !directive.arguments.is_empty();
                        if !one_line_fold {
                            open_definitions += 1;
                        }
                    }
                    DirectiveKind::End => {
                        open_definitions = open_definitions.saturating_sub(1);
                        if open_definitions == 0 {
                            end_span = directive.span;
                            break;
                        }
                    }
                    DirectiveKind::Size => {}
                },
                SyntaxItem::GridRow { .. } => {}
            }
        }
        return Some((Span::new(main.span.start, end_span.end), main.head_span));
    }

    let rows = items.iter().filter_map(|item| match item {
        SyntaxItem::GridRow { cells, span } => {
            Some((*span, cells.first().map_or(*span, |cell| cell.span)))
        }
        _ => None,
    });
    let mut rows = rows.collect::<Vec<_>>();
    let (first, selection) = *rows.first()?;
    let last = rows.pop().map_or(first, |(span, _)| span);
    Some((Span::new(first.start, last.end), selection))
}

fn definition_location(uri: &str, source: &str, offset: usize) -> Value {
    let Ok(compilation) = compile_with_symbols(source) else {
        return Value::Null;
    };
    let Some(key) = compilation.symbols.symbol_at(offset) else {
        return Value::Null;
    };
    let Some(definition) = compilation.symbols.definition(key) else {
        return Value::Null;
    };
    let index = LineIndex::new(source);
    json!({
        "uri": uri,
        "range": range_json(source, &index, definition.span)
    })
}

fn references_locations(
    uri: &str,
    source: &str,
    offset: usize,
    include_declaration: bool,
) -> Value {
    let Ok(compilation) = compile_with_symbols(source) else {
        return json!([]);
    };
    let Some(key) = compilation.symbols.symbol_at(offset) else {
        return json!([]);
    };
    let index = LineIndex::new(source);
    let mut locations = compilation
        .symbols
        .references_to(key)
        .map(|reference| {
            json!({
                "uri": uri,
                "range": range_json(source, &index, reference.span)
            })
        })
        .collect::<Vec<_>>();
    if include_declaration {
        if let Some(definition) = compilation.symbols.definition(key) {
            locations.push(json!({
                "uri": uri,
                "range": range_json(source, &index, definition.span)
            }));
        }
    }
    json!(locations)
}

#[derive(Clone, Debug)]
struct CompletionCandidate {
    label: String,
    detail: &'static str,
}

fn hover_cell(source: &str, index: &LineIndex, offset: usize) -> Value {
    let Some(span) = token_span(source, offset) else {
        return Value::Null;
    };
    if codegrid_syntax::lex(source).tokens.iter().any(|token| {
        token.kind == codegrid_syntax::TokenKind::Comment
            && token.span.start <= offset
            && offset < token.span.end
    }) {
        return Value::Null;
    }
    let Some(token) = source.get(span.start..span.end) else {
        return Value::Null;
    };
    let description = match parse_cell_token(token) {
        Ok(CellToken::Empty) => "Empty cell.".to_owned(),
        Ok(CellToken::Entry(direction)) => format!(
            "Main Board Entry facing {}. It initializes the VM position and direction and behaves as empty when execution visits it.",
            direction_name(direction)
        ),
        Ok(CellToken::Instruction {
            primary,
            attachment: None,
        }) => primary_description(primary),
        Ok(CellToken::Instruction {
            primary,
            attachment: Some(attachment),
        }) => format!(
            "{}\n\nAttachment: {}.",
            primary_description(primary),
            attachment_description(attachment)
        ),
        Err(_) => match parse_directive_head(token) {
            Ok(DirectiveKind::Definition(DefinitionTarget::Main)) => {
                "Defines the program’s outer Main CodeGrid and Main Board.".to_owned()
            }
            Ok(DirectiveKind::Definition(DefinitionTarget::Custom(slot))) => {
                format!("Defines Custom CodeGrid {}.", slot.get())
            }
            Ok(DirectiveKind::Definition(DefinitionTarget::RelativeFunction(slot))) => {
                format!("Defines Function F{} in the current CodeGrid.", slot.get())
            }
            Ok(DirectiveKind::Definition(DefinitionTarget::RelativeFoldedBlock(slot))) => {
                format!("Defines Folded Block M{} in the current Board.", slot.get())
            }
            Ok(DirectiveKind::Definition(DefinitionTarget::Function { codegrid, slot })) => {
                format!("Defines Function F{} in {codegrid:?}.", slot.get())
            }
            Ok(DirectiveKind::Definition(DefinitionTarget::FoldedBlock { board, slot })) => {
                format!("Defines Folded Block M{} in {board:?}.", slot.get())
            }
            Ok(DirectiveKind::Definition(
                DefinitionTarget::RelativeFunctionFoldedBlock { function, slot },
            )) => format!(
                "Defines Folded Block M{} in Function F{} of the current CodeGrid.",
                slot.get(),
                function.get()
            ),
            Ok(DirectiveKind::End) => "Closes the current explicitly delimited definition.".to_owned(),
            Ok(DirectiveKind::Size) => {
                "Declares positive dimensions with `@size WIDTHxHEIGHT` for the applicable board scope.".to_owned()
            }
            Err(_) => return Value::Null,
        },
    };
    hover_value(
        source,
        index,
        span,
        format!("**`{token}`**\n\n{description}"),
    )
}

fn hover_value(source: &str, index: &LineIndex, span: Span, markdown: String) -> Value {
    json!({
        "contents": { "kind": "markdown", "value": markdown },
        "range": range_json(source, index, span)
    })
}

fn token_span(source: &str, offset: usize) -> Option<Span> {
    let mut offset = offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let start = source[..offset]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or(0, |(position, character)| position + character.len_utf8());
    let end = source[offset..]
        .char_indices()
        .find(|(_, character)| character.is_whitespace())
        .map_or(source.len(), |(relative, _)| offset + relative);
    (start < end).then_some(Span::new(start, end))
}

fn primary_description(primary: PrimaryInstruction) -> String {
    let semantics = match primary {
        PrimaryInstruction::Direction(direction) => {
            format!("Set the thread direction to {}.", direction_name(direction))
        }
        PrimaryInstruction::RandomDirection => {
            "Choose a direction from the thread's deterministic random stream.".to_owned()
        }
        PrimaryInstruction::IfZero(direction) => format!(
            "Set the direction to {} when the selected register is zero; otherwise keep the current direction.",
            direction_name(direction)
        ),
        PrimaryInstruction::Read(direction) => format!(
            "Read from the outer input or the Custom caller's data stack into the selected register. If the source is empty, keep the register and set direction to {}.",
            direction_name(direction)
        ),
        PrimaryInstruction::Clear => "Set the selected register to zero.".to_owned(),
        PrimaryInstruction::Add => "Increment the selected register with 8-bit wrapping.".to_owned(),
        PrimaryInstruction::Sub => "Decrement the selected register with 8-bit wrapping.".to_owned(),
        PrimaryInstruction::MoveRegisterPointer(direction) => format!(
            "Move the thread's register pointer one slot {}, wrapping between R0 and R9.",
            match direction {
                PointerDirection::Left => "left",
                PointerDirection::Right => "right",
            }
        ),
        PrimaryInstruction::Output => {
            "Append the selected byte to outer output, or push it onto the Custom caller's data stack.".to_owned()
        }
        PrimaryInstruction::OutputImmediate(digit) => format!(
            "Output the raw byte {} without changing registers or the register pointer; in Custom code, push it onto the caller's data stack. No Instruction Code or Attachments.", digit.get()
        ),
        PrimaryInstruction::Push => {
            "Push the selected register byte onto this thread's data stack.".to_owned()
        }
        PrimaryInstruction::PopAdd => {
            "Pop a byte and add it to the selected register with 8-bit wrapping; on an empty stack, do nothing."
                .to_owned()
        }
        PrimaryInstruction::Decode => {
            "Decode the selected byte as an Instruction Code and push a valid item onto the instruction stack; invalid codes do nothing."
                .to_owned()
        }
        PrimaryInstruction::Encode => {
            "Pop an item from the instruction stack and write its Instruction Code to the selected register; an empty stack does nothing."
                .to_owned()
        }
        PrimaryInstruction::Call(slot) => {
            format!("Call Function F{} in the current CodeGrid.", slot.get())
        }
        PrimaryInstruction::Return => {
            "Return through the current Function call frame; caller movement occurs on a separate AfterCall tick."
                .to_owned()
        }
        PrimaryInstruction::Nand => {
            "Pop a byte and replace the selected register with the 8-bit complement of its AND with that byte; an empty stack does nothing."
                .to_owned()
        }
        PrimaryInstruction::MemoryLoad => {
            "Read the byte at Page × 256 plus the selected register and push it onto the data stack."
                .to_owned()
        }
        PrimaryInstruction::MemoryStore => {
            "Pop a byte and stage it at Page × 256 plus the selected register; an empty stack does nothing."
                .to_owned()
        }
        PrimaryInstruction::MovePage(direction) => format!(
            "{} the thread's arbitrary-precision Page value by one.",
            match direction {
                PageDirection::Increment => "Increment",
                PageDirection::Decrement => "Decrement",
            }
        ),
        PrimaryInstruction::Shift(direction) => format!(
            "Shift the selected byte {} by one bit.",
            match direction {
                ShiftDirection::Left => "left with the high bit discarded",
                ShiftDirection::Right => "right logically with zero entering the high bit",
            }
        ),
        PrimaryInstruction::FoldedBlock(slot) => {
            format!("Enter Folded Block M{} on the current Board.", slot.get())
        }
        PrimaryInstruction::Custom(slot) => {
            format!("Run Custom CodeGrid C{} synchronously in an isolated context.", slot.get())
        }
        PrimaryInstruction::CustomReturn => {
            "Terminate this Custom internal thread; the invocation returns when all its Entry threads have returned."
                .to_owned()
        }
        PrimaryInstruction::Halt => {
            "Request whole-VM termination after the current Global Tick commits.".to_owned()
        }
    };
    format!("Full Primary instruction\n\n{semantics}")
}

fn attachment_description(attachment: AttachmentInstruction) -> &'static str {
    match attachment {
        AttachmentInstruction::ReadCode => "ReadCode (`*`)",
        AttachmentInstruction::WriteCode => "WriteCode (`=`)",
        AttachmentInstruction::Repeat(count) => match count {
            2 => "Repeat x2",
            3 => "Repeat x3",
            4 => "Repeat x4",
            _ => "Repeat x5",
        },
    }
}

fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

fn completion_items(source: &str, index: &LineIndex, offset: usize) -> Vec<Value> {
    if codegrid_syntax::lex(source).tokens.iter().any(|token| {
        token.kind == codegrid_syntax::TokenKind::Comment
            && token.span.start <= offset
            && offset < token.span.end
    }) {
        return Vec::new();
    }

    let end = offset.min(source.len());
    let end = (0..=end)
        .rev()
        .find(|candidate| source.is_char_boundary(*candidate))
        .unwrap_or(0);
    let prefix_start = source[..end]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or(0, |(position, character)| position + character.len_utf8());
    let prefix = &source[prefix_start..end];
    let candidates = if prefix.starts_with('@') {
        directive_candidates(source, offset)
    } else {
        cell_candidates(prefix)
    };

    candidates
        .into_iter()
        .filter(|candidate| candidate.label.starts_with(prefix))
        .map(|candidate| {
            let label = candidate.label;
            json!({
                "label": label.clone(),
                "kind": 14,
                "detail": candidate.detail,
                "textEdit": {
                    "range": range_json(source, index, Span::new(prefix_start, end)),
                    "newText": label
                }
            })
        })
        .collect()
}

fn directive_candidates(source: &str, byte_offset: usize) -> Vec<CompletionCandidate> {
    let parsed = parse_syntax(source);
    let context = completion_context(source, byte_offset);
    let mut main_declared = false;
    let mut grid_started = false;
    for item in &parsed.items {
        let span = match item {
            SyntaxItem::Directive(directive) => directive.span,
            SyntaxItem::GridRow { span, .. } => *span,
        };
        if span.start >= byte_offset {
            break;
        }
        match item {
            SyntaxItem::Directive(directive)
                if matches!(
                    directive.kind,
                    DirectiveKind::Definition(DefinitionTarget::Main)
                ) =>
            {
                main_declared = true;
            }
            SyntaxItem::GridRow { .. } => grid_started = true,
            _ => {}
        }
    }
    let mut candidates = Vec::new();
    if !main_declared && !grid_started {
        candidates.push(CompletionCandidate {
            label: "@main".to_owned(),
            detail: "Define the outer Main CodeGrid",
        });
    }
    candidates.push(CompletionCandidate {
        label: "@size".to_owned(),
        detail: "Declare dimensions for the applicable board scope",
    });
    if context.can_close_block {
        candidates.push(CompletionCandidate {
            label: "@end".to_owned(),
            detail: "Close the current definition",
        });
    }
    if main_declared && !context.can_close_block && !context.inside_folded_block {
        for slot in 0..10 {
            if !context
                .custom_slots
                .iter()
                .any(|existing| existing.get() == slot)
            {
                candidates.push(CompletionCandidate {
                    label: format!("@C{slot}"),
                    detail: "Define a Custom CodeGrid",
                });
            }
        }
    }
    if context.inside_codegrid {
        for slot in 0..10 {
            if !context
                .function_slots
                .iter()
                .any(|existing| existing.get() == slot)
            {
                candidates.push(CompletionCandidate {
                    label: format!("@F{slot}"),
                    detail: "Define a Function in the current CodeGrid",
                });
            }
        }
    }
    if !context.inside_folded_block && (context.inside_codegrid || context.has_grid) {
        for slot in 0..10 {
            if !context
                .folded_block_slots
                .iter()
                .any(|existing| existing.get() == slot)
            {
                candidates.push(CompletionCandidate {
                    label: format!("@M{slot}"),
                    detail: "Define a Folded Block in the current Board",
                });
            }
        }
    }
    candidates
}

fn cell_candidates(prefix: &str) -> Vec<CompletionCandidate> {
    let mut candidates = Vec::new();
    for token in ["~^", "~v", "~<", "~>"] {
        candidates.push(CompletionCandidate {
            label: token.to_owned(),
            detail: "Entry marker",
        });
    }
    candidates.push(CompletionCandidate {
        label: "_".to_owned(),
        detail: "Empty cell",
    });
    for primary in PrimaryInstruction::source_forms() {
        let token = primary.token();
        if parse_cell_token(&token).is_ok() {
            candidates.push(CompletionCandidate {
                label: token.clone(),
                detail: "Full Primary instruction",
            });
        }
        for attachment in AttachmentInstruction::ALL {
            let attached = format!("{token}{}", attachment.token());
            if parse_cell_token(&attached).is_ok() {
                candidates.push(CompletionCandidate {
                    label: attached,
                    detail: "Primary instruction with Attachment",
                });
            }
        }
    }
    candidates
        .into_iter()
        .filter(|candidate| candidate.label.starts_with(prefix))
        .collect()
}
fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message, "data": {"error_number": codegrid_model::error_number("lsp", &code.to_string())} }
    })
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return if content_length.is_none() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "JSON-RPC headers ended before the message body.",
                ))
            };
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                content_length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
                );
            }
        }
    }
    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Missing Content-Length header.")
    })?;
    if length > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "JSON-RPC message exceeds the configured size limit.",
        ));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

fn range_json(source: &str, index: &LineIndex, span: Span) -> Value {
    json!({
        "start": position_json(source, index, span.start),
        "end": position_json(source, index, span.end)
    })
}

fn position_json(source: &str, index: &LineIndex, byte_offset: usize) -> Value {
    let (line, byte_column) = index.line_and_byte_column(source, byte_offset);
    let line_start = index.line_start(line).unwrap_or(source.len());
    let utf16_column = source
        .get(line_start..line_start.saturating_add(byte_column))
        .map(|prefix| prefix.encode_utf16().count())
        .unwrap_or_default();
    json!({
        "line": u32::try_from(line).unwrap_or(u32::MAX),
        "character": u32::try_from(utf16_column).unwrap_or(u32::MAX)
    })
}

fn byte_offset_at_position(source: &str, index: &LineIndex, position: &Value) -> Option<usize> {
    let line = usize::try_from(position.get("line")?.as_u64()?).ok()?;
    let character = position.get("character")?.as_u64()?;
    let line_start = index.line_start(line)?;
    let line_end = index.line_end(source, line)?;
    let text = source.get(line_start..line_end)?;
    let mut utf16_column = 0u64;
    for (byte_offset, character_at) in text.char_indices() {
        let next_column = utf16_column + character_at.len_utf16() as u64;
        if character < next_column {
            return Some(line_start + byte_offset);
        }
        if character == next_column {
            return Some(line_start + byte_offset + character_at.len_utf8());
        }
        utf16_column = next_column;
    }
    Some(line_end)
}

fn apply_content_change(document: &mut String, change: &Value, replacement: &str) -> bool {
    let Some(range) = change.get("range") else {
        document.clear();
        document.push_str(replacement);
        return true;
    };
    let index = LineIndex::new(document);
    let Some(start) = range
        .get("start")
        .and_then(|position| byte_offset_at_position(document, &index, position))
    else {
        return false;
    };
    let Some(end) = range
        .get("end")
        .and_then(|position| byte_offset_at_position(document, &index, position))
    else {
        return false;
    };
    if start <= end {
        document.replace_range(start..end, replacement);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_content_change, byte_offset_at_position, completion_items, position_json, serve,
        Lifecycle, Server,
    };
    use codegrid_syntax::LineIndex;
    use serde_json::json;
    use std::io::{BufRead, Cursor, Read};

    fn append_message(stream: &mut Vec<u8>, message: serde_json::Value) {
        let body = serde_json::to_vec(&message).expect("test JSON must serialize");
        stream.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        stream.extend_from_slice(&body);
    }

    fn read_messages(stream: &[u8]) -> Vec<serde_json::Value> {
        let mut reader = Cursor::new(stream);
        let mut messages = Vec::new();
        while (reader.position() as usize) < stream.len() {
            let mut header = String::new();
            let mut length = None;
            loop {
                header.clear();
                reader
                    .read_line(&mut header)
                    .expect("test response headers must be readable");
                if header == "\r\n" || header == "\n" {
                    break;
                }
                if let Some(value) = header.strip_prefix("Content-Length: ") {
                    length = Some(
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("test response length must be numeric"),
                    );
                }
            }
            let mut body = vec![0; length.expect("response must include its body size")];
            reader
                .read_exact(&mut body)
                .expect("test response body must be complete");
            messages.push(serde_json::from_slice(&body).expect("test response must contain JSON"));
        }
        messages
    }

    #[test]
    fn converts_lsp_utf16_columns_and_crlf_to_utf8_offsets() {
        let source = "a🙂\r\nb";
        let index = LineIndex::new(source);

        assert_eq!(
            byte_offset_at_position(source, &index, &json!({ "line": 0, "character": 3 })),
            Some(5)
        );
        assert_eq!(
            byte_offset_at_position(source, &index, &json!({ "line": 1, "character": 1 })),
            Some(8)
        );
        assert_eq!(
            position_json(source, &index, 5),
            json!({ "line": 0, "character": 3 })
        );
    }

    #[test]
    fn applies_incremental_edits_in_utf16_coordinates() {
        let mut document = "a🙂b\r\n".to_owned();
        apply_content_change(
            &mut document,
            &json!({
                "range": {
                    "start": { "line": 0, "character": 1 },
                    "end": { "line": 0, "character": 3 }
                }
            }),
            "x",
        );

        assert_eq!(document, "axb\r\n");
        assert!(apply_content_change(
            &mut document,
            &json!({
                "range": {
                    "start": { "line": 0, "character": 2 },
                    "end": { "line": 0, "character": 3 }
                }
            }),
            "c",
        ));
        assert_eq!(document, "axc\r\n");
    }

    #[test]
    fn completions_expose_full_directives_primary_tokens_and_attachments() {
        let source = "@main
@size 3x1
~> #
@end
";
        let index = LineIndex::new(source);
        let completion_offset = source
            .find(
                "#
",
            )
            .expect("source includes a partial IF_ZERO")
            + 1;
        let items = completion_items(source, &index, completion_offset);
        let labels = items
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"#^"));
        assert!(labels.contains(&"#v"));
        assert!(labels.contains(&"#<"));
        assert!(labels.contains(&"#>"));
        for supported in ["#0", "#]"] {
            assert!(labels.contains(&supported), "missing {supported}");
        }
        for unsupported in ["x2", "#10", "$10", "[00", "]x2"] {
            assert!(!labels.contains(&unsupported), "unexpected {unsupported}");
        }
        let all_labels = super::cell_candidates("")
            .into_iter()
            .map(|candidate| candidate.label)
            .collect::<Vec<_>>();
        for supported in ["[0", "$0", "?", "!", ",^*", "+x2"] {
            assert!(all_labels.iter().any(|label| label == supported));
        }

        let directive_source = "@";
        let directive_index = LineIndex::new(directive_source);
        let directive_items = completion_items(directive_source, &directive_index, 1);
        let directive_labels = directive_items
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect::<Vec<_>>();
        assert!(directive_labels.contains(&"@main"));
        assert!(directive_labels.contains(&"@size"));
        assert!(!directive_labels.contains(&"@C0"));
        assert!(!directive_labels.contains(&"@F0"));
        assert!(!directive_labels.contains(&"@M0"));

        let entry_source = "@main
~";
        let entry_index = LineIndex::new(entry_source);
        let entry_items = completion_items(entry_source, &entry_index, entry_source.len());
        let entry_labels = entry_items
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(entry_labels, ["~^", "~v", "~<", "~>"]);
    }

    #[test]
    fn structural_completions_follow_compiler_scope_context() {
        let open_main = "@main\n~> _\n@";
        let labels = completion_items(open_main, &LineIndex::new(open_main), open_main.len())
            .into_iter()
            .filter_map(|item| item["label"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        assert!(labels.contains(&"@F0".to_owned()));
        assert!(labels.contains(&"@M0".to_owned()));
        assert!(labels.contains(&"@end".to_owned()));
        assert!(!labels.contains(&"@C0".to_owned()));

        let top_level = "@main\n~> _\n@end\n@";
        let labels = completion_items(top_level, &LineIndex::new(top_level), top_level.len())
            .into_iter()
            .filter_map(|item| item["label"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        assert!(labels.contains(&"@C0".to_owned()));
        assert!(!labels.contains(&"@F0".to_owned()));
    }

    #[test]
    fn hover_identifies_full_primary_instructions_and_attachments() {
        let source = "~> ,> #^ $> + - . ;
";
        let index = LineIndex::new(source);
        let hover = |token: &str| {
            let offset = source.find(token).expect("source contains the token");
            super::hover_cell(source, &index, offset + 1)
        };
        for token in [",>", "#^", "$>", "+", ";"] {
            assert!(hover(token)["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("Full Primary instruction"));
        }
        for token in ["#0", "[0"] {
            let hover = super::hover_cell(token, &LineIndex::new(token), 1);
            assert!(hover["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("Full Primary instruction"));
        }
        let attached = super::hover_cell("+x3", &LineIndex::new("+x3"), 2);
        assert!(attached["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("Repeat x3"));
    }

    #[test]
    fn hover_describes_main_structure_entries_and_empty_cells() {
        let source = "@main\n@size 2x1\n~> _\n@end\n";
        let index = LineIndex::new(source);
        for (token, phrase) in [
            ("@main", "outer Main CodeGrid"),
            ("@size", "positive dimensions"),
            ("~>", "Main Board Entry"),
            ("_", "Empty cell"),
            ("@end", "Closes the current"),
        ] {
            let offset = source.find(token).expect("source contains the token") + 1;
            let hover = super::hover_cell(source, &index, offset);
            assert!(
                hover["contents"]["value"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(phrase),
                "hover for {token} must describe {phrase}"
            );
        }
    }
    #[test]
    fn protocol_lifecycle_publishes_current_diagnostics_and_clears_closed_documents() {
        let uri = "file:///workspace/example.cg";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": { "uri": uri, "languageId": "codegrid", "version": 1, "text": "~x\n" }
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [{ "text": "~> ;\n" }]
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didClose",
                "params": { "textDocument": { "uri": uri } }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let messages = read_messages(&output);

        assert_eq!(messages[0]["id"], 1);
        assert_eq!(
            messages[0]["result"]["capabilities"]["textDocumentSync"]["change"],
            2
        );
        let published = messages
            .iter()
            .filter(|message| message["method"] == "textDocument/publishDiagnostics")
            .collect::<Vec<_>>();
        assert_eq!(published.len(), 3);
        assert!(!published[0]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(published[1]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(published[2]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(messages.last().unwrap()["id"], 2);
    }

    #[test]
    fn stale_document_versions_do_not_replace_the_current_buffer_or_diagnostics() {
        let uri = "file:///workspace/stale.cg";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": { "uri": uri, "languageId": "codegrid", "version": 2, "text": "~x\n" }
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri, "version": 1 },
                    "contentChanges": [{ "text": "~> ;\n" }]
                }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let published = read_messages(&output)
            .into_iter()
            .filter(|message| message["method"] == "textDocument/publishDiagnostics")
            .collect::<Vec<_>>();

        assert_eq!(published.len(), 1);
        assert_eq!(published[0]["params"]["version"], 2);
        assert!(!published[0]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn diagnostics_match_compiler_messages_and_use_utf16_ranges_for_crlf_documents() {
        let uri = "file:///workspace/diagnostics.cg";
        let source = "// 🙂\r\n@main\r\n~> _x\r\n@end\r\n";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": { "uri": uri, "languageId": "codegrid", "version": 1, "text": source }
                }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let published = read_messages(&output)
            .into_iter()
            .find(|message| message["method"] == "textDocument/publishDiagnostics")
            .expect("opening a document must publish diagnostics");
        let diagnostics = published["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics must be an array");
        let compiler_diagnostics = codegrid_compiler::check(source);
        let index = LineIndex::new(source);

        assert_eq!(diagnostics.len(), compiler_diagnostics.len());
        for (actual, expected) in diagnostics.iter().zip(&compiler_diagnostics) {
            assert_eq!(actual["message"], expected.message);
            assert_eq!(
                actual["data"]["error_number"],
                json!(expected.error_number())
            );
            assert_eq!(
                actual["severity"],
                match expected.severity {
                    codegrid_syntax::Severity::Error => 1,
                    codegrid_syntax::Severity::Warning => 2,
                }
            );
            assert_eq!(
                actual["range"],
                super::range_json(source, &index, expected.span)
            );
        }
        assert_eq!(diagnostics[0]["range"]["start"]["line"], 2);
        assert_eq!(diagnostics[0]["range"]["start"]["character"], 3);
        assert_eq!(diagnostics[0]["range"]["end"]["character"], 5);
    }

    #[test]
    fn process_exit_requires_a_prior_shutdown_request() {
        let mut input = Vec::new();
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));
        let mut output = Vec::new();

        assert!(!serve(Cursor::new(input), &mut output).expect("exit notification is valid"));
        assert!(output.is_empty());
    }

    #[test]
    fn protocol_formatting_returns_a_single_whole_document_edit() {
        let uri = "file:///workspace/format.cg";
        let source = "@MAIN\n~v   _ _\n@END\n";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": { "uri": uri, "languageId": "codegrid", "version": 1, "text": source }
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/formatting",
                "params": {
                    "textDocument": { "uri": uri },
                    "options": { "tabSize": 4, "insertSpaces": true }
                }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let messages = read_messages(&output);
        let formatted = messages
            .iter()
            .find(|message| message["id"] == 2)
            .expect("format response must be present");
        assert_eq!(formatted["result"][0]["newText"], "@main\n~v _ _\n@end\n");
        assert_eq!(formatted["result"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn protocol_formatting_leaves_rejected_source_unchanged() {
        let uri = "file:///workspace/unsupported.cg";
        let source = "@main\n~> ?\n@end\n";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": uri, "version": 1, "text": source } }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/formatting",
                "params": { "textDocument": { "uri": uri } }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));
        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let response = read_messages(&output)
            .into_iter()
            .find(|message| message["id"] == 2)
            .expect("format response must be present");
        assert_eq!(response["result"], json!([]));
    }

    #[test]
    fn document_symbols_expose_the_single_main_board_with_utf16_ranges() {
        let uri = "file:///workspace/outline.cg";
        let source = "// café\r\n@main\r\n@size 2x1\r\n~> ;\r\n@end\r\n";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": { "uri": uri, "languageId": "codegrid", "version": 1, "text": source }
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/documentSymbol",
                "params": { "textDocument": { "uri": uri } }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let response = read_messages(&output)
            .into_iter()
            .find(|message| message["id"] == 2)
            .expect("document symbol response must be present");
        let symbol = &response["result"][0];
        assert_eq!(symbol["name"], "Main Board");
        assert_eq!(symbol["kind"], 2);
        assert_eq!(
            symbol["range"]["start"],
            json!({ "line": 1, "character": 0 })
        );
        assert_eq!(symbol["range"]["end"], json!({ "line": 4, "character": 4 }));
        assert_eq!(
            symbol["selectionRange"],
            json!({
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 5 }
            })
        );
    }

    #[test]
    fn implicit_main_board_outline_selects_its_first_cell() {
        let source = "// valid implicit board\n~> ;\n";
        let symbols = super::document_symbols(source, &LineIndex::new(source));
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0]["name"], "Main Board");
        assert_eq!(
            symbols[0]["selectionRange"]["start"],
            json!({ "line": 1, "character": 0 })
        );
        assert_eq!(
            symbols[0]["selectionRange"]["end"],
            json!({ "line": 1, "character": 2 })
        );
    }

    #[test]
    fn full_outline_and_symbol_navigation_cover_function_definitions_and_calls() {
        let uri = "file:///workspace/navigation.cg";
        let source = "@main\n~> [0\n@end main\n@main.F0\n~v ]\n@end main.F0\n";
        let symbols = super::document_symbols(source, &LineIndex::new(source));
        let names = symbols
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"Main Board"));
        assert!(names.contains(&"@main.F0"));

        let call_offset = source.find("[0").expect("source contains Function call") + 1;
        let definition = super::definition_location(uri, source, call_offset);
        assert_eq!(definition["uri"], uri);
        assert_eq!(definition["range"]["start"]["line"], 3);

        let references = super::references_locations(uri, source, call_offset, true);
        assert_eq!(references.as_array().map(Vec::len), Some(2));
        assert!(references
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["range"]["start"]["line"] == 3));
    }

    #[test]
    fn document_outline_omits_custom_only_programs_without_a_main_board() {
        let source = "@C0\n~> ;\n@end\n";
        assert!(super::document_symbols(source, &LineIndex::new(source)).is_empty());
    }

    #[test]
    fn initialize_advertises_full_source_navigation_features() {
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let messages = read_messages(&output);
        let capabilities = &messages[0]["result"]["capabilities"];
        assert_eq!(capabilities["hoverProvider"], true);
        assert_eq!(capabilities["definitionProvider"], true);
        assert_eq!(capabilities["referencesProvider"], true);
        assert_eq!(capabilities["documentSymbolProvider"], true);
        assert_eq!(capabilities["documentFormattingProvider"], true);
        assert!(capabilities["completionProvider"].is_object());
    }
    #[test]
    fn protocol_enforces_initialize_initialized_and_shutdown_order() {
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "shutdown" }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 5, "method": "textDocument/hover", "params": {} }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let messages = read_messages(&output);

        assert_eq!(messages[1]["error"]["code"], -32002);
        assert_eq!(messages[2]["error"]["code"], -32600);
        assert!(messages[3]["result"].is_null());
        assert_eq!(messages[4]["error"]["code"], -32600);
    }

    #[test]
    fn protocol_rejects_malformed_json_rpc_envelopes() {
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "1.0", "id": 2, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "exit" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(!serve(Cursor::new(input), &mut output).expect("invalid requests are recoverable"));
        let messages = read_messages(&output);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["id"], 1);
        assert_eq!(messages[0]["error"]["code"], -32600);
        assert_eq!(messages[1]["id"], 2);
        assert_eq!(messages[1]["error"]["code"], -32600);
        assert_eq!(messages[2]["id"], 3);
        assert_eq!(messages[2]["error"]["code"], -32600);
    }

    #[test]
    fn invalid_or_unversioned_incremental_changes_do_not_advance_document_state() {
        let uri = "file:///workspace/atomic-change.cg";
        let mut input = Vec::new();
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "method": "initialized" }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": uri, "version": 1, "text": "~x\n" } }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri },
                    "contentChanges": [{ "text": "~> ;\n" }]
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [
                        { "text": "~> ;\n" },
                        {
                            "range": {
                                "start": { "line": 20, "character": 0 },
                                "end": { "line": 20, "character": 0 }
                            },
                            "text": "ignored"
                        }
                    ]
                }
            }),
        );
        append_message(
            &mut input,
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [{ "text": "~x\n" }]
                }
            }),
        );
        append_message(
            &mut input,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        );
        append_message(&mut input, json!({ "jsonrpc": "2.0", "method": "exit" }));

        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output).expect("server protocol should run"));
        let published = read_messages(&output)
            .into_iter()
            .filter(|message| message["method"] == "textDocument/publishDiagnostics")
            .collect::<Vec<_>>();

        assert_eq!(published.len(), 2);
        assert_eq!(published[1]["params"]["version"], 2);
        assert!(!published[1]["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn incremental_change_batches_apply_each_range_to_the_updated_text() {
        let uri = "file:///workspace/ordered-change.cg";
        let mut server = Server {
            lifecycle: Lifecycle::Running,
            ..Server::default()
        };
        let mut output = Vec::new();
        server
            .handle_notification(
                "textDocument/didOpen",
                &json!({
                    "textDocument": { "uri": uri, "version": 1, "text": "~x\n" }
                }),
                &mut output,
            )
            .expect("open notification should be handled");
        server
            .handle_notification(
                "textDocument/didChange",
                &json!({
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 1 },
                                "end": { "line": 0, "character": 2 }
                            },
                            "text": "> + _"
                        },
                        {
                            "range": {
                                "start": { "line": 0, "character": 3 },
                                "end": { "line": 0, "character": 4 }
                            },
                            "text": ";"
                        }
                    ]
                }),
                &mut output,
            )
            .expect("change notification should be handled");

        let document = server.documents.get(uri).expect("document remains open");
        assert_eq!(document.version, Some(2));
        assert_eq!(document.text, "~> ; _\n");
    }
}
