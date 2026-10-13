import { createHash } from 'node:crypto';

// Publication can be accepted before npm finishes scanning and serving a version.
// Confirm the exact public bytes before exposing a root package that needs them.
export async function verifyRegistryArtifact(name, version, expectedSha256, timeoutMs = 600_000) {
  const deadline = Date.now() + timeoutMs;
  const registry = 'https://registry.npmjs.org/';
  const encoded = name.replace('/', '%2f');
  let waiting = false;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${registry}${encoded}/${version}?verify=${Date.now()}`, {
        headers: { 'Cache-Control': 'no-cache' }, signal: AbortSignal.timeout(20_000),
      });
      if (response.ok) {
        const metadata = await response.json();
        const url = new URL(metadata.dist.tarball);
        if (url.origin !== new URL(registry).origin || url.protocol !== 'https:') {
          throw new Error('Unexpected registry artifact host');
        }
        const artifact = await fetch(url, { signal: AbortSignal.timeout(60_000) });
        if (artifact.ok) {
          const hash = createHash('sha256').update(Buffer.from(await artifact.arrayBuffer())).digest('hex');
          if (hash !== expectedSha256) throw new Error(`Registry artifact checksum mismatch: ${name}@${version}`);
          return { name, version, sha256: hash, registryVerified: true };
        }
      } else if (response.status !== 404 && response.status < 500 && response.status !== 429) {
        throw new Error(`Registry verification refused with HTTP ${response.status}`);
      }
    } catch (error) {
      if (error.message?.includes('checksum mismatch') || error.message?.includes('Unexpected registry') || error.message?.includes('verification refused')) throw error;
    }
    if (!waiting) { console.log(`Waiting for public registry availability: ${name}@${version}`); waiting = true; }
    await new Promise(resolve => setTimeout(resolve, 10_000));
  }
  throw new Error(`Registry availability timed out: ${name}@${version}; accepted publication is retained`);
}
