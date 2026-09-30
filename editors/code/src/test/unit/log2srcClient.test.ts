import * as assert from 'assert';

import { Log2srcClient, ProcessRunner } from '../../log2srcClient';

function setPlatformArch(platform: NodeJS.Platform | string, arch: string): void {
    Object.defineProperty(process, 'platform', { value: platform, configurable: true });
    Object.defineProperty(process, 'arch', { value: arch, configurable: true });
}

suite('Log2srcClient Test Suite', () => {
    let originalPlatform: string;
    let originalArch: string;

    setup(() => {
        originalPlatform = process.platform;
        originalArch = process.arch;
        setPlatformArch('darwin', 'arm64');
    });

    teardown(() => {
        setPlatformArch(originalPlatform, originalArch);
    });

    test('queryMapping parses a single JSON object from stdout', () => {
        const runner: ProcessRunner = {
            execFileSync: (): Buffer => Buffer.from('{"exceptionTrace":[],"variables":[],"srcRef":{"sourcePath":"basic.rs","lineNumber":1,"column":1,"name":"foo"}}'),
            readFile: (): Buffer => Buffer.alloc(0),
        };
        const client = new Log2srcClient(runner, __dirname);
        const mapping = client.queryMapping(['/src'], '/log.log', undefined, 0);

        assert.strictEqual(mapping.srcRef?.sourcePath, 'basic.rs');
    });

    test('queryMappings parses one JSON object per line', () => {
        const runner: ProcessRunner = {
            execFileSync: (): Buffer => Buffer.from(
                '{"exceptionTrace":[],"variables":[],"srcRef":{"sourcePath":"basic.rs","lineNumber":1,"column":1,"name":"foo"}}\n' +
                '{"exceptionTrace":[],"variables":[]}\n' +
                '{"exceptionTrace":[],"variables":[],"srcRef":{"sourcePath":"basic.rs","lineNumber":2,"column":1,"name":"bar"}}\n'
            ),
            readFile: (): Buffer => Buffer.alloc(0),
        };
        const client = new Log2srcClient(runner, __dirname);
        const mappings = client.queryMappings(['/src'], '/log.log', undefined, 0, 3);

        assert.strictEqual(mappings.length, 3);
        assert.strictEqual(mappings[0].srcRef?.lineNumber, 1);
        assert.strictEqual(mappings[1].srcRef, undefined);
        assert.strictEqual(mappings[2].srcRef?.lineNumber, 2);
    });

    test('passes multiple source dirs and log format as separate -d flags', () => {
        let capturedArgs: string[] = [];
        const runner: ProcessRunner = {
            execFileSync: (_file: string, args: string[]): Buffer => {
                capturedArgs = args;
                return Buffer.from('{"exceptionTrace":[],"variables":[]}');
            },
            readFile: (): Buffer => Buffer.alloc(0),
        };
        const client = new Log2srcClient(runner, __dirname);

        client.queryMapping(['/src/a', '/src/b'], '/log.log', '(?<body>.*)', 5);

        assert.deepStrictEqual(capturedArgs, [
            '-d', '/src/a',
            '-d', '/src/b',
            '--log', '/log.log',
            '--start', '5',
            '--count', '1',
            '-f', '(?<body>.*)'
        ]);
    });
});
