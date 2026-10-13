import { readFile, writeFile, mkdir, cp, chmod, readdir, lstat } from 'node:fs/promises';
import { createHash, sign, createPublicKey } from 'node:crypto';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const cli = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(cli, '..');
const platform = process.argv[2] || `${process.platform}-${process.arch}`;
const triples = {'linux-x64':'x86_64-unknown-linux-gnu','linux-arm64':'aarch64-unknown-linux-gnu','darwin-arm64':'aarch64-apple-darwin'};
if (!triples[platform]) throw new Error('Unsupported CLI package target');
if (!process.env.MULTI_CODEX_CLI_SIGNING_KEY_FILE) throw new Error('Set MULTI_CODEX_CLI_SIGNING_KEY_FILE to a private signing-key path outside the repository');
const key = await readFile(process.env.MULTI_CODEX_CLI_SIGNING_KEY_FILE);
const pub = createPublicKey(key).export({format:'pem',type:'spki'}).toString();
const pinned = await import('../npm/root/bin/public-key.mjs');
if (pub !== pinned.publicKey) throw new Error('Signing key does not match the pinned package verification key');
const native = platform === `${process.platform}-${process.arch}`;
function run(command,args,cwd=cli){ const result=spawnSync(command,args,{cwd,stdio:'inherit',env:process.env});if(result.status!==0)throw new Error(`${command} failed`); }
run('cargo',['build','--locked','--release','-p','multi-codex-cli',...(native?[]:['--target',triples[platform]])]);
const built=join(cli,'target',...(native?[]:[triples[platform]]),'release');
const destination=join(cli,'npm/platforms',platform);
await mkdir(join(destination,'bin'),{recursive:true});
for(const name of ['multi-codex','multi-codex-account-helper']){await cp(join(built,name),join(destination,'bin',name));await chmod(join(destination,'bin',name),0o755);}
const bridge=join(destination,'bin/resources/bridge');
await mkdir(bridge,{recursive:true});
// Build extension sources independently; npm packages never require the desktop app.
run(process.platform==='win32'?'npm.cmd':'npm',['run','build'],join(root,'extension'));
const manifest=JSON.parse(await readFile(join(root,'extension/package.json'),'utf8'));
delete manifest.scripts;delete manifest.devDependencies;
await writeFile(join(bridge,'package.json'),JSON.stringify(manifest,null,2)+'\n');
for(const name of ['dist','resources','LICENSE','README.md'])await cp(join(root,'extension',name),join(bridge,name),{recursive:true});
await mkdir(join(bridge,'bin'),{recursive:true});
await cp(join(built,'multi-codex-account-helper'),join(bridge,'bin/multi-codex-account-helper'));
await chmod(join(bridge,'bin/multi-codex-account-helper'),0o755);
const helper=await readFile(join(bridge,'bin/multi-codex-account-helper'));
await writeFile(join(bridge,'desktop-helper.json'),JSON.stringify({schema:1,os:platform.startsWith('darwin')?'macos':'linux',architecture:platform.endsWith('arm64')?'aarch64':'x86_64',target:triples[platform],version:(await readFile(join(cli,'crates/core/Cargo.toml'),'utf8')).match(/^version = "([^"]+)"/m)[1],digest:createHash('sha256').update(helper).digest('hex')})+'\n');
await mkdir(join(destination,'resources'),{recursive:true});
await writeFile(join(destination,'resources/platform.json'),JSON.stringify({platform,engineBundled:false})+'\n');
for(const packageRoot of [destination,join(cli,'npm/root')]){await cp(join(root,'LICENSE'),join(packageRoot,'LICENSE'));await cp(join(cli,'README.md'),join(packageRoot,'README.md'));}
const files={};
async function walk(relative){const path=join(destination,relative);const stat=await lstat(path);if(stat.isSymbolicLink())throw new Error('Package symlinks are refused');if(stat.isDirectory()){for(const entry of (await readdir(path)).sort())await walk(`${relative}/${entry}`);}else if(stat.isFile())files[relative]=createHash('sha256').update(await readFile(path)).digest('hex');else throw new Error('Unsupported package entry');}
await walk('bin');await walk('resources');
const version=JSON.parse(await readFile(join(destination,'package.json'),'utf8')).version;
const payload=Buffer.from(JSON.stringify({schema:1,platform,version,files})+'\n');
await writeFile(join(destination,'payload.json'),payload);await writeFile(join(destination,'payload.sig'),sign(null,payload,key).toString('base64')+'\n');
const artifacts=join(cli,'artifacts');await mkdir(artifacts,{recursive:true});
for(const packageRoot of [destination,join(cli,'npm/root')])run('npm',['pack','--ignore-scripts','--pack-destination',artifacts],packageRoot);
console.log(`Prepared signed ${platform} and root npm tarballs. No publication performed.`);
