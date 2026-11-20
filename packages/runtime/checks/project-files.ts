import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { crc32 } from 'node:zlib';
import { listProjectDirectory, previewProjectFile, searchProjectContents, searchProjectFiles, resolveProjectFile, readProjectFile } from '../src/project-files.js';
import { REGEX_INVALID, REGEX_TIMEOUT, startProjectRegexSearch } from '../src/project-search-regex.js';

const dir = await mkdtemp(join(tmpdir(), 'hive project files '));
try {
  const repo = join(dir, 'repository'); await mkdir(repo);
  execFileSync('git', ['init', '-q', repo]);
  await writeFile(join(repo, '.gitignore'), 'ignored*\n');
  await writeFile(join(repo, 'ignored.txt'), 'private unlisted phrase');
  await writeFile(join(repo, 'tracked ignored.txt'), 'tracked');
  execFileSync('git', ['-C', repo, 'add', 'tracked ignored.txt']);
  const nested = join(repo, 'nested'); await mkdir(nested);
  await writeFile(join(nested, '中文 space.txt'), 'hello');
  for (let i = 0; i < 105; i++) await writeFile(join(nested, `file-${String(i).padStart(3, '0')}.txt`), 'x');
  const replacedDirectory = join(repo, 'directory-replacement');
  await mkdir(replacedDirectory);
  await writeFile(join(replacedDirectory, 'tracked.txt'), 'old');
  execFileSync('git', ['-C', repo, 'add', 'directory-replacement/tracked.txt']);
  await rm(replacedDirectory, { recursive: true });
  await writeFile(replacedDirectory, 'new');
  const all = await searchProjectFiles(repo, '');
  assert(!all.paths.includes('ignored.txt'));
  assert(all.paths.includes('directory-replacement'));
  assert(all.paths.includes('nested/file-000.txt'));
  assert.deepEqual((await searchProjectFiles(repo, 'directory-replacement')).paths, ['directory-replacement']);
  assert((await searchProjectFiles(repo, 'tracked ignored')).paths.includes('tracked ignored.txt'));
  await mkdir(join(repo, 'empty'));
  const rootEntries = (await listProjectDirectory(repo, '')).entries;
  assert.deepEqual(rootEntries.slice(0, 2), [{path: 'empty', directory: true}, {path: 'nested', directory: true}]);
  assert(!rootEntries.some(entry => entry.path === '.git'));
  assert.deepEqual(await listProjectDirectory(repo, 'empty'), {entries: [], total: 0});
  assert.equal((await listProjectDirectory(repo, 'nested', 100)).entries.length, 6);
  const page = await searchProjectFiles(nested, 'file-');
  assert.equal(page.total, 105); assert.equal(page.paths.length, 100);
  assert.deepEqual((await searchProjectFiles(nested, 'file-', 100)).paths, Array.from({length: 5}, (_, i) => `file-${i + 100}.txt`));
  assert.deepEqual((await searchProjectFiles(nested, '中文')).paths, ['中文 space.txt']);
  assert.equal(await resolveProjectFile(nested, '中文 space.txt'), await realpath(join(nested, '中文 space.txt')));
  assert.equal((await readProjectFile(nested, '中文 space.txt')).bytes.toString(), 'hello');
  await writeFile(join(nested, 'binary.bin'), Buffer.from([0, 1]));
  await assert.rejects(readProjectFile(nested, 'binary.bin'));
  await assert.rejects(previewProjectFile(nested, 'binary.bin'), /not text/);
  const images = join(dir, 'images');
  await mkdir(images);
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC', 'base64');
  const screenshot = '截图 preview.png';
  await writeFile(join(images, screenshot), png);
  // A valid PNG with a large ancillary comment exercises the encoded-byte
  // threshold independently from decoded pixel count.
  const comment = Buffer.concat([Buffer.from('tEXtComment\0'), Buffer.alloc(1024 * 1024, 'x')]);
  const length = Buffer.alloc(4);
  const checksum = Buffer.alloc(4);
  length.writeUInt32BE(comment.length - 4);
  checksum.writeUInt32BE(crc32(comment));
  const largePng = Buffer.concat([png.subarray(0, -12), length, comment, checksum, png.subarray(-12)]);
  await writeFile(join(images, 'large.png'), largePng);
  await writeFile(join(images, 'oversize.png'), Buffer.concat([png, Buffer.alloc(4 * 1024 * 1024)]));
  await writeFile(join(images, 'named.png'), 'text named like an image');
  await writeFile(join(images, 'large-text.png'), Buffer.alloc(1024 * 1024 + 1, 'x'));
  assert.deepEqual(await previewProjectFile(images, screenshot, true), {
    image: {format: 'png', data: png.toString('base64')}, sha256: createHash('sha256').update(png).digest('hex'),
  });
  await assert.rejects(previewProjectFile(images, screenshot), /not text/);
  await assert.rejects(previewProjectFile(images, screenshot, false), /not text/);
  assert.deepEqual(await previewProjectFile(images, 'large.png', true), {
    image: {format: 'png', data: largePng.toString('base64')}, sha256: createHash('sha256').update(largePng).digest('hex'),
  });
  await assert.rejects(previewProjectFile(images, 'large.png'), /Project file exceeds preview limit/);
  await assert.rejects(previewProjectFile(images, 'large-text.png', true), /Project file exceeds preview limit/);
  await assert.rejects(previewProjectFile(images, 'oversize.png', true), /Project image exceeds preview limit/);
  await assert.rejects(previewProjectFile(images, '../images/' + screenshot, true));
  assert.deepEqual(await previewProjectFile(images, 'named.png', true), await previewProjectFile(images, 'named.png'));
  if (process.platform !== 'win32') {
    await symlink(join(images, screenshot), join(images, 'local-link.png'));
    await assert.rejects(previewProjectFile(images, 'local-link.png', true));
  }
  await writeFile(join(nested, 'unicode.txt'), 'first line\nCafe\u0301 MATCH\n');
  await writeFile(join(nested, 'large.txt'), Buffer.alloc(1024 * 1024 + 1, 'a'));
  await assert.rejects(previewProjectFile(nested, 'large.txt'), /preview limit/);
  assert.deepEqual(await searchProjectContents(repo, 'CAFÉ match'), {
    matches: [{path: 'nested/unicode.txt', line: 2, snippet: {
      text: 'Cafe\u0301 MATCH', ranges: [[0, 12]], leadingEllipsis: false, trailingEllipsis: false,
    }}], total: 1, limited: true,
  });
  const nextContentPage = await searchProjectContents(repo, 'x', 100);
  assert.equal(nextContentPage.total, 105);
  assert.deepEqual(nextContentPage.matches.map(({path, line}) => ({path, line})), Array.from({length: 5}, (_, i) => ({
    path: `nested/file-${i + 100}.txt`, line: 1,
  })));
  const scatteredLines = [3, 350, 620, ...Array.from({length: 101}, (_, i) => 701 + i)];
  const many = 'nested/many matching lines.md';
  const matching = new Set(scatteredLines);
  const strictLastLine = 798;
  await writeFile(join(repo, many), Array.from({length: 801}, (_, i) => {
    const line = i + 1;
    if (!matching.has(line)) return '';
    return line <= strictLastLine
      ? 'RE\u0301PE\u0301TE\u0301 MARKER · réPÉTÉ MARKER'
      : 'Re\u0301pe\u0301te\u0301 marker · réPÉTÉ MARKER';
  }).join('\r\n'));
  const laterPath = 'nested/more matching lines.txt';
  await writeFile(join(repo, laterPath), '\nRÉPÉTÉ MARKER\n');
  const expected = [...scatteredLines.map(line => ({path: many, line})), {path: laterPath, line: 2}];
  for (const offset of [0, 100, 104, expected.length]) {
    const {matches, ...counts} = await searchProjectContents(repo, 'RÉPÉTÉ MARKER', offset);
    assert.deepEqual(counts, {total: expected.length, limited: true});
    assert.deepEqual(matches.map(({path, line}) => ({path, line})), expected.slice(offset, offset + 100));
    assert(matches.every(match => (match.snippet?.ranges.length ?? 0) >= 1));
  }
  const strict = expected.filter(result => result.path !== many || result.line <= strictLastLine);
  for (const offset of [0, 100, strict.length]) {
    const {matches, ...counts} = await searchProjectContents(repo, 'RÉPÉTÉ MARKER', offset, undefined, true);
    assert.deepEqual(counts, {total: strict.length, limited: true});
    assert.deepEqual(matches.map(({path, line}) => ({path, line})), strict.slice(offset, offset + 100));
    assert(matches.every(match => match.snippet?.ranges.length === 1));
  }
  const excerpts = 'nested/excerpts.txt';
  await writeFile(join(repo, excerpts), [
    '🙂 Cafe\u0301 accent', '👩‍💻 developer', 'ΟΣ Greek context',
    `${'a'.repeat(9000)} LONG_ASCII_MARKER`,
    `${'é'.repeat(95)}👩‍💻${'é'.repeat(4200)} UNICODE_ONLY_MARKER`,
    'CONTROL_MARKER\u001bhidden', 'many match-range match-range match-range match-range match-range match-range match-range match-range match-range',
  ].join('\r\n'));
  const resultAt = async (query: string, line: number) => {
    const result = (await searchProjectContents(repo, query)).matches.find(hit => hit.path === excerpts && hit.line === line);
    assert(result);
    return result;
  };
  const {snippet: accent} = await resultAt('CAFÉ accent', 1);
  assert.equal(accent?.text, '🙂 Cafe\u0301 accent');
  assert.deepEqual(accent?.ranges, [[5, 18]]);
  assert.deepEqual((await resultAt('👩', 2)).snippet?.ranges, []);
  assert.deepEqual((await resultAt('👩‍💻', 2)).snippet?.ranges, [[0, 11]]);
  assert.deepEqual((await resultAt('ος', 3)).snippet?.ranges, []);
  const {snippet: tail} = await resultAt('LONG_ASCII_MARKER', 4);
  assert(tail?.leadingEllipsis && !tail.trailingEllipsis);
  assert.equal(tail.text.slice(tail.ranges[0]?.[0], tail.ranges[0]?.[1]), 'LONG_ASCII_MARKER');
  const {snippet: unicode} = await resultAt('UNICODE_ONLY_MARKER', 5);
  assert.equal(unicode?.text, 'é'.repeat(95));
  assert(unicode.trailingEllipsis);
  assert.deepEqual(unicode.ranges, []);
  assert.equal((await resultAt('CONTROL_MARKER', 6)).snippet, undefined);
  assert.equal((await resultAt('match-range', 7)).snippet?.ranges.length, 8);
  const wholeRoot = join(dir, 'whole words');
  await mkdir(wholeRoot);
  const wordPath = 'Words.txt';
  const wordLines = [
    'scatter cat cat', 'cat_fox', 'cat2', '猫cat', '🙂cat', '𐐀cat', 'cat𝟙', 'cat\u20dd', 'CAT',
    'C++17', '-C++-', 'x+++ ', '小猫', ' 猫。', '👩‍💻', '👩!', 'Cafe\u0301', 'Caféine',
  ];
  await writeFile(join(wholeRoot, wordPath), [...wordLines, ...Array(102).fill('cat')].join('\r\n'));
  const whole = await searchProjectContents(wholeRoot, 'cat', 0, undefined, false, true);
  const validLines = [1, 5, 9, ...Array.from({length: 102}, (_, i) => wordLines.length + 1 + i)];
  assert.equal(whole.total, validLines.length);
  assert.deepEqual(whole.matches.map(hit => hit.line), validLines.slice(0, 100));
  assert.deepEqual(whole.matches[0]?.snippet?.ranges, [[8, 11], [12, 15]]);
  assert.deepEqual(whole.matches[1]?.snippet?.ranges, [[4, 7]]);
  assert.deepEqual((await searchProjectContents(wholeRoot, 'cat', 100, undefined, false, true)).matches.map(hit => hit.line), validLines.slice(100));
  assert.equal((await searchProjectContents(wholeRoot, 'cat', 0, undefined, true, true)).total, whole.total - 1);
  assert((await searchProjectContents(wholeRoot, 'cat')).total > whole.total);
  for (const [query, lines] of [
    ['C++', [11]], ['++', [12]], ['猫', [14]], ['👩', [16]], ['Café', [17]], ['cat cat', [1]],
  ] as const) {
    const result = await searchProjectContents(wholeRoot, query, 0, undefined, true, true);
    assert.deepEqual(result.matches.map(hit => hit.line), lines, query);
  }
  assert.deepEqual((await searchProjectFiles(wholeRoot, 'ord')).paths, [wordPath]);
  const regexRoot = join(dir, 'regular expressions');
  await mkdir(regexRoot);
  const regexPath = 'Regex.txt';
  await writeFile(join(regexRoot, regexPath), [
    '🙂 Cafe\u0301 [42]', 'scatter cat cat', 'cat_fox', 'CAT', 'item-12 extra', ...Array(102).fill('item-42'),
  ].join('\r\n'));
  const regexSearch = (query: string, offset = 0, caseSensitive = false, wholeWord = false) =>
    searchProjectContents(regexRoot, query, offset, undefined, caseSensitive, wholeWord, true);
  const regexFirst = await regexSearch('^item-[0-9]+$');
  const regexLast = await regexSearch('^item-[0-9]+$', 100);
  assert.equal(regexFirst.total, 102);
  assert.equal(regexLast.total, regexFirst.total);
  assert.deepEqual([...regexFirst.matches, ...regexLast.matches].map(({path, line}) => ({path, line})),
    Array.from({length: 102}, (_, i) => ({path: regexPath, line: 6 + i})));
  assert.deepEqual(regexLast.matches[0]?.snippet?.ranges, [[0, 7]]);
  assert.equal((await searchProjectContents(regexRoot, '^item-[0-9]+$')).total, 0);
  const rawRegex = (await regexSearch('Cafe\\p{Mark}')).matches[0]?.snippet;
  assert.equal(rawRegex?.text, '🙂 Cafe\u0301 [42]');
  assert.deepEqual(rawRegex?.ranges, [[5, 11]]);
  assert.equal((await regexSearch('Café')).total, 0);
  const wholeRegex = await regexSearch('c[ae]t', 0, false, true);
  assert.deepEqual(wholeRegex.matches.map(hit => hit.line), [2, 4]);
  assert.deepEqual(wholeRegex.matches[0]?.snippet?.ranges, [[8, 11], [12, 15]]);
  assert.deepEqual((await regexSearch('c[ae]t', 0, true, true)).matches.map(hit => hit.line), [2]);
  const anchors = await regexSearch('^');
  assert.equal(anchors.total, 107);
  assert.equal(anchors.matches[0]?.snippet?.text, '🙂 Cafe\u0301 [42]');
  assert.deepEqual(anchors.matches[0]?.snippet?.ranges, []);
  assert.deepEqual((await regexSearch('(?=item)')).matches.slice(0, 2).map(({path, line, snippet}) => ({path, line, ranges: snippet?.ranges})), [
    {path: regexPath, line: 5, ranges: []}, {path: regexPath, line: 6, ranges: []},
  ]);
  assert.deepEqual((await regexSearch('(?=🙂)|Cafe\\p{Mark}')).matches[0]?.snippet?.ranges, [[5, 11]]);
  assert.deepEqual((await regexSearch('^|[0-9]+|$')).matches[0]?.snippet?.ranges, [[13, 15]]);
  await writeFile(join(regexRoot, 'blank.txt'), '\r\ntext\r\n\r\n');
  assert.deepEqual((await regexSearch('^$')).matches, [{path: 'blank.txt', line: 1}, {path: 'blank.txt', line: 3}]);
  await assert.rejects(regexSearch('['), {message: REGEX_INVALID});
  const controller = new AbortController();
  const cancelWorker = await startProjectRegexSearch('.', true, undefined, 8, controller.signal);
  const cancelledScan = cancelWorker.scan('a\n'.repeat(500_000), 0, 1);
  controller.abort();
  await assert.rejects(cancelledScan, {name: 'AbortError'});
  await cancelWorker.close();
  const shortBudgetWorker = await startProjectRegexSearch('.', true, undefined, 8, undefined, 1);
  await assert.rejects(shortBudgetWorker.scan('a\n'.repeat(500_000), 0, 1), {message: REGEX_TIMEOUT});
  await shortBudgetWorker.close();
  const supersededWorker = await startProjectRegexSearch('.', true, undefined, 8);
  const latestWorker = await startProjectRegexSearch('.', true, undefined, 8);
  await assert.rejects(supersededWorker.scan('a', 0, 1), {name: 'AbortError'});
  assert.equal((await latestWorker.scan('a', 0, 1)).total, 1);
  await latestWorker.close();
  const names = join(regexRoot, 'names');
  await mkdir(names);
  execFileSync('git', ['init', '-q', names]);
  await mkdir(join(names, 'other'));
  await mkdir(join(names, 'src'));
  const numberedPaths = Array.from({length: 263}, (_, index) => `src/file-${String(index).padStart(3, '0')}.test.ts`);
  await Promise.all(numberedPaths.map(path => writeFile(join(names, path), path)));
  for (const path of ['src/Upper.TEST.TS', 'other/file-000.test.ts', 'src/marker .txt', 'src/marker.txt', 'src/🙂Unicode.ts', 'src/ignored.test.ts']) {
    await writeFile(join(names, path), path);
  }
  await writeFile(join(names, '.gitignore'), 'src/ignored.test.ts\n');
  if (process.platform !== 'win32') {
    await symlink(join(names, 'other/file-000.test.ts'), join(names, 'src/link.test.ts'));
    await writeFile(join(names, 'src', 'line\nbreak.ts'), 'raw line break in a file path');
  }
  const namePattern = '^src/.*\\.test\\.ts$';
  const nameRegex = (query: string, offset = 0, caseSensitive = false, signal?: AbortSignal) =>
    searchProjectFiles(names, query, offset, signal, caseSensitive, true);
  const expectedNames = ['src/Upper.TEST.TS', ...numberedPaths];
  for (const offset of [0, 100, 200, 300]) {
    assert.deepEqual(await nameRegex(namePattern, offset), {paths: expectedNames.slice(offset, offset + 100), total: expectedNames.length});
  }
  assert.deepEqual(await nameRegex(namePattern, 200, true), {paths: numberedPaths.slice(200), total: numberedPaths.length});
  assert.deepEqual(await searchProjectFiles(names, namePattern), {paths: [], total: 0});
  assert.deepEqual((await nameRegex('^src/marker ')).paths, ['src/marker .txt']);
  assert.deepEqual((await nameRegex('(?=src/\\u{1F642})')).paths, ['src/🙂Unicode.ts']);
  if (process.platform !== 'win32') {
    assert.deepEqual((await nameRegex('^src/line\\nbreak\\.ts$')).paths, ['src/line\nbreak.ts']);
  }
  await assert.rejects(nameRegex('['), {message: REGEX_INVALID});
  await assert.rejects(nameRegex(namePattern, 0, false, AbortSignal.abort()), {name: 'AbortError'});
  const wordNames = join(dir, 'whole names');
  await mkdir(join(wordNames, 'src'), {recursive: true});
  await mkdir(join(wordNames, 'cat'));
  const wholePaths = [
    'cat/README.md', 'src/CAT-notes.md', 'src/cat.txt', 'src/🙂cat.txt',
    ...Array.from({length: 103}, (_, index) => `src/cat-${String(index).padStart(3, '0')}.txt`),
  ].sort();
  const partialPaths = ['src/scatter.txt', 'src/cat_id.txt', 'src/cat2.txt', 'src/猫cat.txt', 'src/𐐀cat.txt', 'src/cat\u20dd.txt'];
  await Promise.all([...wholePaths, ...partialPaths].map(path => writeFile(join(wordNames, path), path)));
  const wholeName = (query: string, offset = 0, regex = false, caseSensitive = false, signal?: AbortSignal) =>
    searchProjectFiles(wordNames, query, offset, signal, caseSensitive, regex, true);
  for (const offset of [0, 100]) {
    const expected = {paths: wholePaths.slice(offset, offset + 100), total: wholePaths.length};
    assert.deepEqual(await wholeName('cat', offset), expected);
    assert.deepEqual(await wholeName('c[ae]t', offset, true), expected);
  }
  for (const [query, regex] of [['CAT', false], ['C[AE]T', true]] as const) {
    assert.deepEqual(await wholeName(query, 0, regex, true), {paths: ['src/CAT-notes.md'], total: 1});
  }
  assert.equal((await searchProjectFiles(wordNames, 'cat')).total, wholePaths.length + partialPaths.length);
  assert.deepEqual(await wholeName(''), await searchProjectFiles(wordNames, ''));
  await assert.rejects(wholeName('cat', 0, true, false, AbortSignal.abort()), {name: 'AbortError'});
  const namedPath = 'nested/Cafe\u0301-ReadMe.md';
  await writeFile(join(repo, namedPath), 'case-sensitive path fixture');
  const [returnedPath] = (await searchProjectFiles(repo, 'café-readme')).paths;
  assert.equal(returnedPath?.normalize('NFC'), namedPath.normalize('NFC'));
  assert.deepEqual((await searchProjectFiles(repo, 'café-readme', 0, undefined, true)).paths, []);
  assert.deepEqual((await searchProjectFiles(repo, 'Café-ReadMe', 0, undefined, true)).paths, [returnedPath]);
  assert.deepEqual((await searchProjectFiles(repo, 'CAFÉ', 0, undefined, false, false, true)).paths, [returnedPath]);
  const preview = await previewProjectFile(repo, 'nested/unicode.txt');
  assert.equal(preview.text, 'first line\nCafe\u0301 MATCH\n');
  assert.equal(preview.sha256, createHash('sha256').update(preview.text).digest('hex'));
  assert.equal((await searchProjectContents(repo, 'private unlisted phrase')).total, 0);
  await assert.rejects(searchProjectContents(repo, 'match', 0, AbortSignal.abort()), {name: 'AbortError'});
  await assert.rejects(listProjectDirectory(repo, '../'));
  for (const path of ['../ignored.txt', '/etc/passwd', '.git/config', 'x\0y']) await assert.rejects(resolveProjectFile(repo, path));
  if (process.platform !== 'win32') {
    await symlink(dir, join(nested, 'escape'));
    await assert.rejects(resolveProjectFile(nested, 'escape/repository/.gitignore'));
    await assert.rejects(listProjectDirectory(nested, 'escape'));
    await assert.rejects(previewProjectFile(nested, 'escape/repository/.gitignore'));
    assert(!(await searchProjectFiles(nested, 'escape')).paths.length);
  }
  const ignoredRoot = join(repo, 'ignored-project'); await mkdir(ignoredRoot);
  await writeFile(join(ignoredRoot, 'keyboard-alpha.txt'), 'searchable content');
  assert.deepEqual((await searchProjectFiles(ignoredRoot, 'keyboard')).paths, ['keyboard-alpha.txt']);
  assert.deepEqual((await searchProjectContents(ignoredRoot, 'searchable')).matches.map(({path, line}) => ({path, line})), [{path: 'keyboard-alpha.txt', line: 1}]);
  assert(!(await searchProjectFiles(repo, 'keyboard')).paths.length);
  const plain = join(dir, 'plain'); await mkdir(plain); await mkdir(join(plain, 'node_modules'));
  await writeFile(join(plain, 'yes.txt'), 'x'); await writeFile(join(plain, 'node_modules', 'no.txt'), 'x');
  assert.deepEqual((await searchProjectFiles(plain, '')).paths, ['yes.txt']);
  await assert.rejects(searchProjectFiles(join(dir, 'missing'), ''));
  await assert.rejects(searchProjectFiles(repo, '\0'));
  await assert.rejects(searchProjectFiles(repo, '', -1));
  console.log('PROJECT_FILES_OK');
} finally { await rm(dir, { recursive: true, force: true }); }
