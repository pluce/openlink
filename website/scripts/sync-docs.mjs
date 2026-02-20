import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

const websiteRoot = process.cwd();
const repoRoot = path.resolve(websiteRoot, '..');
const outRoot = path.join(websiteRoot, 'src', 'content', 'docs', 'generated');

const docsToMirror = [
  // SDK docs (kept with same structure)
  'docs/sdk/README.md',
  'docs/sdk/quickstart-raw-nats.md',
  'docs/sdk/concepts.md',
  'docs/sdk/integration-architecture.md',
  'docs/sdk/addressing-routing.md',
  'docs/sdk/envelopes-and-stack.md',
  'docs/sdk/nats-transport.md',
  'docs/sdk/stations-presence.md',
  'docs/sdk/integration-checklist.md',
  'docs/sdk/conformance-profile.md',
  'docs/sdk/high-level-api-contract.md',
  'docs/sdk/reference/README.md',
  'docs/sdk/reference/cpdlc-messages.md',

  // Crate docs (kept with same structure)
  'crates/openlink-models/README.md',
  'crates/openlink-sdk/README.md',
  'crates/openlink-auth/README.md',
  'crates/openlink-server/README.md',
  'crates/openlink-cli/README.md',
  'crates/openlink-gui/README.md',
  'crates/openlink-loadtest/README.md',
  'crates/mock-oidc/README.md',

  // Spec docs
  'spec/README.md',
];

const metadataByPath = {
  'docs/sdk/README.md': { title: 'SDK Integrator Guide', order: 1 },
  'docs/sdk/concepts.md': { title: 'Concepts', order: 2 },
  'docs/sdk/integration-architecture.md': { title: 'Integration Architecture', order: 3 },
  'docs/sdk/addressing-routing.md': { title: 'Addressing & Routing', order: 4 },
  'docs/sdk/envelopes-and-stack.md': { title: 'Envelopes & Stack', order: 5 },
  'docs/sdk/nats-transport.md': { title: 'NATS Transport', order: 6 },
  'docs/sdk/stations-presence.md': { title: 'Stations Presence', order: 7 },
  'docs/sdk/quickstart-raw-nats.md': { title: 'Quickstart (Raw NATS)', order: 8 },
  'docs/sdk/integration-checklist.md': { title: 'Integration Checklist', order: 9 },
  'docs/sdk/conformance-profile.md': { title: 'Conformance Profile', order: 10 },
  'docs/sdk/high-level-api-contract.md': { title: 'High-Level API Contract', order: 11 },
  'docs/sdk/reference/README.md': { title: 'SDK Reference', order: 12 },
  'docs/sdk/reference/cpdlc-messages.md': { title: 'CPDLC Messages', order: 13 },

  'crates/openlink-models/README.md': { title: 'openlink-models', order: 1 },
  'crates/openlink-sdk/README.md': { title: 'openlink-sdk', order: 2 },
  'crates/openlink-auth/README.md': { title: 'openlink-auth', order: 3 },
  'crates/openlink-server/README.md': { title: 'openlink-server', order: 4 },
  'crates/openlink-cli/README.md': { title: 'openlink-cli', order: 5 },
  'crates/openlink-gui/README.md': { title: 'openlink-gui', order: 6 },
  'crates/openlink-loadtest/README.md': { title: 'openlink-loadtest', order: 7 },
  'crates/mock-oidc/README.md': { title: 'mock-oidc', order: 8 },

  'spec/README.md': { title: 'Spec Reference', order: 1 },
};

const rewriteInternalDocLinks = (content, relativeSource, currentSlug) =>
  content.replace(/(!?\[[^\]]*\])\(([^)]+)\)/g, (match, label, target) => {
    if (label.startsWith('!')) {
      return match;
    }

    const trimmed = target.trim();
    if (
      trimmed.startsWith('#') ||
      trimmed.startsWith('/') ||
      /^[a-z][a-z0-9+.-]*:/i.test(trimmed)
    ) {
      return match;
    }

    const [withoutHash, hash = ''] = trimmed.split('#', 2);
    const [pathPart, query = ''] = withoutHash.split('?', 2);

    if (!/\.mdx?$/i.test(pathPart)) {
      return match;
    }

    const sourceDir = path.posix.dirname(relativeSource);
    const resolvedSourcePath = path.posix.normalize(path.posix.join(sourceDir, pathPart));
    const withoutExt = resolvedSourcePath.replace(/\.mdx?$/i, '');
    const readmeToDir = withoutExt.replace(/\/README$/i, '');
    const targetSlug = `generated/${readmeToDir}`;
    let routed = path.posix.relative(currentSlug, targetSlug);

    if (!routed || routed === '') {
      routed = '.';
    }

    if (!routed.endsWith('/')) {
      routed = `${routed}/`;
    }

    const rebuilt = `${routed}${query ? `?${query}` : ''}${hash ? `#${hash}` : ''}`;
    return `${label}(${rebuilt})`;
  });

const slugFromSourcePath = (relativeSource) => {
  const withoutExt = relativeSource.replace(/\.md$/i, '');
  if (/\/README$/i.test(withoutExt)) {
    return `generated/${withoutExt.replace(/\/README$/i, '')}`;
  }
  return undefined;
};

const withFrontmatter = (content, title, description, srcPath, includeHeader = false, order, slug, linkSourcePath = srcPath) => {
  const clean = content.replace(/^\uFEFF/, '');
  const hasMarkdownHeader = /^\s*#\s+/m.test(clean);
  const header = includeHeader && !hasMarkdownHeader ? `# ${title}\n\n` : '';
  const sidebarOrder = Number.isFinite(order) ? `\nsidebar:\n  order: ${order}` : '';
  const slugLine = slug ? `\nslug: ${slug}` : '';
  const linked = rewriteInternalDocLinks(clean, linkSourcePath, slug || `generated/${linkSourcePath.replace(/\.md$/i, '')}`);
  return `---\ntitle: ${title}\ndescription: ${description}${slugLine}${sidebarOrder}\n---\n\n> Source: ${srcPath} (synced automatically)\n\n${header}${linked}`;
};

const titleFromPath = (filePath) => path.basename(filePath, path.extname(filePath));

await rm(outRoot, { recursive: true, force: true });

// Overview is composed from dedicated markdown + workspace README
{
  const introPath = 'docs/website/openlink-overview-intro.md';
  const readmePath = 'README.md';
  const intro = await readFile(path.join(repoRoot, introPath), 'utf8');
  const readme = await readFile(path.join(repoRoot, readmePath), 'utf8');
  const overviewContent = `${intro}\n\n${readme}`;
  const overviewTarget = path.join(outRoot, 'overview.md');
  await mkdir(path.dirname(overviewTarget), { recursive: true });
  await writeFile(
    overviewTarget,
    withFrontmatter(
      overviewContent,
      'OpenLink Overview',
      'Why OpenLink, architecture, and full workspace overview.',
      `${introPath} + ${readmePath}`,
      false,
      1,
      'generated/overview',
      readmePath,
    ),
    'utf8',
  );
}

for (const relativeSource of docsToMirror) {
  const sourceFile = path.join(repoRoot, relativeSource);
  const targetFile = path.join(outRoot, relativeSource);
  const targetDir = path.dirname(targetFile);
  await mkdir(targetDir, { recursive: true });

  const content = await readFile(sourceFile, 'utf8');
  const meta = metadataByPath[relativeSource] || {};
  const title = meta.title || titleFromPath(relativeSource);
  const description = `Mirrored documentation from ${relativeSource}`;
  const slug = slugFromSourcePath(relativeSource);

  await writeFile(
    targetFile,
    withFrontmatter(content, title, description, relativeSource, true, meta.order, slug),
    'utf8',
  );
}

console.log(
  `Synced ${docsToMirror.length + 1} documentation pages into ${path.relative(websiteRoot, outRoot)}`,
);
