#!/usr/bin/env python3
"""Explicit four-range authorization; credential stays in memory, cloud files deleted."""
import argparse
import json
from pathlib import Path
import subprocess
import time
import urllib.request
import urllib.error
import urllib.parse
from run_asr import file_sha256


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise RuntimeError('redirect refused')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cases', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--authorization', type=Path, required=True)
    a = p.parse_args()
    authorization = json.loads(a.authorization.read_text())
    expected = {f'dev-{i}': (i*60000, (i+1)*60000) for i in range(4)}
    if authorization.get('provider') != 'Google Gemini' or authorization.get('ranges_ms') != [list(v) for v in expected.values()]:
        p.error('explicit authorization must match these four 60s ranges')
    cases = {name: json.loads((a.cases/name/'case.json').read_text()) for name in expected}
    for name,(lo,hi) in expected.items():
        c = cases[name]
        if (c['case_id'], c['role'], c['start_ms'], c['end_ms']) != ('ja-kokichi-vol1-development','development',lo,hi):
            p.error('case outside authorized ranges')
        if file_sha256(a.cases/name/'audio.wav') != c['audio_sha256']:
            p.error('PCM digest mismatch')
    credential = subprocess.run(['security','find-generic-password','-s','com.chai-yinfeng.atogaki.translation',
                                 '-a','asr.gemini-transcribe','-w'], capture_output=True, text=True)
    if credential.returncode or not credential.stdout.strip():
        raise SystemExit('Gemini credential unavailable')
    key = credential.stdout.strip()
    opener = urllib.request.build_opener(NoRedirect())
    def call(url, body=None, headers=None, method='POST'):
        request = urllib.request.Request(url,body,{'x-goog-api-key':key,**(headers or {})},method=method)
        try:
            with opener.open(request,timeout=180) as response:
                data = response.read()
                return json.loads(data) if data else {}, response.headers
        except urllib.error.HTTPError as e:
            raise RuntimeError('HTTP '+str(e.code)) from None
    for name,c in cases.items():
        root = a.output/name
        root.mkdir(parents=True, exist_ok=False)
        file_name = None
        before = time.monotonic()
        status = dict(audio_sha256=c['audio_sha256'], model='gemini-3.5-transcribe', reference_status='model_reference_unreviewed',
                      authorization_sha256=file_sha256(a.authorization), range_ms=[c['start_ms'],c['end_ms']])
        try:
            pcm = (a.cases/name/'audio.wav').read_bytes()
            _,headers = call('https://generativelanguage.googleapis.com/upload/v1beta/files',
                json.dumps({'file':{'display_name':'atogaki-controlled-'+name+'-60s'}}).encode(),
                {'Content-Type':'application/json','X-Goog-Upload-Protocol':'resumable','X-Goog-Upload-Command':'start',
                 'X-Goog-Upload-Header-Content-Length':str(len(pcm)),'X-Goog-Upload-Header-Content-Type':'audio/wav'})
            url = headers['X-Goog-Upload-URL']
            parsed = urllib.parse.urlsplit(url)
            if parsed.scheme!='https' or parsed.hostname!='generativelanguage.googleapis.com' or parsed.username:
                raise RuntimeError('unexpected upload origin')
            uploaded,_ = call(url,pcm,{'X-Goog-Upload-Offset':'0','X-Goog-Upload-Command':'upload, finalize','Content-Type':'audio/wav'})
            file_name = uploaded['file']['name']
            config = {'transcription_config':{'language_codes':['ja-JP'],
                      'mode':{'type':'verbatim','timestamp_granularities':['word']}}}
            body = {'model':status['model'],'input':[{'type':'audio','uri':uploaded['file']['uri'],'mime_type':'audio/wav'}],
                    'generation_config':config}
            (root/'request.json').write_text(json.dumps(dict(model=status['model'], generation_config=config, audio_sha256=c['audio_sha256']),indent=2)+'\n')
            response,_ = call('https://generativelanguage.googleapis.com/v1beta/interactions',json.dumps(body).encode(),{'Content-Type':'application/json'})
            (root/'response.json').write_text(json.dumps(response,ensure_ascii=False,indent=2)+'\n')
            words, texts = [], []
            for step in response.get('steps',[]):
                for content in step.get('content',[]):
                    if content.get('type')=='text':
                        texts.append(content.get('text',''))
                        words += [v for v in content.get('annotations',[]) if v.get('type')=='word_info']
            if response.get('status')!='completed' or not words:
                raise RuntimeError('incomplete response or no word timestamps')
            (root/'reference.json').write_text(json.dumps(dict(status, text=''.join(texts), words=words,
                          response_model=response.get('model')),ensure_ascii=False,indent=2)+'\n')
            status['status']='completed'
        except Exception as e:
            status.update(status='failed', error_type=type(e).__name__, error=str(e))
        finally:
            status['upload_created'] = bool(file_name)
            if file_name:
                status['remote_file_name'] = file_name
                try:
                    call('https://generativelanguage.googleapis.com/v1beta/'+file_name,method='DELETE')
                    status['upload_deleted']=True
                except Exception as e:
                    status.update(upload_deleted=False, cleanup_error_type=type(e).__name__)
            status['elapsed_s'] = time.monotonic()-before
            (root/'status.json').write_text(json.dumps(status,indent=2)+'\n')
        print(name+': '+status['status']+'; deleted='+str(status.get('upload_deleted')),flush=True)
        if status.get('upload_created') and not status.get('upload_deleted'):
            raise RuntimeError('cloud cleanup failed; stop batch to avoid accumulating files')


if __name__ == '__main__':
    main()
