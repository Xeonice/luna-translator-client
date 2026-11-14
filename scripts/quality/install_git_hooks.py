#!/usr/bin/env python3
"""Install repository-local hooks only into an existing Git repository."""
import argparse
import json
from pathlib import Path
import subprocess


def install(root):
    root = Path(root).resolve()
    result = subprocess.run(['git', 'rev-parse', '--show-toplevel'], cwd=root, text=True, capture_output=True)
    if result.returncode:
        return {'status': 'blocked', 'reason': 'git_repository_not_initialized', 'installed': False}
    if Path(result.stdout.strip()).resolve() != root:
        raise ValueError('refuse to configure a different parent Git repository')
    for name in ('pre-commit', 'pre-push'):
        path = root / '.githooks' / name
        if not path.is_file() or path.is_symlink():
            raise ValueError('missing regular repository hook: ' + name)
        path.chmod(path.stat().st_mode | 0o111)
    subprocess.run(['git', 'config', '--local', 'core.hooksPath', '.githooks'], cwd=root, check=True)
    return {'status': 'passed', 'installed': True, 'hooks_path': '.githooks',
            'remote_required_checks_verified': False}


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    result = install(p.parse_args().root)
    print(json.dumps(result, ensure_ascii=False))
    raise SystemExit(0 if result['installed'] else 2)
