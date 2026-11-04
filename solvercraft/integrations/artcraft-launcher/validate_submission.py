"""Check the pinned catalog proposal and optional real Windows release archive.

This never edits an upstream checkout, creates a PR or approves a community app.
The optional scratch exercise applies the patch only to a new local temp copy.
"""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import io
import json
import shutil
import stat
import struct
import subprocess
import tempfile
import zipfile

HERE = Path(__file__).resolve().parent
MAX_BYTES = 512 * 1024 * 1024
MAX_ENTRIES = 4096


def require(condition, message):
    if not condition:
        raise ValueError(message)


def blob(data):
    return hashlib.sha1(b'blob ' + str(len(data)).encode('ascii') + b'\0' + data).hexdigest()


def check_archive(data):
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        entries = archive.infolist()
        require(0 < len(entries) <= MAX_ENTRIES, 'Archive entry quota exceeded')
        total = 0
        seen = set()
        files = {}
        for entry in entries:
            path = PurePosixPath(entry.filename)
            require(entry.filename and not entry.filename.startswith('/') and '\\' not in entry.filename and ':' not in entry.filename, 'Unsafe ZIP path')
            raw_parts = entry.filename.rstrip('/').split('/')
            require(all(part not in {'', '.', '..'} for part in raw_parts) and not any(ord(c) < 32 for c in entry.filename), 'Unsafe ZIP path')
            identity = entry.filename.rstrip('/').casefold()
            require(identity not in seen, 'Duplicate or case-colliding ZIP path')
            seen.add(identity)
            require(stat.S_IFMT(entry.external_attr >> 16) != stat.S_IFLNK, 'ZIP links are not allowed')
            require(not entry.flag_bits & 1, 'Encrypted ZIP entries are not allowed')
            total += entry.file_size
            require(total <= MAX_BYTES, 'Unpacked byte quota exceeded')
            if not entry.is_dir():
                files[path] = entry
        roots = [p for p in files if len(p.parts) == 1 and p.name.casefold() == 'solvercraft.exe']
        if roots:
            require(len(roots) == 1, 'Ambiguous application executable')
            executable = roots[0]
        else:
            wrappers = {p.parts[0] for p in files}
            require(len(wrappers) == 1 and all(len(p.parts) >= 2 for p in files), 'Mixed portable ZIP layout')
            roots = [p for p in files if len(p.parts) == 2 and p.name.casefold() == 'solvercraft.exe']
            require(len(roots) == 1, 'Expected solvercraft.exe at the portable archive root')
            executable = roots[0]
        binary = archive.read(files[executable])
        require(len(binary) >= 88 and binary[:2] == b'MZ', 'Application is not a Windows PE executable')
        offset = struct.unpack_from('<I', binary, 60)[0]
        require(offset <= len(binary) - 26 and binary[offset:offset + 4] == b'PE\0\0', 'Invalid PE header')
        require(struct.unpack_from('<H', binary, offset + 4)[0] == 0x8664, 'Expected Windows x64 executable')
        require(struct.unpack_from('<H', binary, offset + 24)[0] == 0x20b, 'Expected PE32+ executable')
        return {'entries': len(entries), 'unpacked_bytes': total, 'executable': str(executable), 'executable_sha256': hashlib.sha256(binary).hexdigest()}


def self_test():
    pe = bytearray(128)
    pe[:2] = b'MZ'
    struct.pack_into('<I', pe, 60, 64)
    pe[64:68] = b'PE\0\0'
    struct.pack_into('<H', pe, 68, 0x8664)
    struct.pack_into('<H', pe, 88, 0x20b)

    def fixture(items):
        target = io.BytesIO()
        with zipfile.ZipFile(target, 'w') as archive:
            for name, body in items:
                archive.writestr(name, body)
        return target.getvalue()

    assert check_archive(fixture([('solvercraft.exe', pe)]))['executable'] == 'solvercraft.exe'
    assert check_archive(fixture([('package/Solvercraft.exe', pe), ('package/LICENSE', b'license')]))['executable'] == 'package/Solvercraft.exe'
    rejected = [
        [('solvercraft-app.exe', pe)],
        [('bin/solvercraft.exe', pe), ('LICENSE', b'license')],
        [('solvercraft.exe', pe), ('../escape', b'x')],
        [('solvercraft.exe', pe), ('SOLVERCRAFT.EXE', pe)],
        [('solvercraft.exe', b'not a PE file')],
        [('one/solvercraft.exe', pe), ('two/data', b'x')],
        [('./solvercraft.exe', pe)],
    ]
    for items in rejected:
        try:
            check_archive(fixture(items))
        except ValueError:
            continue
        raise AssertionError('Unsafe archive accepted: ' + repr([name for name, _ in items]))
    return {'archive_contract_self_tests': 9}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--launcher', type=Path, help='Exact pinned source checkout; checked read-only')
    parser.add_argument('--scratch-parent', type=Path, help='Optional owned directory for an isolated patch-application copy')
    parser.add_argument('--package', type=Path, help='Actual solvercraft-0.14.2-windows-x64-portable.zip')
    parser.add_argument('--sums', type=Path, help='Defaults to SHA256SUMS.txt beside --package')
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--report', type=Path, help='Write the compact check result as JSON')
    args = parser.parse_args()
    manifest = json.loads((HERE / 'submission.json').read_text('utf8'))
    patch = HERE / 'solvercraft-catalog.patch'
    require(hashlib.sha256(patch.read_bytes()).hexdigest() == manifest['patch_sha256'], 'Catalog patch hash mismatch')
    require(hashlib.sha256((HERE / 'solvercraft.png').read_bytes()).hexdigest() == manifest['icon_sha256'], 'Original icon hash mismatch')
    result = {'status': 'review-artifact-only', 'upstream_revision': manifest['upstream_revision'], 'upstream_modified': False, 'release_published': False}
    if args.self_test:
        result.update(self_test())
    if args.launcher:
        checkout = args.launcher.resolve()
        for item in manifest['base_files']:
            require(blob((checkout / item['path']).read_bytes()) == item['sha'], 'Pinned upstream file mismatch: ' + item['path'])
        subprocess.run(['git', '-c', 'core.autocrlf=false', '-C', str(checkout), 'apply', '--check', str(patch)], check=True)
        result['pinned_base_files_verified'] = len(manifest['base_files'])
        result['git_apply_check'] = True
        if args.scratch_parent:
            parent = args.scratch_parent.resolve()
            parent.mkdir(parents=True, exist_ok=True)
            scratch = Path(tempfile.mkdtemp(prefix='launcher-submission-', dir=parent)).resolve()
            require(scratch.parent == parent, 'Scratch copy escaped its owned parent')
            for item in manifest['base_files']:
                target = scratch / item['path']
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(checkout / item['path'], target)
            subprocess.run(['git', '-c', 'core.autocrlf=false', '-C', str(scratch), 'apply', str(patch)], check=True)
            for item in manifest['patched_files']:
                require(blob((scratch / item['path']).read_bytes()) == item['git_blob'], 'Applied patch identity mismatch: ' + item['path'])
            icon = scratch / 'assets/app-icons/solvercraft.png'
            icon.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(HERE / 'solvercraft.png', icon)
            require(hashlib.sha256(icon.read_bytes()).hexdigest() == manifest['icon_sha256'], 'Copied icon mismatch')
            result['isolated_application_verified'] = True
            result['patched_text_files'] = len(manifest['patched_files'])
            result['translation_languages'] = 10
    if args.package:
        package = args.package.resolve()
        require(package.name == manifest['windows_asset'], 'Release asset name does not match the launcher Windows x64 portable contract')
        sums = args.sums or package.parent / 'SHA256SUMS.txt'
        matching = [line.split(maxsplit=1) for line in sums.read_text('utf8').splitlines() if line.strip()]
        matching = [parts for parts in matching if len(parts) == 2 and parts[1].lstrip('*') == package.name]
        require(len(matching) == 1, 'Expected one SHA256SUMS.txt entry for the portable ZIP')
        data = package.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        require(matching[0][0].lower() == digest, 'Portable ZIP checksum mismatch')
        result['local_package'] = dict(asset=package.name, sha256=digest, **check_archive(data))
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf8', newline='\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
