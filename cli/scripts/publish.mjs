// Explicit owner-authorized publication, with private npm configuration in a temp directory.
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { verifyRegistryArtifact } from './registry.mjs';
const cli = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const token = process.env.MULTI_CODEX_NPM_TOKEN;
if (!process.argv.includes('--yes')) throw new Error('Publication requires explicit --yes');
if (!token) throw new Error('Provide MULTI_CODEX_NPM_TOKEN in the process environment; never pass a token argument');
const platform = process.argv.find(value => ['linux-x64','linux-arm64','darwin-arm64'].includes(value));
if (!platform) throw new Error('Choose one verified platform explicitly');
const packageVersion = JSON.parse(await readFile(join(cli,'npm/root/package.json'),'utf8')).version;
for (const name of ['packed-command-acceptance.json','tamper-acceptance.json']) {
  const result = JSON.parse(await readFile(join(cli,'artifacts',name),'utf8'));
  if (result.passed !== true || result.version !== packageVersion) throw new Error(`Required local package check did not pass: ${name}`);
}
const inventory = JSON.parse(await readFile(join(cli,'artifacts/package-inventory.json'),'utf8'));
if (inventory.passed !== true) throw new Error('Package inventory did not pass');
const verified = new Map();
for (const name of [`multi-codex-${platform}`, '@russeldp/multi-codex']) {
  const filename = `${name.replace('@','').replace('/','-')}-${packageVersion}.tgz`;
  const bytes = await readFile(join(cli,'artifacts',filename));
  const hash = createHash('sha256').update(bytes).digest('hex');
  if (!inventory.packages.some(entry => entry.file === filename && entry.name === name && entry.version === packageVersion && entry.sha256 === hash)) throw new Error(`Package inventory mismatch: ${name}`);
  verified.set(name, hash);
}
const temporary = await mkdtemp(join(tmpdir(),'multi-codex-npm-publish-'));
const reports = [];
try {
  const config = join(temporary,'npmrc');
  await writeFile(config,'registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=${MULTI_CODEX_NPM_TOKEN}\n',{mode:0o600});
  const env = {...process.env,NPM_CONFIG_USERCONFIG:config,NPM_CONFIG_REGISTRY:'https://registry.npmjs.org/',NPM_CONFIG_LOGS_DIR:join(temporary,'logs')};
  for (const name of [`multi-codex-${platform}`,'@russeldp/multi-codex']) {
    const tarball = join(cli,'artifacts',`${name.replace('@','').replace('/','-')}-${packageVersion}.tgz`);
    const bytes = await readFile(tarball);
    if (createHash('sha256').update(bytes).digest('hex') !== verified.get(name)) throw new Error('Package changed after verification');
    const result = spawnSync('npm',['publish',tarball,'--access','public','--tag','latest','--ignore-scripts','--dry-run=false','--registry','https://registry.npmjs.org/'],{cwd:cli,env,encoding:'utf8',timeout:120000});
    const safe = `${result.stdout || ''}\n${result.stderr || ''}`.split(token).join('[redacted]');
    reports.push({name,version:packageVersion,sha256:createHash('sha256').update(bytes).digest('hex'),published:result.status===0});
    await writeFile(join(cli,'artifacts/publication-result.json'),JSON.stringify({completed:false,packages:reports},null,2)+'\n');
    if (result.status!==0) {console.error(safe);throw new Error(`npm publication failed for ${name}; earlier successful packages were retained`);}
    console.log(`Accepted publication ${name}@${packageVersion}`);
    const publicResult = await verifyRegistryArtifact(name, packageVersion, verified.get(name));
    Object.assign(reports[reports.length - 1], publicResult);
    await writeFile(join(cli,'artifacts/publication-result.json'),JSON.stringify({completed:name==='@russeldp/multi-codex',packages:reports},null,2)+'\n');
    console.log(`Verified public package ${name}@${packageVersion}`);
  }
} finally { await rm(temporary,{recursive:true,force:true}); }
