"""Install the delivered APK; prove native cold/repeated startup without a browser.
Authenticated tests use an in-process synthetic transport, never production writes.
"""
import json
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

OUT = Path('output/android-startup')
OUT.mkdir(parents=True, exist_ok=True)
PACKAGE = 'net.castilloit.bastion.preview'
COMPONENT = PACKAGE + '/net.castilloit.bastion.MainActivity'

def adb(*args, check=True, binary=False, timeout=60):
    result = subprocess.run(['adb', *args], capture_output=True, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-2000:])
    return result.stdout if binary else result.stdout.decode(errors='replace')

def launch(name):
    adb('shell', 'am', 'force-stop', PACKAGE)
    adb('logcat', '-c')
    start = adb('shell', 'am', 'start', '-W', '-n', COMPONENT)
    (OUT / (name + '-start.txt')).write_text(start)
    deadline = time.monotonic() + 40
    while time.monotonic() < deadline:
        crash = adb('logcat', '-d', '-s', 'AndroidRuntime:E')
        if PACKAGE in crash:
            (OUT / (name + '-crash.txt')).write_text(crash)
            raise AssertionError(crash[-5000:])
        adb('shell', 'uiautomator', 'dump', '/sdcard/native.xml', check=False)
        text = adb('shell', 'cat', '/sdcard/native.xml', check=False)
        try:
            tree = ET.fromstring(text)
        except ET.ParseError:
            continue
        nodes = list(tree.iter('node'))
        if any(n.get('text') == 'Iniciar sesión' for n in nodes):
            assert any(n.get('class') == 'android.widget.EditText' and n.get('package') == PACKAGE for n in nodes)
            assert not any(n.get('class') == 'android.webkit.WebView' for n in nodes)
            activities = adb('shell', 'dumpsys', 'activity', 'activities')
            resumed = [line for line in activities.splitlines() if 'ResumedActivity' in line or 'topResumedActivity' in line]
            assert any(COMPONENT in line or PACKAGE in line for line in resumed), resumed
            (OUT / (name + '.xml')).write_text(text)
            (OUT / (name + '-activities.txt')).write_text('\n'.join(resumed))
            return
        time.sleep(1)
    raise AssertionError('Native login did not appear')

adb('install', '-r', sys.argv[1])
adb('shell', 'pm', 'disable-user', '--user', '0', 'com.android.chrome', check=False)
try:
    launch('cold-start-no-browser')
    launch('reopen-no-browser')
    adb('shell', 'wm', 'size', '720x1280')
    adb('shell', 'wm', 'density', '360')  # 320 dp phone
    adb('shell', 'settings', 'put', 'system', 'font_scale', '1.3')
    launch('small-screen-no-browser')
    adb('install', '-r', sys.argv[2])
    result = adb('shell', 'am', 'instrument', '-w', PACKAGE + '.test/androidx.test.runner.AndroidJUnitRunner', timeout=360)
    (OUT / 'instrumentation.txt').write_text(result)
    if 'FAILURES' in result or 'OK (' not in result or 'INSTRUMENTATION_FAILED' in result:
        raise AssertionError(result[-6000:])
    adb('pull', '/sdcard/Android/data/' + PACKAGE + '/files/native-qa', str(OUT), check=False)
    (OUT / 'result.json').write_text(json.dumps({'native_cold_start': True, 'native_reopen': True, 'browser_disabled': True, 'phone_320dp_font_130': True, 'authenticated_fixture_tests': True}, indent=2))
    print(result[-1500:])
finally:
    adb('shell', 'pm', 'enable', 'com.android.chrome', check=False)
    adb('shell', 'wm', 'size', 'reset', check=False)
    adb('shell', 'wm', 'density', 'reset', check=False)
    adb('shell', 'settings', 'put', 'system', 'font_scale', '1.0', check=False)
