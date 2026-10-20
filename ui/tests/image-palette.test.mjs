import test from 'node:test';
import assert from 'node:assert/strict';
import {extractPalette,paletteVariables,contrast,hex} from '../src/image-palette.ts';
const unhex=value=>[1,3,5].map(i=>parseInt(value.slice(i,i+2),16));
test('palette ignores transparent pixels and a rare saturated outlier',()=>{
  const pixels=[];
  for(let i=0;i<200;i++)pixels.push(60,100,150,255);
  pixels.push(255,0,0,255,0,255,0,0);
  const palette=extractPalette(pixels);
  assert.equal(hex(palette.dominant),'#3c6496');
  assert.equal(hex(palette.accent),'#3c6496');
  assert.throws(()=>extractPalette([0,0,0,0]),/可见颜色/);
});
test('generated reading and button colors remain readable for light dark and gray images',()=>{
  for(const color of [[255,255,255],[0,0,0],[240,220,10],[255,0,100],[0,220,255],[128,128,128]]){
    const vars=paletteVariables({dominant:color,accent:color,swatches:[color]});
    assert.ok(contrast(unhex(vars['--ink']),unhex(vars['--reading-fill']))>=7);
    assert.ok(contrast(unhex(vars['--muted']),unhex(vars['--surface']))>=4.5);
    assert.ok(contrast(unhex(vars['--accent']),[255,250,241])>=4.5);
  }
});
test('three role selections independently control scene reading and accent colors',()=>{
  const palette={dominant:[30,70,120],accent:[30,100,140],swatches:[]};
  const original=paletteVariables(palette);
  const scene=paletteVariables(palette,.45,{ground:[180,50,40]});
  assert.notEqual(scene['--page-ground'],original['--page-ground']);
  assert.equal(scene['--accent'],original['--accent']);
  assert.equal(scene['--surface'],original['--surface']);
  const reading=paletteVariables(palette,.45,{reading:[210,170,60]});
  assert.notEqual(reading['--surface'],original['--surface']);
  assert.equal(reading['--accent'],original['--accent']);
  const accent=paletteVariables(palette,.45,{accent:[170,50,110],reading:palette.accent});
  assert.notEqual(accent['--accent'],original['--accent']);
  assert.equal(accent['--surface'],original['--surface']);
  for(const ground of [[255,255,255],[0,0,0]])for(const reading of [[255,240,0],[0,80,255]])for(const accent of [[0,255,255],[255,0,0]]){
    const colors=paletteVariables(palette,.45,{ground,reading,accent});
    assert.ok(contrast(unhex(colors['--ink']),unhex(colors['--reading-fill']))>=7);
    assert.ok(contrast(unhex(colors['--accent']),[255,250,241])>=4.5);
  }
});
test('extracted choices represent different colors rather than adjacent bins of one hue',()=>{
  const pixels=[];
  for(const color of [[40,90,130],[45,92,132],[210,180,60],[220,230,230]])for(let i=0;i<100;i++)pixels.push(...color,255);
  const palette=extractPalette(pixels);
  assert.equal(palette.swatches.length,3);
  assert.ok(palette.swatches.some(color=>color[0]>180&&color[2]<100));
});

test('choices retain small accents distributed across bins and offer more than three colors',()=>{
  const pixels=[];
  const colors=[[25,50,80],[60,105,145],[110,155,185],[200,220,230],[70,120,90],[130,150,55],[175,70,60],[180,100,150]];
  for(const color of colors)for(let i=0;i<500;i++)pixels.push(...color,255);
  // Each yellow bin is rare, but together they form a meaningful small accent.
  for(let i=0;i<24;i++)pixels.push(210+i%6*5,180+i%4*5,25+i%3*5,255);
  pixels.push(255,0,255,255);
  const palette=extractPalette(pixels);
  assert.ok(palette.swatches.length>3&&palette.swatches.length<=12);
  assert.ok(palette.swatches.some(color=>color[0]>200&&color[1]>170&&color[2]<80));
  assert.ok(!palette.swatches.some(color=>color[0]>240&&color[1]<10&&color[2]>240));
});
