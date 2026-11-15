#!/usr/bin/env node
// One version for the whole project.
//
//   node scripts/version.mjs              print the version (from package.json)
//   node scripts/version.mjs check        fail if Cargo.toml disagrees
//   node scripts/version.mjs set 0.3.0    write it to package.json(-lock) and Cargo.toml
//   node scripts/version.mjs dev 42       print the dev build version, e.g. 0.2.0-dev.42
//
// tauri.conf.json reads its version from package.json, so it never drifts.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pkgPath = join(root, 'package.json');
const lockPath = join(root, 'package-lock.json');
const cargoPath = join(root, 'Cargo.toml');
const SEMVER = /^\d+\.\d+\.\d+$/;
// The [workspace.package] version line in the root Cargo.toml.
const CARGO_VERSION = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)(")/;

const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));
const cargo = readFileSync(cargoPath, 'utf8');
const cargoVersion = cargo.match(CARGO_VERSION)?.[2];

const [cmd, arg] = process.argv.slice(2);
const fail = (msg) => {
  console.error(msg);
  process.exit(1);
};

switch (cmd ?? 'get') {
  case 'get':
    console.log(pkg.version);
    break;
  case 'check':
    if (!cargoVersion) fail('Cargo.toml: no [workspace.package] version found');
    if (cargoVersion !== pkg.version)
      fail(`Version mismatch: package.json ${pkg.version}, Cargo.toml ${cargoVersion}`);
    console.log(`Version ${pkg.version} ok`);
    break;
  case 'set': {
    if (!SEMVER.test(arg ?? '')) fail('Usage: version.mjs set <major.minor.patch>');
    pkg.version = arg;
    writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
    const lock = JSON.parse(readFileSync(lockPath, 'utf8'));
    lock.version = arg;
    if (lock.packages?.['']) lock.packages[''].version = arg;
    writeFileSync(lockPath, JSON.stringify(lock, null, 2) + '\n');
    writeFileSync(cargoPath, cargo.replace(CARGO_VERSION, `$1${arg}$3`));
    console.log(`Version set to ${arg} (run cargo check to refresh Cargo.lock)`);
    break;
  }
  case 'dev':
    if (!/^\d+$/.test(arg ?? '')) fail('Usage: version.mjs dev <build number>');
    console.log(`${pkg.version}-dev.${arg}`);
    break;
  default:
    fail(`Unknown command: ${cmd}`);
}
