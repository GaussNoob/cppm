#!/usr/bin/env bash
set -euo pipefail
umask 027
release=${1:?Informe o SHA}
[[ "$release" =~ ^[a-f0-9]{40}$ ]] || exit 2
base=/srv/cppm
exec 9>"$base/deploy.lock"
flock -w 120 9
incoming="$base/incoming/$release"
cd "$incoming"
python3 - <<'PY'
import hashlib, pathlib, re, tarfile
checksum=pathlib.Path('site.tar.gz.sha256').read_text().strip()
assert re.fullmatch(r'[a-f0-9]{64}  site\.tar\.gz',checksum), 'Invalid checksum manifest'
assert hashlib.file_digest(open('site.tar.gz','rb'),'sha256').hexdigest()==checksum[:64] if hasattr(hashlib,'file_digest') else hashlib.sha256(pathlib.Path('site.tar.gz').read_bytes()).hexdigest()==checksum[:64]
with tarfile.open('site.tar.gz','r:gz') as archive:
    entries=archive.getmembers()
    assert len(entries)<30000 and sum(e.size for e in entries)<1024**3, 'Archive too large'
    for e in entries:
        p=pathlib.PurePosixPath(e.name)
        assert not p.is_absolute() and '..' not in p.parts, 'Unsafe path'
        assert e.isfile() or e.isdir(), 'Links and special files forbidden'
PY
if [[ ! -f "$base/releases/$release/.complete" ]]; then
  test ! -e "$base/releases/$release" || { echo 'Incomplete release requires administrator inspection'; exit 1; }
  install -d -m 2750 "$base/releases/$release"
  tar --no-same-owner --no-same-permissions -xzf site.tar.gz -C "$base/releases/$release"
  [[ $(cat "$base/releases/$release/RELEASE") == "$release" ]]
  test -s "$base/releases/$release/index.html"
  find "$base/releases/$release" -type d -exec chmod 2750 {} +
  find "$base/releases/$release" -type f -exec chmod 640 {} +
  touch "$base/releases/$release/.complete"
fi
previous=$(readlink -f "$base/current" || true)
switched=0
rollback() {
  rc=$?
  trap - EXIT
  if (( rc != 0 && switched == 1 )); then
    if [[ -n "$previous" && -d "$previous" ]]; then
      ln -s "$previous" "$base/current.rollback"
      mv -Tf "$base/current.rollback" "$base/current"
      echo 'Previous release restored' >&2
    else
      rm -f "$base/current"
    fi
  fi
  exit "$rc"
}
trap rollback EXIT
ln -s "$base/releases/$release" "$base/current.next"
mv -Tf "$base/current.next" "$base/current"
switched=1
healthy=0
for attempt in {1..5}; do
  if [[ $(curl -fsS --max-time 10 -H 'Host: cppm.gaussnoob.com.br' http://127.0.0.1/RELEASE) == "$release" ]] && curl -fsS --max-time 10 --resolve cppm.gaussnoob.com.br:443:127.0.0.1 https://cppm.gaussnoob.com.br/ >/dev/null; then healthy=1; break; fi
  sleep 1
done
(( healthy == 1 ))
switched=0
echo "cppm: $release active"
mapfile -t versions < <(find "$base/releases" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %f\n' | sort -rn | awk '{print $2}')
kept=0
for candidate in "${versions[@]}"; do
  [[ "$candidate" =~ ^[a-f0-9]{40}$ ]] || continue
  kept=$((kept+1))
  if (( kept <= 3 )) || [[ "$candidate" == "$release" || "$base/releases/$candidate" == "$previous" ]]; then continue; fi
  rm -rf -- "$base/releases/$candidate"
done
rm -rf -- "$incoming"
trap - EXIT
