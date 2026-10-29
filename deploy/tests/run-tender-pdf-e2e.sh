#!/usr/bin/env bash
# Runs only on the disposable GitHub runner, with a synthetic database/storage.
set -euo pipefail
user=$(id -un)
group=$(id -gn)
base=/var/lib/bastion-tender
opt=/opt/bastion-pdf
socket=/run/systemd/system/bastion-tender-launcher.socket
unit=/run/systemd/system/bastion-tender-launcher@.service
bind=/run/bastion-tender-job
launcher=/usr/local/libexec/bastion-tender-launcher
worker=/usr/local/libexec/bastion-tender-pdf-worker
converter=/usr/local/libexec/bastion-tender-image-converter
for path in "$base" "$opt" "$socket" "$unit" "$bind" "$launcher" "$worker" "$converter"; do test ! -e "$path"; done
cleanup() {
  sudo systemctl stop bastion-tender-launcher.socket >/dev/null 2>&1 || true
  sudo systemctl stop 'bastion-tender-launcher@*.service' >/dev/null 2>&1 || true
  sudo rm -f -- "$socket" "$unit" "$launcher" "$worker" "$converter"
  sudo systemctl daemon-reload >/dev/null 2>&1 || true
  if test -d "$base/private"; then
    for volume in "$base"/private/*/volume; do
      if sudo mountpoint -q "$volume"; then sudo umount "$volume" || return 1; fi
    done
  fi
  sudo rm -rf -- "$base" "$opt" "$bind"
}
trap cleanup EXIT
sudo install -d -o root -g "$group" -m 0750 "$base"
sudo install -d -o root -g "$group" -m 0730 "$base/incoming"
sudo install -d -o root -g root -m 0700 "$base/private"
sudo install -d -o root -g root -m 0755 "$bind" "$opt" /usr/local/libexec
sudo cp -a "$RUNNER_TEMP/bastion-qpdf" "$opt/qpdf-12.4.2"
sudo install -m 0755 "$BASTION_TYPST_BIN" "$opt/typst"
sudo install -d -o root -g root -m 0755 "$opt/typst-packages" "$opt/test-templates"
sudo install -m 0755 target/x86_64-unknown-linux-gnu/release/tender-pdf-worker "$worker"
sudo install -m 0755 deploy/bastion-tender-image-converter.py "$converter"
python3 - "$user" "$opt/typst" <<'PY' | sudo tee "$launcher" >/dev/null
from pathlib import Path
import sys
s=Path('deploy/bastion-tender-launcher.py').read_text()
s=s.replace("pwd.getpwnam('bastion')", f"pwd.getpwnam('{sys.argv[1]}')")
s=s.replace('CAPACITY = 5 * 1024**3', 'CAPACITY = 64 * 1024**2')
s=s.replace("    except Exception:\n        print('error', flush=True)", "    except Exception as exc:\n        print(type(exc).__name__ + ': ' + str(exc), file=sys.stderr)\n        print('error', flush=True)")
s=s.replace("stderr=subprocess.DEVNULL,\n                          timeout=timeout", "stderr=None,\n                          timeout=timeout")
s=s.replace("    except Exception:\n        print('error', flush=True)", "    except Exception as exc:\n        print(type(exc).__name__ + ': ' + str(exc), file=sys.stderr)\n        print('error', flush=True)")
s=s.replace("stderr=subprocess.DEVNULL,\n                          timeout=timeout", "stderr=None,\n                          timeout=timeout")
s=s.replace('Environment=BASTION_TYPST_BIN=/usr/local/bin/typst', f'Environment=BASTION_TYPST_BIN={sys.argv[2]}')
print(s,end='')
PY
sudo chmod 0755 "$launcher"
sed -e "s/SocketUser=bastion/SocketUser=$user/" -e "s/SocketGroup=bastion/SocketGroup=$group/" deploy/bastion-tender-launcher.socket.example | sudo tee "$socket" >/dev/null
sudo cp deploy/bastion-tender-launcher@.service.example "$unit"
sudo python3 - "$opt/test-templates" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1])
files={
 'carta-de-oferta.typ':'#import "variables.typ": datos\nOferta #datos.lpn',
 'presentacion-de-oferta.typ':'#import "variables.typ": datos\nPresentacion #datos.lpn',
 'lote1-bid.typ':'#import "items1_bid.typ": items\n#let total_lote = decimal("0.00")\nLote #items.len()',
 'lote1-std.typ':'#import "items1_std.typ": items\n#let total_lote = decimal("0.00")\nLote #items.len()',
 'funciones.typ':'#let f(x) = x',
 'dj-integridad.typ':'= Integridad\n#variables.lpn',
 'informacion_sobre_el_oferente.typ':'= Oferente\n#variables.lpn',
}
for name,content in files.items():
 file=p/name; file.write_text(content); file.chmod(0o644)
PY
sudo systemctl daemon-reload
sudo systemctl start bastion-tender-launcher.socket
export BASTION_TENDER_TEMPLATE_DIR="$opt/test-templates"
if ! if ! cargo test --locked --package bastion-licitaciones --test pdf_e2e -- --nocapture; then
  sudo journalctl --no-pager -n 80 -u 'bastion-tender-launcher@*.service' -u 'bastion-tender-*.service' || true
  exit 1
fi; then
  sudo journalctl --no-pager -n 80 -u 'bastion-tender-launcher@*.service' -u 'bastion-tender-*.service' || true
  exit 1
fi
