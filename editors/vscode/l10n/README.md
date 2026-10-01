# Editor localization

The game language inventory is `locales.json`: English plus German, French,
Spanish (Spain), Traditional Chinese, Japanese, Simplified Chinese, Korean,
Portuguese (Brazil), and Russian. This inventory follows the user's recorded
language selection for the editor.

VS Code uses `package.nls.json` and `package.nls.<locale>.json` for manifest
strings. Runtime presentation uses `vscode.l10n.t` with the bundles in this
directory, declared by the package's `l10n` property. English source messages
are the runtime catalog keys. Positional arguments use `{0}`, `{1}`, and so on.
Unsupported display languages and unknown host details fall back to English.

The VS Code identifiers are `en`, `de`, `fr`, `es`, `zh-tw`, `ja`, `zh-cn`, `ko`,
`pt-br`, and `ru`. Spanish uses VS Code's `es` identifier; Portuguese is
specifically Brazilian Portuguese. UI text follows the VS Code display language
after restart. No language setting is sent to the compiler or VM.

Translate presentation text only. Preserve instruction spellings and canonical
names, stable error codes, source examples, configuration properties, watch
paths, DAP request names, and serialized VM fields. The shared catalog
[`resources/codegrid-error-messages.json`](../../../resources/codegrid-error-messages.json)
contains all 249 error numbers in ten languages. `codegrid-error-messages.json`
in this directory is its generated packaging copy; edit only the shared source
and run `node scripts/generate_error_messages.js` from the repository root.
Error summaries use the shared lookup instead of VS Code UI bundles.
Core and transport messages remain unchanged;
the editor translates at display boundaries. LSP related information and
`CodedError.originalMessage` preserve original diagnostic text. Structured VM
error details remain available for inspection. Never classify an error from its
localized message.

The localization suite checks all catalogs for key coverage, placeholders, and
stable editor codes. To exercise an actual translated extension host, install
the matching VS Code language pack into `.vscode-test/localization-extensions`
using `.vscode-test/localization-user` as its user-data directory, set
`CODEGRID_TEST_LOCALE`, and run `npm test`. Locale runs select the localization
suite; an ordinary `npm test` still runs the complete editor suite.
