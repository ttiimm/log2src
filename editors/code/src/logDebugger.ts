/**
 * logDebugger.ts handles tracking the state of the "log driven debugger".
 * 
 */

import { DebugProtocol } from '@vscode/debugprotocol';
import * as path from 'path';


export class LogDebugger {

    private _breakPoints = new Map<string, DebugProtocol.Breakpoint[]>();
    private _log: string | undefined = undefined;
    private _line = 1;
    private _logLines = Number.MAX_SAFE_INTEGER;

    public constructor() {
    }

    setToLog(log: string, logLines: number): void {
        this._log = path.resolve(log);
        this._logLines = logLines;
    }

    setBreakpoints(source: string, breakpoints: DebugProtocol.SourceBreakpoint[]): DebugProtocol.Breakpoint[] {
        const bps = new Array<DebugProtocol.Breakpoint>();
        const sourcePath = path.resolve(source);
        this._breakPoints.set(sourcePath, bps);
        breakpoints.forEach((breakpoint) => {
            const verified = breakpoint.line > 0 && breakpoint.line < this._logLines;
            bps.push({ line: breakpoint.line, verified: verified });
        });
        return bps;
    }

    hasBreakpoints(): boolean {
        const bps = (this._log && this._breakPoints.get(this._log)) || [];
        return bps.length !== 0;
    }

    linenum(): number {
        return this._line;
    }

    stepForward(): void {
        this._line = Math.min(this._logLines, this._line + 1);
    }

    stepBackward(): void {
        this._line = Math.max(1, this._line - 1);
    }

    gotoBreakpoint(reverse = false): void {
        this._line = this.findNextLineToStop(reverse);
    }

    private findNextLineToStop(reverse = false): number {
        const bps = (this._log && this._breakPoints.get(this._log)) || [];
        let bp;
        if (reverse) {
            bp = bps.findLast((bp) => bp.line !== undefined && this._line > bp.line);
        } else {
            bp = bps.find((bp) => bp.line !== undefined && this._line < bp.line);
        }

        if (bp !== undefined && bp.line !== undefined) {
            return bp.line;
        } else {
            return reverse ? 1 : this._logLines;
        }
    }
}