import { existsSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
const root=resolve(import.meta.dirname,'..');
const env={...process.env};
const pathKey=Object.keys(env).find(k=>k.toLowerCase()==='path')??'PATH';
const originalPath=env[pathKey]??'';
for(const k of Object.keys(env)){if(k.toLowerCase()==='path')delete env[k];}
env.PATH=originalPath;
const local=join(root,'.tools','rustup','toolchains');
if(existsSync(local)) {
  const toolchain=readdirSync(local).find(n=>n.startsWith('stable-'));
  if(toolchain){env.PATH=join(local,toolchain,'bin')+';'+env.PATH;env.CARGO_HOME=join(root,'.tools','cargo');env.RUSTUP_HOME=join(root,'.tools','rustup');}
}
if(existsSync('E:/mingw64/bin/gcc.exe')) env.PATH='E:/mingw64/bin;'+env.PATH;
const action=process.argv[2]??'dev';
const command=['test','check'].includes(action)?'cargo':process.execPath;
const args=action==='test'?['test','--manifest-path','src-tauri/Cargo.toml','--no-default-features',...process.argv.slice(3)]:action==='check'?['check','--manifest-path','src-tauri/Cargo.toml',...process.argv.slice(3)]:[join(root,'node_modules','@tauri-apps','cli','tauri.js'),action,...process.argv.slice(3)];
const result=spawnSync(command,args,{cwd:root,env,stdio:'inherit'});
if(result.error) console.error(result.error.message);
process.exit(result.status??1);
