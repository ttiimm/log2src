# Tasks

- [ ] Handle running CLI with no log format.
  - TSS: Doesn't this work already?  I echo
    the body of the log message into log2src
    and it can find the message.
- [ ] Extract a thread id from log when available and associate with source ref.
- [ ] Generate call stack from exceptions.
- [ ] Support multiple source roots from CLI.
- [ ] Serialize state for re-use on subsequent executions

## Extension

- [ ] Work with non .log extension (.json, etc).
- [ ] Basic test coverage
- [ ] Support src -> log breakpoints

## Languages

- [X] Python
- [ ] Go
- [ ] JavaScript
- [ ] Typescript
