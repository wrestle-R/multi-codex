#!/usr/bin/env python3
"""Native compositor acceptance with disposable accounts; no global settings writes."""
from pathlib import Path
from gi.repository import Gio, GLib
import subprocess, os, json, time, uuid
import argparse
parser=argparse.ArgumentParser();parser.add_argument('--backend',choices=['kde','gnome'],required=True);parser.add_argument('--launcher',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
assert os.environ.get('MULTI_CODEX_TEST_COMPOSITOR_ISOLATED')=='1','Run only inside a disposable compositor and session bus'
root=Path(os.environ['HOME']).parent
repo=Path(__file__).resolve().parents[2];out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
connection=Gio.bus_get_sync(Gio.BusType.SESSION,None)
reply=[];loop=GLib.MainLoop()
def handle(conn,sender,path,interface,method,params,invocation):
 reply.append(json.loads(params.unpack()[1]));invocation.return_value(GLib.Variant('()',()));loop.quit()
interface=Gio.DBusNodeInfo.new_for_xml('<node><interface name="com.multicodex.KWinReply1"><method name="Publish"><arg type="s" direction="in"/><arg type="s" direction="in"/></method></interface></node>').interfaces[0]
connection.register_object('/com/multicodex/KWinReply',interface,handle,None,None)
next_script=0
def execute_kde(body):
 global next_script
 next_script+=1;name=f'mc-hud-test-{next_script}'
 path=root/f'{name}.js'
 path.write_text('try { '+body+" } catch(e) { callDBus("+json.dumps(connection.get_unique_name())+", '/com/multicodex/KWinReply', 'com.multicodex.KWinReply1', 'Publish', 'inspect', JSON.stringify({error:String(e)})); }")
 result=subprocess.check_output(['busctl','--user','call','org.kde.KWin','/Scripting','org.kde.kwin.Scripting','loadScript','ss',str(path),name],text=True)
 index=int(result.split()[1]);reply.clear()
 subprocess.check_call(['busctl','--user','call','org.kde.KWin',f'/Scripting/Script{index}','org.kde.kwin.Script','run'])
 timeout=GLib.timeout_add_seconds(5,lambda:loop.quit());loop.run()
 if reply:GLib.source_remove(timeout)
 subprocess.check_call(['busctl','--user','call','org.kde.KWin','/Scripting','org.kde.kwin.Scripting','unloadScript','s',name],stdout=subprocess.DEVNULL)
 assert reply,'KWin script callback timed out'
 assert not isinstance(reply[0],dict) or 'error' not in reply[0],reply[0]
 return reply[0]
def publish(expr):return "callDBus("+json.dumps(connection.get_unique_name())+", '/com/multicodex/KWinReply', 'com.multicodex.KWinReply1', 'Publish', 'inspect', JSON.stringify("+expr+"));"
def execute(body):
 if args.backend=='kde':return execute_kde(body)
 proxy=Gio.DBusProxy.new_sync(connection,Gio.DBusProxyFlags.NONE,None,'org.gnome.Shell','/org/gnome/Shell','org.gnome.Shell',None)
 success,raw=proxy.call_sync('Eval',GLib.Variant('(s)',('(function(){'+body+'})()')),Gio.DBusCallFlags.NONE,5000,None).unpack()
 assert success,raw
 return json.loads(raw)
def windows():
 if args.backend=='kde':return execute(publish("workspace.windowList().map(w=>({title:w.caption,pid:w.pid,geometry:w.frameGeometry,above:w.keepAbove,sticky:w.onAllDesktops,fullscreen:w.fullScreen}))"))
 return execute("return JSON.stringify(global.get_window_actors().map(a=>a.meta_window).map(w=>({title:w.get_title(),pid:w.get_pid(),geometry:w.get_frame_rect(),above:w.is_above(),sticky:w.is_on_all_workspaces(),fullscreen:w.is_fullscreen()})));")
def mutate(kde,gnome):return execute(kde+publish('true') if args.backend=='kde' else gnome+"return JSON.stringify(true);")
def until(condition):
 deadline=time.monotonic()+25
 while time.monotonic()<deadline:
  v=windows()
  if condition(v):return v
  time.sleep(.2)
 raise RuntimeError('KWin window condition timed out: '+json.dumps(v))
data=root/'data/accounts';home=root/'global';profile_id=str(uuid.uuid4());profile=data/'profiles'/profile_id/'codex-home';profile.mkdir(parents=True);home.mkdir()
auth=b'{"auth_mode":"apikey","OPENAI_API_KEY":"inert-kde-hud-fixture"}';(profile/'auth.json').write_bytes(auth)
metadata=json.dumps([{'id':profile_id,'name':'KDE disposable avatar','authMode':'apikey','createdAt':'2026-10-14T00:00:00Z','updatedAt':'2026-10-14T00:00:00Z'}]).encode();(data/'profiles.json').write_bytes(metadata)
(data/'migrations').mkdir();(data/'migrations/sharing-queue.json').write_text(json.dumps({'schema':1,'enabled':False,'profiles':{}}))
env={k:v for k,v in os.environ.items() if not k.startswith(('CODEX_','OPENAI_','MULTI_CODEX_','VSCODE_')) and k not in ['ELECTRON_RUN_AS_NODE','NODE_OPTIONS']}
env.update(WAYLAND_DISPLAY=os.environ['WAYLAND_DISPLAY'],GDK_BACKEND='wayland',MULTI_CODEX_DATA_DIR=str(data),CODEX_HOME=str(home),WEBKIT_DISABLE_DMABUF_RENDERER='1')
log=(out/'launcher.log').open('wb');app=subprocess.Popen([str(args.launcher.resolve())],env=env,stdout=log,stderr=log)
report={'passed':False,'backend':args.backend,'isolatedVirtualCompositor':True,'checks':[]}
try:
 v=until(lambda v:any(w['title']=='Multi Codex HUD' and w['above'] and w['sticky'] and w['geometry']['height']<150 for w in v));report['initialWindows']=v;report['checks'].append('compact HUD above ordinary windows and on all workspaces')
 subprocess.check_call(['busctl','--user','call','com.multicodex.Hud','/com/multicodex/Hud','com.multicodex.Hud1','Toggle']);until(lambda v:all(w['title']!='Multi Codex HUD' for w in v));report['checks'].append('native HUD toggle hides')
 subprocess.check_call(['busctl','--user','call','com.multicodex.Hud','/com/multicodex/Hud','com.multicodex.Hud1','Toggle']);until(lambda v:any(w['title']=='Multi Codex HUD' for w in v));report['checks'].append('native HUD toggle restores')
 mutate("workspace.windowList().find(w=>w.caption==='Multi Codex').fullScreen=true;", "global.get_window_actors().find(a=>a.meta_window.get_title()==='Multi Codex').meta_window.make_fullscreen();");until(lambda v:all(w['title']!='Multi Codex HUD' for w in v));report['checks'].append('fullscreen hides HUD')
 mutate("workspace.windowList().find(w=>w.caption==='Multi Codex').fullScreen=false;", "global.get_window_actors().find(a=>a.meta_window.get_title()==='Multi Codex').meta_window.unmake_fullscreen();");until(lambda v:any(w['title']=='Multi Codex HUD' for w in v));report['checks'].append('leaving fullscreen restores HUD')
 mutate("workspace.windowList().find(w=>w.caption==='Multi Codex').closeWindow();", "global.get_window_actors().find(a=>a.meta_window.get_title()==='Multi Codex').meta_window.delete(global.get_current_time());");until(lambda v:all(w['title']!='Multi Codex' for w in v) and any(w['title']=='Multi Codex HUD' for w in v));assert app.poll() is None;report['checks'].append('closing launcher preserves HUD and process')
 assert (profile/'auth.json').read_bytes()==auth;assert (data/'profiles.json').read_bytes()==metadata;report.update(passed=True,fixtureBytesUnchanged=True)
 print(json.dumps(report,indent=2))
except Exception as e:report['failure']=str(e);raise
finally:
 app.terminate()
 try:app.wait(timeout=10)
 except subprocess.TimeoutExpired:app.kill();app.wait()
 log.close();(out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
