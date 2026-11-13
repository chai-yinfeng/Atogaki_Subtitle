import copy
import unittest
from compare_two_pass import comparable


class ComparabilityTests(unittest.TestCase):
    def metadata(self):
        return dict(cases=[dict(id='case',audio_sha256='audio')],translation_context=dict(scene='',protected_terms=['name']),
                    server=['llama-server','--port','123','--model','model.gguf'],
                    worker_base=['python','--translation-url','http://127.0.0.1:123/v1'],
                    source_hashes={'model.gguf':'model','llama-server':'binary','worker.py':'old'})

    def test_only_transport_port_and_worker_source_can_differ(self):
        a=self.metadata(); b=copy.deepcopy(a)
        b['server'][2]='456';b['worker_base'][2]='http://127.0.0.1:456/v1';b['source_hashes']['worker.py']='new'
        comparable(a,b)

    def test_changed_audio_glossary_runtime_or_weights_are_rejected(self):
        a=self.metadata()
        mutations=[lambda b:b['cases'][0].update(audio_sha256='different'),
                   lambda b:b['translation_context'].update(protected_terms=['new']),
                   lambda b:b['source_hashes'].update({'model.gguf':'changed'}),
                   lambda b:b['server'].append('--new-option')]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                b=copy.deepcopy(a);mutate(b)
                with self.assertRaises(ValueError):comparable(a,b)


if __name__=='__main__':unittest.main()
