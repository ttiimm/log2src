import * as assert from 'assert';
import { DebugProtocol } from '@vscode/debugprotocol';

import {
    BinaryNotFoundError,
    DebugSession,
    EditorEffects,
    ILaunchRequestArguments,
    OutputSink,
    ProcessRunner
} from '../../debugAdapter';
import { LogDebugger } from '../../logDebugger';

type PatchedSession = DebugSession & {
    sendResponse: (response: DebugProtocol.Response) => void;
    sendEvent: (...args: unknown[]) => void;
    dispose?: () => void;
};

interface RequestCapture<R extends DebugProtocol.Response> {
    response: R | undefined;
    eventCount: number;
}

/**
 * Patches sendResponse and sendEvent on a session, invokes fn, then restores
 * the originals. Returns the captured response and number of events sent.
 */
function captureRequest<R extends DebugProtocol.Response>(
    session: PatchedSession,
    fn: () => void
): RequestCapture<R> {
    let response: R | undefined;
    let eventCount = 0;

    const originalSendResponse = session.sendResponse.bind(session);
    const originalSendEvent = session.sendEvent.bind(session);
    session.sendResponse = (resp: DebugProtocol.Response) => { response = resp as R; };
    session.sendEvent = () => { eventCount++; };
    try {
        fn();
    } finally {
        session.sendResponse = originalSendResponse;
        session.sendEvent = originalSendEvent;
    }
    return { response, eventCount };
}

function setPlatformArch(platform: NodeJS.Platform | string, arch: string): void {
    Object.defineProperty(process, 'platform', {
        value: platform,
        configurable: true
    });
    Object.defineProperty(process, 'arch', {
        value: arch,
        configurable: true
    });
}

function createSession(
    logDebugger: LogDebugger,
    processRunner?: ProcessRunner,
    editorEffects?: EditorEffects
): DebugSession {
    setPlatformArch('darwin', 'arm64');
    const effects = editorEffects ?? {
        openAndFocus: (_log: string, _line: number): void => { },
        highlightLine: (_log: string, _line: number): void => { },
        clearHighlights: (): void => { }
    };
    const runner = processRunner ?? {
        execFileSync: (_file: string, _args: string[]): Buffer => Buffer.alloc(0),
        readFile: (_path: string): Buffer => Buffer.alloc(0)
    };
    const output: OutputSink = {
        appendLine: (message: string): void => console.log(message)
    }
    return new DebugSession(logDebugger, effects, output, runner);
}


suite('DebugAdapter Test Suite', () => {
    let debugSession: DebugSession | undefined;
    let logDebugger: LogDebugger;
    let originalPlatform: string;
    let originalArch: string;

    setup(() => {
        originalPlatform = process.platform;
        originalArch = process.arch;
        logDebugger = new LogDebugger();
    });

    teardown(() => {
        setPlatformArch(originalPlatform, originalArch);

        if (debugSession) {
            try {
                (debugSession as PatchedSession).dispose?.();
            } catch (e) {
                // Ignore
            }
            debugSession = undefined;
        }
    });

    suite('Constructor Tests', () => {
        test('Should create debug session with correct binary path for darwin-arm64', () => {
            setPlatformArch('darwin', 'arm64');

            debugSession = new DebugSession(logDebugger);

            assert.ok(debugSession, 'Debug session should be created');
        });

        test('Should create debug session with correct binary path for linux-x64', () => {
            setPlatformArch('linux', 'x64');

            debugSession = new DebugSession(logDebugger);

            assert.ok(debugSession, 'Debug session should be created');
        });

        test('Should create debug session with correct binary path for win32-x64', () => {
            setPlatformArch('win32', 'x64');

            debugSession = new DebugSession(logDebugger);

            assert.ok(debugSession, 'Debug session should be created');
        });

        test('Should throw BinaryNotFoundError for unsupported platform', () => {
            setPlatformArch('unsupported', 'unsupported');

            assert.throws(() => {
                new DebugSession(logDebugger);
            }, BinaryNotFoundError, 'Should throw BinaryNotFoundError for unsupported platform');
        });
    });

    suite('Initialize Request Tests', () => {
        setup(() => {
            debugSession = createSession(logDebugger);
        });

        test('Should handle initialize request correctly', () => {
            const args: DebugProtocol.InitializeRequestArguments = {
                clientID: 'test-client',
                clientName: 'Test Client',
                adapterID: 'log2src',
                pathFormat: 'path'
            };
            const response: DebugProtocol.InitializeResponse = {
                request_seq: 1,
                success: true,
                command: 'initialize',
                seq: 1,
                type: 'response',
                body: {}
            };

            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest<DebugProtocol.InitializeResponse>(
                session,
                () => (session as any).initializeRequest(response, args)
            );

            assert.ok(captured, 'Response should be sent');
            assert.ok(captured!.body, 'Response should have body');
            assert.strictEqual(captured!.body.supportsStepBack, true);
            assert.strictEqual(captured!.body.supportTerminateDebuggee, true);
            assert.strictEqual(eventCount, 1, 'InitializedEvent should be sent');
        });
    });

    suite('Breakpoint Tests', () => {
        setup(() => {
            debugSession = createSession(logDebugger);
        });

        test('Should set breakpoints correctly', () => {
            const sourcePath = '/test/source/file.log';
            const args: DebugProtocol.SetBreakpointsArguments = {
                source: { path: sourcePath },
                breakpoints: [{ line: 10 }, { line: 20 }, { line: 30 }]
            };
            const response: DebugProtocol.SetBreakpointsResponse = {
                request_seq: 1, success: true, command: 'setBreakpoints',
                seq: 1, type: 'response', body: { breakpoints: [] }
            };

            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest<DebugProtocol.SetBreakpointsResponse>(
                session,
                () => (session as any).setBreakPointsRequest(response, args)
            );

            assert.ok(captured, 'Response should be sent');
            assert.ok(captured!.body.breakpoints, 'Response should have breakpoints');
            assert.strictEqual(captured!.body.breakpoints.length, 3, 'Should have 3 breakpoints');
            captured!.body.breakpoints.forEach((bp, index) => {
                assert.strictEqual(bp.line, args.breakpoints![index].line, `Breakpoint ${index} should have correct line`);
            });
            assert.strictEqual(eventCount, 1, 'Should send stopped event');
        });

        test('Should handle empty breakpoints array', () => {
            const sourcePath = '/test/source/file.log';
            const args: DebugProtocol.SetBreakpointsArguments = {
                source: { path: sourcePath },
                breakpoints: []
            };
            const response: DebugProtocol.SetBreakpointsResponse = {
                request_seq: 1, success: true, command: 'setBreakpoints',
                seq: 1, type: 'response', body: { breakpoints: [] }
            };

            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest<DebugProtocol.SetBreakpointsResponse>(
                session,
                () => (session as any).setBreakPointsRequest(response, args)
            );

            assert.ok(captured, 'Response should be sent');
            assert.strictEqual(captured!.body.breakpoints.length, 0, 'Should have no breakpoints');
            assert.strictEqual(eventCount, 0, 'Should not send stopped event for empty breakpoints');
            assert.strictEqual(logDebugger.hasBreakpoints(), false, 'LogDebugger should report no breakpoints');
        });
    });

    suite('Launch Request Tests', () => {
        let logPath: string;
        let args: ILaunchRequestArguments;
        let response: DebugProtocol.LaunchResponse;
        let processRunner: ProcessRunner;
        let openAndFocusCalled: number;
        let focusedLog: string | undefined;
        let focusedLine: number | undefined;
        let editorEffects: EditorEffects;

        setup(() => {
            logPath = '/test/source/file.log';
            args = {
                source: '/test/source/file.rs',
                log: logPath,
                log_format: '',
                trace: false,
                noDebug: false
            };

            response = {
                request_seq: 1,
                success: true,
                command: 'launch',
                seq: 1,
                type: 'response'
            };

            processRunner = {
                execFileSync: (_file: string, _args: string[]): Buffer => Buffer.alloc(0),
                readFile: (_path: string): Buffer => Buffer.from('line1\nline2\n')
            };

            openAndFocusCalled = 0;
            editorEffects = {
                openAndFocus: (log: string, line: number): void => {
                    openAndFocusCalled++;
                    focusedLog = log;
                    focusedLine = line;
                },
                highlightLine: (_log: string, _line: number): void => { },
                clearHighlights: (): void => { }
            };
        });

        test('sends entry event when no breakpoints', () => {
            debugSession = createSession(logDebugger, processRunner, editorEffects);
            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest<DebugProtocol.LaunchResponse>(
                session,
                () => (session as any).launchRequest(response, args)
            );

            assert.ok(captured, 'Launch response should be sent');
            assert.strictEqual(eventCount, 1, 'Should send entry stopped event when no breakpoints are set');
            assert.strictEqual(openAndFocusCalled, 1, 'Should open and focus log once');
            assert.strictEqual(focusedLog, logPath, 'Should focus the launched log file');
            assert.strictEqual(focusedLine, logDebugger.linenum(), 'Should focus current debugger line');
        });

        test('does not send entry event when breakpoint', () => {
            logDebugger.setBreakpoints(logPath, [{ line: 1 }]);

            debugSession = createSession(logDebugger, processRunner, editorEffects);
            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest<DebugProtocol.LaunchResponse>(
                session,
                () => (session as any).launchRequest(response, args)
            );

            assert.ok(captured, 'Launch response should be sent');
            assert.strictEqual(eventCount, 0, 'Should not send entry stopped event when no breakpoints are set');
            assert.strictEqual(openAndFocusCalled, 1, 'Should open and focus log once');
            assert.strictEqual(focusedLog, logPath, 'Should focus the launched log file');
            assert.strictEqual(focusedLine, logDebugger.linenum(), 'Should focus current debugger line');
        });
    });

    suite('Continue/Stepping Request Tests', () => {
        setup(() => {
            const logPath = "path-to-log";
            debugSession = createSession(logDebugger);
            logDebugger.setToLog(logPath, 5);
            logDebugger.setBreakpoints(logPath, [{line: 1}, {line: 3}]);
        });

        test('continue moves to next breakpoint', () => {
            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest(
                session,
                () => (session as any).continueRequest({body: undefined}, {threadId: 1})
            );

            assert.ok(captured);
            assert.strictEqual(eventCount, 1);
            assert.strictEqual(logDebugger.linenum(), 3);
        });

        test('reverse continue moves to previous breakpoint', () => {
            logDebugger.gotoBreakpoint();
            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest(
                session,
                () => (session as any).reverseContinueRequest({body: undefined}, {threadId: 1})
            );

            assert.ok(captured);
            assert.strictEqual(eventCount, 1);
            assert.strictEqual(logDebugger.linenum(), 1);
        });

        test('next request moves to next line', () => {
            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest(
                session,
                () => (session as any).nextRequest({body: undefined}, {threadId: 1})
            );

            assert.ok(captured);
            assert.strictEqual(eventCount, 1);
            assert.strictEqual(logDebugger.linenum(), 2);
        });

        test('step back request moves to previous line', () => {
            logDebugger.gotoBreakpoint();
            const session = debugSession as PatchedSession;
            const { response: captured, eventCount } = captureRequest(
                session,
                () => (session as any).stepBackRequest({body: undefined}, {threadId: 1})
            );

            assert.ok(captured);
            assert.strictEqual(eventCount, 1);
            assert.strictEqual(logDebugger.linenum(), 2);
        });
    });

    suite('Stack Trace Request Tests', () => {
        let logPath: string;
        let launchArgs: ILaunchRequestArguments;
        let args: DebugProtocol.StackTraceArguments;
        let response: DebugProtocol.StackTraceResponse;
        let processRunner: ProcessRunner;
        let openAndFocusCalled: number;
        let focusedLog: string | undefined;
        let focusedLine: number | undefined;
        let editorEffects: EditorEffects;

        setup(() => {
            logPath = '/test/source/file.log';
            args = {
                threadId: 1
            };

            response = {
                request_seq: 2,
                success: true,
                command: 'stackTrace',
                seq: 2,
                type: 'response',
                body: {
                    stackFrames: [],
                    totalFrames: 0
                }
            };

            processRunner = {
                execFileSync: (_file: string, _args: string[]): Buffer => Buffer.from(
`
{
    "logRef": {},
    "srcRef": {
        "sourcePath": "l2s-example/examples/basic.rs",
        "language": "Rust",
        "lineNumber": 13,
        "endLineNumber": 13,
        "column": 11,
        "name": "foo",
        "text": "\\"Hello from foo i={}\\"",
        "quality": 14,
        "pattern": "(?s)^Hello from foo i=(.+)$",
        "args": ["Placeholder"],
        "vars": ["i"]
    },
    "variables": [
        {
            "expr": "i",
            "value": "1"
        }
    ]
}
`
                ),
                readFile: (_path: string): Buffer => Buffer.alloc(0),
            };

            openAndFocusCalled = 0;
            editorEffects = {
                openAndFocus: (log: string, line: number): void => {
                    openAndFocusCalled++;
                    focusedLog = log;
                    focusedLine = line;
                },
                highlightLine: (_log: string, _line: number): void => { },
                clearHighlights: (): void => { }
            };
            launchArgs = {
                source: 'l2s-example/examples/basic.rs',
                log: logPath,
                log_format: '^\\[\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z \\w+ \\w+\\]\\s+(?<body>.*)$',
                trace: false,
                noDebug: false
            };
        });

        test('stack trace request with log format', () => {
            debugSession = createSession(logDebugger, processRunner, editorEffects);
            const session = debugSession as PatchedSession;
            (session as any).primeLaunchState(launchArgs);

            const { response: captured, eventCount } = captureRequest<DebugProtocol.StackTraceResponse>(
                session,
                () => (session as any).stackTraceRequest(response, args)
            );

            assert.ok(captured, 'Stack trace response should be sent');
            assert.ok(captured!.body, 'Stack trace response should include a body');
            assert.strictEqual(captured!.body.stackFrames.length, 1, 'Should have one stack frame');
            assert.strictEqual(captured!.body.totalFrames, 1, 'Should report one total frame');
            assert.strictEqual(captured!.body.stackFrames[0].name, 'foo', 'Frame name should come from srcRef');
            assert.strictEqual(
                captured!.body.stackFrames[0].source?.path,
                'l2s-example/examples/basic.rs',
                'Frame source path should match srcRef'
            );
            assert.strictEqual(captured!.body.stackFrames[0].source?.name, 'basic.rs', 'Frame source name should be basename');
            assert.strictEqual(captured!.body.stackFrames[0].line, 13, 'Frame line should match mapped line number');
            assert.strictEqual(eventCount, 0, 'No events should be sent');
            // once for primeLaunchState and another for when stackTraceRequest is called
            assert.strictEqual(openAndFocusCalled, 2, 'Should open and focus log once');
            assert.strictEqual(focusedLog, logPath, 'Should focus the launched log file');
            assert.strictEqual(focusedLine, logDebugger.linenum(), 'Should focus current debugger line');
        });
    });
});
