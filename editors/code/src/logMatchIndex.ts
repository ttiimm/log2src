/**
 * logMatchIndex.ts caches log2src mappings per log document so the CodeLens/command
 * navigation providers can share one binary invocation per line instead of one per lookup.
 */

import { Log2srcClient, LogMapping } from './log2srcClient';

export class LogMatchIndex {
    private readonly _client: Log2srcClient;
    // docUri -> (0-based log line -> mapping)
    private readonly _cache = new Map<string, Map<number, LogMapping>>();

    public constructor(client: Log2srcClient) {
        this._client = client;
    }

    /**
     * Returns matches (lines with a resolved srcRef) for [startLine, endLine], querying the
     * binary only if any line in the range hasn't been queried yet for this document.
     */
    public getMatchesForRange(
        docUri: string,
        logFile: string,
        sourceDirs: string[],
        logFormat: string | undefined,
        startLine: number,
        endLine: number
    ): Map<number, LogMapping> {
        let cacheForDoc = this._cache.get(docUri);
        if (!cacheForDoc) {
            cacheForDoc = new Map();
            this._cache.set(docUri, cacheForDoc);
        }

        let needsQuery = false;
        for (let line = startLine; line <= endLine; line++) {
            if (!cacheForDoc.has(line)) {
                needsQuery = true;
                break;
            }
        }

        if (needsQuery) {
            try {
                const count = endLine - startLine + 1;
                const mappings = this._client.queryMappings(sourceDirs, logFile, logFormat, startLine, count);
                mappings.forEach((mapping, i) => cacheForDoc!.set(startLine + i, mapping));
            } catch (error) {
                // leave uncached lines out of the result; caller retries on the next request
                console.error(`log2src: failed to query mappings for ${logFile}`, error);
            }
        }

        const result = new Map<number, LogMapping>();
        for (let line = startLine; line <= endLine; line++) {
            const mapping = cacheForDoc.get(line);
            if (mapping && mapping.srcRef) {
                result.set(line, mapping);
            }
        }
        return result;
    }

    /** Drop cached results for a document, e.g. after it changes on disk. */
    public invalidate(docUri: string): void {
        this._cache.delete(docUri);
    }
}
