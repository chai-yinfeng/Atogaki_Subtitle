// Controlled-launch association only; not an OS responsibility/parent assertion.
const {execFileSync}=require('node:child_process');
function parseProcesses(output){
 return output.split('\n').filter(line=>line.trim()).map(line=>{
  const fields=line.trim().split(/\s+/);
  if(fields.length<9)throw Error('Unexpected ps process identity format');
  return {pid:Number(fields[0]),ppid:Number(fields[1]),rssKiB:Number(fields[2]),startedAt:fields.slice(3,8).join(' '),command:fields.slice(8).join(' ')};
 });
}
function snapshot(){return parseProcesses(execFileSync('/bin/ps',['-axo','pid=,ppid=,rss=,lstart=,comm='],{encoding:'utf8'}));}
const sameIdentity=(a,b)=>a.pid===b.pid&&a.command===b.command&&a.startedAt===b.startedAt;
function newWebKitProcesses(before,after){
 return after.filter(row=>/\/com\.apple\.WebKit\.(WebContent|GPU|Networking)(?:\.xpc\/|$)/.test(row.command)&&!before.some(old=>sameIdentity(old,row)));
}
function sampleIdentities(expected,rows){
 return expected.map(identity=>{
  const row=rows.find(row=>row.pid===identity.pid);
  if(!row||!sameIdentity(identity,row))throw Error(`Associated WebKit PID ${identity.pid} exited or changed identity`);
  return row;
 });
}
module.exports={parseProcesses,snapshot,sameIdentity,newWebKitProcesses,sampleIdentities};
