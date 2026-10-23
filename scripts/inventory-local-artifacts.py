"""Read-only inventory of explicitly configured artifact roots; never deletes.

Configuration: {"roots": [{"path": "/absolute/path", "purpose": "...",
"version": "...", "retention": "retain", "recursive": true,
"hash_contents": false}]}. Enable content hashing only for stable artifacts.
Unhashed metadata is not evidence of duplicate file contents. Keep the output
private: paths and filenames can describe personal datasets.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--config', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
if args.output.exists() or args.output.is_symlink():
    parser.error('Output already exists; keep earlier evidence')
config = json.loads(args.config.read_text())
inventories = []


def fail_walk(error):
    raise error


def describe(path, root, hash_contents):
    before = path.lstat()
    item = {'path': str(path.relative_to(root)) or '.',
            'bytes': before.st_size, 'mtime_ns': before.st_mtime_ns}
    if stat.S_ISLNK(before.st_mode):
        item.update(kind='symlink', target=os.readlink(path))
    elif stat.S_ISDIR(before.st_mode):
        item['kind'] = 'directory'
    elif stat.S_ISREG(before.st_mode):
        item['kind'] = 'file'
        item['sha256'] = None
        if hash_contents:
            # Refuse links, including a link substituted after lstat.
            descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
            with os.fdopen(descriptor, 'rb') as stream:
                opened = os.fstat(stream.fileno())
                if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
                    raise RuntimeError(f'File changed before hashing: {path}')
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
                after = os.fstat(stream.fileno())
            if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                raise RuntimeError(f'File changed while hashing: {path}')
            item['sha256'] = digest
    else:
        item['kind'] = 'special-retain'
    return item


for entry in config['roots']:
    root = Path(entry['path'])
    if not root.is_absolute() or not entry.get('purpose') or not entry.get('retention'):
        parser.error('Each root needs an absolute path, purpose and retention policy')
    result = {**entry, 'entries': []}
    if not root.exists() and not root.is_symlink():
        result['status'] = 'missing'
    else:
        result['status'] = 'inventoried'
        paths = [root]
        if entry.get('recursive', False) and root.is_dir() and not root.is_symlink():
            for directory, dirs, files in os.walk(root, followlinks=False, onerror=fail_walk):
                dirs.sort()
                paths.extend(Path(directory) / name for name in sorted(dirs + files))
        for path in paths:
            if path.absolute() == args.output.absolute():
                continue
            result['entries'].append(describe(path, root, entry.get('hash_contents', False)))
    payload = json.dumps(result['entries'], ensure_ascii=False, sort_keys=True).encode()
    result['metadata_manifest_sha256'] = hashlib.sha256(payload).hexdigest()
    result['regular_file_bytes'] = sum(e['bytes'] for e in result['entries'] if e['kind'] == 'file')
    result['hashed_files'] = sum(bool(e.get('sha256')) for e in result['entries'])
    inventories.append(result)

args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open('x') as stream:
    json.dump({'schema_version': 1, 'read_only': True, 'roots': inventories}, stream,
              ensure_ascii=False, indent=2)
    stream.write('\n')
print(json.dumps({'output': str(args.output), 'roots': len(inventories),
                  'hashed_files': sum(r['hashed_files'] for r in inventories)}))
