import * as assert from 'assert';

import { LogMatchIndex } from '../../logMatchIndex';
import { Log2srcClient, LogMapping } from '../../log2srcClient';

function mapping(sourcePath: string, lineNumber: number): LogMapping {
    return {
        exceptionTrace: [],
        variables: [],
        srcRef: { sourcePath, lineNumber, column: 1, name: 'foo' }
    };
}

suite('LogMatchIndex Test Suite', () => {
    test('queries once per uncached range and caches results', () => {
        let queryCount = 0;
        const client = {
            queryMappings: (): LogMapping[] => {
                queryCount++;
                return [mapping('a.rs', 1), mapping('a.rs', 2)];
            }
        } as unknown as Log2srcClient;
        const index = new LogMatchIndex(client);

        const first = index.getMatchesForRange('/log.log', ['/src'], undefined, 0, 1);
        const second = index.getMatchesForRange('/log.log', ['/src'], undefined, 0, 1);

        assert.strictEqual(queryCount, 1, 'should only query the binary once for the same range');
        assert.strictEqual(first.get(0)?.srcRef?.lineNumber, 1);
        assert.strictEqual(second.get(1)?.srcRef?.lineNumber, 2);
    });

    test('omits lines without a srcRef from the result', () => {
        const client = {
            queryMappings: (): LogMapping[] => [
                { exceptionTrace: [], variables: [] },
                mapping('a.rs', 2)
            ]
        } as unknown as Log2srcClient;
        const index = new LogMatchIndex(client);

        const matches = index.getMatchesForRange('/log.log', ['/src'], undefined, 0, 1);

        assert.strictEqual(matches.has(0), false);
        assert.strictEqual(matches.get(1)?.srcRef?.lineNumber, 2);
    });

    test('invalidate clears cached results for a document', () => {
        let queryCount = 0;
        const client = {
            queryMappings: (): LogMapping[] => {
                queryCount++;
                return [mapping('a.rs', 1)];
            }
        } as unknown as Log2srcClient;
        const index = new LogMatchIndex(client);

        index.getMatchesForRange('/log.log', ['/src'], undefined, 0, 0);
        index.invalidate('/log.log');
        index.getMatchesForRange('/log.log', ['/src'], undefined, 0, 0);

        assert.strictEqual(queryCount, 2, 'should re-query after invalidation');
    });

    test('swallows binary errors and leaves the range uncached', () => {
        const client = {
            queryMappings: (): LogMapping[] => {
                throw new Error('binary failed');
            }
        } as unknown as Log2srcClient;
        const index = new LogMatchIndex(client);

        const matches = index.getMatchesForRange('/log.log', ['/src'], undefined, 0, 0);

        assert.strictEqual(matches.size, 0);
    });
});
