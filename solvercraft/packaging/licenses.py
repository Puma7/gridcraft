"""Collect resolved dependency notices into a distribution staging directory.

Uses the locked root and SDK metadata, not a stale hand-maintained package list.
The published index contains package/version/SPDX/source only, never local paths.
"""
from pathlib import Path
from pathlib import PurePosixPath
import argparse
import hashlib
import json
import shutil
import subprocess

PREFIXES = ('LICENSE', 'COPYING', 'COPYRIGHT', 'NOTICE', 'UNLICENSE')


def source_root(package):
    manifest = Path(package['manifest_path']).resolve()
    if package['source'].startswith('registry+'):
        return manifest.parent
    if package['source'].startswith('git+'):
        for candidate in manifest.parents:
            if candidate.parent.parent.name == 'checkouts':
                return candidate
    raise ValueError('Unsupported dependency source for ' + package['name'])


def collect(repository, output):
    repository = repository.resolve()
    output = output.resolve()
    if output == repository or repository.is_relative_to(output):
        raise ValueError('Choose a dedicated distribution license directory')
    packages = {}
    notice_root = Path(__file__).resolve().parent / 'notices'
    notice_manifest = notice_root / 'manifest.json'
    pinned = json.loads(notice_manifest.read_text('utf8'))['packages'] if notice_manifest.is_file() else []
    pinned = {(record['name'], record['version']): record for record in pinned}
    for manifest, requested in [(repository / 'Cargo.toml', {'solvercraft-app'}), (repository / 'solvercraft/Cargo.toml', {'solvercraft-cli', 'solvercraft-mcp'})]:
        result = subprocess.run(['cargo', 'metadata', '--manifest-path', str(manifest), '--format-version', '1', '--locked', '--offline', '--filter-platform', 'x86_64-pc-windows-msvc'],
                                capture_output=True, text=True, check=True)
        metadata = json.loads(result.stdout)
        nodes = {node['id']: node for node in metadata['resolve']['nodes']}
        selected = {package['id'] for package in metadata['packages'] if package['name'] in requested}
        pending = list(selected)
        while pending:
            current = pending.pop()
            for dependency in nodes[current]['deps']:
                if not any(kind['kind'] != 'dev' for kind in dependency['dep_kinds']):
                    continue
                if dependency['pkg'] not in selected:
                    selected.add(dependency['pkg'])
                    pending.append(dependency['pkg'])
        for package in metadata['packages']:
            if package['id'] in selected and package.get('source'):
                packages[package['id']] = package
    rows = []
    missing = []
    for package in sorted(packages.values(), key=lambda p: (p['name'], p['version'], p['source'])):
        boundary = source_root(package)
        directory = Path(package['manifest_path']).resolve().parent
        files = set()
        while directory.is_relative_to(boundary):
            for file in directory.iterdir():
                if file.is_file() and file.name.upper().startswith(PREFIXES):
                    files.add(file.resolve())
                elif file.is_dir() and file.name.upper() in {'LICENSES', 'LICENCES'}:
                    files.update(item.resolve() for item in file.rglob('*') if item.is_file())
            if directory == boundary:
                break
            directory = directory.parent
        if package.get('license_file'):
            declared = Path(package['license_file'])
            if not declared.is_absolute():
                declared = Path(package['manifest_path']).parent / declared
            files.add(declared.resolve())
        suffix = hashlib.sha256(package['source'].encode('utf8')).hexdigest()[:8]
        identity = package['name'] + '-' + package['version'] + '-' + suffix
        copied = []
        notice_sources = []
        for file in sorted(files):
            if not file.is_relative_to(boundary) or not file.is_file():
                raise ValueError('Dependency license escapes source boundary: ' + identity)
            relative = file.relative_to(boundary)
            destination = output / 'dependencies' / identity / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, destination)
            copied.append(relative.as_posix())
        if not copied and (package['name'], package['version']) in pinned:
            record = pinned[(package['name'], package['version'])]
            base = notice_root / (package['name'] + '-' + package['version'])
            for item in record['files']:
                relative = PurePosixPath(item['path'])
                if relative.is_absolute() or any(part in {'.', '..'} for part in relative.parts) or '\\' in item['path'] or ':' in item['path']:
                    raise ValueError('Invalid pinned notice path')
                file = (base / Path(*relative.parts)).resolve()
                if not file.is_relative_to(base.resolve()) or hashlib.sha256(file.read_bytes()).hexdigest() != item['sha256']:
                    raise ValueError('Pinned dependency notice hash mismatch: ' + identity)
                destination = output / 'dependencies' / identity / 'upstream' / Path(*relative.parts)
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(file, destination)
                copied.append('upstream/' + relative.as_posix())
                notice_sources.append({'path': 'upstream/' + relative.as_posix(), 'source': item['source'], 'sha256': item['sha256']})
        if not copied:
            vcs_file = Path(package['manifest_path']).parent / '.cargo_vcs_info.json'
            missing.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'),
                            'repository': package.get('repository'),
                            'vcs': json.loads(vcs_file.read_text('utf8')) if vcs_file.exists() else None})
        rows.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'),
                     'source': package['source'], 'notice_files': copied, 'pinned_notice_sources': notice_sources})
    output.mkdir(parents=True, exist_ok=True)
    if missing:
        (output / 'missing-notices.json').write_text(json.dumps(missing, indent=2) + '\n', encoding='utf8', newline='\n')
        raise ValueError('Missing resolved notices for ' + str(len(missing)) + ' packages; inspect missing-notices.json')
    (output / 'resolved-dependencies.json').write_text(json.dumps(rows, indent=2) + '\n', encoding='utf8', newline='\n')
    (output / 'THIRD_PARTY.md').write_text('# Resolved root and SDK dependency notices\n\n' +
        '\n'.join('- ' + row['name'] + ' ' + row['version'] + ': ' + str(row['license']) for row in rows) + '\n', encoding='utf8', newline='\n')
    return {'resolved_packages': len(rows), 'notice_files': sum(len(row['notice_files']) for row in rows)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(collect(args.repository, args.output)))
