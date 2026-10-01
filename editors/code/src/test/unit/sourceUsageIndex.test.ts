import * as assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { SourceUsage, } from '../../log2srcClient';
import { SourceUsageIndex } from '../../sourceUsageIndex';

suite('SourceUsageIndex Test Suite', () => {
    let tempDir: string;
    let logFile: string;

    setup(() => {
        tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'log2src-usage-test-'));
        logFile = path.join(tempDir, 'app.log');
        fs.writeFileSync(logFile, 'first message\n');
    });

    teardown(() => {
        fs.rmSync(tempDir, { recursive: true, force: true });
    });

    test('caches by log metadata and query settings', async () => {
        const expected: SourceUsage[] = [];
        const queries: Array<[string[], string, string | undefined]> = [];
        const index = new SourceUsageIndex({
            querySourceUsages: async (roots, file, format) => {
                queries.push([roots, file, format]);
                return expected;
            },
        });

        await index.getUsages(['/src'], logFile, undefined);
        await index.getUsages(['/src'], logFile, undefined);
        await index.getUsages(['/src'], logFile, '(?<body>.*)');
        fs.appendFileSync(logFile, 'second message changes size\n');
        await index.getUsages(['/src'], logFile, '(?<body>.*)');

        assert.strictEqual(queries.length, 3);
    });

    test('retries a failed scan instead of caching the rejection', async () => {
        let attempts = 0;
        const index = new SourceUsageIndex({
            querySourceUsages: async () => {
                attempts++;
                if (attempts === 1) {
                    throw new Error('scan failed');
                }
                return [];
            },
        });

        await assert.rejects(index.getUsages(['/src'], logFile, undefined), /scan failed/);
        assert.deepStrictEqual(await index.getUsages(['/src'], logFile, undefined), []);
        assert.strictEqual(attempts, 2);
    });
});