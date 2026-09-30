/**
 * log2srcClient.ts centralizes invocation of the log2src Rust binary: locating the
 * platform-specific binary and running/parsing its JSON output. Shared by the debug
 * adapter and the non-debug (CodeLens/DocumentLink/command) navigation providers.
 */

import { execFileSync as nodeExecFileSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

export interface ProcessRunner {
    execFileSync(file: string, args: string[]): Buffer;
    readFile(path: string): Buffer;
}

export const defaultProcessRunner: ProcessRunner = {
    execFileSync: (file: string, args: string[]): Buffer =>
        nodeExecFileSync(file, args) as Buffer,
    readFile: (path: string): Buffer =>
        fs.readFileSync(path)
};

export class BinaryNotFoundError extends Error {
    constructor(message: string) {
        super(message);
        this.name = "BinaryNotFoundError";
        if (Error.captureStackTrace) {
            Error.captureStackTrace(this, BinaryNotFoundError);
        }
    }
}

const PLATFORM_TO_BINARY = new Map<string, string>([
    ["darwin-arm64", "../bin/darwin-arm64/log2src"],
    ["darwin-x64", "../bin/darwin-x64/log2src"],
    ["linux-x64", "../bin/linux-x64/log2src"],
    ["win32-x64", "../bin/win-x64/log2src.exe"],
]);

/** Returns the binary path relative to the extension's `out/` directory for the current platform. */
export function resolveRelativeBinaryPath(): string {
    const relativePath = PLATFORM_TO_BINARY.get(`${process.platform}-${process.arch}`);
    if (!relativePath) {
        throw new BinaryNotFoundError(
            `No binary available for platform: ${process.platform} and architecture: ${process.arch}`
        );
    }
    return relativePath;
}

export interface CallSite {
    name: string,
    sourcePath: string,
    lineNumber: number
}

export interface VariablePair {
    expr: string,
    value: string,
}

export interface SourceRef {
    sourcePath: string,
    lineNumber: number,
    column: number,
    name: string,
}

export interface LogMapping {
    srcRef?: SourceRef,
    exceptionTrace: Array<CallSite>,
    variables: Array<VariablePair>
}

/**
 * Thin wrapper around invoking the log2src binary and parsing its JSON output.
 */
export class Log2srcClient {
    private readonly _binaryPath: string;
    private readonly _processRunner: ProcessRunner;

    /**
     * @param processRunner injectable process runner, primarily for tests.
     * @param moduleDir directory the binary path is resolved relative to; callers should
     *   pass their own `__dirname` so bundling into different `out/*.js` entries still resolves.
     */
    public constructor(processRunner: ProcessRunner = defaultProcessRunner, moduleDir: string = __dirname) {
        this._processRunner = processRunner;
        this._binaryPath = path.resolve(moduleDir, resolveRelativeBinaryPath());
    }

    /** Query a single log-to-source mapping (count=1) at the given 0-based log line. */
    public queryMapping(sourceDirs: string[], logFile: string, logFormat: string | undefined, start: number): LogMapping {
        const stdout = this.run(sourceDirs, logFile, logFormat, start, 1);
        return JSON.parse(stdout.trim()) as LogMapping;
    }

    /** Query multiple log-to-source mappings starting at a given 0-based log line. */
    public queryMappings(sourceDirs: string[], logFile: string, logFormat: string | undefined, start: number, count: number): LogMapping[] {
        const stdout = this.run(sourceDirs, logFile, logFormat, start, count);
        return stdout.split('\n')
            .map(line => line.trim())
            .filter(line => line.length > 0)
            .map(line => JSON.parse(line) as LogMapping);
    }

    private run(sourceDirs: string[], logFile: string, logFormat: string | undefined, start: number, count: number): string {
        const args: string[] = [];
        for (const dir of sourceDirs) {
            args.push('-d', dir);
        }
        args.push('--log', logFile, '--start', String(start), '--count', String(count));
        if (logFormat) {
            args.push('-f', logFormat);
        }
        return this._processRunner.execFileSync(this._binaryPath, args).toString('utf8');
    }
}
