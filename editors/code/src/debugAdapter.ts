/**
 * debugAdapter.ts implements the Debug Adapter protocol and integrates it with the log2src
 * "debugger".
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
import * as path from 'path';

import { outputChannel } from './extension';
import { LogDebugger } from './logDebugger';

interface CallSite {
    name: string,
    sourcePath: string,
    lineNumber: number
}

interface LogMapping {
    srcRef: SourceRef,
    exceptionTrace: Array<CallSite>,
    variables: Array<VariablePair>
}

interface VariablePair {
    expr: string,
    value: string,
}

interface SourceRef {
    sourcePath: string,
    lineNumber: number,
    column: number,
    name: string,
}

export interface ILaunchRequestArguments extends DebugProtocol.LaunchRequestArguments {
    // the source to debug, currently a single file
    source: string;
    // the log files to use for "debugging"
    log: string;
    // the format of the log to parse the file name and line number
    log_format: string;
    // enable logging
    trace?: boolean;
    // If true, the launch request should launch the program without enabling debugging.
    noDebug?: boolean;
}

interface IAttachRequestArguments extends ILaunchRequestArguments { }


const PLATFORM_TO_BINARY = new Map<string, string>([
    ["darwin-arm64", "../bin/darwin-arm64/log2src"],
    ["darwin-x64", "../bin/darwin-x64/log2src"],
    ["linux-x64", "../bin/linux-x64/log2src"],
    ["win32-x64", "../bin/win-x64/log2src.exe"],
]);


export class BinaryNotFoundError extends Error {
    constructor(message: string) {
        super(message);
        this.name = "BinaryNotFoundError";
        if (Error.captureStackTrace) {
            Error.captureStackTrace(this, BinaryNotFoundError);
        }
    }
}

export class DebugSession extends LoggingDebugSession {

    private static _threadID = 1;
    private _binaryPath: string;
    private readonly _variableHandles = new Handles<'locals'>();
    private _launchArgs: ILaunchRequestArguments = { source: "", log: "", log_format: "" };
    private readonly _highlightDecoration: vscode.TextEditorDecorationType;
    private _mapping?: LogMapping = undefined;
    private readonly _logDebugger: LogDebugger;

    /**
     * Create a new debug adapter to use with a debug session.
     */
    public constructor(logDebugger: LogDebugger) {
        super("log2src-dap.txt");

        this._binaryPath = PLATFORM_TO_BINARY.get(`${process.platform}-${process.arch}`)!;

        if (!this._binaryPath) {
            throw new BinaryNotFoundError(
                `No binary available for platform: ${process.platform} and architecture: ${process.arch}`
            );
        }

        this._logDebugger = logDebugger;
        this.setDebuggerLinesStartAt1(true);
        this.setDebuggerColumnsStartAt1(true);

        const focusColor = new vscode.ThemeColor('editor.focusedStackFrameHighlightBackground');
        this._highlightDecoration = vscode.window.createTextEditorDecorationType({ "backgroundColor": focusColor });
        outputChannel.appendLine("Starting up...");
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

        response.body = response.body || {};
        response.body.supportsStepBack = true;
        // response.body.supportsBreakpointLocationsRequest = true;
        response.body.supportTerminateDebuggee = true;

        this.sendResponse(response);
        this.sendEvent(new InitializedEvent());
    }

    protected setBreakPointsRequest(response: DebugProtocol.SetBreakpointsResponse, args: DebugProtocol.SetBreakpointsArguments) {
        console.log(`setBreakPointsRequest ${JSON.stringify(args)}`);

        const source = args.source.path as string;
        const bps = args.breakpoints || [];
        const breakpoints = this._logDebugger.setBreakPoint(source, bps);

        response.body = {
            breakpoints: breakpoints
        };

        if (breakpoints.length > 0) {
            this.sendEvent(new StoppedEvent('breakpoint', DebugSession._threadID));
        }
        return this.sendResponse(response);
    }

    protected attachRequest(response: DebugProtocol.AttachResponse, args: IAttachRequestArguments) {
        console.log(`attachRequest`);
        return this.launchRequest(response, args);
    }

    protected launchRequest(response: DebugProtocol.LaunchResponse, args: ILaunchRequestArguments) {
        outputChannel.appendLine(`launchRequest ${JSON.stringify(args)}`);

        // make sure to 'Stop' the buffered logging if 'trace' is not set
        logger.setup(args.trace ? Logger.LogLevel.Verbose : Logger.LogLevel.Error, false);

        this._launchArgs = args;
        this.openLogAndFocus();
        const execFile = require('child_process').execFileSync;
        let stdout = execFile('wc', ['-l', this._launchArgs.log]);
        const logLines = +stdout.toString().trim().split(" ")[0] || Number.MAX_VALUE
        this._logDebugger.setToLog(this._launchArgs.log, logLines);

        if (!this._logDebugger.hasBreakpoints()) {
            this.sendEvent(new StoppedEvent('entry', DebugSession._threadID));
        }
        this.sendResponse(response);
    }

    private openLogAndFocus() {
        const editors = this.findEditors();
        if (editors.length >= 1) {
            this.focusEditor(editors[0]);
        } else {
            Promise.resolve(vscode.workspace.openTextDocument(this._launchArgs.log))
                .then(doc => {
                    return vscode.window.showTextDocument(doc, {
                        viewColumn: vscode.ViewColumn.Beside,
                        preserveFocus: false
                    });
                })
                .then(editor => {
                    this.focusEditor(editor);
                    return editor;
                })
                .catch(error => {
                    const message = `Failed to open log file: ${error.message}`;
                    outputChannel.appendLine(message);
                    console.error(message);
                });
        }
    }

    protected threadsRequest(response: DebugProtocol.ThreadsResponse): void {
        console.log(`threadsRequest`);

        // just sending back junk for now
        response.body = {
            threads: [
                new Thread(DebugSession._threadID, "thread 1"),
            ]
        };
        this.sendResponse(response);
    }

    protected continueRequest(response: DebugProtocol.ContinueResponse, args: DebugProtocol.ContinueArguments): void {
        console.log(`continueRequest ${JSON.stringify(args)}`);

        this._logDebugger.gotoNextBreakpoint();
        this.sendEvent(new StoppedEvent('breakpoint', DebugSession._threadID));
        this.sendResponse(response);
    }

    protected reverseContinueRequest(response: DebugProtocol.ReverseContinueResponse, args: DebugProtocol.ReverseContinueArguments): void {
        console.log(`reverseContinueRequest ${JSON.stringify(args)}`);

        this._logDebugger.gotoNextBreakpoint(true);
        this.sendEvent(new StoppedEvent('breakpoint', DebugSession._threadID));
        this.sendResponse(response);
    }

    protected nextRequest(response: DebugProtocol.NextResponse, args: DebugProtocol.NextArguments): void {
        console.log(`nextRequest ${JSON.stringify(args)} line=${this._logDebugger.linenum()}`);
        this._logDebugger.stepForward()
        this.sendEvent(new StoppedEvent('step', DebugSession._threadID));
        this.sendResponse(response);
    }

    protected stepBackRequest(response: DebugProtocol.StepBackResponse, args: DebugProtocol.StepBackArguments): void {
        console.log(`stepBackRequest ${JSON.stringify(args)} line=${this._logDebugger.linenum()}`);
        this._logDebugger.stepBackward();
        this.sendEvent(new StoppedEvent('step', DebugSession._threadID));
        this.sendResponse(response);
    }

    protected stackTraceRequest(response: DebugProtocol.StackTraceResponse, args: DebugProtocol.StackTraceArguments): void {
        console.log(`stackTraceRequest ${JSON.stringify(args)}`);

        const log2srcPath = path.resolve(__dirname, this._binaryPath);
        const execFile = require('child_process').execFileSync;
        const start = this._logDebugger.linenum() - 1;

        const editors = this.findEditors();
        if (editors.length > 0) {
            this.focusEditor(editors[0]);
        }

        let l2sArgs = ['-d', this._launchArgs.source,
            '--log', this._launchArgs.log,
            '--start', start,
            '--count', 1]
        if (this._launchArgs.log_format !== undefined && this._launchArgs.log_format !== "") {
            l2sArgs.push("-f");
            l2sArgs.push(this._launchArgs.log_format);
        }
        outputChannel.appendLine(`args ${l2sArgs.join(" ")}`);
        let stdout = execFile(log2srcPath, l2sArgs);
        this._mapping = JSON.parse(stdout.toString('utf8'));
        outputChannel.appendLine(`mapped ${JSON.stringify(this._mapping)}`);

        let index = 0;
        const currentFrame = this.buildStackFrame(index++, this._mapping?.srcRef);
        const stack: StackFrame[] = [];
        stack.push(currentFrame);

        response.body = {
            stackFrames: stack,
            totalFrames: stack.length
        };

        this.sendResponse(response);
    }

    private findEditors(): vscode.TextEditor[] {
        const target = path.resolve(this._launchArgs.log);
        return vscode.window.visibleTextEditors.filter((editor) => editor.document.fileName === target);
    }

    private focusEditor(editor: vscode.TextEditor) {
        const start = this._logDebugger.linenum() - 1;
        let range = new vscode.Range(
            new vscode.Position(start, 0),
            new vscode.Position(start, Number.MAX_VALUE)
        );
        editor.setDecorations(this._highlightDecoration, [range]);
        editor.revealRange(
            range,
            vscode.TextEditorRevealType.InCenter
        );
    }

    private buildStackFrame(index: number, srcRef?: SourceRef): StackFrame {
        let name = "???";
        let lineNumber = -1;
        let sourceName = "???";
        let sourcePath = "???";


        if (srcRef !== null && srcRef !== undefined) {
            name = srcRef.name;
            lineNumber = srcRef.lineNumber;
            const codeSrcPath = path.parse(srcRef.sourcePath);
            sourceName = codeSrcPath.base;
            sourcePath = srcRef.sourcePath;
        }

        return new StackFrame(
            index,
            name,
            new Source(sourceName, sourcePath),
            this.convertDebuggerLineToClient(lineNumber)
        );
    }

    protected scopesRequest(response: DebugProtocol.ScopesResponse, args: DebugProtocol.ScopesArguments): void {
        console.log(`scopesRequest ${JSON.stringify(args)}`);

        response.body = {
            scopes: [
                new Scope("Locals", this._variableHandles.create('locals'), false),
            ]
        };
        this.sendResponse(response);
    }

    protected variablesRequest(response: DebugProtocol.VariablesResponse, args: DebugProtocol.VariablesArguments, request?: DebugProtocol.Request): void {
        console.log(`variablesRequest ${JSON.stringify(args)}`);

        let vs: DebugProtocol.Variable[] = [];

        const v = this._variableHandles.get(args.variablesReference);
        if (v === 'locals' && this._mapping !== undefined) {
            for (let pair of this._mapping.variables) {
                vs.push({
                    name: pair.expr,
                    value: pair.value,
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
