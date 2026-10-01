import * as fs from 'fs';
import * as path from 'path';

import { Log2srcClient, SourceUsage } from './log2srcClient';

export type SourceUsageQuery = Pick<Log2srcClient, 'querySourceUsages'>;

export class SourceUsageIndex {
    private readonly _client: SourceUsageQuery;
    private _cacheKey: string | undefined;
    private _usages: Promise<SourceUsage[]> | undefined;

    public constructor(client: SourceUsageQuery) {
        this._client = client;
    }

    public getUsages(sourceRoots: string[], logFile: string, logFormat: string | undefined, signal?: AbortSignal): Promise<SourceUsage[]> {
        const stat = fs.statSync(logFile);
        if (!stat.isFile()) {
            throw new Error(`Configured log path is not a file: ${logFile}`);
        }

        const cacheKey = JSON.stringify([
            path.resolve(logFile),
            stat.size,
            stat.mtimeMs,
            sourceRoots.map(root => path.resolve(root)),
            logFormat ?? '',
        ]);
        if (this._cacheKey !== cacheKey || !this._usages) {
            this._cacheKey = cacheKey;
            this._usages = this._client.querySourceUsages(sourceRoots, logFile, logFormat, signal)
                .catch(error => {
                    if (this._cacheKey === cacheKey) {
                        this._usages = undefined;
                    }
                    throw error;
                });
        }
        return this._usages;
    }

    public invalidate(): void {
        this._cacheKey = undefined;
        this._usages = undefined;
    }
}