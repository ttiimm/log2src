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
            logDebugger.setBreakPoint(logPath, [{ line: 10 }]);
            assert.ok(logDebugger.hasBreakpoints());
        });

        test('hasBreakpoints is false after empty breakpoints set', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakPoint(logPath, []);
            assert.strictEqual(logDebugger.hasBreakpoints(), false);
        });

        test('gotoNextBreakpoint advances to next breakpoint', () => {
            logDebugger.setToLog(logPath, 100);
            // setBreakPoint moves _line to the first breakpoint (10) when starting at line 1
            logDebugger.setBreakPoint(logPath, [{ line: 10 }, { line: 20 }, { line: 30 }]);
            assert.strictEqual(logDebugger.linenum(), 10);
            logDebugger.gotoNextBreakpoint();
            assert.strictEqual(logDebugger.linenum(), 20);
            logDebugger.gotoNextBreakpoint();
            assert.strictEqual(logDebugger.linenum(), 30);
        });

        test('gotoNextBreakpoint reverse retreats to previous breakpoint', () => {
            logDebugger.setToLog(logPath, 100);
            // setBreakPoint moves _line to 10; navigate forward to 30, then reverse to 20
            logDebugger.setBreakPoint(logPath, [{ line: 10 }, { line: 20 }, { line: 30 }]);
            logDebugger.gotoNextBreakpoint();   // 10 → 20
            logDebugger.gotoNextBreakpoint();   // 20 → 30
            logDebugger.gotoNextBreakpoint(true); // 30 → 20
            assert.strictEqual(logDebugger.linenum(), 20);
        });

        test('gotoNextBreakpoint past last clamps to logLines', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakPoint(logPath, [{ line: 10 }]);
            logDebugger.gotoNextBreakpoint();
            logDebugger.gotoNextBreakpoint();
            assert.strictEqual(logDebugger.linenum(), 100);
        });

        test('gotoNextBreakpoint before first in reverse clamps to line 1', () => {
            logDebugger.setToLog(logPath, 100);
            logDebugger.setBreakPoint(logPath, [{ line: 10 }]);
            logDebugger.gotoNextBreakpoint(true);
            assert.strictEqual(logDebugger.linenum(), 1);
        });
    });
});