"""Install/remove one fixed text-save fault only in seed_structure_undo_qa data.

Never accepts arbitrary SQL or a formal data directory. This script does not
launch, stop, or otherwise control any App process.
"""
import argparse
import json
from pathlib import Path
import re
import sqlite3
import tempfile

NAME = 'qa_subtitle_text_save_abort'
SQL = (f'CREATE TRIGGER {NAME} BEFORE UPDATE OF source_text ON local_subtitle_segments '
       "WHEN NEW.source_text <> OLD.source_text BEGIN "
       "SELECT RAISE(ABORT,'QA injected text save failure'); END")


def open_fixture(seed_path):
    seed = json.loads(seed_path.read_text())
    directory = Path(seed['data_dir']).resolve(strict=True)
    database = Path(seed['database']).resolve(strict=True)
    if (Path(tempfile.gettempdir()).resolve() not in directory.parents
            or not re.fullmatch(r'atogaki-token-qa-[0-9a-f-]{36}', directory.name)
            or database != directory / 'atogaki.sqlite'):
        raise ValueError('Only the unique temporary seed fixture is allowed')
    marker = json.loads((directory / 'seed.json').read_text())
    if marker['job_id'] != seed['job_id'] or marker['database'] != seed['database']:
        raise ValueError('Seed does not match its fixture marker')
    connection = sqlite3.connect('file:' + str(database) + '?mode=rw', uri=True)
    try:
        jobs = connection.execute('SELECT job_id FROM local_jobs').fetchall()
        runs = connection.execute('SELECT config_json FROM local_asr_runs').fetchall()
        if jobs != [(seed['job_id'],)] or not runs or not all(
                json.loads(row[0]).get('synthetic_qa') is True for row in runs):
            raise ValueError('Database must contain only the synthetic seed task and runs')
        return connection, database
    except BaseException:
        connection.close()
        raise


def apply_fault(seed_path, action, record_path):
    connection, database = open_fixture(seed_path)
    try:
        connection.execute('BEGIN IMMEDIATE')
        current = connection.execute(
            'SELECT sql FROM sqlite_master WHERE type=\'trigger\' AND name=?', (NAME,)).fetchone()
        if action == 'install':
            if current:
                raise ValueError('Fault trigger already exists')
            connection.execute(SQL)
            actual = connection.execute(
                'SELECT sql FROM sqlite_master WHERE name=?', (NAME,)).fetchone()[0]
            # Refuse to replace a previous record, before committing the fault.
            with record_path.open('x') as stream:
                json.dump({'database': str(database), 'name': NAME, 'sql': actual}, stream, indent=2)
                stream.write('\n')
        elif action == 'remove':
            record = json.loads(record_path.read_text())
            if (record.get('database') != str(database) or record.get('name') != NAME
                    or current != (record.get('sql'),) or current != (SQL,)):
                raise ValueError('Refusing to remove a missing or modified fault trigger')
            connection.execute(f'DROP TRIGGER {NAME}')
        else:
            raise ValueError('Unknown action')
        connection.commit()
    except BaseException:
        connection.rollback()
        raise
    finally:
        connection.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'remove'])
    parser.add_argument('--seed', required=True, type=Path)
    parser.add_argument('--record', required=True, type=Path)
    args = parser.parse_args()
    apply_fault(args.seed, args.action, args.record)
    print('Verified synthetic fixture fault: ' + args.action)


if __name__ == '__main__':
    main()
