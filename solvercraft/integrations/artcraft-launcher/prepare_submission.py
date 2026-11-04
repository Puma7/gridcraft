"""Rebuild the review-only launcher patch from an exact pinned source checkout."""
from pathlib import Path
import argparse
import difflib
import hashlib
import json

HERE = Path(__file__).resolve().parent
PIN = 'ec205366ebe42635e155b9ab246f036052fef406'
COPY = [
    'Optimization and simulation',
    'Optimization and simulation, locally.',
    'Solve numerical models and run simulations with a shared Rust SDK. The desktop app and local agent tools use the same calculation engine.',
    'Optimize models',
    'Solve supported linear and integer models with explicit result status.',
    'Simulate uncertainty',
    'Use seeded Monte Carlo simulation and inspect statistical summaries.',
    'Work with local agents',
    'Use the standalone stdio MCP tool with on-demand local execution.',
]


def git_blob(data):
    return hashlib.sha1(b'blob ' + str(len(data)).encode('ascii') + b'\0' + data).hexdigest()


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError('Pinned source anchor changed: ' + old[:75])
    return text.replace(old, new, 1)


def table(headers, rows):
    widths = [max(len(row[i]) for row in [headers, *rows]) for i in range(len(headers))]
    line = lambda row: '| ' + ' | '.join(value.ljust(width) for value, width in zip(row, widths)) + ' |\n'
    return line(headers) + '|' + '|'.join('-' * (width + 2) for width in widths) + '|\n' + ''.join(line(row) for row in rows)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('reference', type=Path)
    args = parser.parse_args()
    reference = args.reference.resolve()
    baseline = json.loads((reference / 'base-files.json').read_text('utf8'))
    if baseline['revision'] != PIN:
        raise ValueError('Wrong upstream revision')
    originals = {}
    for item in baseline['files']:
        raw = (reference / item['path']).read_bytes()
        if git_blob(raw) != item['sha']:
            raise ValueError('Reference Git blob mismatch: ' + item['path'])
        originals[item['path']] = raw.decode('utf8')
    changed = {}
    path = 'crates/core/src/catalog.rs'
    text = originals[path]
    text = replace_once(text, 'the sixteen apps the launcher knows about.', 'the seventeen apps the launcher knows about.')
    text = replace_once(text, 'pub const SUITE: [CraftApp; 16]', 'pub const SUITE: [CraftApp; 17]')
    row = '''    CraftApp {
        kind: AppKind::Community,
        publisher: "Puma7",
        msi_publisher: Some("Puma7"),
        bundle: Some("io.github.puma7.solvercraft"),
        site: Some("https://github.com/Puma7/gridcraft"),
        // No publisher Developer ID has been verified: do not install a macOS DMG.
        mac_team: None,
        ..app(
            "solvercraft",
            "Solvercraft",
            "Optimization and simulation",
            [[0x16, 0x98, 0x8d], [0x0c, 0x6b, 0x63], [0x43, 0xd4, 0xc2]],
            "Puma7/gridcraft",
        )
    },
'''
    text = replace_once(text, '    CraftApp {\n        kind: AppKind::Community,\n        publisher: "Tangency Labs",', row + '    CraftApp {\n        kind: AppKind::Community,\n        publisher: "Tangency Labs",')
    text = replace_once(text, '        "gridcraft" => include_bytes!("../../../assets/app-icons/gridcraft.png"),', '        "gridcraft" => include_bytes!("../../../assets/app-icons/gridcraft.png"),\n        "solvercraft" => include_bytes!("../../../assets/app-icons/solvercraft.png"),')
    text = replace_once(text, 'fn suite_has_sixteen_apps_with_unique_ids()', 'fn suite_has_seventeen_apps_with_unique_ids()')
    text = replace_once(text, 'assert_eq!(SUITE.len(), 16);', 'assert_eq!(SUITE.len(), 17);')
    test = '''    #[test]
    fn solvercraft_is_community_owned_and_has_no_unverified_mac_installer() {
        let app = CraftApp::by_id("solvercraft").unwrap();
        assert_eq!(app.kind, AppKind::Community);
        assert_eq!(app.repo, "Puma7/gridcraft");
        assert_eq!(app.publisher, "Puma7");
        assert_eq!(app.bundle_id(), "io.github.puma7.solvercraft");
        assert_eq!(app.website(), "https://github.com/Puma7/gridcraft");
        assert_eq!(app.mac_team, None);
        assert!(!app.installs(crate::PackageKind::Dmg));
        assert!(app.installs(crate::PackageKind::PortableZip));
    }

'''
    text = replace_once(text, '    #[test]\n    fn solidcraft_is_a_community_app_with_its_own_bundle_id()', test + '    #[test]\n    fn solidcraft_is_a_community_app_with_its_own_bundle_id()')
    changed[path] = text

    path = 'crates/core/src/details.rs'
    text = replace_once(originals[path], 'pub const DETAILS: [AppDetails; 16]', 'pub const DETAILS: [AppDetails; 17]')
    detail = '''    AppDetails {
        id: "solvercraft",
        headline: "Optimization and simulation, locally.",
        about: "Solve numerical models and run simulations with a shared Rust SDK. The desktop app and local agent tools use the same calculation engine.",
        features: &[
            feature("Optimize models", "Solve supported linear and integer models with explicit result status."),
            feature("Simulate uncertainty", "Use seeded Monte Carlo simulation and inspect statistical summaries."),
            feature("Work with local agents", "Use the standalone stdio MCP tool with on-demand local execution."),
        ],
        // No picture URL is invented before the shared screenshots repository accepts it.
        shots: &[],
        platforms: "Windows",
    },
'''
    text = replace_once(text, '    AppDetails {\n        id: "solidcraft",', detail + '    AppDetails {\n        id: "solidcraft",')
    changed[path] = text

    path = 'crates/native/src/desktop_entry.rs'
    changed[path] = replace_once(originals[path], '        "gridcraft" => ("Office;Spreadsheet;", None),', '        "gridcraft" => ("Office;Spreadsheet;", None),\n        "solvercraft" => ("Office;Science;Math;", None),')
    path = 'ATTRIBUTION.md'
    changed[path] = originals[path].rstrip() + '\n\n## Solvercraft community-app submission\n\n' + table(
        ['File', 'Work', 'Author', 'Source', 'License'],
        [['`assets/app-icons/solvercraft.png`', 'Solvercraft numerical-chart icon, 128 px', 'Solvercraft contributors',
          'Original procedural artwork; recipe distributed with the Solvercraft catalog submission', 'MIT OR Apache-2.0']],
    )

    translations = json.loads((HERE / 'translations.json').read_text('utf8'))
    for language, values in translations.items():
        if len(values) != len(COPY) or any(not value or '\t' in value or '\n' in value for value in values):
            raise ValueError('Malformed translation values: ' + language)
        path = f'crates/ui-egui/src/i18n/{language}.tsv'
        additions = '\n# Solvercraft community-app submission\n'
        additions += ''.join(f'solvercraft\t{key}\t{value}\n' for key, value in zip(COPY, values))
        changed[path] = originals[path].rstrip() + '\n' + additions

    patch = ''
    for path, text in changed.items():
        patch += f'diff --git a/{path} b/{path}\n'
        patch += ''.join(difflib.unified_diff(originals[path].splitlines(True), text.splitlines(True), fromfile='a/' + path, tofile='b/' + path, n=3))
    (HERE / 'solvercraft-catalog.patch').write_text(patch, encoding='utf8', newline='\n')
    manifest = {
        'schema': 'solvercraft.artcraft-launcher-submission.v1',
        'status': 'requires-upstream-maintainer-decision',
        'upstream_repository': baseline['repository'],
        'upstream_revision': PIN,
        'source_repository': 'Puma7/gridcraft',
        'app_id': 'solvercraft',
        'app_name': 'Solvercraft',
        'publisher': 'Puma7',
        'bundle_id': 'io.github.puma7.solvercraft',
        'release_version': '0.14.2',
        'release_tag': 'v0.14.2',
        'release_availability': 'not-asserted',
        'windows_asset': 'solvercraft-0.14.2-windows-x64-portable.zip',
        'windows_executable': 'solvercraft.exe',
        'linux_asset_contract_only': 'solvercraft-0.14.2-linux-x86_64.tar.gz',
        'linux_executable_contract_only': 'bin/solvercraft',
        'mac_installer_enabled': False,
        'patch_sha256': hashlib.sha256(patch.encode('utf8')).hexdigest(),
        'icon_sha256': hashlib.sha256((HERE / 'solvercraft.png').read_bytes()).hexdigest(),
        'base_files': [item for item in baseline['files'] if item['path'] in changed],
        'patched_files': [{'path': path, 'git_blob': git_blob(text.encode('utf8'))} for path, text in changed.items()],
    }
    (HERE / 'submission.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf8', newline='\n')
    print(json.dumps({'changed_text_files': len(changed), 'patch_sha256': manifest['patch_sha256'], 'upstream_revision': PIN}))


if __name__ == '__main__':
    main()
