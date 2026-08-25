# Tasks


## Extension

- [ ] Bump `editors/code/package.json` version to 0.1.0 and add `repository.directory`, `keywords`, `icon` field
- [ ] Verify `engines.vscode` vs `@types/vscode` API compatibility
- [ ] Write real 0.1.0 CHANGELOG entry (replace placeholder)
- [ ] Exclude `out/test/**` in `editors/code/.vscodeignore`
- [ ] Update CI (`vscode-extension` job) to build per-platform vsix via `vsce package --target <platform>` instead of one universal vsix
- [ ] Document manual publish runbook (publisher/PAT setup, `vsce login`, `vsce publish --target` per platform)
- [ ] Source an extension icon (manual, blocking first publish)
- [ ] Register Marketplace publisher `ttiimm` + generate PAT (manual, external)

- [ ] Work with non .log extension (.json, etc).
- [ ] Visualize the exceptions/traces
- [ ] Support src -> log breakpoints

## Languages

- [ ] Go
- [ ] JavaScript
- [ ] Typescript
