import * as assert from 'assert';
import { LogDebugger } from '../../logDebugger';

suite('LogDebugger Test Suite', () => {
    let logDebugger: LogDebugger;

    setup(() => {
        logDebugger = new LogDebugger();
    });

    suite('Line navigation', () => {
        test('starts at line 1', () => {
            assert.strictEqual(logDebugger.linenum(), 1);
        });

        test('stepForward increments line', () => {
            logDebugger.setToLog('/fake/log', 100);
            logDebugger.stepForward();
            assert.strictEqual(logDebugger.linenum(), 2);
        });

        test('stepForward clamps at logLines', () => {
            logDebugger.setToLog('/fake/log', 3);
            logDebugger.stepForward();
            logDebugger.stepForward();
            logDebugger.stepForward();
            assert.strictEqual(logDebugger.linenum(), 3);
        });

        test('stepBackward decrements line', () => {
            logDebugger.setToLog('/fake/log', 100);
            logDebugger.stepForward();
            logDebugger.stepForward();
            logDebugger.stepBackward();
            assert.strictEqual(logDebugger.linenum(), 2);
        });

        test('stepBackward clamps at line 1', () => {
            logDebugger.setToLog('/fake/log', 100);
            logDebugger.stepBackward();
            assert.strictEqual(logDebugger.linenum(), 1);
        });
    });

    suite('Breakpoints', () => {
        const logPath = '/fake/log.log';

        test('hasBreakpoints is false with no breakpoints set', () => {
            logDebugger.setToLog(logPath, 100);
            assert.strictEqual(logDebugger.hasBreakpoints(), false);
        });

        test('hasBreakpoints is true after breakpoints set', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakpoints(logPath, [{ line: 10 }]);
            assert.ok(logDebugger.hasBreakpoints());
        });

        test('hasBreakpoints is false after empty breakpoints set', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakpoints(logPath, []);
            assert.strictEqual(logDebugger.hasBreakpoints(), false);
        });

        test('gotoNextBreakpoint advances to next breakpoint', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakpoints(logPath, [{ line: 10 }, { line: 20 }, { line: 30 }]);
            assert.strictEqual(logDebugger.linenum(), 1);
            logDebugger.gotoBreakpoint();
            assert.strictEqual(logDebugger.linenum(), 10);
            logDebugger.gotoBreakpoint();
            assert.strictEqual(logDebugger.linenum(), 20);
        });

        test('gotoNextBreakpoint reverse retreats to previous breakpoint', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakpoints(logPath, [{ line: 10 }, { line: 20 }, { line: 30 }]);
            logDebugger.gotoBreakpoint();   // 1 → 10
            logDebugger.gotoBreakpoint();   // 10 → 20
            logDebugger.gotoBreakpoint(true); // 20 → 10
            assert.strictEqual(logDebugger.linenum(), 10);
        });

        test('gotoNextBreakpoint past last clamps to logLines', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakpoints(logPath, [{ line: 10 }]);
            logDebugger.gotoBreakpoint();
            logDebugger.gotoBreakpoint();
            assert.strictEqual(logDebugger.linenum(), 100);
        });

        test('gotoNextBreakpoint before first in reverse clamps to line 1', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakpoints(logPath, [{ line: 10 }]);
            logDebugger.gotoBreakpoint(true);
            assert.strictEqual(logDebugger.linenum(), 1);
        });
    });
});