"""Build UV-5RM_Channels.html (read, edit, make bands, write) and a test copy that drives it against a
simulated radio.

usage: py build_channels_page.py CHANNEL_CSV FIRMWARE_SRC BACKGROUND_PNG OUT_DIR [RADIO_DUMP]

CHANNEL_CSV is the built-in list (Clark_County_NV_UV-5RM_CHIRP.csv). With RADIO_DUMP (a channel table
read from a radio programmed with that list), it also writes channels_test.html next to this script:
open it in Chrome and every value in the RESULT line should read true or match the expected text.
"""
import base64, io, json, os, re, sys
from PIL import Image

csv_path, firmware_src, bg_path, out_dir = sys.argv[1:5]
dump_path = sys.argv[5] if len(sys.argv) > 5 else None
here = os.path.dirname(os.path.abspath(__file__))


def embed(v):
    return json.dumps(v).replace("</", "<\\/")


csv_text = open(csv_path, encoding="utf-8", newline="").read()

# Tone lists come from the firmware, so the page offers exactly what the radio's menus do.
settings = open(os.path.join(firmware_src, "src", "app", "settings.rs"), encoding="utf-8").read()
dcs = re.findall(r"0o(\d+)", re.search(r"DCS_TABLE\s*:[^=]*=\s*\[(.*?)\];", settings, re.S).group(1))
ctcss = [int(x) for x in re.findall(r"\d+", re.search(r"CTCSS_TABLE\s*:[^=]*=\s*\[(.*?)\];", settings, re.S).group(1))]
assert len(dcs) == 105 and dcs[33] == "205", (len(dcs), dcs[33])
assert len(ctcss) == 50 and ctcss[0] == 670 and ctcss[12] == 1000, ctcss[:13]

# The page and the firmware must agree on where band records live.
modes = open(os.path.join(firmware_src, "src", "app", "modes.rs"), encoding="utf-8").read()
assert "BAND_SLOT: u16 = 1000;" in modes and "MAX_BANDS: u8 = 16;" in modes and "BAND_MARK: u8 = b'B';" in modes

# Quick messages: the built-in ones come from the TEXT app, so the page shows what the radio does.
# The page writes them with the satellite-table command, which must land where TEXT reads them.
text_app = open(os.path.join(firmware_src, "apps", "textapp", "src", "logic.rs"), encoding="utf-8").read()
quick = re.findall(r'b"((?:[^"\\]|\\.)*)"', re.search(r"QUICK_DEFAULTS\s*:[^=]*=\s*\[(.*?)\];", text_app, re.S).group(1))
assert len(quick) == 10 and all(len(q) <= 50 and "\\" not in q for q in quick), quick
assert "QUICK_ADDR: u32 = 0x19_4000;" in text_app and "QUICK_BYTES: u32 = 64;" in text_app and "QUICK_MAGIC: u8 = b'Q';" in text_app
fmap = open(os.path.join(firmware_src, "src", "flash_map.rs"), encoding="utf-8").read()
cps = open(os.path.join(firmware_src, "src", "cps", "mod.rs"), encoding="utf-8").read()
app_addr = int(re.search(r"OVERLAY_APP_ADDR: u32 = (0x[0-9A-Fa-f_]+);", fmap).group(1).replace("_", ""), 16)
slots = int(re.search(r"OVERLAY_SLOT_COUNT: u8 = (\d+);", fmap).group(1))
assert app_addr + slots * 32 * 1024 == 0x194000 and "SAT_TABLE_BASE: u32 = addr::OVERLAY_DATA_ADDR;" in cps

# Same black-level pull as the radio's logo.
img = Image.open(bg_path).convert("RGB")
bg = img.getpixel((5, 5))[0]
lut = [max(0, c - bg) * 255 // (255 - bg) for c in range(256)]
img = img.point(lut * 3)
buf = io.BytesIO()
img.save(buf, "PNG", optimize=True)
background = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()

page = open(os.path.join(here, "channels_template.html"), encoding="utf-8").read()
for key, val in (("__SKULL__", background), ("__CSV__", embed(csv_text)), ("__DCS__", embed(dcs)), ("__CTCSS__", embed(ctcss)), ("__QUICK__", embed(quick))):
    assert key in page, key
    page = page.replace(key, val)
out = os.path.join(out_dir, "UV-5RM_Channels.html")
with open(out, "w", encoding="utf-8", newline="\n") as f:
    f.write(page)
print("wrote", out, os.path.getsize(out), "bytes")

if dump_path:
    dump = open(dump_path, "rb").read()[:32000]
    assert len(dump) == 32000
    tests = open(os.path.join(here, "channels_tests.js"), encoding="utf-8").read()
    inject = "<script>const RADIO_DUMP_B64 = " + embed(base64.b64encode(dump).decode()) + ";\n" + tests + "</script>\n</body>"
    test_out = os.path.join(here, "channels_test.html")
    with open(test_out, "w", encoding="utf-8", newline="\n") as f:
        f.write(page.replace("</body>", '<pre id="testout"></pre>\n' + inject))
    print("wrote", test_out)
