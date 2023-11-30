/**
 * debugAdapter.ts implements the Debug Adapter protocol and integrates it with the logdbg
 * "debugger".
 * 
 * Care should be given to make sure that this module is independent from VS Code so that it
 * could potentially be used in other IDE.
 */

import { 
    Logger, logger,
    LoggingDebugSession, Thread, StackFrame, Source,
    InitializedEvent,
    StoppedEvent,
} from '@vscode/debugadapter';
import { DebugProtocol } from '@vscode/debugprotocol';
import * as vscode from 'vscode';


interface SourceRef {
    lineNumber: number,
    column: number,
    name: string
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
    private breakPoints = new Map<string, DebugProtocol.Breakpoint[]>();
    private line = 1;
    private launchArgs: ILaunchRequestArguments = {source: "", log: ""};
    private highlightDecoration: vscode.TextEditorDecorationType;

    /**
     * Create a new debug adapter to use with a debug session.
     */
    public constructor() {
        super("logdbg-dap.txt");

        this.setDebuggerLinesStartAt1(true);
        this.setDebuggerColumnsStartAt1(true);

        const focusColor = new vscode.ThemeColor('editor.focusedStackFrameHighlightBackground');
        this.highlightDecoration = vscode.window.createTextEditorDecorationType({"backgroundColor": focusColor});
    }

    protected disconnectRequest(response: DebugProtocol.DisconnectResponse, args: DebugProtocol.DisconnectArguments, request?: DebugProtocol.Request): void {
        console.log(`disconnectRequest suspend: ${args.suspendDebuggee}, terminate: ${args.terminateDebuggee}`);
        vscode.window.visibleTextEditors.forEach((editor) => editor.setDecorations(this.highlightDecoration, []));
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
        const clientLines = args.lines || [];

        clientLines.forEach((line) => {
            let bps = this.breakPoints.get(path);
            if (!bps) {
                bps = new Array<DebugProtocol.Breakpoint>();
                this.breakPoints.set(path, bps);
            }
            bps.push({line: line, verified: false});
        });
        
        const breakpoints = this.breakPoints.get(path) || [];
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

        this.launchArgs = args;

        // TODO do we need this?
        // wait 1 second until configuration has finished (and configurationDoneRequest has been called)
        // await this._configurationDone.wait(1000);
        if (this.breakPoints.size === 0) {
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

    protected nextRequest(response: DebugProtocol.NextResponse, args: DebugProtocol.NextArguments): void {
        console.log(`nextRequest ${JSON.stringify(args)} line=${this.line}`);
        this.line++;
        this.sendEvent(new StoppedEvent('step', DebugSession.threadID));
        this.sendResponse(response);
    }

    protected stepBackRequest(response: DebugProtocol.StepBackResponse, args: DebugProtocol.StepBackArguments): void {
        console.log(`stepBackRequest ${JSON.stringify(args)} line=${this.line}`);
        this.line--;
        this.sendEvent(new StoppedEvent('step', DebugSession.threadID));
        this.sendResponse(response);
    }

    protected stackTraceRequest(response: DebugProtocol.StackTraceResponse, args: DebugProtocol.StackTraceArguments): void {
        console.log(`stackTraceRequest ${JSON.stringify(args)}`);
        console.log(' ');

        var path = require('path');
        var logdbgPath = path.resolve(__dirname, '../bin/logdbg');
        var execFile = require('child_process').execFileSync;
        
        let bps = this.breakPoints.get(this.launchArgs.log) || [{line: 1, verified: false}];
        var bpLine = bps[0].line || 1;
        let line = this.line;
        if (this.line < bpLine) {
            line = bpLine;
            this.line = bpLine;
        }
        let start = line - 1;
        let end = line;

        const editors = vscode.window.visibleTextEditors.filter((editor) => editor.document.fileName === this.launchArgs.log);
        if (editors !== undefined && editors.length >= 1) {
            const editor = editors[0];
            let range = new vscode.Range(
                new vscode.Position(start, 0),
                new vscode.Position(start, Number.MAX_VALUE)
            );
            editor.setDecorations(this.highlightDecoration, [range]);
        }

        let stdout = execFile(logdbgPath, ['--source', this.launchArgs.source,
                                           '--log', this.launchArgs.log,
                                           '--start', start,
                                           '--end', end]);
        let srcRef: SourceRef = JSON.parse(stdout);
        response.body = {
            stackFrames: [new StackFrame(0, srcRef.name, this.createSource(this.launchArgs.source), this.convertDebuggerLineToClient(srcRef.lineNumber))],
            totalFrames: 1
        };

        this.sendResponse(response);
    }

    private createSource(filePath: string): Source {
        return new Source("basic.rs", filePath);
    }
}
