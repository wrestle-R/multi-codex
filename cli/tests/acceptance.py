#!/usr/bin/env python3
"""Exercise CLI process boundaries with native keyring and disposable accounts."""
import argparse, hashlib, json, os, pathlib, subprocess, tempfile, time
parser=argparse.ArgumentParser();parser.add_argument('--binary',required=True);parser.add_argument('--output',required=True);args=parser.parse_args()
binary=str(pathlib.Path(args.binary).resolve());report={'passed':False,'version':json.loads(subprocess.check_output([binary,'--version','--json'],text=True))['data']['version'],'checks':{}}
with tempfile.TemporaryDirectory(prefix='multi-codex-cli-acceptance-') as temporary:
 root=pathlib.Path(temporary);data=root/'data with spaces';global_home=root/'global';global_home.mkdir();(global_home/'auth.json').write_text('{"protected":"default-auth-sentinel"}');original=hashlib.sha256((global_home/'auth.json').read_bytes()).hexdigest()
 tools=root/'tools';tools.mkdir();journal=root/'launches.jsonl'
 engine=tools/'codex';engine.write_text('''#!/usr/bin/python3
import json,os,sys
if sys.argv[1:3]==['login','status']:
 sys.exit(0)
if '-C' in sys.argv:
 with open(os.environ['CLI_TEST_JOURNAL'],'a') as stream: stream.write(json.dumps({'home':os.environ.get('CODEX_HOME'),'sqlite':os.environ.get('CODEX_SQLITE_HOME'),'workspace':sys.argv[sys.argv.index('-C')+1],'foreignAuth':os.environ.get('OPENAI_API_KEY')})+'\\n')
 sys.exit(0)
sys.exit(1)
''');engine.chmod(0o700)
 terminal=tools/'kitty';terminal.write_text('''#!/usr/bin/python3
import os,subprocess,sys
args=sys.argv[1:]
if '--directory' in args:
 i=args.index('--directory');os.chdir(args[i+1]);args=args[i+2:]
subprocess.run(args,check=True)
''');terminal.chmod(0o700)
 env={**os.environ,'PATH':str(tools)+os.pathsep+os.environ['PATH'],'CLI_TEST_JOURNAL':str(journal),'OPENAI_API_KEY':'inert-foreign-override','WAYLAND_DISPLAY':os.environ.get('WAYLAND_DISPLAY','fixture')}
 def run(*command,success=True,stdin=None):
  result=subprocess.run([binary,'--data-dir',str(data),'--global-codex-home',str(global_home),'--json',*command],input=stdin,text=True,capture_output=True,env=env,timeout=40)
  if success and result.returncode:raise AssertionError('CLI command failed: '+str(command)+' '+result.stdout+' '+result.stderr)
  if not success and not result.returncode:raise AssertionError('Expected refusal: '+str(command))
  parsed=json.loads(result.stdout);assert parsed['schemaVersion']==1
  return parsed
 names=['Fixture account A','Fixture account B'];created=[]
 try:
  assert run('accounts','list')['data']==[]
  run('settings','set','codex-path',str(engine))
  for name in names:
   result=run('accounts','import','--name',name,'--stdin',stdin='{"auth_mode":"apikey","OPENAI_API_KEY":"inert-cli-native-keyring-fixture"}')
   created.append(result['data']['id'])
  report['checks']['nativeKeyringImportAcrossProcesses']=len(run('accounts','list')['data'])==2
  for home in ['codex-home','bridge-runtime/windows/'+created[1],'vscode-user-data/User/globalStorage/russeldanielpaul.multi-codex/runtime/windows/'+created[1]]:
   nested_env={**env,'CODEX_HOME':str(data/'profiles'/created[0]/home)}
   nested=subprocess.run([binary,'--data-dir',str(data),'--json','accounts','list'],env=nested_env,text=True,capture_output=True,timeout=40)
   assert nested.returncode==0 and len(json.loads(nested.stdout)['data'])==2
  report['checks']['managementInsideManagedTerminal']=True
  run('accounts','edit',names[0],'--notes','retained notes')
  assert run('accounts','show',names[0])['data']['notes']=='retained notes'
  run('accounts','import','--name',names[0],'--stdin',stdin='{"auth_mode":"apikey","OPENAI_API_KEY":"inert-cli-native-keyring-fixture"}',success=False)
  run('accounts','remove',names[0],success=False)
  report['checks']['crudUniqueNamesAndExplicitConsent']=True
  workspace=root/'workspace with spaces';workspace.mkdir()
  for name in names:run('launch','--account',name,'--target','cli','--terminal','kitty','--workspace',str(workspace))
  deadline=time.monotonic()+10
  while (not journal.exists() or len(journal.read_text().splitlines())<2) and time.monotonic()<deadline:time.sleep(.1)
  rows=[json.loads(line) for line in journal.read_text().splitlines()]
  assert len(rows)==2 and len({row['home'] for row in rows})==2
  assert all(row['home']==row['sqlite'] and row['workspace']==str(workspace) and row['foreignAuth'] is None for row in rows)
  report['checks']['terminalArgumentIsolationWithSpaces']=True
  run('usage','--all',success=False)
  run('settings','set','terminal','kitty');assert run('settings','get','terminal')['data']=='kitty'
  run('settings','set','not-a-setting','value',success=False)
  run('storage','cache','clear',success=False)
  run('storage','chats','preview','--account',names[0],success=False)
  run('storage','backups','verify','--account',names[0],success=False)
  report['checks']['scopeAndCapabilityRefusals']=True
  run('status');run('storage');run('storage','backups','list');run('terminals','list');run('doctor')
  assert hashlib.sha256((global_home/'auth.json').read_bytes()).hexdigest()==original
  report['checks']['defaultAuthUnchanged']=True
 finally:
  for identifier in created:run('accounts','remove',identifier,'--yes')
  if created:assert run('accounts','list')['data']==[]
 report['checks']['ownedAccountsAndNativeCredentialsRemoved']=True
 report['passed']=True
pathlib.Path(args.output).parent.mkdir(parents=True,exist_ok=True);pathlib.Path(args.output).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
