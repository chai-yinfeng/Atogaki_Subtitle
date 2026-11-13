#!/usr/bin/env node
// Execute the production state and gate without DOM/IPC; not a GUI measurement.
import {readFileSync, writeFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
import {OnlineCaptions, DraftPresentation} from '../../ui/src/online-state.ts';

export function replay(events, pollSeconds = .5) {
  if (!Number.isFinite(pollSeconds) || pollSeconds <= 0) throw new Error('Polling interval must be positive');
  let lastTime = -Infinity;
  for (const event of events) {
    if (!Number.isFinite(event.emission_s) || event.emission_s < lastTime) throw new Error('Events must have finite monotonic emission_s');
    lastTime = event.emission_s;
  }
  const model = new OnlineCaptions();
  model.snapshot({status:{sessionId:'replay', phase:'running',error:null,translationEnabled:true},events:[]});
  const gates = {source:new Map(),translation:new Map()};
  const snapshots = [];
  let previous = '', tick = 0;
  function track(kind, parts, now) {
    const keys = new Set(parts.map(e => e.group_id ?? e.segment_id));
    for (const key of gates[kind].keys()) if (!keys.has(key)) gates[kind].delete(key);
    return parts.map(e => {
      const key = e.group_id ?? e.segment_id;
      if (!gates[kind].has(key)) gates[kind].set(key, new DraftPresentation());
      return gates[kind].get(key).select(e, now * 1000);
    });
  }
  function render(now, trigger) {
    const translations = track('translation', model.tracks().translationParts, now);
    const sources = track('source', model.tracks(translations).sourceParts, now);
    const current = {sources, translations};
    const key = JSON.stringify(current);
    if (key !== previous) snapshots.push({time_s:now, trigger, ...current});
    previous = key;
  }
  for (const [index, event] of events.entries()) {
    while (tick < event.emission_s) {render(tick, 'poll');tick += pollSeconds;}
    model.accept({sessionId:'replay',sequence:index+1,event});
    render(event.emission_s, 'event');
  }
  const end = (events.at(-1)?.emission_s ?? 0) + 2;
  while (tick <= end) {render(tick, 'poll');tick += pollSeconds;}
  const returns = events.filter(e => ['source_draft','source_final','translation_draft','translation_final'].includes(e.kind));
  const versions = returns.map((event, index) => {
    const kind = event.kind.startsWith('source_') ? 'sources' : 'translations';
    const key = event.group_id ?? event.segment_id;
    const shown = snapshots.find(s => s.time_s >= event.emission_s && s[kind].some(e =>
      (e.group_id ?? e.segment_id) === key && e.kind === event.kind && e.text === event.text));
    return {return_index:index, kind:event.kind, key, emission_s:event.emission_s,
      first_same_text_shown_s:shown?.time_s ?? null,
      extra_show_wait_s:shown ? shown.time_s-event.emission_s : null,
      note:'Text/kind match, not request identity or semantic correctness'};
  });
  const waits = {};
  for (const kind of ['source_draft','source_final','translation_draft','translation_final']) {
    const rows = versions.filter(e => e.kind === kind);
    const values = rows.map(e => e.extra_show_wait_s).filter(e => e !== null).sort((a,b)=>a-b);
    waits[kind] = {returns:rows.length, not_shown:rows.length-values.length,
      p50_extra_s:values[Math.max(0,Math.ceil(values.length*.5)-1)] ?? null,
      p95_extra_s:values[Math.max(0,Math.ceil(values.length*.95)-1)] ?? null,
      max_extra_s:values.at(-1) ?? null};
  }
  const visible = new Map();
  for (const snapshot of snapshots) for (const event of snapshot.translations) {
    const key = event.group_id;
    if (!visible.has(key)) visible.set(key,{group_id:key,first_shown_s:snapshot.time_s,last_text_change_s:snapshot.time_s,
      first_final_shown_s:null,last_seen_s:snapshot.time_s,text:event.text});
    const row = visible.get(key);
    if (row.text !== event.text) row.last_text_change_s=snapshot.time_s;
    if (event.kind === 'translation_final' && row.first_final_shown_s === null) row.first_final_shown_s=snapshot.time_s;
    row.text=event.text;
    row.last_seen_s=snapshot.time_s;
  }
  const groups = [...visible.values()].map(({text,...row})=>row);
  return {waits, groups, notes:['Production OnlineCaptions/DraftPresentation; immediate event render plus 500ms polling.',
    'Polling phase assumed zero; native delivery, snapshot transport and DOM paint excluded.',
    'Wait includes reading-window eligibility, source/translation pairing and rewrite gating, not gating alone.',
    'Superseded versions may never display. Same text may match a later revision.'], snapshots, versions};
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [input, output] = process.argv.slice(2);
  if (!input || !output) throw new Error('Usage: node --experimental-strip-types replay_presentation.mjs events.jsonl output.json');
  const events = readFileSync(input, 'utf8').trim().split('\n').map(JSON.parse);
  const result = replay(events);
  writeFileSync(output, JSON.stringify(result, null, 2)+'\n');
  console.log(JSON.stringify({snapshots:result.snapshots.length, versions:result.versions.length,
    not_shown:result.versions.filter(e=>e.first_same_text_shown_s===null).length}));
}
