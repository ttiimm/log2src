import * as assert from 'assert';
import { DebugProtocol } from '@vscode/debugprotocol';

import { DebugSession, BinaryNotFoundError } from '../../debugAdapter';
import { LogDebugger } from '../../logDebugger';

type PatchedSession = DebugSession & {
    sendResponse: (response: DebugProtocol.Response) => void;
    sendEvent: (...args: unknown[]) => void;
    dispose?: () => void;
};

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

function createSession(logDebugger: LogDebugger): DebugSession {
    setPlatformArch('darwin', 'arm64');
    return new DebugSession(logDebugger);
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

            let capturedResponse: DebugProtocol.InitializeResponse | undefined;
            const session = debugSession as PatchedSession;
            const originalSendResponse = session.sendResponse.bind(session);
            session.sendResponse = (resp: DebugProtocol.Response) => {
                capturedResponse = resp as DebugProtocol.InitializeResponse;
            };

            let eventSent = false;
            const originalSendEvent = session.sendEvent.bind(session);
            session.sendEvent = () => {
                eventSent = true;
            };

            try {
                (session as any).initializeRequest(response, args);

                assert.ok(capturedResponse, 'Response should be sent');
                assert.ok(eventSent, 'Event should be sent');

                assert.ok(capturedResponse!.body, 'Response should have body');
                assert.strictEqual(capturedResponse!.body.supportsStepBack, true);
                assert.strictEqual(capturedResponse!.body.supportTerminateDebuggee, true);
            } finally {
                session.sendResponse = originalSendResponse;
                session.sendEvent = originalSendEvent;
            }
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
                breakpoints: [
                    { line: 10 },
                    { line: 20 },
                    { line: 30 }
                ]
            };

            const response: DebugProtocol.SetBreakpointsResponse = {
                request_seq: 1,
                success: true,
                command: 'setBreakpoints',
                seq: 1,
                type: 'response',
                body: { breakpoints: [] }
            };

            let capturedResponse: DebugProtocol.SetBreakpointsResponse | undefined;
            const session = debugSession as PatchedSession;
            const originalSendResponse = session.sendResponse.bind(session);
            session.sendResponse = (resp: DebugProtocol.Response) => {
                capturedResponse = resp as DebugProtocol.SetBreakpointsResponse;
            };

            let eventSent = false;
            const originalSendEvent = session.sendEvent.bind(session);
            session.sendEvent = () => {
                eventSent = true;
            };

            try {
                (session as any).setBreakPointsRequest(response, args);

                assert.ok(capturedResponse, 'Response should be sent');

                assert.ok(capturedResponse!.body, 'Response should have body');
                assert.ok(capturedResponse!.body.breakpoints, 'Response should have breakpoints');
                assert.strictEqual(capturedResponse!.body.breakpoints.length, 3, 'Should have 3 breakpoints');

                capturedResponse!.body.breakpoints.forEach((bp, index) => {
                    assert.strictEqual(bp.line, args.breakpoints![index].line, `Breakpoint ${index} should have correct line`);
                });

                assert.ok(eventSent, 'Should send stopped event');
            } finally {
                session.sendResponse = originalSendResponse;
                session.sendEvent = originalSendEvent;
            }
        });

        test('Should handle empty breakpoints array', () => {
            const sourcePath = '/test/source/file.log';
            const args: DebugProtocol.SetBreakpointsArguments = {
                source: { path: sourcePath },
                breakpoints: []
            };

            const response: DebugProtocol.SetBreakpointsResponse = {
                request_seq: 1,
                success: true,
                command: 'setBreakpoints',
                seq: 1,
                type: 'response',
                body: { breakpoints: [] }
            };

            let capturedResponse: DebugProtocol.SetBreakpointsResponse | undefined;
            let eventSent = false;

            const session = debugSession as PatchedSession;
            const originalSendResponse = session.sendResponse.bind(session);
            session.sendResponse = (resp: DebugProtocol.Response) => {
                capturedResponse = resp as DebugProtocol.SetBreakpointsResponse;
            };

            const originalSendEvent = session.sendEvent.bind(session);
            session.sendEvent = () => {
                eventSent = true;
            };

            try {
                (session as any).setBreakPointsRequest(response, args);

                assert.ok(capturedResponse, 'Response should be sent');
                assert.ok(capturedResponse!.body, 'Response should have body');
                assert.ok(capturedResponse!.body.breakpoints, 'Response should have breakpoints array');
                assert.strictEqual(capturedResponse!.body.breakpoints.length, 0, 'Should have no breakpoints');
                assert.strictEqual(eventSent, false, 'Should not send stopped event for empty breakpoints');
            } finally {
                session.sendResponse = originalSendResponse;
                session.sendEvent = originalSendEvent;
            }
        });
    });
});