"""Local text translation adapter; audio never leaves the worker."""
import json
import os
import urllib.parse
import urllib.request


class HyMT2:
    """Loopback text MT adapter for Hy-MT2 and Index-Translate."""
    def __init__(self, base_url, model, target, timeout=120, *, backend='hy-mt2',
                 source_language='ja', protected_terms=(), scene=''):
        parsed = urllib.parse.urlparse(base_url)
        if parsed.scheme != 'http' or parsed.hostname not in ('127.0.0.1', 'localhost', '::1'):
            raise ValueError('online experiment requires a local HTTP llama-server')
        if parsed.username or parsed.password or parsed.query or parsed.fragment:
            raise ValueError('credentials/query/fragment are not allowed in base URL')
        self.endpoint = base_url.rstrip('/') + '/chat/completions'
        self.model, self.target, self.timeout = model, target, timeout
        if backend not in ('hy-mt2', 'index-translate'):
            raise ValueError('unsupported local translation backend')
        self.backend, self.source_language = backend, source_language
        self.protected_terms, self.scene = tuple(protected_terms), scene
        # Do not inherit system proxies for private transcript requests.
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                                NoRedirect())

    def _prompt(self, source, context=(), lookahead_source=''):
        reference = '\n'.join([source, lookahead_source, *(src for src, _ in context)])
        terms, used = [], 0
        for term in self.protected_terms:
            if term and term in reference and term not in terms and used+len(term) <= 1200:
                terms.append(term)
                used += len(term)
        background = []
        if self.scene:
            background.append('[User-provided scene]\n'+self.scene)
        if terms:
            background.append('[Proper names / terms: preserve verbatim]\n'+'\n'.join(terms))
        if context:
            background.append('[Previous source and translation pairs]\n'+'\n\n'.join(
                f'Group {i+1}\nSource: {src}\nTranslation (wording reference only): {dst}'
                for i, (src, dst) in enumerate(context)))
        if lookahead_source:
            background.append('[Following source context: reference only; do not translate]\n'+lookahead_source)
        rules = ('Translate only the current source. Use previous/following source to resolve '
                 'references and omissions when clearly supported. Previous translations are only '
                 'wording references and may be wrong; source text takes precedence. '
                 'Scene hints must not override explicit source facts. '
                 'Do not repeat context, invent facts, or complete an unfinished sentence. '
                 'Preserve listed proper names verbatim. Output only the translation.')
        background = '\n\n'.join(background)
        if self.backend == 'index-translate':
            languages = {'ja': '日语', 'en': '英语', 'ko': '韩语', 'zh': '中文'}
            target = '简体中文' if self.target in ('Simplified Chinese', 'Chinese') else self.target
            prompt = (f'请将以下{languages.get(self.source_language, "")}文本翻译成{target}，并且严格遵循所有约束要求。\n\n'
                      f'〖源文〗\n{source}\n\n〖约束要求〗\n{rules}\n\n'
                      f'〖背景信息（仅供参考，不翻译）〗\n{background}\n\n只输出译文，不要有任何额外说明。')
        elif background:
            # Keep the official Hy-MT background/source ordering.
            prompt = (f'[Background Information]\n{background}\n\n'
                      f'Please translate the following text into {self.target}, taking the '
                      f'provided background information into consideration. {rules}\n\n'
                      f'[Source Text]\n{source}')
        else:
            prompt = (f'Translate the following segment into {self.target}, '
                      f'{rules}\n\n{source}')
        # Bound the complete prompt as well as history. UTF-8 bytes are a
        # conservative guard, not a model-token measurement. Remove whole old
        # pairs first, then optional right context; never cut names or facts.
        if len(prompt.encode('utf-8')) > 6000:
            if context:
                return self._prompt(source, context[1:], lookahead_source)
            if lookahead_source:
                return self._prompt(source, context, '')
            raise ValueError('MT prompt exceeds local budget; shorten scene or glossary hints')
        return prompt

    def translate(self, source, context=(), lookahead_source=''):
        prompt = self._prompt(source, context, lookahead_source)
        body = dict(model=self.model, stream=False, temperature=0.0, max_tokens=1024,
                    messages=[dict(role='user', content=prompt)])
        if self.backend == 'index-translate':
            body['chat_template_kwargs'] = dict(enable_thinking=False)
        headers = {'Content-Type': 'application/json'}
        key = os.environ.get('ATOGAKI_ONLINE_LLAMA_KEY')
        if key:
            headers['Authorization'] = 'Bearer ' + key
        request = urllib.request.Request(self.endpoint, json.dumps(body).encode(), headers)
        with self.client.open(request, timeout=self.timeout) as response:
            raw = json.load(response)
        if raw['choices'][0].get('finish_reason') != 'stop':
            raise ValueError('translation was not completed normally')
        result = raw['choices'][0]['message']['content']
        if self.backend == 'index-translate' and isinstance(result, str):
            if '</think>' in result:
                result = result.split('</think>', 1)[1]
            if '<think>' in result:
                raise ValueError('translation contains an unfinished thinking block')
        if not isinstance(result, str) or not result.strip():
            raise ValueError('empty translation')
        return result.strip()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('redirects are disabled for local translation')
