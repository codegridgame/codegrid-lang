# Shared error translations

`codegrid-error-messages.json` is the single authored translation catalog for
all 249 registered four-digit error numbers and ten supported languages.
Each entry preserves its registry layer and symbolic or transport code.
Message wording is presentation data; error detection uses the stable identity.

Supported locales are `en`, `de`, `fr`, `es`, `zh-tw`, `ja`, `zh-cn`, `ko`,
`pt-br`, and `ru`. Every error contains a complete message in every locale.
For example, a JSON consumer can read
`catalog.errors["1004"].messages["zh-cn"]`.

Run `node scripts/generate_error_messages.js` after editing this file. It validates
coverage against the [error registry](../spec/codegrid-error-codes.json) and
generates the Rust table and the packaged VS Code JSON copy. Never edit those
generated copies. `node scripts/check_error_codes.js` checks that they are current.

Rust consumers use `codegrid_model::error_message("1004", "zh-CN")`.
The runtime API and level API re-export this function so their adapters do not
need additional dependencies. Resolve symbolic identities with
`error_number(layer, code)` first. Unknown numbers return `None`; use
`fallback_error_message(locale)` for a generic summary.

Editor consumers use `errorMessage(number, locale)` or
`errorMessageByCode(layer, code, locale)` from `src/language/errorCatalog.ts`.
The latter requires a layer because transport aliases can share spellings.
Unknown identities return the catalog's localized generic fallback.

Both lookup implementations normalize case, underscores, and regional language
tags. Chinese `Hant`, `TW`, `HK`, and `MO` select `zh-tw`; other Chinese tags
select `zh-cn`. Portuguese tags select the supported `pt-br` translation.
Unsupported languages select English. Hosts pass their locale explicitly;
the language core never reads an operating-system locale.

The editor uses these summaries at error display boundaries. Original messages,
source positions, structured details, and protocol results remain available.
English display preserves a supplied original message with its dynamic details.
Other hosts can use the same catalog without changing their raw error contract.
