import test from 'node:test';import assert from 'node:assert/strict';import {createRequire} from 'node:module';
const {OrbitSelection}=createRequire(import.meta.url)('../dist/core.cjs');
const a='00000000-0000-0000-0000-000000000001',b='00000000-0000-0000-0000-000000000002';
test('only successful frontend navigation selects a chat, never reads or cached load notifications',()=>{
 const ui=new OrbitSelection();ui.incoming({method:'thread/started',params:{thread:{id:b}}});assert.equal(ui.selected,null);
 ui.outgoing({id:1,method:'thread/resume',params:{threadId:a}});ui.incoming({id:1,result:{thread:{id:a}}});assert.equal(ui.selected,a);
 ui.outgoing({id:2,method:'thread/read',params:{threadId:b}});ui.incoming({id:2,result:{thread:{id:b}}});assert.equal(ui.selected,a);
 ui.outgoing({id:3,method:'thread/resume',params:{threadId:b}});ui.incoming({id:3,error:{code:-1,message:'failed'}});assert.equal(ui.selected,a);
 ui.outgoing({id:4,method:'thread/resume',params:{threadId:b}});ui.incoming({id:4,result:{thread:{id:b}}});assert.equal(ui.selected,b);
 ui.outgoing({id:5,method:'thread/unsubscribe',params:{threadId:a}});ui.incoming({id:5,result:{}});assert.equal(ui.selected,b);
 ui.outgoing({id:6,method:'thread/unsubscribe',params:{threadId:b}});ui.incoming({id:6,result:{}});assert.equal(ui.selected,null);
});
test('engine eviction keeps UI selection while explicit close and handoff clear it',()=>{
 const ui=new OrbitSelection();ui.outgoing({id:1,method:'turn/start',params:{threadId:a}});ui.incoming({id:1,result:{turn:{id:'turn'}}});ui.incoming({method:'thread/status/changed',params:{threadId:a,status:{type:'notLoaded'}}});assert.equal(ui.selected,a);ui.incoming({method:'thread/closed',params:{threadId:a}});assert.equal(ui.selected,null);
 ui.outgoing({id:2,method:'thread/resume'});ui.reset();ui.incoming({id:2,result:{thread:{id:a}}});assert.equal(ui.selected,null);
 ui.outgoing({id:3,method:'turn/start',params:{threadId:'invalid'}});ui.incoming({id:3,result:{turn:{id:'turn'}}});assert.equal(ui.selected,null);
});

test('fresh composer invalidates the prior view and prewarmed results never select cached chats',()=>{
 const ui=new OrbitSelection();
 for(const [id,thread] of [[1,a],[2,b]]){ui.outgoing({id,method:'thread/start'});ui.incoming({id,result:{thread:{id:thread}}});}
 assert.equal(ui.selected,null);
 ui.outgoing({id:3,method:'turn/start',params:{threadId:a}});ui.incoming({id:3,result:{turn:{id:'turn'}}});assert.equal(ui.selected,a);
 ui.outgoing({id:4,method:'thread/start'});ui.incoming({id:4,result:{thread:{id:b}}});assert.equal(ui.selected,null);
 ui.outgoing({id:5,method:'thread/unsubscribe',params:{threadId:b}});ui.incoming({id:5,result:{}});assert.equal(ui.selected,null);
 ui.outgoing({id:6,method:'turn/start',params:{threadId:a}});ui.outgoing({id:7,method:'thread/start'});ui.incoming({id:6,result:{turn:{id:'stale'}}});assert.equal(ui.selected,null);
});
