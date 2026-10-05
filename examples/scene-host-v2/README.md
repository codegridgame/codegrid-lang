# Scene Host Contract v2 Examples

Design data for the [Scene Host Contract v2](../../spec/codegrid-scene-host-contract-v2.md). The Rust `SafetyProfileV2` loader accepts the profile example; current v1 API/profile loaders reject it. The native `LevelApiV2` session accepts API-2 requests. Browser/server v2 transports are implemented; actual execution evidence comes from the separate scene conformance fixtures. These documents do not demonstrate execution or host parity.

- [profile-local-v2.json](profile-local-v2.json): every required v2 trusted field; local illustrative ceilings, not production policy.
- [feedback-request.json](feedback-request.json): a complete API-2 Debug feedback request; handle 1 is illustrative and must be a live owned evaluation handle.
- [feedback-data.json](feedback-data.json): operation data only, to be carried in the versioned ok envelope; two visible Robot initialization events.
- [visible-case-failure.json](visible-case-failure.json): a visible_cases element only, showing InvalidOutput for B after a complete frame; it is not a complete evaluation result.

All u64 values use canonical decimal strings; byte/version/geometry fields use numbers. Full envelope/result common fields follow the retained Level API rules. Never submit a response fragment as an author file or mistake these logical handles for durable IDs.
