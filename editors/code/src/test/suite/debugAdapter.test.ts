import * as assert from 'assert';
import { DebugProtocol } from '@vscode/debugprotocol';

import { DebugSession, BinaryNotFoundError } from '../../debugAdapter';


suite('DebugAdapter Test Suite', () => {
    let debugSession: DebugSession | undefined;
    let originalPlatform: string;
    let originalArch: string;

    setup(() => {
        originalPlatform = process.platform;
        originalArch = process.arch;
    });

    teardown(() => {
        Object.defineProperty(process, 'platform', {
            value: originalPlatform,
            configurable: true
        });
        Object.defineProperty(process, 'arch', {
            value: originalArch,
            configurable: true
        });

        if (debugSession) {
            try {
                (debugSession as any).dispose?.();
            } catch (e) {
                // Ignore
            }
            debugSession = undefined;
        }
    });

    suite('Constructor Tests', () => {
        test('Should create debug session with correct binary path for darwin-arm64', () => {
            Object.defineProperty(process, 'platform', {
                value: 'darwin',
                configurable: true
            });
            Object.defineProperty(process, 'arch', {
                value: 'arm64',
                configurable: true
            });

            debugSession = new DebugSession();

            assert.ok(debugSession, 'Debug session should be created');
        });

        test('Should create debug session with correct binary path for linux-x64', () => {
            Object.defineProperty(process, 'platform', {
                value: 'linux',
                configurable: true
            });
            Object.defineProperty(process, 'arch', {
                value: 'x64',
                configurable: true
            });

            debugSession = new DebugSession();

            assert.ok(debugSession, 'Debug session should be created');
        });

        test('Should create debug session with correct binary path for win32-x64', () => {
            Object.defineProperty(process, 'platform', {
                value: 'win32',
                configurable: true
            });
            Object.defineProperty(process, 'arch', {
                value: 'x64',
                configurable: true
            });

            debugSession = new DebugSession();

            assert.ok(debugSession, 'Debug session should be created');
        });

        test('Should throw BinaryNotFoundError for unsupported platform', () => {
            Object.defineProperty(process, 'platform', {
                value: 'unsupported',
                configurable: true
            });
            Object.defineProperty(process, 'arch', {
                value: 'unsupported',
                configurable: true
            });

            assert.throws(() => {
                new DebugSession();
            }, BinaryNotFoundError, 'Should throw BinaryNotFoundError for unsupported platform');
        });
    });

    suite('Initialize Request Tests', () => {
        setup(() => {
            Object.defineProperty(process, 'platform', {
                value: 'darwin',
                configurable: true
            });
            Object.defineProperty(process, 'arch', {
                value: 'arm64',
                configurable: true
            });
            debugSession = new DebugSession();
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
            const originalSendResponse = (debugSession as any).sendResponse.bind(debugSession);
            (debugSession as any).sendResponse = (resp: any) => {
                capturedResponse = resp;
            };

            let eventSent = false;
            const originalSendEvent = (debugSession as any).sendEvent.bind(debugSession);
            (debugSession as any).sendEvent = () => {
                eventSent = true;
            };

            try {
                (debugSession as any).initializeRequest(response, args);

                assert.ok(capturedResponse, 'Response should be sent');
                assert.ok(eventSent, 'Event should be sent');

                assert.ok(capturedResponse!.body, 'Response should have body');
                assert.strictEqual(capturedResponse!.body.supportsStepBack, true);
                assert.strictEqual(capturedResponse!.body.supportTerminateDebuggee, true);
            } finally {
                (debugSession as any).sendResponse = originalSendResponse;
                (debugSession as any).sendEvent = originalSendEvent;
            }
        });
    });

    suite('Breakpoint Tests', () => {
        setup(() => {
            Object.defineProperty(process, 'platform', {
                value: 'darwin',
                configurable: true
            });
            Object.defineProperty(process, 'arch', {
                value: 'arm64',
                configurable: true
            });
            debugSession = new DebugSession();
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
            const originalSendResponse = (debugSession as any).sendResponse.bind(debugSession);
            (debugSession as any).sendResponse = (resp: any) => {
                capturedResponse = resp;
            };

            let eventSent = false;
            const originalSendEvent = (debugSession as any).sendEvent.bind(debugSession);
            (debugSession as any).sendEvent = () => {
                eventSent = true;
            };

            try {
                (debugSession as any).setBreakPointsRequest(response, args);

                assert.ok(capturedResponse, 'Response should be sent');

                assert.ok(capturedResponse!.body, 'Response should have body');
                assert.ok(capturedResponse!.body.breakpoints, 'Response should have breakpoints');
                assert.strictEqual(capturedResponse!.body.breakpoints.length, 3, 'Should have 3 breakpoints');

                capturedResponse!.body.breakpoints.forEach((bp, index) => {
                    assert.strictEqual(bp.line, args.breakpoints![index].line, `Breakpoint ${index} should have correct line`);
                });

                assert.ok(eventSent, 'Should send stopped event');
            } finally {
                (debugSession as any).sendResponse = originalSendResponse;
                (debugSession as any).sendEvent = originalSendEvent;
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

            const originalSendResponse = (debugSession as any).sendResponse.bind(debugSession);
            (debugSession as any).sendResponse = (resp: any) => {
                capturedResponse = resp;
            };

            const originalSendEvent = (debugSession as any).sendEvent.bind(debugSession);
            (debugSession as any).sendEvent = () => {
                eventSent = true;
            };

            try {
                (debugSession as any).setBreakPointsRequest(response, args);

                assert.ok(capturedResponse, 'Response should be sent');
                assert.ok(capturedResponse!.body, 'Response should have body');
                assert.ok(capturedResponse!.body.breakpoints, 'Response should have breakpoints array');
                assert.strictEqual(capturedResponse!.body.breakpoints.length, 0, 'Should have no breakpoints');
                assert.strictEqual(eventSent, false, 'Should not send stopped event for empty breakpoints');
            } finally {
                (debugSession as any).sendResponse = originalSendResponse;
                (debugSession as any).sendEvent = originalSendEvent;
            }
        });
    });
});