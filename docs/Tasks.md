# Tasks


## Extension

- [x] Bump `editors/code/package.json` version to 0.1.0 and add `repository.directory`, `keywords`, `icon` field
- [x] Verify `engines.vscode` vs `@types/vscode` API compatibility
- [x] Write real 0.1.0 CHANGELOG entry (replace placeholder)
- [x] Exclude `out/test/**` in `editors/code/.vscodeignore`
- [x] Update CI (`vscode-extension` job) to build per-platform vsix via `vsce package --target <platform>` instead of one universal vsix
- [x] Source an extension icon (manual, blocking first publish)
- [ ] Register Marketplace publisher `ttiimm` + generate PAT (manual, external)

- [ ] Work with non .log extension (.json, etc).
- [ ] Visualize the exceptions/traces
- [ ] Support src -> log breakpoints

## Languages

- [ ] Go
- [ ] JavaScript
- [ ] Typescript
