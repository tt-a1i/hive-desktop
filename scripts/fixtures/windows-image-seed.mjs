import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { digest, imageIndex, images, verifyImages } from './windows-image-fixture.mjs';

const [root, resources, mode] = process.argv.slice(2);
const project = join(root, 'project');
const state = join(root, 'state');
const pi = join(root, 'offline-pi');
let runtime;
try {
  if (mode === 'verify') {
    const seed = JSON.parse(await readFile(join(root, 'seed.json'), 'utf8'));
    const dist = join(resources, 'runtime', 'dist');
    const { createDesktopSessionManager } = await import(pathToFileURL(join(dist, 'agents/pi-session-manager.js')).href);
    const manager = createDesktopSessionManager(project, dirname(seed.sessionFile), seed.sessionFile);
    verifyImages(manager, images());
    await writeFile(join(root, 'verified.json'), JSON.stringify({ historyAfter: digest(await readFile(seed.sessionFile)) }));
    process.exitCode = 0;
  } else {
  await mkdir(project);
  await mkdir(state);
  await mkdir(pi);
  Object.assign(process.env, { HIVE_DESKTOP_DATA_DIR: state, PI_CODING_AGENT_DIR: pi, PI_OFFLINE: '1' });
  const dist = join(resources, 'runtime', 'dist');
  const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
  const { createDesktopSessionManager } = await import(pathToFileURL(join(dist, 'agents/pi-session-manager.js')).href);
  runtime = await createDesktopRuntime(state, () => {});
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: project, name: 'Image memory fixture', autostart_orchestrator: false,
  });
  const photo = await runtime.chats.create(workspace.id, 'pi');
  const empty = await runtime.chats.create(workspace.id, 'pi');
  runtime.chats.rename(photo.id, 'History photographs');
  runtime.chats.rename(empty.id, 'Empty conversation');
  runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: photo.id } }));
  await runtime.close();
  runtime = undefined;

  const originals = images();
  const encoded = originals.map(image => image.toString('base64'));
  const manager = () => createDesktopSessionManager(project, dirname(photo.sessionFile), photo.sessionFile);
  const history = manager();
  for (let index = 0; index < 48; index++) {
    const text = index === 0 ? 'EARLY_HISTORY_IMAGE_01' :
      index === 47 ? 'LATEST_HISTORY_IMAGE_48' : `History photo ${index + 1}`;
    history.appendMessage({ role: 'user', timestamp: 1790000000000 + index, content: [
      { type: 'text', text },
      { type: 'image', mimeType: 'image/png', data: encoded[imageIndex(index)] },
    ] });
  }
  verifyImages(manager(), originals);
  await writeFile(join(state, 'desktop-language.json'), JSON.stringify('en'));
  await writeFile(join(root, 'seed.json'), JSON.stringify({
    sessionFile: photo.sessionFile,
    historyBefore: digest(await readFile(photo.sessionFile)),
    images: originals.map((image, i) => ({
      sha256: digest(image), bytes: image.length, dimensions: [[848, 569], [900, 650], [992, 684]][i],
    })),
  }));
  }
} finally {
  if (runtime) await runtime.close();
}
