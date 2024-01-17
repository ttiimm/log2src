/**
 * debugAdapter.ts implements the Debug Adapter protocol and integrates it with the log2src
 * "debugger".
 * 
 * Care should be given to make sure that this module is independent from VS Code so that it
 * could potentially be used in other IDE.
 */

import { 
    Logger, logger,
    LoggingDebugSession, 
    Thread, StackFrame, Scope, Source,
    InitializedEvent, StoppedEvent,
    Handles,
} from '@vscode/debugadapter';
import { DebugProtocol } from '@vscode/debugprotocol';
import * as vscode from 'vscode';


interface LogMapping {
    srcRef: SourceRef,
    variables: Map<string, string>,
    stack: Array<Array<SourceRef>> 
}

interface SourceRef {
    lineNumber: number,
    column: number,
    name: string,
}

interface ILaunchRequestArguments extends DebugProtocol.LaunchRequestArguments {
    // the source to debug, currently a single file
    source: string;
    // the log files to use for "debugging"
    log: string;
    // enable logging
    trace?: boolean;
    // If true, the launch request should launch the program without enabling debugging.
    noDebug?: boolean;
}

interface IAttachRequestArguments extends ILaunchRequestArguments { }


export class DebugSession extends LoggingDebugSession {

    private static threadID = 1;
    private _breakPoints = new Map<string, DebugProtocol.Breakpoint[]>();
    private _variableHandles = new Handles<'locals'>();
    private _line = 1;
    private _launchArgs: ILaunchRequestArguments = {source: "", log: ""};
    private _logLines = Number.MAX_SAFE_INTEGER;
    private _highlightDecoration: vscode.TextEditorDecorationType;
    private _mapping?: LogMapping = undefined;

    /**
     * Create a new debug adapter to use with a debug session.
     */
    public constructor() {
        super("log2src-dap.txt");

        this.setDebuggerLinesStartAt1(true);
        this.setDebuggerColumnsStartAt1(true);

        const focusColor = new vscode.ThemeColor('editor.focusedStackFrameHighlightBackground');
        this._highlightDecoration = vscode.window.createTextEditorDecorationType({"backgroundColor": focusColor});
    }

    protected disconnectRequest(response: DebugProtocol.DisconnectResponse, args: DebugProtocol.DisconnectArguments, request?: DebugProtocol.Request): void {
        console.log(`disconnectRequest suspend: ${args.suspendDebuggee}, terminate: ${args.terminateDebuggee}`);
        vscode.window.visibleTextEditors.forEach((editor) => editor.setDecorations(this._highlightDecoration, []));
        this.sendResponse(response);
    }

    /**
     * The 'initialize' request is the first request called by the frontend
     * to interrogate the features the debug adapter provides.
     */
    protected initializeRequest(response: DebugProtocol.InitializeResponse, args: DebugProtocol.InitializeRequestArguments): void {
        console.log(`initializeRequest: ${JSON.stringify(args)}`);
        console.log(' ');

        response.body = response.body || {};
        response.body.supportsStepBack = true;
        // response.body.supportsBreakpointLocationsRequest = true;
        response.body.supportTerminateDebuggee = true;
        
        this.sendResponse(response);
        this.sendEvent(new InitializedEvent());
    }

    protected setBreakPointsRequest(response: DebugProtocol.SetBreakpointsResponse, args: DebugProtocol.SetBreakpointsArguments) {
        console.log(`setBreakPointsRequest ${JSON.stringify(args)}`);
        console.log(' ');

        const path = args.source.path as string;
        // TODO handle lines?
        const bps = args.breakpoints || [];
        this._breakPoints.set(path, new Array<DebugProtocol.Breakpoint>());
        bps.forEach((sourceBp) => {
            if (this._line === 1) {
                this._line = sourceBp.line;
            }
            let bps = this._breakPoints.get(path) || [];
            const verified = sourceBp.line > 0 && sourceBp.line < this._logLines;
            bps.push({line: sourceBp.line, verified: verified});
        });
        const breakpoints = this._breakPoints.get(path) || [];
        response.body = {
            breakpoints: breakpoints
        };

        if (breakpoints.length > 0) {
            this.sendEvent(new StoppedEvent('breakpoint', DebugSession.threadID));
        }
        return this.sendResponse(response);
    }

    protected attachRequest(response: DebugProtocol.AttachResponse, args: IAttachRequestArguments) {
        console.log(`attachRequest`);
        console.log(' ');
        return this.launchRequest(response, args);
    }

    protected launchRequest(response: DebugProtocol.LaunchResponse, args: ILaunchRequestArguments) {
        console.log(`launchRequest ${JSON.stringify(args)}`);
        console.log(' ');

        // make sure to 'Stop' the buffered logging if 'trace' is not set
        logger.setup(args.trace ? Logger.LogLevel.Verbose : Logger.LogLevel.Stop, false);

        this._launchArgs = args;
        var execFile = require('child_process').execFileSync;
        let stdout = execFile('wc', ['-l', this._launchArgs.log]);
        this._logLines = +stdout.toString().trim().split(" ")[0] || Number.MAX_VALUE;

        // TODO do we need this?
        // wait 1 second until configuration has finished (and configurationDoneRequest has been called)
        // await this._configurationDone.wait(1000);
        if (this._breakPoints.size === 0) {
            this.sendEvent(new StoppedEvent('entry', DebugSession.threadID));
        }
        this.sendResponse(response);
    }

    protected threadsRequest(response: DebugProtocol.ThreadsResponse): void {
        console.log(`threadsRequest`);
        console.log(' ');

        // just sending back junk for now
        response.body = {
            threads: [
                new Thread(DebugSession.threadID, "thread 1"),
            ]
        };
        this.sendResponse(response);
    }

    protected continueRequest(response: DebugProtocol.ContinueResponse, args: DebugProtocol.ContinueArguments): void {
        console.log(`continueRequest ${JSON.stringify(args)}`);
        console.log(' ');

        const next = this.findNextLineToStop();
        this._line = next;
        this.sendEvent(new StoppedEvent('breakpoint', DebugSession.threadID));
        this.sendResponse(response);
    }

    protected reverseContinueRequest(response: DebugProtocol.ReverseContinueResponse, args: DebugProtocol.ReverseContinueArguments): void {
        console.log(`reverseContinueRequest ${JSON.stringify(args)}`);
        console.log(' ');

        const next = this.findNextLineToStop(true);
        this._line = next;
        this.sendEvent(new StoppedEvent('breakpoint', DebugSession.threadID));
        this.sendResponse(response);
    }

    private findNextLineToStop(reverse = false): number {
        const bps = this._breakPoints.get(this._launchArgs.log) || [];
        let bp;
        if (reverse) {
            bp = bps.findLast((bp) => {
                return reverse ? 
                    (bp.line !== undefined && this._line > bp.line) : 
                    (bp.line !== undefined && this._line < bp.line);
            });
        } else {
            bp = bps.find((bp) => {
                return reverse ? 
                    (bp.line !== undefined && this._line > bp.line) : 
                    (bp.line !== undefined && this._line < bp.line);
            });
        }
        
        if (bp !== undefined && bp.line !== undefined) {
            return bp.line;
        } else {
            return reverse ? 1 : this._logLines;
        }
    }

    protected nextRequest(response: DebugProtocol.NextResponse, args: DebugProtocol.NextArguments): void {
        console.log(`nextRequest ${JSON.stringify(args)} line=${this._line}`);
        console.log(' ');
        this._line = Math.min(this._logLines, this._line + 1);
        this.sendEvent(new StoppedEvent('step', DebugSession.threadID));
        this.sendResponse(response);
    }

    protected stepBackRequest(response: DebugProtocol.StepBackResponse, args: DebugProtocol.StepBackArguments): void {
        console.log(`stepBackRequest ${JSON.stringify(args)} line=${this._line}`);
        console.log(' ');
        this._line = Math.max(1, this._line - 1);
        this.sendEvent(new StoppedEvent('step', DebugSession.threadID));
        this.sendResponse(response);
    }

    protected stackTraceRequest(response: DebugProtocol.StackTraceResponse, args: DebugProtocol.StackTraceArguments): void {
        console.log(`stackTraceRequest ${JSON.stringify(args)}`);
        console.log(' ');

        var path = require('path');
        var log2srcPath = path.resolve(__dirname, '../bin/log2src');
        var execFile = require('child_process').execFileSync;
        let start = this._line - 1;
        let end = this._line;

        const editors = vscode.window.visibleTextEditors.filter((editor) => editor.document.fileName === this._launchArgs.log);
        if (editors !== undefined && editors.length >= 1) {
            const editor = editors[0];
            let range = new vscode.Range(
                new vscode.Position(start, 0),
                new vscode.Position(start, Number.MAX_VALUE)
            );
            editor.setDecorations(this._highlightDecoration, [range]);
        }

        let stdout = execFile(log2srcPath, ['--source', this._launchArgs.source,
                                           '--log', this._launchArgs.log,
                                           '--start', start,
                                           '--end', end]);
        this._mapping = JSON.parse(stdout);

        const sourceName = path.basename(this._launchArgs.source);
        let index = 0;
        const currentFrame = this.buildStackFrame(index++, sourceName, this._mapping?.srcRef);
        const stack: StackFrame[] = [];
        stack.push(currentFrame);

        if (this._mapping?.stack.length === 1 && this._mapping?.stack[0].length > 0) {
            this._mapping?.stack[0].forEach((srcRef) => {
                const frame = this.buildStackFrame(index++, sourceName, srcRef);
                stack.push(frame);
            });
        }

        response.body = {
            stackFrames: stack,
            totalFrames: stack.length
        };

        this.sendResponse(response);
    }

    private buildStackFrame(index: number, sourceName: string, srcRef?: SourceRef): StackFrame {
        let name = "???";
        let lineNumber = -1;

        if (srcRef !== undefined) {
            name = srcRef.name;
            lineNumber = srcRef.lineNumber;
        }

        return new StackFrame(
            index,
            name,
            new Source(sourceName, this._launchArgs.source),
            this.convertDebuggerLineToClient(lineNumber)
        );
    }

    protected scopesRequest(response: DebugProtocol.ScopesResponse, args: DebugProtocol.ScopesArguments): void {
        console.log(`scopesRequest ${JSON.stringify(args)}`);
        console.log(' ');

        response.body = {
            scopes: [
                new Scope("Locals", this._variableHandles.create('locals'), false),
            ]
        };
        this.sendResponse(response);
    }

    protected variablesRequest(response: DebugProtocol.VariablesResponse, args: DebugProtocol.VariablesArguments, request?: DebugProtocol.Request): void {
        console.log(`variablesRequest ${JSON.stringify(args)}`);
        console.log(' ');

        let vs: DebugProtocol.Variable[] = [];
    
        const v = this._variableHandles.get(args.variablesReference);
        if (v === 'locals' && this._mapping !== undefined) {
            for (let [key, value] of Object.entries(this._mapping.variables)) {
                vs.push({
                    name: key,
                    value: value,
                    variablesReference: 0
                });
            }        
        }
    
        response.body = {
            variables: vs
        };
        this.sendResponse(response);
    }
}
