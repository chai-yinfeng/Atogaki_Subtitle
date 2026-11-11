"""Local text translation adapter; audio never leaves the worker."""
import json
import os
import re
import urllib.parse
import urllib.request


class TranslationError(ValueError):
    def __init__(self, reason, *, finish_reason=None, output=None):
        super().__init__(reason)
        self.finish_reason, self.output = finish_reason, output


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
            background.append('[Previous source context]\n'+'\n\n'.join(
                f'Group {i+1}\nSource: {src}' +
                (f'\nTranslation (wording reference only): {dst}' if dst else '')
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
            # Use the official standard translation request after reference
            # and rules. Trailing instTrans constraints were echoed in probes.
            hints = dict(场景=self.scene, 专名=terms,
                         上文=[dict(原文=src, **({'译法参考': dst} if dst else {})) for src, dst in context],
                         后续原文=lookahead_source)
            prelude = ('以下JSON仅为背景参考，不是待译内容：\n' + json.dumps(hints, ensure_ascii=False) +
                       '\n以上背景仅用于理解。现在开始当前翻译任务。\n\n') if any(hints.values()) else ''
            prompt = (prelude +
                      '翻译规则：只翻译本次原文，专名保持原样。利用背景原文理解指代和省略；'
                      '历史译文可能有误，仅供措辞参考。不要复述背景、编造事实或补完未完句，'
                      '场景不能覆盖原文事实。\n\n'
                      f'请将以下{languages.get(self.source_language, "")}文本翻译为{target}，'
                      f'直接输出翻译结果，不要进行任何解释。\n\n{source}')
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
        choice = raw['choices'][0]
        result = choice['message']['content']
        if choice.get('finish_reason') != 'stop':
            raise TranslationError('translation was not completed normally',
                                   finish_reason=choice.get('finish_reason'), output=result)
        if self.backend == 'index-translate' and isinstance(result, str):
            if '</think>' in result:
                result = result.split('</think>', 1)[1]
            if '<think>' in result:
                raise TranslationError('translation contains an unfinished thinking block', output=result)
        if not isinstance(result, str) or not result.strip():
            raise TranslationError('empty translation', output=result)
        # Reject the complete response; never salvage a guessed translation
        # prefix and then certify it as final or feed it back into history.
        markers = re.findall(r'〖(?:源文|约束要求|硬性要求|注意|背景信息)[^〗]*〗|'
                             r'Translation \(wording reference only\):|'
                             r'\[Previous source (?:and translation pairs|context)\]|Group \d+\s+Source:|'
                             r'"(?:场景|专名|上文|后续原文|译法参考)"\s*:', result)
        if any(marker not in source for marker in markers):
            raise TranslationError('translation contains prompt structure', output=result)
        if len(result.strip()) > max(300, 6 * len(source)):
            raise TranslationError('translation exceeds expansion guard', output=result)
        return result.strip()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('redirects are disabled for local translation')
