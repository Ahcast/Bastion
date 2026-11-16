"""Launch the installed APK, observe the real login, and test a missing browser.

Runs only on a disposable CI emulator. No accounts or production writes.
"""
import json
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

OUT = Path('output/android-startup')
OUT.mkdir(parents=True, exist_ok=True)
PACKAGE = 'net.castilloit.bastion.preview'
COMPONENT = PACKAGE + '/net.castilloit.bastion.MainActivity'


def adb(*args, check=True, binary=False):
    result = subprocess.run(['adb', *args], capture_output=True, timeout=45)
    if check and result.returncode:
        raise RuntimeError(f'adb {args}: {result.stderr.decode(errors="replace")}')
    return result.stdout if binary else result.stdout.decode(errors='replace')


def ui():
    adb('shell', 'uiautomator', 'dump', '/sdcard/bastion-startup.xml', check=False)
    text = adb('shell', 'cat', '/sdcard/bastion-startup.xml', check=False)
    try:
        return text, ET.fromstring(text)
    except ET.ParseError:
        return text, None


def capture(name):
    (OUT / (name + '-crashes.txt')).write_text(adb('logcat', '-d', '-s', 'AndroidRuntime:E'))
    (OUT / (name + '-activities.txt')).write_text(adb('shell', 'dumpsys', 'activity', 'activities'))
    (OUT / (name + '-browser.txt')).write_text(adb('logcat', '-d', '-s', 'TWALauncherActivity:V', 'TwaLauncher:V', 'chromium:E'))
    (OUT / (name + '.png')).write_bytes(adb('exec-out', 'screencap', '-p', binary=True))
    text, _ = ui()
    (OUT / (name + '.xml')).write_text(text)


def check_crash():
    crash = adb('logcat', '-d', '-s', 'AndroidRuntime:E')
    if PACKAGE in crash:
        raise AssertionError(crash[-12000:])


def dismiss_browser_onboarding(tree):
    # Only Chrome onboarding on a fresh emulator; never tap website controls.
    allowed = {'Accept & continue', 'Use without an account', 'No thanks', 'Got it'}
    for node in tree.iter('node'):
        if node.get('package') != 'com.android.chrome' or node.get('text') not in allowed:
            continue
        bounds = re.findall(r'\d+', node.get('bounds', ''))
        if len(bounds) == 4:
            left, top, right, bottom = map(int, bounds)
            adb('shell', 'input', 'tap', str((left + right) // 2), str((top + bottom) // 2))
            return True
    return False


def launch(name, missing_browser=False):
    adb('shell', 'am', 'force-stop', PACKAGE)
    adb('logcat', '-c')
    result = adb('shell', 'am', 'start', '-W', '-n', COMPONENT, check=False)
    (OUT / (name + '-start.txt')).write_text(result)
    try:
        deadline = time.monotonic() + 100
        while time.monotonic() < deadline:
            check_crash()
            text, tree = ui()
            if tree is not None:
                if missing_browser and ('App Unavailable' in text or 'unavailable' in text.lower()):
                    return
                if not missing_browser and 'Iniciar sesión' in text and 'Correo' in text:
                    return
                if not missing_browser:
                    dismiss_browser_onboarding(tree)
            time.sleep(2)
        raise AssertionError(f'{name}: expected visible login or missing-browser explanation')
    finally:
        capture(name)


try:
    (OUT / 'android-version.txt').write_text(adb('shell', 'getprop', 'ro.build.version.release'))
    (OUT / 'packages.txt').write_text(adb('shell', 'pm', 'list', 'packages'))
    adb('install', '-r', sys.argv[1])
    launch('cold-start')
    adb('shell', 'input', 'keyevent', 'KEYCODE_HOME')
    launch('reopen')
    adb('shell', 'am', 'force-stop', 'com.android.chrome')
    adb('shell', 'pm', 'disable-user', '--user', '0', 'com.android.chrome')
    try:
        launch('no-browser', missing_browser=True)
    finally:
        adb('shell', 'pm', 'enable', 'com.android.chrome', check=False)
    (OUT / 'result.json').write_text(json.dumps({'login_visible': True, 'reopen': True, 'no_browser_no_crash': True}))
    print('PASS: installed APK opens the real login, reopens, and handles no browser without crashing.')
except Exception as error:
    print(error, file=sys.stderr)
    sys.exit(1)
