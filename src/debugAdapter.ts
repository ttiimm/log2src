/**
 * debugAdapter.ts implements the Debug Adapter protocol and integrates it with the logdbg
 * "debugger".
 * 
 * Care should be given to make sure that this module is independent from VS Code so that it
 * could potentially be used in other IDE.
 */

import { 
    Logger, logger,
    LoggingDebugSession,
    InitializedEvent,
} from '@vscode/debugadapter';
import { DebugProtocol } from '@vscode/debugprotocol';


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

    /**
     * Create a new debug adapter to use with a debug session.
     */
    public constructor() {
        super("logdbg-dap.txt");

        this.setDebuggerLinesStartAt1(true);
        this.setDebuggerColumnsStartAt1(true);
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
        
        this.sendResponse(response);
        this.sendEvent(new InitializedEvent());
    }

    protected setBreakPointsRequest(response: DebugProtocol.SetBreakpointsResponse, args: DebugProtocol.SetBreakpointsArguments) {
        console.log(`setBreakPointsRequest ${JSON.stringify(args)}`);
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

        // TODO do we need this?
        // wait 1 second until configuration has finished (and configurationDoneRequest has been called)
        // await this._configurationDone.wait(1000);

        // TODO start logdbg here

        this.sendResponse(response);
    }
}
